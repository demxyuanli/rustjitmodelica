use std::collections::{HashMap, HashSet};
use std::time::Instant;

use rayon::prelude::*;
use xxhash_rust::xxh64::Xxh64;

use crate::analysis::analyze_initial_equations;
use crate::ast::{Equation, Expression};
use crate::backend_dae::{
    build_simulation_dae, ida_component_id_for_states, ClockPartition as BackendClockPartition,
    SimulationDae,
};
use crate::cache::artifact_bundle::CompiledArtifactBundle;
use crate::cache::{artifact_cache, artifact_key};
use crate::cache::external_resolve_cache;
use crate::diag::fallback_counter;
use crate::diag::fallback_counter::FallbackCounterSnapshot;
use crate::diag::fallback_registry;
use crate::diag::WarningInfo;
use crate::flatten::ArraySizePolicy;
use crate::flatten::ValidationMode;
use crate::flatten::{cache_sqlite, flatten_cache};
use crate::i18n;
use crate::jit::native::{builtin_jit_symbol_names, builtin_jit_symbol_ptrs};
use crate::jit::Jit;

use crate::compiler::{
    adaptive::{AdaptiveParameterEngine, ModelStats},
    c_codegen, collect_all_called_names, collect_external_calls, collect_external_raw_call_sites,
    inline, jacobian,
    solvable_scale_warn, Artifacts, ClockPartitionScheduleEntry,
    CompileOutput, CompilePerfReport, CompileStopPhase, Compiler, ValidationAnalyzedSummary,
};
use crate::compiler::pipeline::{
    analyze_equations, build_runtime_algorithms, classify_variables,
    collect_newton_tearing_var_names, flatten_and_inline, stage_trace_enabled,
};

use super::super::aot_coverage::{maybe_coverage_target_warning_message, maybe_write_aot_cache_marker};
use super::super::cache_qperf::{build_cache_scope_stage_map, sum_cache_stage_metric};
use super::super::clock_partition_parse::parse_clock_partition_trigger;
use super::super::constfold_opt::optimize_equations_for_constfold_dce;
use super::super::disk_cache::{
    analyze_cache, analysis_summary_disk_cache_enabled, analysis_summary_disk_key,
    backend_dae_disk_cache_enabled, backend_dae_disk_key, flat_model_hash,
    pipeline_analysis_disk_cache_enabled, pipeline_analysis_disk_key,
    try_read_analysis_summary_disk, try_read_backend_dae_disk, try_read_pipeline_analysis_disk,
    try_write_analysis_summary_disk, try_write_backend_dae_disk, try_write_pipeline_analysis_disk,
    AnalyzeCacheEntry,
};
use super::super::env_perf::{
    env_flag, env_u64, jit_partition_scan_parallel_enabled, jit_stub_parallel_enabled,
    perf_trace_enabled,
};

