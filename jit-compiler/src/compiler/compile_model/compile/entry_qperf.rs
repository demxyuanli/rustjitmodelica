use std::collections::HashMap;

use crate::compiler::CompilePerfReport;
use crate::flatten::{cache_sqlite, flatten_cache};

use super::super::cache_qperf::{build_cache_scope_stage_map, sum_cache_stage_metric};

pub(crate) fn apply_flatten_qperf(
    perf_report: &mut CompilePerfReport,
    qperf: &HashMap<String, u64>,
    model_name: &str,
    perf_trace: bool,
) {
        perf_report.parse_us = *qperf.get("parse_us").unwrap_or(&0);
        perf_report.inheritance_us = *qperf.get("inheritance_us").unwrap_or(&0);
        perf_report.decl_expand_us = *qperf.get("decl_expand_us").unwrap_or(&0);
        perf_report.decl_expand_param_pass_us =
            *qperf.get("decl_expand_param_pass_us").unwrap_or(&0);
        perf_report.decl_expand_array_dim_us =
            *qperf.get("decl_expand_array_dim_us").unwrap_or(&0);
        perf_report.decl_expand_decl_loop_us =
            *qperf.get("decl_expand_decl_loop_us").unwrap_or(&0);
        perf_report.decl_expand_try_load_sub_model_us =
            *qperf.get("decl_expand_try_load_sub_model_us").unwrap_or(&0);
        perf_report.decl_expand_flatten_inheritance_us =
            *qperf.get("decl_expand_flatten_inheritance_us").unwrap_or(&0);
        perf_report.decl_expand_apply_modification_us =
            *qperf.get("decl_expand_apply_modification_us").unwrap_or(&0);
        perf_report.decl_expand_param_substitute_fold_us =
            *qperf.get("decl_expand_param_substitute_fold_us").unwrap_or(&0);
        perf_report.eq_expand_us = *qperf.get("eq_expand_us").unwrap_or(&0);
        perf_report.eq_expand_prep_us = *qperf.get("eq_expand_prep_us").unwrap_or(&0);
        perf_report.eq_expand_equations_us = *qperf.get("eq_expand_equations_us").unwrap_or(&0);
        perf_report.eq_expand_algorithms_us = *qperf.get("eq_expand_algorithms_us").unwrap_or(&0);
        perf_report.eq_expand_initial_equations_us =
            *qperf.get("eq_expand_initial_equations_us").unwrap_or(&0);
        perf_report.eq_expand_initial_algorithms_us =
            *qperf.get("eq_expand_initial_algorithms_us").unwrap_or(&0);
        perf_report.inherit_flat_template_cache_hit =
            *qperf.get("inherit_flat_template_cache_hit").unwrap_or(&0);
        perf_report.inherit_flat_template_cache_miss =
            *qperf.get("inherit_flat_template_cache_miss").unwrap_or(&0);
        let inherit_flat_template_cache_total = perf_report
            .inherit_flat_template_cache_hit
            .saturating_add(perf_report.inherit_flat_template_cache_miss);
        perf_report.inherit_flat_template_cache_hit_ratio =
            if inherit_flat_template_cache_total == 0 {
                0.0
            } else {
                perf_report.inherit_flat_template_cache_hit as f64
                    / inherit_flat_template_cache_total as f64
            };
        perf_report.resolve_connections_us = *qperf.get("resolve_connections_us").unwrap_or(&0);
        perf_report.clock_infer_us = *qperf.get("clock_infer_us").unwrap_or(&0);
        perf_report.constrainedby_us = *qperf.get("constrainedby_us").unwrap_or(&0);
        perf_report.cache_deps_match_us = *qperf.get("cache_deps_match_us").unwrap_or(&0);
        perf_report.cache_get_us = *qperf.get("cache_get_us").unwrap_or(&0);
        perf_report.cache_deserialize_us = *qperf.get("cache_deserialize_us").unwrap_or(&0);
        perf_report.inline_us = *qperf.get("inline_us").unwrap_or(&0);
        perf_report.inline_substitute_us = *qperf.get("inline_substitute_us").unwrap_or(&0);
        perf_report.inline_load_model_us = *qperf.get("inline_load_model_us").unwrap_or(&0);
        perf_report.inline_call_sites = *qperf.get("inline_call_sites").unwrap_or(&0);
        perf_report.inline_single_output_inlines =
            *qperf.get("inline_single_output_inlines").unwrap_or(&0);
        perf_report.inline_pass_decl_start_values_us =
            *qperf.get("inline_pass_decl_start_values_us").unwrap_or(&0);
        perf_report.inline_pass_equations_us = *qperf.get("inline_pass_equations_us").unwrap_or(&0);
        perf_report.inline_pass_initial_equations_us =
            *qperf.get("inline_pass_initial_equations_us").unwrap_or(&0);
        perf_report.inline_pass_algorithms_us =
            *qperf.get("inline_pass_algorithms_us").unwrap_or(&0);
        perf_report.inline_pass_initial_algorithms_us =
            *qperf.get("inline_pass_initial_algorithms_us").unwrap_or(&0);
        perf_report.inline_resolve_calls = *qperf.get("inline_resolve_calls").unwrap_or(&0);
        perf_report.inline_resolve_first_hit =
            *qperf.get("inline_resolve_first_hit").unwrap_or(&0);
        perf_report.inline_resolve_candidates_total =
            *qperf.get("inline_resolve_candidates_total").unwrap_or(&0);
        perf_report.inline_resolve_probes_total =
            *qperf.get("inline_resolve_probes_total").unwrap_or(&0);
        perf_report.inline_resolve_probe_1 = *qperf.get("inline_resolve_probe_1").unwrap_or(&0);
        perf_report.inline_resolve_probe_2 = *qperf.get("inline_resolve_probe_2").unwrap_or(&0);
        perf_report.inline_resolve_probe_3 = *qperf.get("inline_resolve_probe_3").unwrap_or(&0);
        perf_report.inline_resolve_probe_4 = *qperf.get("inline_resolve_probe_4").unwrap_or(&0);
        perf_report.inline_resolve_probe_ge5 =
            *qperf.get("inline_resolve_probe_ge5").unwrap_or(&0);
        perf_report.inline_input_declarations =
            *qperf.get("inline_input_declarations").unwrap_or(&0) as usize;
        perf_report.inline_input_equations =
            *qperf.get("inline_input_equations").unwrap_or(&0) as usize;
        perf_report.inline_input_initial_equations =
            *qperf.get("inline_input_initial_equations").unwrap_or(&0) as usize;
        perf_report.inline_input_algorithms =
            *qperf.get("inline_input_algorithms").unwrap_or(&0) as usize;
        perf_report.inline_input_initial_algorithms =
            *qperf.get("inline_input_initial_algorithms").unwrap_or(&0) as usize;
        perf_report.inline_declarations_with_start_value =
            *qperf.get("inline_declarations_with_start_value").unwrap_or(&0) as usize;
        perf_report.inline_parallel_poc_enabled =
            *qperf.get("inline_parallel_poc_enabled").unwrap_or(&0) > 0;
        perf_report.flatten_parallel_poc_enabled =
            *qperf.get("flatten_parallel_poc_enabled").unwrap_or(&0) > 0;
        perf_report.guard_cooldown_enter = *qperf.get("guard_cooldown_enter").unwrap_or(&0);
        perf_report.guard_cooldown_active = *qperf.get("guard_cooldown_active").unwrap_or(&0);
        perf_report.guard_cooldown_exit = *qperf.get("guard_cooldown_exit").unwrap_or(&0);
        perf_report.guard_reason = if *qperf.get("guard_reason_policy_off").unwrap_or(&0) > 0 {
            "policy_off".to_string()
        } else if *qperf.get("guard_reason_cooldown_active").unwrap_or(&0) > 0 {
            "cooldown_active".to_string()
        } else if *qperf
            .get("guard_reason_degrade_low_share_small_model")
            .unwrap_or(&0)
            > 0
        {
            "degrade_low_share_small_model".to_string()
        } else if *qperf.get("guard_reason_none").unwrap_or(&0) > 0 {
            "none".to_string()
        } else {
            String::new()
        };
        perf_report.qcache_deps_match_us = *qperf.get("qcache_deps_match_us").unwrap_or(&0);
        perf_report.cache_l0_hits = *qperf.get("cache_L0_hits").unwrap_or(&0);
        perf_report.cache_l1_hits = *qperf.get("cache_L1_hits").unwrap_or(&0);
        perf_report.cache_l2_hits = *qperf.get("cache_L2_hits").unwrap_or(&0);
        perf_report.cache_l0_writes = *qperf.get("cache_L0_writes").unwrap_or(&0);
        perf_report.cache_l1_writes = *qperf.get("cache_L1_writes").unwrap_or(&0);
        perf_report.cache_l2_writes = *qperf.get("cache_L2_writes").unwrap_or(&0);
        perf_report.deps_mismatch = *qperf.get("cache_deps_mismatch").unwrap_or(&0);
        perf_report.cache_scope_stage_hits = build_cache_scope_stage_map(&qperf, "cache_stage_hits:");
        perf_report.cache_scope_stage_misses =
            build_cache_scope_stage_map(&qperf, "cache_stage_misses:");
        perf_report.cache_scope_stage_invalidations =
            build_cache_scope_stage_map(&qperf, "cache_stage_invalidations:");
        perf_report.flat_full_cache_hits = sum_cache_stage_metric(&qperf, "cache_stage_hits:", "flat_full");
        perf_report.flat_full_cache_misses =
            sum_cache_stage_metric(&qperf, "cache_stage_misses:", "flat_full");
        perf_report.flat_full_cache_writes =
            sum_cache_stage_metric(&qperf, "cache_stage_writes:", "flat_full");
        perf_report.flatten_wall_us = *qperf.get("flatten_wall_us").unwrap_or(&0);
        perf_report.inline_wall_us = *qperf.get("inline_wall_us").unwrap_or(&0);
        perf_report.snapshot_write_us = *qperf.get("snapshot_write_us").unwrap_or(&0);
        perf_report.parse_ms = perf_report.parse_us / 1000;
        perf_report.inheritance_ms = perf_report.inheritance_us / 1000;
        perf_report.decl_expand_ms = perf_report.decl_expand_us / 1000;
        perf_report.eq_expand_ms = perf_report.eq_expand_us / 1000;
        perf_report.resolve_connections_ms = perf_report.resolve_connections_us / 1000;
        perf_report.clock_infer_ms = perf_report.clock_infer_us / 1000;
        perf_report.constrainedby_ms = perf_report.constrainedby_us / 1000;
        perf_report.flatten_wall_ms = perf_report.flatten_wall_us / 1000;
        perf_report.inline_wall_ms = perf_report.inline_wall_us / 1000;
        perf_report.snapshot_write_ms = perf_report.snapshot_write_us / 1000;
        perf_report.inline_substitute_ms = perf_report.inline_substitute_us / 1000;
        perf_report.inline_load_model_ms = perf_report.inline_load_model_us / 1000;
        perf_report.inline_pass_decl_start_values_ms =
            perf_report.inline_pass_decl_start_values_us / 1000;
        perf_report.inline_pass_equations_ms = perf_report.inline_pass_equations_us / 1000;
        perf_report.inline_pass_initial_equations_ms =
            perf_report.inline_pass_initial_equations_us / 1000;
        perf_report.inline_pass_algorithms_ms = perf_report.inline_pass_algorithms_us / 1000;
        perf_report.inline_pass_initial_algorithms_ms =
            perf_report.inline_pass_initial_algorithms_us / 1000;
        if perf_trace {
            eprintln!(
                "[perf] query.parse_ms={} parse_us={}",
                perf_report.parse_ms, perf_report.parse_us
            );
            eprintln!(
                "[perf] query.inheritance_ms={} inheritance_us={}",
                perf_report.inheritance_ms, perf_report.inheritance_us
            );
            eprintln!(
                "[perf] query.decl_expand_ms={} decl_expand_us={}",
                perf_report.decl_expand_ms, perf_report.decl_expand_us
            );
            eprintln!(
                "[perf] query.eq_expand_ms={} eq_expand_us={}",
                perf_report.eq_expand_ms, perf_report.eq_expand_us
            );
            eprintln!(
                "[perf] query.decl_expand_breakdown_us param_pass={} array_dim={} decl_loop={}",
                perf_report.decl_expand_param_pass_us,
                perf_report.decl_expand_array_dim_us,
                perf_report.decl_expand_decl_loop_us
            );
            eprintln!(
                "[perf] query.decl_expand_decl_loop_segments_us try_load_sub_model={} flatten_inheritance={} apply_modification={} param_substitute_fold={}",
                perf_report.decl_expand_try_load_sub_model_us,
                perf_report.decl_expand_flatten_inheritance_us,
                perf_report.decl_expand_apply_modification_us,
                perf_report.decl_expand_param_substitute_fold_us
            );
            eprintln!(
                "[perf] query.eq_expand_breakdown_us prep={} equations={} algorithms={} initial_eq={} initial_alg={}",
                perf_report.eq_expand_prep_us,
                perf_report.eq_expand_equations_us,
                perf_report.eq_expand_algorithms_us,
                perf_report.eq_expand_initial_equations_us,
                perf_report.eq_expand_initial_algorithms_us
            );
            eprintln!(
                "[perf] query.inherit_flat_template_cache hit={} miss={} hit_ratio={:.4}",
                perf_report.inherit_flat_template_cache_hit,
                perf_report.inherit_flat_template_cache_miss,
                perf_report.inherit_flat_template_cache_hit_ratio
            );
            eprintln!(
                "[perf] loader.probe cache_miss_calls={} multi_top_scan_count={} multi_top_scan_files={} multi_top_scan_us={} dir_scan_count={} dir_scan_us={}",
                qperf.get("loader_cache_miss_calls").unwrap_or(&0),
                qperf.get("loader_multi_top_scan_count").unwrap_or(&0),
                qperf.get("loader_multi_top_scan_files").unwrap_or(&0),
                qperf.get("loader_multi_top_scan_us").unwrap_or(&0),
                qperf.get("loader_dir_scan_count").unwrap_or(&0),
                qperf.get("loader_dir_scan_us").unwrap_or(&0),
            );
            eprintln!(
                "[perf] inherit.segments_us base_load={} apply_mod={} merge_models={} qualify_short_types={} qualify_probe_count={}",
                qperf.get("inherit_base_load_us").unwrap_or(&0),
                qperf.get("inherit_apply_mod_us").unwrap_or(&0),
                qperf.get("inherit_merge_models_us").unwrap_or(&0),
                qperf.get("qualify_short_types_us").unwrap_or(&0),
                qperf.get("qualify_short_type_probe_count").unwrap_or(&0),
            );
            eprintln!(
                "[perf] query.resolve_connections_ms={} resolve_connections_us={}",
                perf_report.resolve_connections_ms, perf_report.resolve_connections_us
            );
            eprintln!(
                "[perf] query.clock_infer_ms={} clock_infer_us={}",
                perf_report.clock_infer_ms, perf_report.clock_infer_us
            );
            eprintln!(
                "[perf] query.constrainedby_ms={} constrainedby_us={}",
                perf_report.constrainedby_ms, perf_report.constrainedby_us
            );
            eprintln!(
                "[perf] cache.deps_match_us={} cache_get_us={} cache_deserialize_us={} inline_us={}",
                perf_report.cache_deps_match_us,
                perf_report.cache_get_us,
                perf_report.cache_deserialize_us,
                perf_report.inline_us
            );
            eprintln!(
                "[perf] qcache.deps_match_us={}",
                perf_report.qcache_deps_match_us
            );
            eprintln!(
                "[perf] wall.flatten_ms={} inline_ms={} snapshot_write_ms={}",
                perf_report.flatten_wall_ms,
                perf_report.inline_wall_ms,
                perf_report.snapshot_write_ms
            );
            eprintln!(
                "[perf] inline.substitute_ms={} load_model_ms={} call_sites={} single_output_inlines={}",
                perf_report.inline_substitute_ms,
                perf_report.inline_load_model_ms,
                perf_report.inline_call_sites,
                perf_report.inline_single_output_inlines
            );
            eprintln!(
                "[perf] inline.resolve calls={} first_hit={} candidates_total={} probes_total={} buckets=[1:{},2:{},3:{},4:{},5+:{}]",
                perf_report.inline_resolve_calls,
                perf_report.inline_resolve_first_hit,
                perf_report.inline_resolve_candidates_total,
                perf_report.inline_resolve_probes_total,
                perf_report.inline_resolve_probe_1,
                perf_report.inline_resolve_probe_2,
                perf_report.inline_resolve_probe_3,
                perf_report.inline_resolve_probe_4,
                perf_report.inline_resolve_probe_ge5
            );
            eprintln!(
                "[perf] inline.pass_ms decl_start_values={} equations={} initial_equations={} algorithms={} initial_algorithms={}",
                perf_report.inline_pass_decl_start_values_ms,
                perf_report.inline_pass_equations_ms,
                perf_report.inline_pass_initial_equations_ms,
                perf_report.inline_pass_algorithms_ms,
                perf_report.inline_pass_initial_algorithms_ms
            );
            eprintln!(
                "[perf] inline.input_counts decls={} eq={} init_eq={} algs={} init_algs={} decls_with_start={}",
                perf_report.inline_input_declarations,
                perf_report.inline_input_equations,
                perf_report.inline_input_initial_equations,
                perf_report.inline_input_algorithms,
                perf_report.inline_input_initial_algorithms,
                perf_report.inline_declarations_with_start_value
            );
        }
        perf_report.salsa_flat_full_get_us = perf_report
            .cache_get_us
            .saturating_add(perf_report.cache_deserialize_us);
        perf_report.flatten_inline_subst_rewrite_us = perf_report.inline_substitute_us;
        perf_report.flatten_inline_resolve_us = perf_report
            .inline_us
            .saturating_sub(perf_report.inline_substitute_us);
        if let Ok(stats_path) = std::env::var("RUSTMODLICA_CACHE_STATS_JSON") {
            let stats_path = stats_path.trim();
            if !stats_path.is_empty() {
                let mut layers: Vec<cache_sqlite::CacheStatsLayerExport> = Vec::new();
                if let Some(cache_dir) = flatten_cache::flatten_cache_dir() {
                    layers =
                        cache_sqlite::export_sqlite_kind_stats_layers(cache_dir.as_path());
                    if layers.is_empty() {
                        if let Some(cfg) = cache_sqlite::sqlite_config(Some(cache_dir.as_path())) {
                            if let Ok(rows) = cache_sqlite::sqlite_kind_stats(&cfg.path) {
                                layers.push(cache_sqlite::CacheStatsLayerExport {
                                    tier: "legacy".to_string(),
                                    db_path: cfg.path.display().to_string(),
                                    rows,
                                });
                            }
                        }
                    }
                }
                let rows: Vec<cache_sqlite::CacheKindStatRow> = layers
                    .iter()
                    .flat_map(|l| l.rows.iter().cloned())
                    .collect();
                let query_counters: serde_json::Map<String, serde_json::Value> = qperf
                    .iter()
                    .filter(|(k, _)| k.starts_with("cache_"))
                    .map(|(k, v)| (k.clone(), serde_json::json!(v)))
                    .collect();
                let payload = serde_json::json!({
                    "model": model_name,
                    "generated_ms": std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0),
                    "layers": layers,
                    "rows": rows,
                    "query_cache_counters": query_counters,
                    "cache_scope_stage_hits": perf_report.cache_scope_stage_hits,
                    "cache_scope_stage_misses": perf_report.cache_scope_stage_misses,
                    "cache_scope_stage_invalidations": perf_report.cache_scope_stage_invalidations,
                    "flat_full_cache_hits": perf_report.flat_full_cache_hits,
                    "flat_full_cache_misses": perf_report.flat_full_cache_misses,
                    "flat_full_cache_writes": perf_report.flat_full_cache_writes,
                });
                let _ = std::fs::write(stats_path, payload.to_string());
            }
        }
        if let Ok(dep_graph_path) = std::env::var("RUSTMODLICA_DEP_GRAPH_JSON") {
            let p = dep_graph_path.trim();
            if !p.is_empty() {
                let deps = crate::query_db::reverse_dep_snapshot();
                if let Ok(text) = serde_json::to_string(&serde_json::json!({
                    "model": model_name,
                    "generated_ms": std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_millis() as i64)
                        .unwrap_or(0),
                    "entries": deps
                })) {
                    let _ = std::fs::write(p, text);
                }
            }
        }
}
