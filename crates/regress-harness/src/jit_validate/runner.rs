use crate::jit_validate::artifacts::{
    Case, CasePaths, GitInfo, HostInfo, LayerStats, PerfStats, RunManifest, ScenarioResolved,
    Summary, TraceFlags, ValidatePerfReport,
};
use crate::jit_validate::{ensure_cache_dir, normalize_model_list, CacheDirPolicy, RunSpec, Scenario};
use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Instant;

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn safe_slug(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else if ch == '.' {
            out.push('.');
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "case".to_string()
    } else {
        out
    }
}

fn write_json_pretty(path: &Path, v: &impl serde::Serialize) -> Result<()> {
    let text = serde_json::to_string_pretty(v)?;
    std::fs::write(path, text).with_context(|| format!("write {}", path.display()))?;
    Ok(())
}

fn json_get_u64(v: &serde_json::Value, key: &str) -> Option<u64> {
    v.get(key).and_then(|x| x.as_u64())
}

fn resolve_artifact_path(out_dir: &Path, p: &str) -> PathBuf {
    let raw = p.trim();
    if raw.is_empty() {
        return out_dir.to_path_buf();
    }
    let candidate = PathBuf::from(raw);
    if candidate.is_absolute() && candidate.exists() {
        return candidate;
    }
    if candidate.exists() {
        return candidate;
    }
    let joined = out_dir.join(&candidate);
    joined
}

struct ParsedCompilePerf {
    /// Present when `RUSTMODLICA_PERF_SALSA_STATS=1` on the rustmodlica process.
    salsa_process_db_hits: u64,
    salsa_process_db_misses: u64,
    salsa_process_db_evictions: u64,
    flatten_inline_ms: u64,
    flatten_wall_ms: u64,
    flatten_wall_us: u64,
    inline_wall_ms: u64,
    inline_wall_us: u64,
    codegen_wall_ms: u64,
    codegen_wall_us: u64,
    decl_expand_ms: u64,
    eq_expand_ms: u64,
    inline_substitute_ms: u64,
    inline_load_model_ms: u64,
    cache_deserialize_ms: u64,
    cache_l0_hits: u64,
    cache_l1_hits: u64,
    cache_l2_hits: u64,
    cache_l0_writes: u64,
    cache_l1_writes: u64,
    cache_l2_writes: u64,
    deps_mismatch: u64,
    cache_scope_stage_hits: BTreeMap<String, BTreeMap<String, u64>>,
    cache_scope_stage_misses: BTreeMap<String, BTreeMap<String, u64>>,
    cache_scope_stage_invalidations: BTreeMap<String, BTreeMap<String, u64>>,
    stub_compile_ms: u64,
    stub_compile_us: u64,
    clock_partition_scan_ms: u64,
    clock_partition_scan_us: u64,
    parallel_candidate_share_pct: f64,
    cache_warm_ratio: f64,
    /// `Some(100)` when native AOT loaded; `Some(0)` when eligible but not loaded; `None` when not applicable.
    aot_native_proxy_pct: Option<f64>,
}

fn parse_scope_stage_map(c: &serde_json::Value, key: &str) -> BTreeMap<String, BTreeMap<String, u64>> {
    let mut out: BTreeMap<String, BTreeMap<String, u64>> = BTreeMap::new();
    if let Some(obj) = c.get(key).and_then(|x| x.as_object()) {
        for (scope, stage_map) in obj {
            let mut stage_stats = BTreeMap::new();
            if let Some(stages) = stage_map.as_object() {
                for (stage, n) in stages {
                    if let Some(v) = n.as_u64() {
                        stage_stats.insert(stage.clone(), v);
                    }
                }
            }
            out.insert(scope.clone(), stage_stats);
        }
    }
    out
}

fn add_scope_stage_counts_into(
    src: &BTreeMap<String, BTreeMap<String, u64>>,
    stage: &str,
    dst: &mut BTreeMap<String, u64>,
) {
    for (scope, stages) in src {
        if let Some(n) = stages.get(stage) {
            *dst.entry(scope.clone()).or_insert(0) += *n;
        }
    }
}