pub(crate) fn compile(
    compiler: &mut Compiler,
    model_name: &str,
) -> Result<CompileOutput, Box<dyn std::error::Error + Send + Sync>> {
        let stage_trace = stage_trace_enabled();
        let perf_trace = perf_trace_enabled();
        compiler.last_compile_perf = None;
        crate::query_db::perf_reset();

        compiler.warnings.clear();
        compiler.loader.set_quiet(compiler.options.quiet);
        let opts = &compiler.options;
        let model_file_path = format!("{}.mo", model_name.replace('.', "/"));
        let mut perf_report = CompilePerfReport {
            model_name: model_name.to_string(),
            ..Default::default()
        };
        perf_report.jit_incremental_enabled = env_flag("RUSTMODLICA_JIT_INCREMENTAL_RECOMPILE", false);
        perf_report.jit_cache_variant = std::env::var("RUSTMODLICA_JIT_CACHE_VARIANT")
            .ok()
            .unwrap_or_else(|| "speed".to_string());
        perf_report.const_fold_enabled = env_flag("RUSTMODLICA_CONST_FOLD", true);
        perf_report.eq_dce_enabled = env_flag("RUSTMODLICA_EQ_DCE", true);
        perf_report.jit_inline_builtins_enabled = env_flag("RUSTMODLICA_JIT_INLINE_BUILTINS", false);
        perf_report.hotspot_threshold = env_u64("RUSTMODLICA_HOTSPOT_THRESHOLD", 1000);
        perf_report.simd_step_enabled = env_flag("RUSTMODLICA_SIMD_STEP", false);
        perf_report.type_specialization_enabled = env_flag("RUSTMODLICA_JIT_TYPE_SPECIALIZATION", false);
        perf_report.stack_scratch_enabled = env_flag("RUSTMODLICA_JIT_STACK_SCRATCH", false);
        perf_report.runtime_boundary_epoch = env_u64("RUSTMODLICA_RUNTIME_BOUNDARY_EPOCH", 1);
        perf_report.external_resolve_cache_status = "not_run".to_string();
        perf_report.analysis_summary_cache_status = "not_run".to_string();
        perf_report.artifact_bundle_cache_status = "not_run".to_string();
        perf_report.cache_warm_ratio = 0.0;
        perf_report.analysis_pipeline_cache_status = "not_run".to_string();
        perf_report.backend_dae_cache_status = "not_run".to_string();
        perf_report.cache_miss_reason = None;
        perf_report.structural_cache_hit = false;
        perf_report.param_only_update = false;
        perf_report.full_recompile_reason = None;
        let apply_fallback_snapshot = |report: &mut CompilePerfReport| {
            let snap: FallbackCounterSnapshot = fallback_counter::snapshot();
            report.fallback_jit_builtin = snap.jit_builtin;
            report.fallback_jit_variable = snap.jit_variable;
            report.fallback_jit_derivative = snap.jit_derivative;
            report.fallback_jit_equation_skip = snap.jit_equation_skip;
            report.fallback_jit_multi_assign = snap.jit_multi_assign;
            report.fallback_newton_init_accept = snap.newton_init_accept;
            report.fallback_newton_event_accept = snap.newton_event_accept;
            report.fallback_clock_degrade = snap.clock_degrade;
            report.fallback_total = fallback_counter::total(&snap);
        };
        fallback_registry::print_fallback_config();
        if matches!(compiler.options.compile_stop, CompileStopPhase::Full) {
            if let Some(cache_root) = flatten_cache::flatten_cache_dir() {
                let artifact_key = artifact_key::artifact_cache_key(model_name, opts, &compiler.loader);
                if !artifact_cache::artifact_cache_enabled() {
                    perf_report.cache_miss_reason = Some("artifact_cache_disabled".to_string());
                } else if let Some(bundle) = artifact_cache::get(cache_root.as_path(), &artifact_key)
                {
                    if bundle.schema_version != 1 {
                        perf_report.cache_miss_reason = Some("artifact_schema_mismatch".to_string());
                    } else if !crate::cache::closure_hash::deps_match(&bundle.deps) {
                        perf_report.cache_miss_reason = Some("deps_changed".to_string());
                    } else if bundle.codegen_key.version
                        != crate::jit::codegen_cache::CODEGEN_CACHE_VERSION
                    {
                        perf_report.cache_miss_reason =
                            Some("codegen_key_version_mismatch".to_string());
                    } else {
                        let mut runtime_symbols = builtin_jit_symbol_ptrs();
                        for (k, v) in &compiler.external_symbol_ptrs {
                            runtime_symbols.insert(k.clone(), *v);
                        }
                        let cc = crate::jit::codegen_cache::CodegenCache::new();
                        let param_map = crate::jit::codegen_cache::param_values_by_name(
                            &bundle.param_vars,
                            &bundle.params,
                        );
                        if let Some(cached) =
                            cc.get(&bundle.codegen_key, &runtime_symbols, Some(&param_map))
                        {
                            let clock_partitions: Vec<BackendClockPartition> =
                                serde_json::from_str(&bundle.clock_partitions_json)
                                    .unwrap_or_default();
                            let clock_partition_schedule: Vec<ClockPartitionScheduleEntry> =
                                serde_json::from_str(&bundle.clock_partition_schedule_json)
                                    .unwrap_or_default();
                            perf_report.artifact_bundle_cache_status = "hit".to_string();
                            perf_report.structural_cache_hit = true;
                            perf_report.cache_warm_ratio = 1.0;
                            perf_report.jit_compile_ok = true;
                            perf_report.jit_ms = 0;
                            perf_report.codegen_wall_us = 0;
                            perf_report.codegen_wall_ms = 0;
                            apply_fallback_snapshot(&mut perf_report);
                            compiler.last_compile_perf = Some(perf_report);
                            let calc_derivs = cached.func;
                            return Ok(CompileOutput::Simulation(Artifacts {
                                calc_derivs,
                                states: vec![0.0; bundle.state_vars.len()],
                                discrete_vals: vec![0.0; bundle.discrete_vars.len()],
                                params: bundle.params,
                                state_vars: bundle.state_vars,
                                param_vars: bundle.param_vars,
                                discrete_vars: bundle.discrete_vars,
                                output_vars: bundle.output_vars,
                                output_start_vals: bundle.output_start_vals,
                                state_var_index: bundle.state_var_index,
                                clock_partitions,
                                clock_partition_schedule,
                                when_count: bundle.when_count,
                                crossings_count: bundle.crossings_count,
                                t_end: bundle.t_end,
                                dt: bundle.dt,
                                numeric_ode_jacobian: false,
                                symbolic_ode_jacobian: None,
                                newton_tearing_var_names: Vec::new(),
                                atol: bundle.atol,
                                rtol: bundle.rtol,
                                differential_index: bundle.differential_index,
                                ida_component_id: bundle.ida_component_id,
                                solver: bundle.solver,
                                output_interval: bundle.output_interval,
                                result_file: bundle.result_file,
                                user_stub_jits: Vec::new(),
                                calc_derivs_codegen_keepalive: Some(Box::new(cached)),
                                param_only_update: false,
                            }));
                        } else {
                            perf_report.cache_miss_reason =
                                Some("codegen_disk_or_const_fold_miss".to_string());
                        }
                    }
                } else {
                    perf_report.cache_miss_reason = Some("artifact_key_not_found".to_string());
                }
            }
        }
        if !compiler.options.quiet {
            println!(
                "{}",
                i18n::msg("loading_model", &[&model_name as &dyn std::fmt::Display])
            );
        }
        let load_t0 = Instant::now();
        let mut root_model = compiler
            .loader
            .load_model(model_name)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;
        perf_report.load_model_ms = load_t0.elapsed().as_millis() as u64;
        if perf_trace {
            eprintln!("[perf] compile_phase.load_model_ms={}", perf_report.load_model_ms);
        }

        if matches!(compiler.options.compile_stop, CompileStopPhase::Parse) {
            apply_fallback_snapshot(&mut perf_report);
            compiler.last_compile_perf = Some(perf_report);
            return Ok(CompileOutput::ValidationParseOk);
        }

        if root_model.as_ref().is_function {
            if !compiler.options.quiet {
                if compiler.options.function_args.is_some() {
                    println!("{}", i18n::msg0("evaluating_function_args"));
                } else {
                    println!("{}", i18n::msg0("evaluating_function_default"));
                }
            }
            let fr = compiler.run_function_once(model_name);
            let value = match fr {
                Ok(v) => v,
                Err(e) => {
                    if compiler.options.validate_only {
                        let msg = e.to_string();
                        if msg.contains("array/dot/range not supported in function entry eval") {
                            let path = compiler
                                .loader
                                .get_path_for_model(model_name)
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| model_name.to_string());
                            compiler.warnings.push(WarningInfo {
                                path,
                                line: 0,
                                column: 0,
                                message: format!(
                                    "validate: function root accepted without scalar entry eval ({})",
                                    msg
                                ),
                                source: None,
                            });
                            0.0
                        } else {
                            return Err(e);
                        }
                    } else {
                        return Err(e);
                    }
                }
            };
            apply_fallback_snapshot(&mut perf_report);
            compiler.last_compile_perf = Some(perf_report);
            return Ok(CompileOutput::FunctionRun(value));
        }

        if !compiler.options.quiet {
            println!("{}", i18n::msg0("flattening_model"));
        }
        let snap_path = compiler
            .options
            .emit_flat_snapshot
            .as_deref()
            .map(std::path::Path::new);
        let array_sizes_path = compiler
            .options
            .array_sizes_json
            .as_deref()
            .map(std::path::Path::new);
        let array_size_policy = ArraySizePolicy::parse(compiler.options.array_size_policy.as_str());
        let flatten_t0 = Instant::now();
        let frontend = flatten_and_inline(
            &mut root_model,
            model_name,
            &mut compiler.loader,
            compiler.options.compile_stop.clone(),
            compiler.options.validate_only,
            compiler.options.quiet,
            stage_trace,
            snap_path,
            compiler.options.coarse_constrainedby_only,
            ValidationMode::parse(compiler.options.validation_mode.as_str()),
            array_size_policy,
            array_sizes_path,
            compiler.options.warnings_level.as_str(),
        )?;
        perf_report.flatten_inline_ms = flatten_t0.elapsed().as_millis() as u64;
        if perf_trace {
            eprintln!(
                "[perf] compile_phase.flatten_inline_ms={}",
                perf_report.flatten_inline_ms
            );
        }
        let qperf = crate::query_db::perf_snapshot();
        perf_report.parse_us = *qperf.get("parse_us").unwrap_or(&0);
        perf_report.inheritance_us = *qperf.get("inheritance_us").unwrap_or(&0);
        perf_report.decl_expand_us = *qperf.get("decl_expand_us").unwrap_or(&0);
        perf_report.eq_expand_us = *qperf.get("eq_expand_us").unwrap_or(&0);
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
        compiler.last_provenance_index = Some(frontend.provenance_index.clone());
        let flat_model = frontend.flat_model;
        if compiler.options.flat_snapshot_only {
            apply_fallback_snapshot(&mut perf_report);
            compiler.last_compile_perf = Some(perf_report);
            return Ok(CompileOutput::FlatSnapshotDone);
        }
        let total_equations = frontend.total_equations;
        let total_declarations = frontend.total_declarations;
        if matches!(compiler.options.compile_stop, CompileStopPhase::Flatten) {
            apply_fallback_snapshot(&mut perf_report);
            compiler.last_compile_perf = Some(perf_report);
            return Ok(CompileOutput::ValidationFlattenOk {
                total_equations,
                total_declarations,
            });
        }
        if !compiler.options.quiet {
            println!("{}", i18n::msg("flattened_equations", &[&total_equations]));
            println!(
                "{}",
                i18n::msg("flattened_declarations", &[&total_declarations])
            );
            println!("{}", i18n::msg0("analyzing_variables"));
        }

        if matches!(compiler.options.compile_stop, CompileStopPhase::Analyze) {
            let key = flat_model_hash(&flat_model);
            if let Ok(cache) = analyze_cache().read() {
                if let Some(entry) = cache.get(&key) {
                    perf_report.analyze_ms = entry.analyze_ms;
                    perf_report.analysis_summary_cache_status = "mem_hit".to_string();
                    apply_fallback_snapshot(&mut perf_report);
                    compiler.last_compile_perf = Some(perf_report);
                    return Ok(CompileOutput::ValidationAnalyzed(entry.summary.clone()));
                }
            }
            if analysis_summary_disk_cache_enabled() {
                let disk_key = analysis_summary_disk_key(model_name, key, opts);
                if let Some(cache_root) = flatten_cache::flatten_cache_dir() {
                    if let Some(entry) =
                        try_read_analysis_summary_disk(cache_root.as_path(), &disk_key)
                    {
                        perf_report.analyze_ms = entry.analyze_ms;
                        perf_report.analysis_summary_cache_status = "disk_hit".to_string();
                        apply_fallback_snapshot(&mut perf_report);
                        compiler.last_compile_perf = Some(perf_report);
                        return Ok(CompileOutput::ValidationAnalyzed(entry.summary.clone()));
                    }
                }
            }
        }

        let analyze_t0 = Instant::now();
        let flat_h_pipeline = flat_model_hash(&flat_model);
        let (variable_layout, analysis_stage) =
            if pipeline_analysis_disk_cache_enabled() {
                if let Some(cache_root) = flatten_cache::flatten_cache_dir() {
                    let pkey = pipeline_analysis_disk_key(model_name, flat_h_pipeline, opts);
                    if let Some((vl, st)) =
                        try_read_pipeline_analysis_disk(cache_root.as_path(), &pkey)
                    {
                        perf_report.analysis_pipeline_cache_status = "disk_hit".to_string();
                        (vl, st)
                    } else {
                        let mut vl = classify_variables(&flat_model, opts.quiet, stage_trace);
                        if !compiler.options.quiet {
                            println!("{}", i18n::msg0("normalizing_derivatives"));
                            println!("{}", i18n::msg0("performing_structure_analysis"));
                        }
                        let st = analyze_equations(&flat_model, &mut vl, opts, stage_trace);
                        try_write_pipeline_analysis_disk(
                            cache_root.as_path(),
                            &pkey,
                            &vl,
                            &st,
                        );
                        perf_report.analysis_pipeline_cache_status = "disk_put".to_string();
                        (vl, st)
                    }
                } else {
                    perf_report.analysis_pipeline_cache_status = "no_cache_dir".to_string();
                    let mut vl = classify_variables(&flat_model, opts.quiet, stage_trace);
                    if !compiler.options.quiet {
                        println!("{}", i18n::msg0("normalizing_derivatives"));
                        println!("{}", i18n::msg0("performing_structure_analysis"));
                    }
                    let st = analyze_equations(&flat_model, &mut vl, opts, stage_trace);
                    (vl, st)
                }
            } else {
                perf_report.analysis_pipeline_cache_status = "disabled".to_string();
                let mut vl = classify_variables(&flat_model, opts.quiet, stage_trace);
                if !compiler.options.quiet {
                    println!("{}", i18n::msg0("normalizing_derivatives"));
                    println!("{}", i18n::msg0("performing_structure_analysis"));
                }
                let st = analyze_equations(&flat_model, &mut vl, opts, stage_trace);
                (vl, st)
            };
        perf_report.analyze_ms = analyze_t0.elapsed().as_millis() as u64;
        if perf_trace {
            eprintln!("[perf] compile_phase.analyze_ms={}", perf_report.analyze_ms);
        }
        let state_vars_sorted = variable_layout.state_vars;
        let discrete_vars_sorted = variable_layout.discrete_vars;
        let param_vars = variable_layout.param_vars;
        let input_var_names = variable_layout.input_var_names;
        let output_vars = variable_layout.output_vars;
        let output_start_vals = variable_layout.output_start_vals;
        let output_var_index = variable_layout.output_var_index;
        let state_var_index = variable_layout.state_var_index;
        let param_var_index = variable_layout.param_var_index;
        let array_info = variable_layout.array_info;
        let states = variable_layout.states;
        let discrete_vals = variable_layout.discrete_vals;
        let params = variable_layout.params;
        let mut alg_equations = analysis_stage.alg_equations;
        let mut diff_equations = analysis_stage.diff_equations;
        let (folded_alg, dce_alg, folded_vars_alg) = optimize_equations_for_constfold_dce(
            &mut alg_equations,
            perf_report.const_fold_enabled,
            perf_report.eq_dce_enabled,
        );
        let (folded_diff, dce_diff, folded_vars_diff) = optimize_equations_for_constfold_dce(
            &mut diff_equations,
            perf_report.const_fold_enabled,
            perf_report.eq_dce_enabled,
        );
        perf_report.const_fold_count = folded_alg + folded_diff;
        perf_report.eq_dce_removed = dce_alg + dce_diff;
        let param_set: std::collections::HashSet<String> = param_vars.iter().cloned().collect();
        let mut folded_params: Vec<String> = folded_vars_alg
            .into_iter()
            .chain(folded_vars_diff.into_iter())
            .filter(|n| param_set.contains(n))
            .collect();
        folded_params.sort();
        folded_params.dedup();
        perf_report.const_fold_param_count = folded_params.len();
        perf_report.const_fold_param_names = folded_params.join(",");
        let const_fold_param_pairs: Vec<(String, f64)> = folded_params
            .iter()
            .filter_map(|n| {
                param_var_index
                    .get(n)
                    .and_then(|&idx| params.get(idx).map(|&v| (n.clone(), v)))
            })
            .collect();
        let block_causality = analysis_stage.block_causality;
        perf_report.state_count = state_vars_sorted.len();
        perf_report.discrete_count = discrete_vars_sorted.len();
        perf_report.param_count = param_vars.len();
        perf_report.alg_eq_count = alg_equations.len();
        perf_report.diff_eq_count = diff_equations.len();
        let differential_index = analysis_stage.differential_index;
        let constraint_equation_count = analysis_stage.constraint_equation_count;
        let constant_conflict_count = analysis_stage.constant_conflict_count;
        let blt_degrade_guard_triggered = analysis_stage.blt_degrade_guard_triggered;
        let blt_degrade_guard_limit = analysis_stage.blt_degrade_guard_limit;
        let blt_degrade_guard_equation_count = analysis_stage.blt_degrade_guard_equation_count;
        let symbolic_index_signal_count = analysis_stage.symbolic_index_signal_count;
        let implicit_derivative_constraint_count =
            analysis_stage.implicit_derivative_constraint_count;
        perf_report.blt_degrade_guard_triggered = blt_degrade_guard_triggered;
        perf_report.blt_degrade_guard_limit = blt_degrade_guard_limit;
        perf_report.blt_degrade_guard_equation_count = blt_degrade_guard_equation_count;
        perf_report.symbolic_index_signal_count = symbolic_index_signal_count;
        perf_report.implicit_derivative_constraint_count = implicit_derivative_constraint_count;
        let numeric_ode_jacobian = analysis_stage.numeric_ode_jacobian;
        let ode_jacobian_sparse = analysis_stage.ode_jacobian_sparse;
        let symbolic_ode_jacobian_matrix = analysis_stage.symbolic_ode_jacobian_matrix;

        if differential_index > 1 && opts.warnings_level != "none" {
            let method_note = if opts.index_reduction_method == "none" {
                "index reduction not applied (use --index-reduction-method=dummyDerivative); simulation may be unreliable".to_string()
            } else {
                format!(
                    "{} constraint equation(s) before reduction; differential index {}",
                    constraint_equation_count, differential_index
                )
            };
            compiler.warnings.push(WarningInfo {
                path: model_file_path.clone(),
                line: 0,
                column: 0,
                message: format!(
                    "differential index is {}; {}",
                    differential_index, method_note
                ),
                source: None,
            });
        }
        if constant_conflict_count > 0 && opts.warnings_level != "none" {
            compiler.warnings.push(WarningInfo {
                path: model_file_path.clone(),
                line: 0,
                column: 0,
                message: format!(
                    "equation system contains {} constant contradictory equation(s); simulation will fail unless the model is corrected",
                    constant_conflict_count
                ),
                source: None,
            });
        }
        if opts.warnings_level != "none" {
            if let Some(msg) = maybe_coverage_target_warning_message()? {
                compiler.warnings.push(WarningInfo {
                    path: model_file_path.clone(),
                    line: 0,
                    column: 0,
                    message: msg,
                    source: None,
                });
            }
        } else {
            let _ = maybe_coverage_target_warning_message()?;
        }

        // `initial_info` is used by later simulation-only stages. Keep a safe default for
        // non-Full validation modes.
        let mut initial_variable_count = 0usize;
        if ValidationMode::parse(compiler.options.validation_mode.as_str()) == ValidationMode::Full {
            let mut known_at_initial = HashSet::new();
            known_at_initial.insert("time".to_string());
            for p in &param_vars {
                known_at_initial.insert(p.clone());
            }
            let initial_info =
                analyze_initial_equations(&flat_model.initial_equations, &known_at_initial);
            initial_variable_count = initial_info.variable_count;
            if initial_info.is_underdetermined
                && initial_info.equation_count > 0
                && opts.warnings_level != "none"
            {
                compiler.warnings.push(WarningInfo {
                    path: model_file_path.clone(),
                    line: 0,
                    column: 0,
                    message: format!(
                        "initial equation system underdetermined ({} equations, {} unknowns); consistent initialization may be incomplete",
                        initial_info.equation_count, initial_info.variable_count
                    ),
                    source: None,
                });
            }
            if initial_info.is_overdetermined && opts.warnings_level != "none" {
                compiler.warnings.push(WarningInfo {
                    path: model_file_path.clone(),
                    line: 0,
                    column: 0,
                    message: format!(
                        "initial equation system overdetermined ({} equations, {} unknowns)",
                        initial_info.equation_count, initial_info.variable_count
                    ),
                    source: None,
                });
            }
        }
        let algebraic_loops = alg_equations
            .iter()
            .filter(|e| matches!(e, Equation::SolvableBlock { .. }))
            .count();
        if algebraic_loops > 0 && opts.warnings_level != "none" {
            compiler.warnings.push(WarningInfo {
                path: model_file_path.clone(),
                line: 0,
                column: 0,
                message: format!(
                    "{} algebraic loop(s) (strong component(s)) present, solved with tearing",
                    algebraic_loops
                ),
                source: None,
            });
        }

        let when_equation_count = flat_model
            .equations
            .iter()
            .filter(|e| matches!(e, Equation::When(_, _, _)))
            .count();
        let model_stats = ModelStats {
            state_count: state_vars_sorted.len(),
            discrete_count: discrete_vars_sorted.len(),
            param_count: param_vars.len(),
            alg_eq_count: alg_equations.len(),
            diff_eq_count: diff_equations.len(),
            differential_index,
            algebraic_loop_count: algebraic_loops,
            when_count: when_equation_count,
            crossings_count: 0,
            clock_partition_count: flat_model.clock_partitions.len(),
            total_equations,
            total_declarations,
        };
        let adaptive_resolution = AdaptiveParameterEngine::resolve(&model_stats, opts);
        adaptive_resolution.apply_env_overrides();
        perf_report.adaptive_profile = adaptive_resolution.profile_name();
        perf_report.adaptive_override_count = adaptive_resolution.overrides.len();
        perf_report.adaptive_warning_count = adaptive_resolution.warnings.len();
        if opts.warnings_level != "none" {
            for msg in &adaptive_resolution.warnings {
                compiler.warnings.push(WarningInfo {
                    path: model_file_path.clone(),
                    line: 0,
                    column: 0,
                    message: msg.clone(),
                    source: None,
                });
            }
        }

        if matches!(compiler.options.compile_stop, CompileStopPhase::Analyze) {
            let summary = ValidationAnalyzedSummary {
                state_vars: state_vars_sorted.clone(),
                output_vars: output_vars.clone(),
                total_equations,
                total_declarations,
                alg_equation_count: alg_equations.len(),
                diff_equation_count: diff_equations.len(),
            };
            let key = flat_model_hash(&flat_model);
            if let Ok(mut cache) = analyze_cache().write() {
                const MAX_ENTRIES: usize = 256;
                if cache.len() >= MAX_ENTRIES && !cache.contains_key(&key) {
                    if let Some(k) = cache.keys().next().cloned() {
                        cache.remove(&k);
                    }
                }
                cache.insert(
                    key,
                    AnalyzeCacheEntry {
                        summary: summary.clone(),
                        analyze_ms: perf_report.analyze_ms,
                    },
                );
            }
            if analysis_summary_disk_cache_enabled() {
                if let Some(cache_root) = flatten_cache::flatten_cache_dir() {
                    let disk_key = analysis_summary_disk_key(model_name, key, opts);
                    try_write_analysis_summary_disk(
                        cache_root.as_path(),
                        &disk_key,
                        &AnalyzeCacheEntry {
                            summary: summary.clone(),
                            analyze_ms: perf_report.analyze_ms,
                        },
                    );
                    perf_report.analysis_summary_cache_status = "disk_put".to_string();
                } else {
                    perf_report.analysis_summary_cache_status = "no_cache_dir".to_string();
                }
            } else {
                perf_report.analysis_summary_cache_status = "disabled".to_string();
            }
            apply_fallback_snapshot(&mut perf_report);
            compiler.last_compile_perf = Some(perf_report);
            return Ok(CompileOutput::ValidationAnalyzed(summary));
        }

        let symbolic_ode_jacobian = symbolic_ode_jacobian_matrix.is_some();
        let strong_component_jacobians = false;

        let backend_clock_partitions: Vec<BackendClockPartition> = flat_model
            .clock_partitions
            .iter()
            .map(|p| BackendClockPartition {
                id: p.id.clone(),
                var_names: p.var_names.clone(),
            })
            .collect();
        let dae_t0 = Instant::now();
        let simulation_dae: SimulationDae = if backend_dae_disk_cache_enabled() {
            if let Some(cache_root) = flatten_cache::flatten_cache_dir() {
                let dkey = backend_dae_disk_key(model_name, flat_h_pipeline, opts);
                if let Some(cached) = try_read_backend_dae_disk(cache_root.as_path(), &dkey) {
                    perf_report.backend_dae_cache_status = "disk_hit".to_string();
                    cached
                } else {
                    perf_report.backend_dae_cache_status = "miss_compute".to_string();
                    let built = build_simulation_dae(
                        &state_vars_sorted,
                        &discrete_vars_sorted,
                        &param_vars,
                        &output_vars,
                        &input_var_names,
                        diff_equations.len(),
                        &alg_equations,
                        flat_model.initial_equations.len(),
                        initial_variable_count,
                        when_equation_count,
                        differential_index,
                        constraint_equation_count,
                        &backend_clock_partitions,
                        &block_causality,
                    );
                    try_write_backend_dae_disk(cache_root.as_path(), &dkey, &built);
                    perf_report.backend_dae_cache_status = "disk_put".to_string();
                    built
                }
            } else {
                perf_report.backend_dae_cache_status = "no_cache_dir".to_string();
                build_simulation_dae(
                    &state_vars_sorted,
                    &discrete_vars_sorted,
                    &param_vars,
                    &output_vars,
                    &input_var_names,
                    diff_equations.len(),
                    &alg_equations,
                    flat_model.initial_equations.len(),
                    initial_variable_count,
                    when_equation_count,
                    differential_index,
                    constraint_equation_count,
                    &backend_clock_partitions,
                    &block_causality,
                )
            }
        } else {
            perf_report.backend_dae_cache_status = "disabled".to_string();
            build_simulation_dae(
                &state_vars_sorted,
                &discrete_vars_sorted,
                &param_vars,
                &output_vars,
                &input_var_names,
                diff_equations.len(),
                &alg_equations,
                flat_model.initial_equations.len(),
                initial_variable_count,
                when_equation_count,
                differential_index,
                constraint_equation_count,
                &backend_clock_partitions,
                &block_causality,
            )
        };
        let ida_component_id = ida_component_id_for_states(&simulation_dae, states.len());
        let dae_differential_index = simulation_dae.dae.differential_index;
        perf_report.backend_dae_ms = dae_t0.elapsed().as_millis() as u64;
        if perf_trace {
            eprintln!(
                "[perf] compile_phase.backend_dae_ms={}",
                perf_report.backend_dae_ms
            );
        }

        let algorithms = build_runtime_algorithms(&flat_model, stage_trace);

        let external_t0 = Instant::now();
        let external_list = if external_resolve_cache::external_resolve_cache_enabled() {
            let mut sites: Vec<String> = collect_external_raw_call_sites(
                &alg_equations,
                &diff_equations,
                &algorithms,
            )
            .into_iter()
            .collect();
            sites.sort_unstable();
            if let Some(cache_root) = flatten_cache::flatten_cache_dir() {
                let key = external_resolve_cache::compute_external_resolve_key(
                    model_name,
                    &compiler.loader,
                    &sites,
                    &opts.external_libs,
                );
                if let Some(cached) =
                    external_resolve_cache::try_load(cache_root.as_path(), &key)
                {
                    perf_report.external_resolve_cache_status = "hit".to_string();
                    cached
                } else {
                    let computed = collect_external_calls(
                        &mut compiler.loader,
                        &alg_equations,
                        &diff_equations,
                        &algorithms,
                    );
                    perf_report.external_resolve_cache_status =
                        if external_resolve_cache::try_store(cache_root.as_path(), &key, &computed)
                            .is_ok()
                        {
                            "put".to_string()
                        } else {
                            "miss_compute".to_string()
                        };
                    computed
                }
            } else {
                perf_report.external_resolve_cache_status = "no_cache_dir".to_string();
                collect_external_calls(
                    &mut compiler.loader,
                    &alg_equations,
                    &diff_equations,
                    &algorithms,
                )
            }
        } else {
            perf_report.external_resolve_cache_status = "disabled".to_string();
            collect_external_calls(
                &mut compiler.loader,
                &alg_equations,
                &diff_equations,
                &algorithms,
            )
        };

        let all_called = collect_all_called_names(&alg_equations, &diff_equations, &algorithms);
        let external_names: HashSet<String> =
            external_list.iter().map(|(n, _, _)| n.clone()).collect();
        let stub_cap = all_called.len().saturating_sub(external_names.len());
        #[derive(Clone)]
        struct StubPlan {
            name: String,
            input_names: Vec<String>,
            output_expr: Expression,
        }
        let mut user_stub_jits: Vec<Jit> = Vec::new();
        let mut user_stub_ptrs: HashMap<String, *const u8> = HashMap::with_capacity(stub_cap);
        let mut user_function_bodies: HashMap<String, (Vec<String>, Expression)> =
            HashMap::with_capacity(stub_cap);
        fn candidate_load_names(name: &str) -> Vec<String> {
            let mut cands: Vec<String> = Vec::new();
            if name == "valveCharacteristic" {
                cands.push("Modelica.Fluid.Valves.BaseClasses.ValveCharacteristics.linear".to_string());
            }
            if name.starts_with("world.") {
                cands.push(format!(
                    "Modelica.Mechanics.MultiBody.World.{}",
                    name.trim_start_matches("world.")
                ));
            }
            if name.starts_with("BaseClasses.") {
                let rest = name.trim_start_matches("BaseClasses.");
                cands.push(format!("Modelica.Fluid.Valves.BaseClasses.{}", rest));
                cands.push(format!("Modelica.Fluid.Pipes.BaseClasses.{}", rest));
                cands.push(format!("Modelica.Fluid.Machines.BaseClasses.{}", rest));
                cands.push(format!("Modelica.Fluid.Utilities.{}", name));
            }
            if name.starts_with("Utilities.") {
                cands.push(format!("Modelica.Fluid.{}", name));
                cands.push(format!("Modelica.{}", name));
            }
            if let Some(rest) = name.strip_prefix("from_") {
                cands.push(format!(
                    "Modelica.Blocks.Math.UnitConversions.From_{}",
                    rest
                ));
            }
            if let Some(rest) = name.strip_prefix("to_") {
                cands.push(format!(
                    "Modelica.Blocks.Math.UnitConversions.To_{}",
                    rest
                ));
            }
            if name == "arg" {
                cands.push("Modelica.ComplexMath.arg".to_string());
            }
            if name == "distribution" {
                cands.push("Modelica.Blocks.Noise.Interfaces.distribution".to_string());
                cands.push("Modelica.Math.Distributions.distribution".to_string());
            }
            if name == "oneTrue" {
                cands.push("Modelica.Electrical.Batteries.Utilities.oneTrue".to_string());
            }
            if name == "isPowerOf2" {
                cands.push("Modelica.Electrical.Polyphase.Functions.isPowerOf2".to_string());
                cands.push("Modelica.Electrical.QuasiStatic.Polyphase.Functions.isPowerOf2".to_string());
            }
            if name == "numberOfSymmetricBaseSystems" {
                cands.push("Modelica.Electrical.Polyphase.Functions.numberOfSymmetricBaseSystems".to_string());
                cands.push("Modelica.Electrical.QuasiStatic.Polyphase.Functions.numberOfSymmetricBaseSystems".to_string());
            }
            if name == "factorY2DC" {
                cands.push("Modelica.Electrical.Polyphase.Functions.factorY2DC".to_string());
                cands.push("Modelica.Electrical.QuasiStatic.Polyphase.Functions.factorY2DC".to_string());
            }
            if name == "exlin" {
                cands.push("Modelica.Electrical.Analog.Semiconductors.exlin".to_string());
            }
            if name == "exlin2" {
                cands.push("Modelica.Electrical.Analog.Semiconductors.exlin2".to_string());
            }
            if name.starts_with("Machines.") {
                cands.push(format!("Modelica.Electrical.{}", name));
            }
            if name.starts_with("Mechanics.") {
                cands.push(format!("Modelica.{}", name));
            }
            if name == "Cv" {
                cands.push("Modelica.Units.Conversions".to_string());
            } else if let Some(rest) = name.strip_prefix("Cv.") {
                cands.push(format!("Modelica.Units.Conversions.{}", rest));
            }
            cands.push(name.to_string());
            // Deduplicate while preserving order.
            let mut dedup = HashSet::new();
            cands
                .into_iter()
                .filter(|n| dedup.insert(n.clone()))
                .collect()
        }

        let stub_compile_t0 = Instant::now();
        let mut stub_plans: Vec<StubPlan> = Vec::new();
        perf_report.stub_parallel_enabled = jit_stub_parallel_enabled();
        if perf_report.stub_parallel_enabled {
            let loader_paths = compiler.loader.library_paths.clone();
            let scan_results: Vec<Result<Option<StubPlan>, String>> = all_called
                .par_iter()
                .map(|name| {
                    if inline::is_builtin_function(name) || external_names.contains(name) {
                        return Ok(None);
                    }
                    let mut local_loader = crate::loader::ModelLoader::new();
                    local_loader.set_quiet(true);
                    for p in &loader_paths {
                        local_loader.add_path(p.clone());
                    }
                    let mut func_model = None;
                    for cand in candidate_load_names(name) {
                        if let Ok(m) = local_loader.load_model(&cand) {
                            func_model = Some(m);
                            break;
                        }
                    }
                    let Some(func_model) = func_model else {
                        return Ok(None);
                    };
                    if func_model.external_info.is_some() {
                        return Ok(None);
                    }
                    let Some((input_names, outputs)) = inline::get_function_body(func_model.as_ref()) else {
                        return Ok(None);
                    };
                    if outputs.len() != 1 {
                        return Err(format!(
                            "Function '{}' has {} outputs; JIT callable supports single-output only (FUNC-2).",
                            name, outputs.len()
                        ));
                    }
                    Ok(Some(StubPlan {
                        name: name.clone(),
                        input_names: input_names.clone(),
                        output_expr: outputs[0].1.clone(),
                    }))
                })
                .collect();
            for item in scan_results {
                match item {
                    Ok(Some(p)) => stub_plans.push(p),
                    Ok(None) => {}
                    Err(e) => return Err(e.into()),
                }
            }
            stub_plans.sort_by(|a, b| a.name.cmp(&b.name));
            stub_plans.dedup_by(|a, b| a.name == b.name);
        } else {
            for name in &all_called {
                if inline::is_builtin_function(name) || external_names.contains(name) {
                    continue;
                }
                let mut func_model = None;
                for cand in candidate_load_names(name) {
                    if let Ok(m) = compiler.loader.load_model(&cand) {
                        func_model = Some(m);
                        break;
                    }
                }
                let Some(func_model) = func_model else {
                    continue;
                };
                if func_model.external_info.is_some() {
                    continue;
                }
                let Some((input_names, outputs)) = inline::get_function_body(func_model.as_ref()) else {
                    continue;
                };
                if outputs.len() != 1 {
                    return Err(format!(
                        "Function '{}' has {} outputs; JIT callable supports single-output only (FUNC-2).",
                        name, outputs.len()
                    ).into());
                }
                stub_plans.push(StubPlan {
                    name: name.clone(),
                    input_names: input_names.clone(),
                    output_expr: outputs[0].1.clone(),
                });
            }
        }
        perf_report.stub_candidate_count = stub_plans.len();
        for p in stub_plans {
            let mut stub_jit = Jit::new();
            let ptr = stub_jit
                .compile_user_function_stub(&p.name, &p.input_names, &p.output_expr)
                .map_err(|e| format!("JIT stub for '{}': {}", p.name, e))?;
            user_stub_ptrs.insert(p.name.clone(), ptr);
            user_function_bodies.insert(p.name, (p.input_names, p.output_expr));
            user_stub_jits.push(stub_jit);
        }
        let stub_elapsed = stub_compile_t0.elapsed();
        perf_report.stub_compile_ms = stub_elapsed.as_millis() as u64;
        perf_report.stub_compile_us = stub_elapsed.as_micros() as u64;

        if opts.backend_dae_info {
            jacobian::print_backend_dae_info(
                opts,
                differential_index,
                total_equations,
                total_declarations,
                &state_vars_sorted,
                &discrete_vars_sorted,
                &param_vars,
                &output_vars,
                &flat_model.clocked_var_names,
                &flat_model.equations,
                &alg_equations,
                &flat_model.equations,
                &flat_model.algorithms,
                strong_component_jacobians,
                symbolic_ode_jacobian,
                numeric_ode_jacobian,
                symbolic_ode_jacobian_matrix.as_ref(),
                ode_jacobian_sparse.as_ref(),
                Some(&simulation_dae),
                blt_degrade_guard_triggered,
                blt_degrade_guard_limit,
                blt_degrade_guard_equation_count,
                symbolic_index_signal_count,
                implicit_derivative_constraint_count,
            );
            for (a, b) in &flat_model.clock_signal_connections {
                println!(" * Clock connect (SYNC-6): {} <-> {}", a, b);
            }
        }

        if let Some(ref dir) = compiler.options.emit_c_dir {
            let path = std::path::Path::new(dir);
            let jac = symbolic_ode_jacobian_matrix.as_deref();
            let state_array_layout: Vec<(String, usize, usize)> = flat_model
                .array_sizes
                .iter()
                .filter_map(|(name, &size)| {
                    let first = format!("{}_1", name);
                    state_var_index
                        .get(&first)
                        .copied()
                        .map(|start| (name.clone(), start, size))
                })
                .collect();
            let output_array_layout: Vec<(String, usize, usize)> = flat_model
                .array_sizes
                .iter()
                .filter_map(|(name, &size)| {
                    let first = format!("{}_1", name);
                    output_var_index
                        .get(&first)
                        .copied()
                        .map(|start| (name.clone(), start, size))
                })
                .collect();
            let param_array_layout: Vec<(String, usize, usize)> = flat_model
                .array_sizes
                .iter()
                .filter_map(|(name, &size)| {
                    let first = format!("{}_1", name);
                    param_var_index
                        .get(&first)
                        .copied()
                        .map(|start| (name.clone(), start, size))
                })
                .collect();
            let state_layout_opt = if state_array_layout.is_empty() {
                None
            } else {
                Some(state_array_layout.as_slice())
            };
            let output_layout_opt = if output_array_layout.is_empty() {
                None
            } else {
                Some(output_array_layout.as_slice())
            };
            let param_layout_opt = if param_array_layout.is_empty() {
                None
            } else {
                Some(param_array_layout.as_slice())
            };
            let external_c_names: HashMap<String, String> = external_list
                .iter()
                .map(|(m, c, _)| (m.clone(), c.clone()))
                .collect();
            let external_c_names_opt = if external_c_names.is_empty() {
                None
            } else {
                Some(external_c_names)
            };
            let external_names_set: HashSet<String> =
                external_list.iter().map(|(n, _, _)| n.clone()).collect();
            let user_fn_bodies_opt = if user_function_bodies.is_empty() {
                None
            } else {
                Some(&user_function_bodies)
            };
            match c_codegen::emit_c_files(
                path,
                &state_vars_sorted,
                &param_vars,
                &output_vars,
                &alg_equations,
                jac,
                state_layout_opt,
                output_layout_opt,
                param_layout_opt,
                external_c_names_opt,
                Some(&external_names_set),
                user_fn_bodies_opt,
            ) {
                Ok(files) => {
                    let paths: Vec<String> =
                        files.iter().map(|p| p.display().to_string()).collect();
                    println!("{}", i18n::msg("c_codegen_emitted", &[&paths.join(", ")]));
                }
                Err(e) => {
                    return Err(format!(
                        "C codegen failed: {}{}",
                        e,
                        compiler.source_loc_suffix(model_name)
                    )
                    .into());
                }
            }
        }

        // F2-1: Fail at compile time with clear message if unsupported der(expr) is present
        for eq in alg_equations.iter().chain(diff_equations.iter()) {
            if let Some(hint) = crate::analysis::find_unsupported_der_in_eq(eq) {
                return Err(format!("Unsupported nested der(): {}. (F2-1)", hint).into());
            }
        }

        // 5. JIT Compile
        if !compiler.options.quiet {
            println!("{}", i18n::msg0("jit_compiling"));
            println!(
                "{}",
                i18n::msg("equations_after_sorting", &[&alg_equations.len()])
            );
            println!(
                "{}",
                i18n::msg("state_variables", &[&state_vars_sorted.len()])
            );
            println!(
                "{}",
                i18n::msg("discrete_variables", &[&discrete_vars_sorted.len()])
            );
            println!("{}", i18n::msg("parameters_count", &[&param_vars.len()]));
        }

        let newton_tearing_var_names = collect_newton_tearing_var_names(&alg_equations);
        let aot_cache_status =
            maybe_write_aot_cache_marker(model_name, &alg_equations, &diff_equations, opts);
        perf_report.aot_cache_status = aot_cache_status.as_str().to_string();
        if perf_trace {
            eprintln!("[perf] aot_cache_status={}", aot_cache_status.as_str());
        }
        let partition_scan_t0 = Instant::now();
        perf_report.clock_partition_parallel_enabled = jit_partition_scan_parallel_enabled();
        let build_partition_entry = |part: &BackendClockPartition| {
            let mut var_names: Vec<String> = part.var_names.iter().cloned().collect();
            var_names.sort_unstable();
            let var_set: HashSet<String> = var_names.iter().cloned().collect();

            let mut algorithm_indices = Vec::new();
            for (idx, stmt) in algorithms.iter().enumerate() {
                let mut modified = HashSet::new();
                crate::jit::analysis::collect_modified(stmt, &mut modified);
                if modified.iter().any(|v| var_set.contains(v)) {
                    algorithm_indices.push(idx);
                }
            }

            let mut alg_equation_indices = Vec::new();
            for (idx, eq) in alg_equations.iter().enumerate() {
                let mut modified = HashSet::new();
                crate::jit::analysis::collect_modified_equations(std::slice::from_ref(eq), &mut modified);
                if modified.iter().any(|v| var_set.contains(v)) {
                    alg_equation_indices.push(idx);
                }
            }

            let mut diff_equation_indices = Vec::new();
            for (idx, eq) in diff_equations.iter().enumerate() {
                let mut modified = HashSet::new();
                crate::jit::analysis::collect_modified_equations(std::slice::from_ref(eq), &mut modified);
                if modified.iter().any(|v| var_set.contains(v)) {
                    diff_equation_indices.push(idx);
                }
            }
            ClockPartitionScheduleEntry {
                id: part.id.clone(),
                trigger: parse_clock_partition_trigger(&part.id),
                var_names,
                algorithm_indices,
                alg_equation_indices,
                diff_equation_indices,
            }
        };
        let mut clock_partition_schedule: Vec<ClockPartitionScheduleEntry> =
            if perf_report.clock_partition_parallel_enabled && backend_clock_partitions.len() > 1 {
                backend_clock_partitions.par_iter().map(build_partition_entry).collect()
            } else {
                backend_clock_partitions.iter().map(build_partition_entry).collect()
            };
        clock_partition_schedule.sort_by(|a, b| a.id.cmp(&b.id));
        let partition_elapsed = partition_scan_t0.elapsed();
        perf_report.clock_partition_scan_ms = partition_elapsed.as_millis() as u64;
        perf_report.clock_partition_scan_us = partition_elapsed.as_micros() as u64;

        let lib_paths: Vec<std::path::PathBuf> = if !compiler.options.external_libs.is_empty() {
            compiler.options
                .external_libs
                .iter()
                .map(|p| std::path::PathBuf::from(p))
                .collect()
        } else {
            let mut from_annotation: Vec<std::path::PathBuf> = external_list
                .iter()
                .filter_map(|(_, _, hint)| hint.as_ref())
                .map(|lib_name| {
                    let ext = std::env::consts::DLL_EXTENSION;
                    std::path::PathBuf::from(format!("{}.{}", lib_name, ext))
                })
                .collect();
            from_annotation.sort();
            from_annotation.dedup();
            from_annotation
        };
        if !lib_paths.is_empty() {
            compiler.external_libraries.0.clear();
            compiler.external_symbol_ptrs.clear();
            for path in &lib_paths {
                let lib = unsafe { libloading::Library::new(path.as_path()) }.map_err(|e| {
                    format!("Failed to load external lib '{}': {}", path.display(), e)
                })?;
                for (modelica_name, c_name, _) in &external_list {
                    if compiler.external_symbol_ptrs.contains_key(modelica_name) {
                        continue;
                    }
                    if let Ok(sym) = unsafe { lib.get::<extern "C" fn()>(c_name.as_bytes()) } {
                        let ptr = *sym as *const u8;
                        compiler.external_symbol_ptrs.insert(modelica_name.clone(), ptr);
                    }
                }
                compiler.external_libraries.0.push(lib);
            }
            for (modelica_name, _c_name, _) in &external_list {
                if !compiler.external_symbol_ptrs.contains_key(modelica_name) {
                    return Err(format!(
                        "EXT-2: external function '{}' not found in any loaded library (--external-lib or annotation Library)",
                        modelica_name
                    ).into());
                }
            }
        }

        let mut all_symbols = compiler.external_symbol_ptrs.clone();
        for (k, v) in user_stub_ptrs {
            all_symbols.insert(k, v);
        }
        for (modelica_name, c_name, _) in &external_list {
            if all_symbols.contains_key(modelica_name) {
                continue;
            }
            if let Some(ptr) = crate::jit::native::jit_stub_for_external_c_name(c_name) {
                all_symbols.insert(modelica_name.clone(), ptr);
            }
        }

        let builtins = builtin_jit_symbol_names();
        for name in &external_names {
            if builtins.contains(name.as_str()) || all_symbols.contains_key(name) {
                continue;
            }
            return Err(format!(
                "External function '{}' is not linked. Provide a shared library with this symbol (e.g. --external-lib=<path> or Library annotation).",
                name
            ).into());
        }
        if perf_trace {
            perf_report.external_resolve_ms = external_t0.elapsed().as_millis() as u64;
            eprintln!(
                "[perf] compile_phase.external_resolve_ms={}",
                perf_report.external_resolve_ms
            );
        } else {
            perf_report.external_resolve_ms = external_t0.elapsed().as_millis() as u64;
        }

        let t_end = compiler.options.t_end;
        let dt = compiler.options.dt;
        solvable_scale_warn::push_dense_newton_scale_warnings(
            &alg_equations,
            &mut compiler.warnings,
            model_file_path.clone(),
            &compiler.options.warnings_level,
        );
        if let Some(ref path) = compiler.options.jit_policy_json {
            if std::env::var_os("RUSTMODLICA_JIT_POLICY_JSON").is_none() && !path.trim().is_empty() {
                std::env::set_var("RUSTMODLICA_JIT_POLICY_JSON", path.as_str());
            }
        }
        let compile_stop_s = match compiler.options.compile_stop {
            CompileStopPhase::Full => "full",
            CompileStopPhase::Parse => "parse",
            CompileStopPhase::Flatten => "flatten",
            CompileStopPhase::Analyze => "analyze",
        };
        let layout_fp = crate::cache::sim_bundle_cache::var_layout_fingerprint(
            &state_vars_sorted,
            &discrete_vars_sorted,
            &param_vars,
            &output_vars,
        );
        let connector_connection_degree =
            crate::jit::build_connector_connection_degree(&flat_model.connections);
        let codegen_ck = crate::jit::calc_derivs_codegen_cache_key(
            &state_vars_sorted,
            &discrete_vars_sorted,
            &param_vars,
            &output_vars,
            &array_info,
            &params,
            Some(&connector_connection_degree),
        );
        let sim_bundle_storage = flatten_cache::flatten_cache_dir().and_then(|cache_root| {
            let flat_full_key = flatten_cache::flatten_full_cache_key(
                model_name,
                &compiler.loader,
                ValidationMode::parse(compiler.options.validation_mode.as_str()),
                compile_stop_s,
                compiler.options.coarse_constrainedby_only,
                array_sizes_path,
                array_size_policy,
                compiler.options.warnings_level.as_str(),
            );
            Some((
                cache_root,
                crate::cache::sim_bundle_cache::storage_key(&flat_full_key, model_name),
            ))
        });
        if !crate::cache::sim_bundle_cache::sim_bundle_cache_enabled() {
            perf_report.artifact_bundle_cache_status = "disabled".to_string();
        } else if !user_stub_jits.is_empty() {
            perf_report.artifact_bundle_cache_status = "skipped_stubs".to_string();
        } else if !crate::jit::codegen_cache::codegen_cache_enabled() {
            perf_report.artifact_bundle_cache_status = "skipped_codegen_disk".to_string();
        } else {
            perf_report.artifact_bundle_cache_status = "miss".to_string();
            if let Some((ref cache_root, ref sk)) = sim_bundle_storage {
                if let Some(bundle) =
                    crate::cache::sim_bundle_cache::try_load(cache_root.as_path(), sk)
                {
                    if bundle.var_layout_fingerprint == layout_fp
                        && bundle.codegen_key.stable_hash() == codegen_ck.stable_hash()
                    {
                        let cc = crate::jit::codegen_cache::CodegenCache::new();
                        let param_map =
                            crate::jit::codegen_cache::param_values_by_name(&param_vars, &params);
                        if let Some(cached) =
                            cc.get(&bundle.codegen_key, &all_symbols, Some(&param_map))
                        {
                            let calc_derivs = cached.func;
                            let when_count_b = bundle.when_count;
                            let crossings_count_b = bundle.crossings_count;
                            perf_report.jit_inline_builtin_hits =
                                crate::jit::translator::expr::take_inline_builtin_hits();
                            let mut type_hash = Xxh64::new(0);
                            for p in &params {
                                let is_integer_like = (p.fract().abs() < 1e-12) as u8;
                                type_hash.update(&[is_integer_like]);
                            }
                            perf_report.type_profile_hash =
                                if perf_report.type_specialization_enabled {
                                    format!("{:016x}", type_hash.digest())
                                } else {
                                    "disabled".to_string()
                                };
                            perf_report.jit_cache_partial_recompile =
                                perf_report.jit_incremental_enabled;
                            perf_report.jit_cache_recompiled_functions = 0;
                            perf_report.jit_cache_skipped_functions = 0;
                            perf_report.jit_ms = 0;
                            perf_report.codegen_wall_us = 0;
                            perf_report.codegen_wall_ms = 0;
                            perf_report.artifact_bundle_cache_status = "hit".to_string();
                            perf_report.structural_cache_hit = true;
                            perf_report.cache_warm_ratio = 1.0;
                            let frontend_flatten_seg_us = perf_report
                                .decl_expand_us
                                .saturating_add(perf_report.eq_expand_us)
                                .saturating_add(perf_report.resolve_connections_us);
                            let frontend_inline_seg_us = perf_report
                                .inline_pass_decl_start_values_us
                                .saturating_add(perf_report.inline_pass_equations_us)
                                .saturating_add(perf_report.inline_pass_initial_equations_us)
                                .saturating_add(perf_report.inline_pass_algorithms_us)
                                .saturating_add(perf_report.inline_pass_initial_algorithms_us);
                            let seg_us = frontend_flatten_seg_us
                                .saturating_add(frontend_inline_seg_us)
                                .saturating_add(perf_report.stub_compile_us)
                                .saturating_add(perf_report.clock_partition_scan_us);
                            let total_us = perf_report.flatten_wall_us.max(1);
                            perf_report.parallel_candidate_share_pct = (seg_us as f64
                                * 100.0_f64
                                / total_us as f64)
                                .clamp(0.0, 100.0);
                            crate::query_db::eq_parallel_guard_update_candidate_share(
                                model_name,
                                perf_report.parallel_candidate_share_pct,
                            );
                            perf_report.jit_compile_ok = true;
                            perf_report.jit_error = None;
                            apply_fallback_snapshot(&mut perf_report);
                            compiler.last_compile_perf = Some(perf_report);
                            let cf = Box::new(cached);
                            return Ok(CompileOutput::Simulation(Artifacts {
                                calc_derivs,
                                states,
                                discrete_vals,
                                params,
                                state_vars: state_vars_sorted,
                                param_vars,
                                discrete_vars: discrete_vars_sorted,
                                output_vars,
                                output_start_vals,
                                state_var_index,
                                clock_partitions: backend_clock_partitions,
                                clock_partition_schedule,
                                when_count: when_count_b,
                                crossings_count: crossings_count_b,
                                t_end,
                                dt,
                                numeric_ode_jacobian,
                                symbolic_ode_jacobian: symbolic_ode_jacobian_matrix,
                                newton_tearing_var_names,
                                atol: compiler.options.atol,
                                rtol: compiler.options.rtol,
                                differential_index: dae_differential_index,
                                ida_component_id,
                                solver: compiler.options.solver.clone(),
                                output_interval: compiler.options.output_interval,
                                result_file: compiler.options.result_file.clone(),
                                user_stub_jits,
                                calc_derivs_codegen_keepalive: Some(cf),
                                param_only_update: false,
                            }));
                        } else {
                            perf_report.cache_miss_reason =
                                Some("sim_bundle_codegen_miss".to_string());
                        }
                    }
                }
            }
        }
        let mut jit = if all_symbols.is_empty() {
            Jit::new()
        } else {
            Jit::new_with_extra_symbols(Some(&all_symbols))
        };
        let jit_t0 = Instant::now();
        let res = jit.compile(
            &state_vars_sorted,
            &discrete_vars_sorted,
            &param_vars,
            &output_vars,
            &array_info,
            &alg_equations,
            &diff_equations,
            &algorithms,
            &clock_partition_schedule,
            t_end,
            &params,
            &newton_tearing_var_names,
            &external_names,
            &const_fold_param_pairs,
            &flat_model.stream_connection_set,
            &flat_model.stream_flow_map,
            &connector_connection_degree,
        );
        let jit_elapsed = jit_t0.elapsed();
        perf_report.jit_inline_builtin_hits = crate::jit::translator::expr::take_inline_builtin_hits();
        let mut type_hash = Xxh64::new(0);
        for p in &params {
            let is_integer_like = (p.fract().abs() < 1e-12) as u8;
            type_hash.update(&[is_integer_like]);
        }
        perf_report.type_profile_hash = if perf_report.type_specialization_enabled {
            format!("{:016x}", type_hash.digest())
        } else {
            "disabled".to_string()
        };
        perf_report.jit_cache_partial_recompile = perf_report.jit_incremental_enabled;
        perf_report.jit_cache_recompiled_functions = if perf_report.jit_incremental_enabled { 1 } else { 0 };
        perf_report.jit_cache_skipped_functions = if perf_report.jit_incremental_enabled { 1 } else { 0 };
        perf_report.jit_ms = jit_elapsed.as_millis() as u64;
        perf_report.codegen_wall_us = jit_elapsed.as_micros() as u64;
        perf_report.codegen_wall_ms = perf_report.codegen_wall_us / 1000;
        let frontend_flatten_seg_us = perf_report
            .decl_expand_us
            .saturating_add(perf_report.eq_expand_us)
            .saturating_add(perf_report.resolve_connections_us);
        let frontend_inline_seg_us = perf_report
            .inline_pass_decl_start_values_us
            .saturating_add(perf_report.inline_pass_equations_us)
            .saturating_add(perf_report.inline_pass_initial_equations_us)
            .saturating_add(perf_report.inline_pass_algorithms_us)
            .saturating_add(perf_report.inline_pass_initial_algorithms_us);
        let seg_us = frontend_flatten_seg_us
            .saturating_add(frontend_inline_seg_us)
            .saturating_add(perf_report.stub_compile_us)
            .saturating_add(perf_report.clock_partition_scan_us);
        let total_us = perf_report
            .flatten_wall_us
            .saturating_add(perf_report.codegen_wall_us)
            .max(1);
        perf_report.parallel_candidate_share_pct =
            (seg_us as f64 * 100.0_f64 / total_us as f64).clamp(0.0, 100.0);
        crate::query_db::eq_parallel_guard_update_candidate_share(
            model_name,
            perf_report.parallel_candidate_share_pct,
        );
        if perf_trace {
            eprintln!(
                "[perf] tracks trackA_flatten_wall_us={} trackA_inline_wall_us={} trackB_codegen_wall_us={} trackB_jit_ms={} frontend_flatten_seg_ms={} frontend_inline_seg_ms={} stub_compile_ms={} partition_scan_ms={} flatten_parallel_poc_enabled={} inline_parallel_poc_enabled={} parallel_candidate_share_pct={:.2}",
                perf_report.flatten_wall_us,
                perf_report.inline_wall_us,
                perf_report.codegen_wall_us,
                perf_report.jit_ms,
                frontend_flatten_seg_us / 1000,
                frontend_inline_seg_us / 1000,
                perf_report.stub_compile_ms,
                perf_report.clock_partition_scan_ms,
                perf_report.flatten_parallel_poc_enabled,
                perf_report.inline_parallel_poc_enabled,
                perf_report.parallel_candidate_share_pct
            );
            eprintln!(
                "[perf] guard cooldown_enter={} cooldown_active={} cooldown_exit={} reason={}",
                perf_report.guard_cooldown_enter,
                perf_report.guard_cooldown_active,
                perf_report.guard_cooldown_exit,
                perf_report.guard_reason
            );
            eprintln!("[perf] compile_phase.jit_ms={}", perf_report.jit_ms);
        }

        match res {
            Ok((calc_derivs, when_count, crossings_count)) => {
                perf_report.full_recompile_reason = Some("jit_full_compile".to_string());
                if matches!(compiler.options.compile_stop, CompileStopPhase::Full) {
                    if let Some(cache_root) = flatten_cache::flatten_cache_dir() {
                        let artifact_key_s = artifact_key::artifact_cache_key(model_name, opts, &compiler.loader);
                        let mut deps = Vec::new();
                        for p in compiler.loader.loaded_source_paths() {
                            if let Some(h) = crate::cache::closure_hash::unified_file_hash(&p) {
                                deps.push(crate::flatten::flat_cache_v1::DepHashEntry {
                                    path: p.display().to_string(),
                                    content_hash: h,
                                });
                            }
                        }
                        let codegen_key = crate::jit::calc_derivs_codegen_cache_key(
                            &state_vars_sorted,
                            &discrete_vars_sorted,
                            &param_vars,
                            &output_vars,
                            &array_info,
                            &params,
                            Some(&connector_connection_degree),
                        );
                        let bundle = CompiledArtifactBundle {
                            schema_version: 1,
                            model_name: model_name.to_string(),
                            codegen_key,
                            deps,
                            libs_fingerprint: artifact_key::libs_fingerprint(&compiler.loader),
                            compile_flags_hash: artifact_key::compile_flags_hash(opts),
                            when_count,
                            crossings_count,
                            state_vars: state_vars_sorted.clone(),
                            discrete_vars: discrete_vars_sorted.clone(),
                            param_vars: param_vars.clone(),
                            output_vars: output_vars.clone(),
                            state_var_index: state_var_index.clone(),
                            output_start_vals: output_start_vals.clone(),
                            params: params.clone(),
                            t_end,
                            dt,
                            atol: compiler.options.atol,
                            rtol: compiler.options.rtol,
                            differential_index: dae_differential_index,
                            ida_component_id: ida_component_id.clone(),
                            solver: compiler.options.solver.clone(),
                            output_interval: compiler.options.output_interval,
                            result_file: compiler.options.result_file.clone(),
                            artifact_kind: "codegen_key".to_string(),
                            clock_partitions_json: serde_json::to_string(&backend_clock_partitions)
                                .unwrap_or_default(),
                            clock_partition_schedule_json: serde_json::to_string(
                                &clock_partition_schedule,
                            )
                            .unwrap_or_default(),
                        };
                        if artifact_cache::artifact_deferred_write() {
                            compiler.deferred_artifact = Some(
                                crate::compiler::DeferredArtifactWrite {
                                    cache_root: cache_root.clone(),
                                    key: artifact_key_s,
                                    bundle,
                                },
                            );
                            perf_report.artifact_bundle_cache_status = "deferred".to_string();
                        } else {
                            let _ = artifact_cache::put(cache_root.as_path(), &artifact_key_s, &bundle);
                            perf_report.artifact_bundle_cache_status = "put".to_string();
                        }
                    }
                }
                perf_report.jit_compile_ok = true;
                perf_report.jit_error = None;
                apply_fallback_snapshot(&mut perf_report);
                compiler.last_compile_perf = Some(perf_report);
                if user_stub_jits.is_empty()
                    && crate::cache::sim_bundle_cache::sim_bundle_cache_enabled()
                {
                    if let Some((ref cache_root, ref sk)) = sim_bundle_storage {
                        let bundle = crate::cache::sim_bundle_cache::CompiledSimBundle {
                            schema_ver: 1,
                            codegen_key: codegen_ck.clone(),
                            when_count,
                            crossings_count,
                            var_layout_fingerprint: layout_fp.clone(),
                        };
                        let _ = crate::cache::sim_bundle_cache::try_store(
                            cache_root.as_path(),
                            sk,
                            &bundle,
                        );
                    }
                }
                Ok(CompileOutput::Simulation(Artifacts {
                    calc_derivs,
                    states,
                    discrete_vals,
                    params,
                    state_vars: state_vars_sorted,
                    param_vars,
                    discrete_vars: discrete_vars_sorted,
                    output_vars,
                    output_start_vals,
                    state_var_index,
                    clock_partitions: backend_clock_partitions,
                    clock_partition_schedule,
                    when_count,
                    crossings_count,
                    t_end,
                    dt,
                    numeric_ode_jacobian,
                    symbolic_ode_jacobian: symbolic_ode_jacobian_matrix,
                    newton_tearing_var_names,
                    atol: compiler.options.atol,
                    rtol: compiler.options.rtol,
                    differential_index: dae_differential_index,
                    ida_component_id,
                    solver: compiler.options.solver.clone(),
                    output_interval: compiler.options.output_interval,
                    result_file: compiler.options.result_file.clone(),
                    user_stub_jits,
                    calc_derivs_codegen_keepalive: None,
                    param_only_update: false,
                }))
            }
            Err(e) => {
                let err_text = e.to_string();
                perf_report.jit_compile_ok = false;
                perf_report.jit_error = Some(err_text.clone());
                apply_fallback_snapshot(&mut perf_report);
                compiler.last_compile_perf = Some(perf_report);
                Err(format!(
                    "JIT compilation failed: {}{}",
                    err_text,
                    compiler.source_loc_suffix(model_name)
                )
                .into())
            }
        }

}
