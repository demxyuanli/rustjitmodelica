use crate::ast::{AlgorithmStatement, Equation};
use crate::cache::cache_key::{CacheKeyV2, CacheStage, CompileFlagsKey};
use crate::flatten::flat_cache_v1::DepHashEntry;
use crate::flatten::{ArraySizePolicy, Flattener};
use crate::query_db::{semantic_hash_text, QueryDb};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

pub const EQ_EXPAND_CACHE_SCHEMA_V1: &str = "rustmodlica_eq_expand_cache_v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqExpandResult {
    pub out: Option<EqExpandOut>,
    pub err: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqExpandOut {
    pub equations: Vec<Equation>,
    pub algorithms: Vec<AlgorithmStatement>,
    pub initial_equations: Vec<Equation>,
    pub initial_algorithms: Vec<AlgorithmStatement>,
    pub connections: Vec<(String, String)>,
    pub conditional_connections: Vec<(crate::ast::Expression, (String, String))>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqExpandCacheV1 {
    pub schema: String,
    pub key: String,
    pub model_name: String,
    pub out: Option<EqExpandOut>,
    pub err: Option<String>,
    pub deps: Vec<DepHashEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct EqParallelGuardDb {
    models: HashMap<String, EqParallelGuardEntry>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct EqParallelGuardEntry {
    last_parallel_eq_expand_us: u64,
    last_parallel_candidate_share_pct: f64,
    degrade_streak: u32,
    last_model_size: usize,
    cooldown_remaining: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EqModelTier {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EqParallelMode {
    Off,
    Guarded,
    On,
}

fn eq_parallel_mode() -> EqParallelMode {
    match std::env::var("RUSTMODLICA_EQ_EXPAND_PARALLEL_MODE")
        .ok()
        .map(|v| v.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("on") | Some("always") | Some("force_on") => EqParallelMode::On,
        Some("guarded") | Some("guard") => EqParallelMode::Guarded,
        Some("off") | Some("0") | Some("false") | Some("no") => EqParallelMode::Off,
        _ => EqParallelMode::Off,
    }
}

fn eq_parallel_tier_small_max() -> usize {
    std::env::var("RUSTMODLICA_EQ_PARALLEL_TIER_SMALL_MAX")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(1500)
}

fn eq_parallel_tier_medium_max() -> usize {
    std::env::var("RUSTMODLICA_EQ_PARALLEL_TIER_MEDIUM_MAX")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(8000)
}

fn eq_parallel_model_tier(model_size: usize) -> EqModelTier {
    if model_size <= eq_parallel_tier_small_max() {
        EqModelTier::Small
    } else if model_size <= eq_parallel_tier_medium_max() {
        EqModelTier::Medium
    } else {
        EqModelTier::Large
    }
}

fn eq_parallel_guard_enabled() -> bool {
    std::env::var("RUSTMODLICA_EQ_PARALLEL_GUARD")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(true)
}

fn eq_parallel_guard_model_size_min() -> usize {
    std::env::var("RUSTMODLICA_EQ_PARALLEL_GUARD_MODEL_SIZE_MIN")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(2000)
}

fn eq_parallel_guard_streak_threshold() -> u32 {
    std::env::var("RUSTMODLICA_EQ_PARALLEL_GUARD_STREAK")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(2)
}

fn eq_parallel_guard_degrade_ratio_pct() -> u64 {
    std::env::var("RUSTMODLICA_EQ_PARALLEL_GUARD_DEGRADE_PCT")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v >= 100)
        .unwrap_or(110)
}

fn eq_parallel_guard_share_min_pct() -> f64 {
    std::env::var("RUSTMODLICA_EQ_PARALLEL_GUARD_SHARE_MIN_PCT")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| *v >= 0.0)
        .unwrap_or(15.0)
}

fn eq_parallel_guard_streak_threshold_for_tier(tier: EqModelTier) -> u32 {
    let key = match tier {
        EqModelTier::Small => "RUSTMODLICA_EQ_PARALLEL_GUARD_STREAK_SMALL",
        EqModelTier::Medium => "RUSTMODLICA_EQ_PARALLEL_GUARD_STREAK_MEDIUM",
        EqModelTier::Large => "RUSTMODLICA_EQ_PARALLEL_GUARD_STREAK_LARGE",
    };
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|v| *v > 0)
        .unwrap_or_else(eq_parallel_guard_streak_threshold)
}

fn eq_parallel_guard_degrade_ratio_pct_for_tier(tier: EqModelTier) -> u64 {
    let key = match tier {
        EqModelTier::Small => "RUSTMODLICA_EQ_PARALLEL_GUARD_DEGRADE_PCT_SMALL",
        EqModelTier::Medium => "RUSTMODLICA_EQ_PARALLEL_GUARD_DEGRADE_PCT_MEDIUM",
        EqModelTier::Large => "RUSTMODLICA_EQ_PARALLEL_GUARD_DEGRADE_PCT_LARGE",
    };
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v >= 100)
        .unwrap_or_else(eq_parallel_guard_degrade_ratio_pct)
}

fn eq_parallel_guard_share_min_pct_for_tier(tier: EqModelTier) -> f64 {
    let key = match tier {
        EqModelTier::Small => "RUSTMODLICA_EQ_PARALLEL_GUARD_SHARE_MIN_PCT_SMALL",
        EqModelTier::Medium => "RUSTMODLICA_EQ_PARALLEL_GUARD_SHARE_MIN_PCT_MEDIUM",
        EqModelTier::Large => "RUSTMODLICA_EQ_PARALLEL_GUARD_SHARE_MIN_PCT_LARGE",
    };
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| *v >= 0.0)
        .unwrap_or_else(eq_parallel_guard_share_min_pct)
}