fn parse_compile_perf_metrics(perf_json_path: &Path) -> Option<ParsedCompilePerf> {
    let text = std::fs::read_to_string(perf_json_path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let c = v.get("compile_perf")?;
    let u = |key: &str| json_get_u64(c, key).unwrap_or(0);
    let salsa_hits = json_get_u64(c, "salsa_process_db_hits").unwrap_or(0);
    let salsa_misses = json_get_u64(c, "salsa_process_db_misses").unwrap_or(0);
    let salsa_evictions = json_get_u64(c, "salsa_process_db_evictions").unwrap_or(0);
    let scope_stage_hits = parse_scope_stage_map(c, "cache_scope_stage_hits");
    let scope_stage_misses = parse_scope_stage_map(c, "cache_scope_stage_misses");
    let scope_stage_invalidations = parse_scope_stage_map(c, "cache_scope_stage_invalidations");
    Some(ParsedCompilePerf {
        salsa_process_db_hits: salsa_hits,
        salsa_process_db_misses: salsa_misses,
        salsa_process_db_evictions: salsa_evictions,
        flatten_inline_ms: u("flatten_inline_ms"),
        flatten_wall_ms: u("flatten_wall_ms"),
        flatten_wall_us: u("flatten_wall_us"),
        inline_wall_ms: u("inline_wall_ms"),
        inline_wall_us: u("inline_wall_us"),
        codegen_wall_ms: u("codegen_wall_ms"),
        codegen_wall_us: u("codegen_wall_us"),
        decl_expand_ms: u("decl_expand_ms"),
        eq_expand_ms: u("eq_expand_ms"),
        inline_substitute_ms: u("inline_substitute_ms"),
        inline_load_model_ms: u("inline_load_model_ms"),
        cache_deserialize_ms: u("cache_deserialize_us") / 1000,
        cache_l0_hits: u("cache_l0_hits"),
        cache_l1_hits: u("cache_l1_hits"),
        cache_l2_hits: u("cache_l2_hits"),
        cache_l0_writes: u("cache_l0_writes"),
        cache_l1_writes: u("cache_l1_writes"),
        cache_l2_writes: u("cache_l2_writes"),
        deps_mismatch: u("deps_mismatch"),
        cache_scope_stage_hits: scope_stage_hits,
        cache_scope_stage_misses: scope_stage_misses,
        cache_scope_stage_invalidations: scope_stage_invalidations,
        stub_compile_ms: u("stub_compile_ms"),
        stub_compile_us: u("stub_compile_us"),
        clock_partition_scan_ms: u("clock_partition_scan_ms"),
        clock_partition_scan_us: u("clock_partition_scan_us"),
        parallel_candidate_share_pct: c
            .get("parallel_candidate_share_pct")
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0),
        cache_warm_ratio: c
            .get("cache_warm_ratio")
            .and_then(|x| x.as_f64())
            .unwrap_or(0.0),
        aot_native_proxy_pct: c
            .get("aot_native_load_status")
            .and_then(|x| x.as_str())
            .and_then(|s| {
                if s.contains("LOADED") {
                    Some(100.0)
                } else if s.eq_ignore_ascii_case("not_eligible") {
                    None
                } else {
                    Some(0.0)
                }
            }),
    })
}

/// `RUSTMODLICA_CACHE_STATS_JSON` payload: `query_cache_counters` plus optional top-level scope/stage maps.
fn parse_cache_stats_json(path: &Path) -> Option<(BTreeMap<String, u64>, BTreeMap<String, BTreeMap<String, u64>>, BTreeMap<String, BTreeMap<String, u64>>, BTreeMap<String, BTreeMap<String, u64>>)> {
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let mut counters = BTreeMap::new();
    if let Some(obj) = v.get("query_cache_counters").and_then(|x| x.as_object()) {
        for (k, val) in obj {
            if let Some(n) = val.as_u64() {
                counters.insert(k.clone(), n);
            }
        }
    }
    let hits = parse_scope_stage_map(&v, "cache_scope_stage_hits");
    let misses = parse_scope_stage_map(&v, "cache_scope_stage_misses");
    let inv = parse_scope_stage_map(&v, "cache_scope_stage_invalidations");
    Some((counters, hits, misses, inv))
}

fn update_min_max(opt_min: &mut Option<u64>, opt_max: &mut Option<u64>, v: u64) {
    *opt_min = Some(opt_min.map(|x| x.min(v)).unwrap_or(v));
    *opt_max = Some(opt_max.map(|x| x.max(v)).unwrap_or(v));
}

