use crate::ast::{AlgorithmStatement, Equation};
use crate::cache::cache_key::{CacheKeyV2, CacheStage, CompileFlagsKey};
use crate::flatten::flat_cache_v1::DepHashEntry;
use crate::flatten::{ArraySizePolicy, Flattener};
use crate::query_db::{semantic_hash_text, QueryDb};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
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

pub fn eq_expanded(db: &dyn QueryDb, model_name: String) -> super::EqExpandResPtr {
    let wall = std::time::Instant::now();
    let deps_scope = super::DepScope::begin();
    let libs = db.library_paths();
    let coarse = db.coarse_constrainedby_only();
    let validation_mode = crate::flatten::ValidationMode::parse(db.validation_mode().as_str());
    let st = db.source_text(model_name.clone());
    let root_hash = semantic_hash_text(st.text.as_str());
    let scope = super::scope_from_path(st.path.as_str(), model_name.as_str());
    let key_v2 = CacheKeyV2::builder(
        CacheStage::EqExpand,
        scope.clone(),
        model_name.as_str(),
    )
    .libs_from_paths(libs.as_ref())
    .root_content_hash(root_hash)
    .compile_flags(CompileFlagsKey {
        validation_mode: format!("{validation_mode:?}"),
        compile_stop: db.compile_stop().as_ref().clone(),
        coarse_constrainedby_only: coarse,
        array_size_policy: 0,
        warnings_level: String::new(),
        target_platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
    })
    .build();
    let (cache_enabled, key) = super::key_with_policy(key_v2.to_qualified_key());

    if cache_enabled {
        if let Some(bytes) = crate::flatten::cache_shm::shm_get(key.as_str()) {
            if let Ok(cache) = bincode::deserialize::<EqExpandCacheV1>(&bytes) {
                if cache.schema == EQ_EXPAND_CACHE_SCHEMA_V1
                    && cache.key == key
                    && super::cache_deps_match_for_stage(&scope, "eq_expand", &cache.deps)
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
        if let Some(dir) = crate::flatten::flatten_cache::flatten_cache_dir() {
            if let Some(cfg) = crate::flatten::cache_sqlite::sqlite_config_for_scope(scope.clone(), Some(dir.as_path())) {
                if let Ok(Some(bytes)) =
                    crate::flatten::cache_sqlite::sqlite_get(&cfg.path, key.as_str(), "eq_expand_v1")
                {
                    if let Ok(cache) = bincode::deserialize::<EqExpandCacheV1>(&bytes) {
                        if cache.schema == EQ_EXPAND_CACHE_SCHEMA_V1
                            && cache.key == key
                            && super::cache_deps_match_for_stage(&scope, "eq_expand", &cache.deps)
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

    let mut flattener = Flattener::new();
    flattener.coarse_constrainedby_only = coarse;
    flattener.validation_mode = validation_mode;
    flattener.array_size_policy = ArraySizePolicy::default();
    flattener.warnings_level = "all".to_string();
    for p in libs.as_ref() {
        flattener.loader.add_path(p.into());
    }
    flattener.loader.set_quiet(true);

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
        interner: crate::string_intern::StringInterner::new(),
        // Not needed for this stage.
        inst_records: Vec::new(),
        path_to_inst: HashMap::new(),
    };

    flattener.eq_expand_root_preinherited(root.as_ref(), &mut flat);

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
        if let Some(dir) = crate::flatten::flatten_cache::flatten_cache_dir() {
            if let Some(cfg) = crate::flatten::cache_sqlite::sqlite_config_for_scope(scope.clone(), Some(dir.as_path())) {
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
    }

    crate::query_db::perf::record_us("eq_expand_us", wall.elapsed().as_micros() as u64);
    super::EqExpandResPtr(Arc::new(EqExpandResult {
        out: Some(out),
        err: None,
    }))
}