fn eq_parallel_guard_cooldown_compiles_for_tier(tier: EqModelTier) -> u32 {
    let key = match tier {
        EqModelTier::Small => "RUSTMODLICA_EQ_PARALLEL_GUARD_COOLDOWN_SMALL",
        EqModelTier::Medium => "RUSTMODLICA_EQ_PARALLEL_GUARD_COOLDOWN_MEDIUM",
        EqModelTier::Large => "RUSTMODLICA_EQ_PARALLEL_GUARD_COOLDOWN_LARGE",
    };
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(2)
}

fn eq_parallel_guard_seed_eq_expand_us_for_tier(tier: EqModelTier) -> u64 {
    let key = match tier {
        EqModelTier::Small => "RUSTMODLICA_EQ_PARALLEL_GUARD_SEED_EQ_US_SMALL",
        EqModelTier::Medium => "RUSTMODLICA_EQ_PARALLEL_GUARD_SEED_EQ_US_MEDIUM",
        EqModelTier::Large => "RUSTMODLICA_EQ_PARALLEL_GUARD_SEED_EQ_US_LARGE",
    };
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(match tier {
            EqModelTier::Small => 2_000,
            EqModelTier::Medium => 120_000,
            EqModelTier::Large => 900_000,
        })
}

fn eq_parallel_guard_seed_share_pct_for_tier(tier: EqModelTier) -> f64 {
    let key = match tier {
        EqModelTier::Small => "RUSTMODLICA_EQ_PARALLEL_GUARD_SEED_SHARE_PCT_SMALL",
        EqModelTier::Medium => "RUSTMODLICA_EQ_PARALLEL_GUARD_SEED_SHARE_PCT_MEDIUM",
        EqModelTier::Large => "RUSTMODLICA_EQ_PARALLEL_GUARD_SEED_SHARE_PCT_LARGE",
    };
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| *v >= 0.0)
        .unwrap_or(match tier {
            EqModelTier::Small => 5.0,
            EqModelTier::Medium => 12.0,
            EqModelTier::Large => 25.0,
        })
}

fn eq_parallel_guard_seed_streak_for_tier(tier: EqModelTier) -> u32 {
    let key = match tier {
        EqModelTier::Small => "RUSTMODLICA_EQ_PARALLEL_GUARD_SEED_STREAK_SMALL",
        EqModelTier::Medium => "RUSTMODLICA_EQ_PARALLEL_GUARD_SEED_STREAK_MEDIUM",
        EqModelTier::Large => "RUSTMODLICA_EQ_PARALLEL_GUARD_SEED_STREAK_LARGE",
    };
    std::env::var(key)
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .unwrap_or(0)
}

fn eq_parallel_guard_seed_entry_if_missing(
    guard: &mut EqParallelGuardDb,
    model_name: &str,
    model_size: usize,
) {
    if guard.models.contains_key(model_name) {
        return;
    }
    let tier = eq_parallel_model_tier(model_size);
    guard.models.insert(
        model_name.to_string(),
        EqParallelGuardEntry {
            last_parallel_eq_expand_us: eq_parallel_guard_seed_eq_expand_us_for_tier(tier),
            last_parallel_candidate_share_pct: eq_parallel_guard_seed_share_pct_for_tier(tier),
            degrade_streak: eq_parallel_guard_seed_streak_for_tier(tier),
            last_model_size: model_size,
            cooldown_remaining: 0,
        },
    );
}