fn update_min_max_f64(opt_min: &mut Option<f64>, opt_max: &mut Option<f64>, v: f64) {
    *opt_min = Some(opt_min.map(|x| x.min(v)).unwrap_or(v));
    *opt_max = Some(opt_max.map(|x| x.max(v)).unwrap_or(v));
}

fn update_max_f64(opt: &mut Option<f64>, v: f64) {
    *opt = Some(opt.map(|x| x.max(v)).unwrap_or(v));
}

fn scope_stage_hit_pct(
    hits: &BTreeMap<String, BTreeMap<String, u64>>,
    misses: &BTreeMap<String, BTreeMap<String, u64>>,
    scope: &str,
    stage: &str,
) -> Option<f64> {
    let h = hits
        .get(scope)
        .and_then(|m| m.get(stage))
        .copied()
        .unwrap_or(0);
    let m = misses
        .get(scope)
        .and_then(|m| m.get(stage))
        .copied()
        .unwrap_or(0);
    let t = h + m;
    if t == 0 {
        None
    } else {
        Some(100.0 * h as f64 / t as f64)
    }
}

fn rollup_salsa_db_totals(stats: &PerfStats) -> (u64, u64, u64) {
    let mut h = 0u64;
    let mut m = 0u64;
    let mut e = 0u64;
    for by_model in stats.by_scenario.values() {
        for s in by_model.values() {
            h += s.salsa_process_db_hits_sum;
            m += s.salsa_process_db_misses_sum;
            e += s.salsa_process_db_evictions_sum;
        }
    }
    (h, m, e)
}

fn rollup_cache_layer_totals(stats: &PerfStats) -> (u64, u64, u64, u64, u64, u64) {
    let mut l0h = 0u64;
    let mut l1h = 0u64;
    let mut l2h = 0u64;
    let mut l0w = 0u64;
    let mut l1w = 0u64;
    let mut l2w = 0u64;
    for by_model in stats.by_scenario.values() {
        for m in by_model.values() {
            if let Some(layers) = &m.cache_layer_stats {
                if let Some(v) = layers.get("L0") {
                    l0h += v.hits;
                    l0w += v.writes;
                }
                if let Some(v) = layers.get("L1") {
                    l1h += v.hits;
                    l1w += v.writes;
                }
                if let Some(v) = layers.get("L2") {
                    l2h += v.hits;
                    l2w += v.writes;
                }
            }
        }
    }
    (l0h, l1h, l2h, l0w, l1w, l2w)
}

fn format_validate_perf_track_ab(stats: &PerfStats) -> Option<String> {
    let mut f_us_min: Option<u64> = None;
    let mut f_us_max: Option<u64> = None;
    let mut i_us_min: Option<u64> = None;
    let mut i_us_max: Option<u64> = None;
    let mut c_ms_min: Option<u64> = None;
    let mut c_ms_max: Option<u64> = None;
    let mut c_us_min: Option<u64> = None;
    let mut c_us_max: Option<u64> = None;
    for by_model in stats.by_scenario.values() {
        for m in by_model.values() {
            if let Some(v) = m.flatten_wall_us_min {
                f_us_min = Some(f_us_min.map(|x| x.min(v)).unwrap_or(v));
            }
            if let Some(v) = m.flatten_wall_us_max {
                f_us_max = Some(f_us_max.map(|x| x.max(v)).unwrap_or(v));
            }
            if let Some(v) = m.inline_wall_us_min {
                i_us_min = Some(i_us_min.map(|x| x.min(v)).unwrap_or(v));
            }
            if let Some(v) = m.inline_wall_us_max {
                i_us_max = Some(i_us_max.map(|x| x.max(v)).unwrap_or(v));
            }
            if let Some(v) = m.codegen_wall_ms_min {
                c_ms_min = Some(c_ms_min.map(|x| x.min(v)).unwrap_or(v));
            }
            if let Some(v) = m.codegen_wall_ms_max {
                c_ms_max = Some(c_ms_max.map(|x| x.max(v)).unwrap_or(v));
            }
            if let Some(v) = m.codegen_wall_us_min {
                c_us_min = Some(c_us_min.map(|x| x.min(v)).unwrap_or(v));
            }
            if let Some(v) = m.codegen_wall_us_max {
                c_us_max = Some(c_us_max.map(|x| x.max(v)).unwrap_or(v));
            }
        }
    }
    if f_us_min.is_none()
        && i_us_min.is_none()
        && c_ms_min.is_none()
        && c_us_min.is_none()
    {
        return None;
    }
    Some(format!(
        "jit-validate-perf trackA: flatten_wall_us min/max={:?}/{:?} inline_wall_us min/max={:?}/{:?} | trackB: codegen_wall_ms min/max={:?}/{:?} codegen_wall_us min/max={:?}/{:?}",
        f_us_min, f_us_max, i_us_min, i_us_max, c_ms_min, c_ms_max, c_us_min, c_us_max
    ))
}