fn eq_parallel_guard_path() -> PathBuf {
    if let Ok(v) = std::env::var("LOCALAPPDATA") {
        let p = PathBuf::from(v).join("rustmodlica").join("eq_parallel_guard_v1.json");
        return p;
    }
    std::env::temp_dir().join("rustmodlica_eq_parallel_guard_v1.json")
}

fn eq_parallel_guard_load() -> EqParallelGuardDb {
    let path = eq_parallel_guard_path();
    if let Ok(text) = std::fs::read_to_string(path) {
        if let Ok(v) = serde_json::from_str::<EqParallelGuardDb>(&text) {
            return v;
        }
    }
    EqParallelGuardDb::default()
}

fn eq_parallel_guard_save(db: &EqParallelGuardDb) {
    let path = eq_parallel_guard_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string(db) {
        let _ = std::fs::write(path, text.as_bytes());
    }
}

pub fn eq_parallel_guard_update_candidate_share(model_name: &str, pct: f64) {
    if !eq_parallel_guard_enabled() || model_name.trim().is_empty() {
        return;
    }
    let mut guard = eq_parallel_guard_load();
    let entry = guard.models.entry(model_name.to_string()).or_default();
    entry.last_parallel_candidate_share_pct = pct.clamp(0.0, 100.0);
    eq_parallel_guard_save(&guard);
}

pub fn eq_expanded(db: &dyn QueryDb, model_name: String) -> super::EqExpandResPtr {
    let wall = std::time::Instant::now();
    let deps_scope = super::DepScope::begin();
    let libs = db.library_paths();
    let coarse = db.coarse_constrainedby_only();
    let validation_mode = crate::flatten::ValidationMode::parse(db.validation_mode().as_str());
    let st = db.source_text(model_name.clone());
    let root_hash = semantic_hash_text(st.text.as_str());
    let scope = super::scope_from_path(st.path.as_str(), model_name.as_str());
    let target_platform =
        if crate::cache::msl_pack::context::is_active() && model_name.starts_with("Modelica.") {
            "msl-pack".to_string()
        } else {
            format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
        };
    let key_v2 = CacheKeyV2::builder(
        CacheStage::EqExpand,
        scope.clone(),
        model_name.as_str(),
    )
    .libs_from_path_bufs(libs.as_slice())
    .root_content_hash(root_hash)
    .compile_flags(CompileFlagsKey {
        validation_mode: format!("{validation_mode:?}"),
        compile_stop: db.compile_stop().as_ref().clone(),
        coarse_constrainedby_only: coarse,
        array_size_policy: 0,
        warnings_level: String::new(),
        target_platform,
    })
    .build();
    let (cache_enabled, key) = super::key_with_policy(key_v2.to_qualified_key());

    if cache_enabled {
        if let Some(bytes) = crate::flatten::cache_shm::shm_get(key.as_str()) {
            if let Ok(cache) = bincode::deserialize::<EqExpandCacheV1>(&bytes) {
                if cache.schema == EQ_EXPAND_CACHE_SCHEMA_V1
                    && cache.key == key
                    && super::cache_deps_match_for_stage(
                        &scope,
                        "eq_expand",
                        model_name.as_str(),
                        &cache.deps,
                    )
                {
                    crate::query_db::perf::record_cache_event(
                        scope.prefix(),
                        "eq_expand",
                        crate::query_db::perf::CacheEvent::Hit,
                    );
                    super::dep_record_deps(&cache.deps);
                    crate::query_db::perf::record_us(
                        "eq_expand_us",
                        wall.elapsed().as_micros() as u64,
                    );
                    return super::EqExpandResPtr(Arc::new(EqExpandResult {
                        out: cache.out,
                        err: cache.err,
                    }));
                }
            }
        }
            if let Some(bytes) = super::sqlite_get_with_scope_chain(
                scope,
                key.as_str(),
                "eq_expand_v1",
            ) {
                if let Ok(cache) = bincode::deserialize::<EqExpandCacheV1>(&bytes) {
                    if cache.schema == EQ_EXPAND_CACHE_SCHEMA_V1
                        && cache.key == key
                        && super::cache_deps_match_for_stage(
                        &scope,
                        "eq_expand",
                        model_name.as_str(),
                        &cache.deps,
                    )
                    {
                        crate::query_db::perf::record_cache_event(
                            scope.prefix(),
                            "eq_expand",
                            crate::query_db::perf::CacheEvent::Hit,
                        );
                        let _ = crate::flatten::cache_shm::shm_put(key.as_str(), &bytes);
                        super::dep_record_deps(&cache.deps);
                        crate::query_db::perf::record_us(
                            "eq_expand_us",
                            wall.elapsed().as_micros() as u64,
                        );
                        return super::EqExpandResPtr(Arc::new(EqExpandResult {
                            out: cache.out,
                            err: cache.err,
                        }));
                    }
                }
            }
    }

    let root = db.inheritance_flattened(model_name.clone()).0;
    let decl_res = db.decl_expanded(model_name.clone()).0;
    if let Some(e) = &decl_res.err {
        crate::query_db::perf::record_us("eq_expand_us", wall.elapsed().as_micros() as u64);
        return super::EqExpandResPtr(Arc::new(EqExpandResult {
            out: None,
            err: Some(e.clone()),
        }));
    }
    let Some(decl) = &decl_res.out else {
        crate::query_db::perf::record_us("eq_expand_us", wall.elapsed().as_micros() as u64);
        return super::EqExpandResPtr(Arc::new(EqExpandResult {
            out: None,
            err: Some("decl_expanded returned empty result".to_string()),
        }));
    };

    let eq_expand_prep_t0 = std::time::Instant::now();
    let mut flattener = Flattener::new();
    flattener.coarse_constrainedby_only = coarse;
    flattener.validation_mode = validation_mode;
    flattener.compile_stop_label = db.compile_stop().as_ref().clone();
    flattener.array_size_policy = ArraySizePolicy::default();
    flattener.warnings_level = "all".to_string();
    for p in libs.iter() {
        flattener.loader.add_path(p.clone());
    }
    flattener.loader.set_quiet(true);
    let model_size = root.equations.len().saturating_add(root.algorithms.len());
    let eq_parallel_requested_raw = std::env::var("RUSTMODLICA_FLATTEN_EQ_PARALLEL")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(false);
    let parallel_mode = eq_parallel_mode();
    let eq_parallel_requested = match parallel_mode {
        EqParallelMode::Off => false,
        EqParallelMode::Guarded => eq_parallel_requested_raw,
        EqParallelMode::On => true,
    };
    let mut forced_by_cooldown = false;
    if matches!(parallel_mode, EqParallelMode::Off) {
        flattener.force_disable_eq_parallel = true;
        crate::query_db::perf::record_add("guard_reason_policy_off", 1);
    } else if eq_parallel_requested
        && matches!(parallel_mode, EqParallelMode::Guarded)
        && eq_parallel_guard_enabled()
    {
        let mut guard = eq_parallel_guard_load();
        eq_parallel_guard_seed_entry_if_missing(&mut guard, &model_name, model_size);
        if let Some(entry) = guard.models.get_mut(&model_name) {
            let tier = eq_parallel_model_tier(model_size);
            let streak_threshold = eq_parallel_guard_streak_threshold_for_tier(tier);
            let model_size_min = eq_parallel_guard_model_size_min();
            let share_min_pct = eq_parallel_guard_share_min_pct_for_tier(tier);
            if entry.cooldown_remaining > 0 {
                flattener.force_disable_eq_parallel = true;
                forced_by_cooldown = true;
                crate::query_db::perf::record_add("guard_cooldown_active", 1);
                crate::query_db::perf::record_add("guard_reason_cooldown_active", 1);
                entry.cooldown_remaining = entry.cooldown_remaining.saturating_sub(1);
                if entry.cooldown_remaining == 0 {
                    crate::query_db::perf::record_add("guard_cooldown_exit", 1);
                }
            } else if entry.degrade_streak >= streak_threshold
                && model_size < model_size_min
                && entry.last_parallel_candidate_share_pct < share_min_pct
            {
                flattener.force_disable_eq_parallel = true;
                entry.cooldown_remaining = eq_parallel_guard_cooldown_compiles_for_tier(tier);
                crate::query_db::perf::record_add("guard_cooldown_enter", 1);
                crate::query_db::perf::record_add("guard_reason_degrade_low_share_small_model", 1);
            } else {
                crate::query_db::perf::record_add("guard_reason_none", 1);
            }
        }
        eq_parallel_guard_save(&guard);
    }

    let mut flat = crate::flatten::FlattenedModel {
        // Not needed for equation expansion; keep empty to avoid large clones.
        declarations: Vec::new(),
        // Equation expansion happens here; do not seed from decl_expanded.
        equations: Vec::new(),
        algorithms: Vec::new(),
        initial_equations: Vec::new(),
        initial_algorithms: Vec::new(),
        connections: Vec::new(),
        conditional_connections: Vec::new(),
        instances: decl.instances.clone(),
        array_sizes: decl.array_sizes.clone(),
        clocked_var_names: HashSet::new(),
        clock_partitions: Vec::new(),
        clock_signal_connections: Vec::new(),
        stream_peer_map: HashMap::new(),
        stream_connection_set: HashMap::new(),
        stream_flow_map: HashMap::new(),
        expandable_instances: HashSet::new(),
        interner: crate::string_intern::StringInterner::new(),
        // Not needed for this stage.
        inst_records: Vec::new(),
        path_to_inst: HashMap::new(),
    };

    crate::query_db::perf::record_us(
        "eq_expand_prep_us",
        eq_expand_prep_t0.elapsed().as_micros() as u64,
    );
    flattener.eq_expand_root_preinherited(root.as_ref(), &mut flat);
    let eq_elapsed_us = wall.elapsed().as_micros() as u64;
    if eq_parallel_requested
        && matches!(parallel_mode, EqParallelMode::Guarded)
        && eq_parallel_guard_enabled()
    {
        let mut guard = eq_parallel_guard_load();
        eq_parallel_guard_seed_entry_if_missing(&mut guard, &model_name, model_size);
        let entry = guard.models.entry(model_name.clone()).or_default();
        if !flattener.force_disable_eq_parallel {
            let tier = eq_parallel_model_tier(model_size);
            let degrade_pct = eq_parallel_guard_degrade_ratio_pct_for_tier(tier);
            let prev = entry.last_parallel_eq_expand_us.max(1);
            if eq_elapsed_us.saturating_mul(100) >= prev.saturating_mul(degrade_pct) {
                entry.degrade_streak = entry.degrade_streak.saturating_add(1);
            } else {
                entry.degrade_streak = 0;
            }
            entry.last_parallel_eq_expand_us = eq_elapsed_us;
            entry.last_model_size = model_size;
        } else {
            // Fallback run keeps previous streak so next runs can stay protected until conditions improve.
            entry.last_model_size = model_size;
            if forced_by_cooldown {
                // During cooldown we keep streak untouched; probe happens automatically when cooldown reaches zero.
            }
        }
        eq_parallel_guard_save(&guard);
    }

    let out = EqExpandOut {
        equations: flat.equations,
        algorithms: flat.algorithms,
        initial_equations: flat.initial_equations,
        initial_algorithms: flat.initial_algorithms,
        connections: flat.connections,
        conditional_connections: flat.conditional_connections,
    };

    for p in flattener.loader.loaded_source_paths() {
        if let Some(h) = super::semantic_hash_file_cached(p.as_path()) {
            let p_s = p.display().to_string();
            super::dep_record_file(p_s.as_str(), h.as_str());
        }
    }
    let deps = deps_scope.end();
    crate::query_db::perf::record_cache_event(
        scope.prefix(),
        "eq_expand",
        crate::query_db::perf::CacheEvent::Miss,
    );

    if cache_enabled {
        if let Some(cfg) = crate::flatten::cache_sqlite::sqlite_write_config_for_scope(scope) {
                let cache = EqExpandCacheV1 {
                    schema: EQ_EXPAND_CACHE_SCHEMA_V1.to_string(),
                    key: key.clone(),
                    model_name: model_name.clone(),
                    out: Some(out.clone()),
                    err: None,
                    deps: deps.clone(),
                };
                if let Ok(bytes) = bincode::serialize(&cache) {
            crate::query_db::perf::record_cache_event(
                scope.prefix(),
                "eq_expand",
                crate::query_db::perf::CacheEvent::Write,
            );
                    let _ = crate::flatten::cache_shm::shm_put(key.as_str(), &bytes);
                    let deps_json = serde_json::to_string(&deps).ok();
                    let _ = crate::flatten::cache_sqlite::sqlite_put(
                        &cfg.path,
                        key.as_str(),
                        EQ_EXPAND_CACHE_SCHEMA_V1,
                        "eq_expand_v1",
                        &bytes,
                        deps_json.as_deref(),
                    );
                }
        }
    }

    crate::query_db::perf::record_us("eq_expand_us", eq_elapsed_us);
    super::EqExpandResPtr(Arc::new(EqExpandResult {
        out: Some(out),
        err: None,
    }))
}