fn format_validate_perf_cache_rollup(stats: &PerfStats) -> Option<String> {
    let (l0h, l1h, l2h, l0w, l1w, l2w) = rollup_cache_layer_totals(stats);
    if l0h == 0 && l1h == 0 && l2h == 0 && l0w == 0 && l1w == 0 && l2w == 0 {
        return None;
    }
    let mut qc_keys: HashSet<String> = HashSet::new();
    let mut qc_sum = 0u64;
    for by_model in stats.by_scenario.values() {
        for m in by_model.values() {
            for (k, v) in &m.cache_query_counters {
                qc_keys.insert(k.clone());
                qc_sum += *v;
            }
        }
    }
    let qc_note = if !qc_keys.is_empty() {
        format!(
            " query_cache_counter_distinct_keys={} query_cache_counter_sum={}",
            qc_keys.len(),
            qc_sum
        )
    } else {
        String::new()
    };
    let (sh, sm, se) = rollup_salsa_db_totals(stats);
    let salsa_note = if sh > 0 || sm > 0 || se > 0 {
        format!(
            " | salsa_db hits={} misses={} evictions={}",
            sh, sm, se
        )
    } else {
        String::new()
    };
    Some(format!(
        "jit-validate-perf cache rollup: L0 hits={} writes={} L1 hits={} writes={} L2 hits={} writes={}{}{}",
        l0h, l0w, l1h, l1w, l2h, l2w, qc_note, salsa_note
    ))
}

fn build_perf_stats(out_dir: &Path, cases: &[Case]) -> PerfStats {
    let mut stats = PerfStats::default();
    for c in cases {
        let by_model = stats
            .by_scenario
            .entry(c.scenario.clone())
            .or_default();
        let s = by_model.entry(c.model.clone()).or_default();
        s.runs += 1;
        update_min_max(&mut s.duration_ms_min, &mut s.duration_ms_max, c.duration_ms);
        let mut got_compile_perf = false;
        if let Some(p) = c.perf_json.as_ref() {
            let path = resolve_artifact_path(out_dir, p);
            if let Some(m) = parse_compile_perf_metrics(&path) {
                got_compile_perf = true;
                update_min_max(
                    &mut s.flatten_inline_ms_min,
                    &mut s.flatten_inline_ms_max,
                    m.flatten_inline_ms,
                );
                update_min_max(
                    &mut s.flatten_wall_ms_min,
                    &mut s.flatten_wall_ms_max,
                    m.flatten_wall_ms,
                );
                update_min_max(
                    &mut s.flatten_wall_us_min,
                    &mut s.flatten_wall_us_max,
                    m.flatten_wall_us,
                );
                update_min_max(
                    &mut s.inline_wall_ms_min,
                    &mut s.inline_wall_ms_max,
                    m.inline_wall_ms,
                );
                update_min_max(
                    &mut s.inline_wall_us_min,
                    &mut s.inline_wall_us_max,
                    m.inline_wall_us,
                );
                update_min_max(
                    &mut s.codegen_wall_ms_min,
                    &mut s.codegen_wall_ms_max,
                    m.codegen_wall_ms,
                );
                update_min_max(
                    &mut s.codegen_wall_us_min,
                    &mut s.codegen_wall_us_max,
                    m.codegen_wall_us,
                );
                update_min_max(
                    &mut s.decl_expand_ms_min,
                    &mut s.decl_expand_ms_max,
                    m.decl_expand_ms,
                );
                update_min_max(&mut s.eq_expand_ms_min, &mut s.eq_expand_ms_max, m.eq_expand_ms);
                update_min_max(
                    &mut s.inline_substitute_ms_min,
                    &mut s.inline_substitute_ms_max,
                    m.inline_substitute_ms,
                );
                update_min_max(
                    &mut s.inline_load_model_ms_min,
                    &mut s.inline_load_model_ms_max,
                    m.inline_load_model_ms,
                );
                update_min_max(
                    &mut s.stub_compile_ms_min,
                    &mut s.stub_compile_ms_max,
                    m.stub_compile_ms,
                );
                update_min_max(
                    &mut s.stub_compile_us_min,
                    &mut s.stub_compile_us_max,
                    m.stub_compile_us,
                );
                update_min_max(
                    &mut s.clock_partition_scan_ms_min,
                    &mut s.clock_partition_scan_ms_max,
                    m.clock_partition_scan_ms,
                );
                update_min_max(
                    &mut s.clock_partition_scan_us_min,
                    &mut s.clock_partition_scan_us_max,
                    m.clock_partition_scan_us,
                );
                update_min_max_f64(
                    &mut s.parallel_candidate_share_pct_min,
                    &mut s.parallel_candidate_share_pct_max,
                    m.parallel_candidate_share_pct,
                );
                update_min_max(
                    &mut s.cache_deserialize_ms_min,
                    &mut s.cache_deserialize_ms_max,
                    m.cache_deserialize_ms,
                );
                let layers = s.cache_layer_stats.get_or_insert_with(BTreeMap::new);
                let l0 = layers.entry("L0".to_string()).or_insert_with(LayerStats::default);
                l0.hits += m.cache_l0_hits;
                l0.writes += m.cache_l0_writes;
                let l1 = layers.entry("L1".to_string()).or_insert_with(LayerStats::default);
                l1.hits += m.cache_l1_hits;
                l1.writes += m.cache_l1_writes;
                let l2 = layers.entry("L2".to_string()).or_insert_with(LayerStats::default);
                l2.hits += m.cache_l2_hits;
                l2.writes += m.cache_l2_writes;
                s.salsa_process_db_hits_sum += m.salsa_process_db_hits;
                s.salsa_process_db_misses_sum += m.salsa_process_db_misses;
                s.salsa_process_db_evictions_sum += m.salsa_process_db_evictions;
                if m.deps_mismatch > 0
                    && !l2.recompute_reasons.iter().any(|r| r == "deps_mismatch")
                {
                    l2.recompute_reasons.push("deps_mismatch".to_string());
                }
                for (scope, stage_hits) in &m.cache_scope_stage_hits {
                    let layer = layers
                        .entry(scope.clone())
                        .or_insert_with(LayerStats::default);
                    for (stage, count) in stage_hits {
                        *layer.stage_hits.entry(stage.clone()).or_insert(0) += *count;
                    }
                }
                for (scope, stage_misses) in &m.cache_scope_stage_misses {
                    let layer = layers
                        .entry(scope.clone())
                        .or_insert_with(LayerStats::default);
                    for (stage, count) in stage_misses {
                        layer.misses += *count;
                        *layer.stage_misses.entry(stage.clone()).or_insert(0) += *count;
                    }
                }
                for (scope, stage_invalidations) in &m.cache_scope_stage_invalidations {
                    let layer = layers
                        .entry(scope.clone())
                        .or_insert_with(LayerStats::default);
                    for (stage, count) in stage_invalidations {
                        layer.invalidations += *count;
                        *layer.stage_invalidations.entry(stage.clone()).or_insert(0) += *count;
                    }
                }
                add_scope_stage_counts_into(
                    &m.cache_scope_stage_hits,
                    "flat_full",
                    &mut s.cache_flat_full_layer_hits,
                );
                add_scope_stage_counts_into(
                    &m.cache_scope_stage_misses,
                    "flat_full",
                    &mut s.cache_flat_full_layer_misses,
                );
                add_scope_stage_counts_into(
                    &m.cache_scope_stage_hits,
                    "array_sizes",
                    &mut s.cache_array_sizes_layer_hits,
                );
                add_scope_stage_counts_into(
                    &m.cache_scope_stage_misses,
                    "array_sizes",
                    &mut s.cache_array_sizes_layer_misses,
                );
                if c.run_index == 1 {
                    s.run1_flatten_inline_ms = Some(m.flatten_inline_ms);
                    s.run1_decl_expand_ms = Some(m.decl_expand_ms);
                } else {
                    s.best_after_run1_flatten_inline_ms = Some(
                        s.best_after_run1_flatten_inline_ms
                            .map(|x| x.min(m.flatten_inline_ms))
                            .unwrap_or(m.flatten_inline_ms),
                    );
                    s.best_after_run1_decl_expand_ms = Some(
                        s.best_after_run1_decl_expand_ms
                            .map(|x| x.min(m.decl_expand_ms))
                            .unwrap_or(m.decl_expand_ms),
                    );
                }
                if let Some(v) =
                    scope_stage_hit_pct(&m.cache_scope_stage_hits, &m.cache_scope_stage_misses, "L0", "flat_full")
                {
                    update_max_f64(&mut s.std_flat_full_hit_rate_max, v);
                }
                if let Some(v) =
                    scope_stage_hit_pct(&m.cache_scope_stage_hits, &m.cache_scope_stage_misses, "L1", "flat_full")
                {
                    update_max_f64(&mut s.user_flat_full_hit_rate_max, v);
                }
                if let Some(v) =
                    scope_stage_hit_pct(&m.cache_scope_stage_hits, &m.cache_scope_stage_misses, "L2", "flat_full")
                {
                    update_max_f64(&mut s.l2_flat_full_hit_rate_max, v);
                }
                let proj_wall = m.flatten_wall_ms.saturating_add(m.codegen_wall_ms);
                s.project_rebuild_wall_ms_min = Some(
                    s.project_rebuild_wall_ms_min
                        .map(|x| x.min(proj_wall))
                        .unwrap_or(proj_wall),
                );
                if let Some(p) = m.aot_native_proxy_pct {
                    update_max_f64(&mut s.aot_hit_rate_max, p);
                }
                update_max_f64(&mut s.cache_warm_ratio_max, m.cache_warm_ratio);
            }
        }
        if let Some(p) = c.cache_stats_json.as_ref() {
            let path = resolve_artifact_path(out_dir, p);
            if let Some((qc, cs_hits, cs_misses, cs_inv)) = parse_cache_stats_json(&path) {
                for (k, v) in qc {
                    *s.cache_query_counters.entry(k).or_insert(0) += v;
                }
                if !got_compile_perf {
                    let layers = s.cache_layer_stats.get_or_insert_with(BTreeMap::new);
                    for (scope, stage_hits) in &cs_hits {
                        let layer = layers
                            .entry(scope.clone())
                            .or_insert_with(LayerStats::default);
                        for (stage, count) in stage_hits {
                            *layer.stage_hits.entry(stage.clone()).or_insert(0) += *count;
                        }
                    }
                    for (scope, stage_misses) in &cs_misses {
                        let layer = layers
                            .entry(scope.clone())
                            .or_insert_with(LayerStats::default);
                        for (stage, count) in stage_misses {
                            layer.misses += *count;
                            *layer.stage_misses.entry(stage.clone()).or_insert(0) += *count;
                        }
                    }
                    for (scope, stage_inv) in &cs_inv {
                        let layer = layers
                            .entry(scope.clone())
                            .or_insert_with(LayerStats::default);
                        for (stage, count) in stage_inv {
                            layer.invalidations += *count;
                            *layer.stage_invalidations.entry(stage.clone()).or_insert(0) += *count;
                        }
                    }
                    add_scope_stage_counts_into(
                        &cs_hits,
                        "flat_full",
                        &mut s.cache_flat_full_layer_hits,
                    );
                    add_scope_stage_counts_into(
                        &cs_misses,
                        "flat_full",
                        &mut s.cache_flat_full_layer_misses,
                    );
                    add_scope_stage_counts_into(
                        &cs_hits,
                        "array_sizes",
                        &mut s.cache_array_sizes_layer_hits,
                    );
                    add_scope_stage_counts_into(
                        &cs_misses,
                        "array_sizes",
                        &mut s.cache_array_sizes_layer_misses,
                    );
                }
            }
        }
    }
    stats
}


include!("runner_tail.rs");
