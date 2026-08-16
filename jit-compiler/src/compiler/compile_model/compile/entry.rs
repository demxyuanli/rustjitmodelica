use std::collections::HashSet;
use std::time::Instant;

use crate::backend_dae::ClockPartition as BackendClockPartition;
use crate::cache::{artifact_cache, artifact_key};
use crate::cache::fold_benefit_record;
use crate::diag::fallback_registry;
use crate::diag::WarningInfo;
use crate::flatten::ArraySizePolicy;
use crate::flatten::ValidationMode;
use crate::flatten::flatten_cache;
use crate::i18n;
use crate::jit::native::builtin_jit_symbol_ptrs;

use crate::compiler::{
    Artifacts, ClockPartitionScheduleEntry, CompileOutput, CompilePerfReport, CompileStopPhase,
    Compiler,
};
use crate::compiler::pipeline::{flatten_and_inline, stage_trace_enabled};

use super::super::disk_cache::{
    analyze_cache, analysis_summary_disk_cache_enabled, analysis_summary_disk_key, flat_model_hash,
    try_read_analysis_summary_disk,
};
use super::super::env_perf::{env_flag, env_u64, perf_trace_enabled};

use super::entry_analyze::{compile_analyze, CompileAnalyzeOutcome};
use super::entry_emit::compile_emit_and_partitions;
use super::entry_external::compile_dae_external;
use super::entry_finish::{apply_fallback_snapshot, compile_finish, CompileFinishState};
use super::entry_qperf::apply_flatten_qperf;
use super::entry_stubs::compile_user_stubs;

pub(crate) fn compile(
    compiler: &mut Compiler,
    model_name: &str,
) -> Result<CompileOutput, Box<dyn std::error::Error + Send + Sync>> {
        if !compiler.options.warm_background {
            crate::cache::warmup_control::bump_compile_epoch();
        }
        struct CompileAuxPersistGuard {
            compiler: *mut Compiler,
            model_name: String,
        }
        impl Drop for CompileAuxPersistGuard {
            fn drop(&mut self) {
                // SAFETY: `compiler` outlives this guard for the whole `compile` call.
                unsafe {
                    let c = &mut *self.compiler;
                    if let Some(ref p) = c.last_compile_perf {
                        if let Some(root) = crate::flatten::flatten_cache_dir() {
                            crate::cache::cache_miss_agg::append_from_report(root.as_path(), p);
                            crate::cache::model_hotness_record::record_compile(
                                Some(root.as_path()),
                                self.model_name.as_str(),
                                p.flat_full_cache_hits,
                            );
                        }
                    }
                }
            }
        }
        let _compile_aux_persist = CompileAuxPersistGuard {
            compiler: compiler as *mut Compiler,
            model_name: model_name.to_string(),
        };

        let stage_trace = stage_trace_enabled();
        let perf_trace = perf_trace_enabled();
        compiler.last_compile_perf = None;
        crate::query_db::perf_reset();
        fold_benefit_record::set_tierup_skip_const_fold(false);

        compiler.warnings.clear();
        compiler.loader.set_quiet(compiler.options.quiet);
        let opts = &compiler.options;
        let model_file_path = format!("{}.mo", model_name.replace('.', "/"));
        let mut perf_report = CompilePerfReport {
            model_name: model_name.to_string(),
            ..Default::default()
        };
        if let Some(w) = crate::cache::warmup::peek_last_warmup_run() {
            perf_report.warmup_populated_count = w.ok;
            perf_report.warmup_time_ms = w.elapsed_ms;
            perf_report.warmup_attributable_hits = w.ok as u64;
        }
        perf_report.jit_incremental_enabled = env_flag("RUSTMODLICA_JIT_INCREMENTAL_RECOMPILE", false);
        perf_report.jit_cache_variant = crate::jit::jit_cache_variant_from_env();
        let env_const_fold_requested = env_flag("RUSTMODLICA_CONST_FOLD", true);
        let env_eq_dce_requested = env_flag("RUSTMODLICA_EQ_DCE", true);
        perf_report.const_fold_enabled = env_const_fold_requested;
        perf_report.eq_dce_enabled = env_eq_dce_requested;
        perf_report.jit_inline_builtins_enabled = env_flag("RUSTMODLICA_JIT_INLINE_BUILTINS", false);
        perf_report.hotspot_threshold = env_u64("RUSTMODLICA_HOTSPOT_THRESHOLD", 1000);
        perf_report.simd_step_enabled = env_flag("RUSTMODLICA_SIMD_STEP", true);
        perf_report.type_specialization_enabled = env_flag("RUSTMODLICA_JIT_TYPE_SPECIALIZATION", false);
        perf_report.stack_scratch_enabled = env_flag("RUSTMODLICA_JIT_STACK_SCRATCH", true);
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
        perf_report.compile_tier = "optimized_jit".to_string();
        perf_report.profile_guided = false;
        perf_report.speculation_guard_count = 0u32;
        perf_report.speculation_invalidation_count = 0u32;
        perf_report.condenser_total_elapsed_us = 0u64;
        perf_report.condenser_artifacts_written = 0u32;
        perf_report.condenser_cache_hits = 0u32;
        perf_report.condenser_errors = 0u32;
        perf_report.dual_compile_requested = opts.dual_compile;
        perf_report.dual_compile_ok = false;
        perf_report.dual_compile_speculation_count = 0u32;
        perf_report.dual_compile_status = if opts.dual_compile {
            CompilePerfReport::DUAL_COMPILE_STATUS_NOT_RUN.to_string()
        } else {
            CompilePerfReport::DUAL_COMPILE_STATUS_OFF.to_string()
        };
        perf_report.dual_compile_error = None;
        perf_report.aot_native_load_status =
            CompilePerfReport::AOT_NATIVE_STATUS_NOT_ELIGIBLE.to_string();
        perf_report.aot_native_load_detail = None;

        let leyden_profile = crate::condenser::training_run::load_cached_profile(model_name);
        if leyden_profile.is_some() {
            perf_report.profile_guided = true;
        }

        fallback_registry::print_fallback_config();
        if matches!(compiler.options.compile_stop, CompileStopPhase::Full) {
            if let Some(_cache_root) = flatten_cache::flatten_cache_dir() {
                let artifact_key = artifact_key::artifact_cache_key(model_name, opts, &compiler.loader);
                if !artifact_cache::artifact_cache_enabled() {
                    perf_report.cache_miss_reason = Some("artifact_cache_disabled".to_string());
                    perf_report.cache_miss_events.push(crate::compiler::CacheMissEvent {
                        layer: "artifact".to_string(),
                        reason: "disabled".to_string(),
                        detail: None,
                    });
                } else if let Some(bundle) =
                    artifact_cache::get(&_cache_root, &artifact_key)
                {
                    if bundle.schema_version != 1 {
                        perf_report.cache_miss_reason = Some("artifact_schema_mismatch".to_string());
                        perf_report.cache_miss_events.push(crate::compiler::CacheMissEvent {
                            layer: "artifact".to_string(),
                            reason: "version_mismatch".to_string(),
                            detail: Some(format!("schema_version={}", bundle.schema_version)),
                        });
                    } else if !crate::cache::closure_hash::deps_match(&bundle.deps) {
                        perf_report.cache_miss_reason = Some("deps_changed".to_string());
                        perf_report.cache_miss_events.push(crate::compiler::CacheMissEvent {
                            layer: "artifact".to_string(),
                            reason: "deps_changed".to_string(),
                            detail: None,
                        });
                    } else if bundle.codegen_key.version
                        != crate::jit::codegen_cache::CODEGEN_CACHE_VERSION
                    {
                        perf_report.cache_miss_reason =
                            Some("codegen_key_version_mismatch".to_string());
                        perf_report.cache_miss_events.push(crate::compiler::CacheMissEvent {
                            layer: "artifact".to_string(),
                            reason: "codegen_version_mismatch".to_string(),
                            detail: Some(format!(
                                "cached={} current={}",
                                bundle.codegen_key.version,
                                crate::jit::codegen_cache::CODEGEN_CACHE_VERSION
                            )),
                        });
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
                        let mut cached_fn: Option<crate::jit::codegen_cache::CachedFunction> = None;
                        #[cfg(any(windows, target_os = "linux"))]
                        {
                            if crate::jit::aot_archive::aot_default_archive_native_load_enabled() {
                                if let Some(archives) =
                                    crate::jit::aot_archive::AotArchiveSet::load_tiered()
                                {
                                            let key_h = bundle.codegen_key.stable_hash();
                                            if let Some((blob, ent)) =
                                                archives.lookup_entry_with_blob(model_name, &key_h)
                                            {
                                                if ent.when_count as usize != bundle.when_count
                                                    || ent.crossings_count as usize
                                                        != bundle.crossings_count
                                                {
                                                    perf_report.aot_native_load_status =
                                                        CompilePerfReport::AOT_NATIVE_STATUS_WRONG_KEY
                                                            .to_string();
                                                    perf_report.aot_native_load_detail = Some(
                                                        "toc_when_crossing_mismatch_vs_bundle"
                                                            .to_string(),
                                                    );
                                                } else if !blob.is_empty() {
                                                    cached_fn = crate::jit::codegen_cache::load_aot_code_blob(
                                                        blob,
                                                        &ent.import_symbols,
                                                        &runtime_symbols,
                                                        ent.when_count as usize,
                                                        ent.crossings_count as usize,
                                                    );
                                                    if cached_fn.is_some() {
                                                        perf_report.aot_native_load_status =
                                                            CompilePerfReport::AOT_NATIVE_STATUS_LOADED
                                                                .to_string();
                                                        perf_report.aot_native_load_detail = Some(
                                                            CompilePerfReport::AOT_NATIVE_DETAIL_TOC_MATCH
                                                                .to_string(),
                                                        );
                                                        perf_report.compile_tier =
                                                            "aot_native_loaded".to_string();
                                                    } else {
                                                        perf_report.aot_native_load_status =
                                                            CompilePerfReport::AOT_NATIVE_STATUS_LOAD_FAILED
                                                                .to_string();
                                                        perf_report.aot_native_load_detail = Some(
                                                            CompilePerfReport::AOT_NATIVE_DETAIL_TOC_MATCH
                                                                .to_string(),
                                                        );
                                                    }
                                                } else {
                                                    perf_report.aot_native_load_status =
                                                        CompilePerfReport::AOT_NATIVE_STATUS_NO_BLOB
                                                            .to_string();
                                                }
                                            } else if let Some(blob) = archives.get_code(&key_h) {
                                                if !blob.is_empty() {
                                                    cached_fn = crate::jit::codegen_cache::load_aot_code_blob(
                                                        blob,
                                                        &Vec::new(),
                                                        &runtime_symbols,
                                                        bundle.when_count,
                                                        bundle.crossings_count,
                                                    );
                                                    if cached_fn.is_some() {
                                                        perf_report.aot_native_load_status =
                                                            CompilePerfReport::AOT_NATIVE_STATUS_LOADED
                                                                .to_string();
                                                        perf_report.aot_native_load_detail = Some(
                                                            CompilePerfReport::AOT_NATIVE_DETAIL_KEY_FALLBACK
                                                                .to_string(),
                                                        );
                                                        perf_report.compile_tier =
                                                            "aot_native_loaded".to_string();
                                                    } else {
                                                        perf_report.aot_native_load_status =
                                                            CompilePerfReport::AOT_NATIVE_STATUS_LOAD_FAILED
                                                                .to_string();
                                                        perf_report.aot_native_load_detail = Some(
                                                            CompilePerfReport::AOT_NATIVE_DETAIL_KEY_FALLBACK
                                                                .to_string(),
                                                        );
                                                    }
                                                } else {
                                                    perf_report.aot_native_load_status =
                                                        CompilePerfReport::AOT_NATIVE_STATUS_NO_BLOB
                                                            .to_string();
                                                }
                                            } else if archives.model_names().iter().any(|n| *n == model_name) {
                                                perf_report.aot_native_load_status =
                                                    CompilePerfReport::AOT_NATIVE_STATUS_WRONG_KEY
                                                        .to_string();
                                            } else {
                                                perf_report.aot_native_load_status =
                                                    CompilePerfReport::AOT_NATIVE_STATUS_MODEL_NOT_IN_ARCHIVE
                                                        .to_string();
                                            }
                                        }
                            } else {
                                perf_report.aot_native_load_status =
                                    CompilePerfReport::AOT_NATIVE_STATUS_DISABLED_BY_ENV.to_string();
                                perf_report.aot_native_load_detail = Some(
                                    "RUSTMODLICA_AOT_NATIVE_LOAD=0".to_string(),
                                );
                            }
                        }
                        if cached_fn.is_none() {
                            cached_fn = cc.get(&bundle.codegen_key, &runtime_symbols, Some(&param_map));
                        }
                        if let Some(cached) = cached_fn {
                            let clock_partitions: Vec<BackendClockPartition> =
                                serde_json::from_str(&bundle.clock_partitions_json)
                                    .unwrap_or_default();
                            let clock_partition_schedule: Vec<ClockPartitionScheduleEntry> =
                                serde_json::from_str(&bundle.clock_partition_schedule_json)
                                    .unwrap_or_default();
                            perf_report.artifact_bundle_cache_status = "hit".to_string();
                            perf_report.structural_cache_hit = true;
                            perf_report.cache_warm_ratio = 1.0;
                            perf_report.cache_structural_hit_count += 1;
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
                                jit_module_keepalive: None,
                                param_only_update: false,
                                dual_compile_generic: None,
                                dual_compile_keepalive: None,
                            }));
                        } else {
                            perf_report.cache_miss_reason =
                                Some("codegen_disk_or_const_fold_miss".to_string());
                            perf_report.cache_miss_events.push(crate::compiler::CacheMissEvent {
                                layer: "codegen".to_string(),
                                reason: "codegen_disk_or_const_fold_miss".to_string(),
                                detail: None,
                            });
                        }
                    }
                } else {
                    perf_report.cache_miss_reason = Some("artifact_key_not_found".to_string());
                    perf_report.cache_miss_events.push(crate::compiler::CacheMissEvent {
                        layer: "artifact".to_string(),
                        reason: "key_not_found".to_string(),
                        detail: Some(format!("key={}", artifact_key.chars().take(40).collect::<String>())),
                    });
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
        perf_report.salsa_process_db_hit =
            crate::query_db::salsa_session::consume_salsa_process_db_hit();
        // Leyden `FlattenCondenser` runs next and calls `compile()` recursively; that inner
        // `compile` executes `query_db::perf_reset()` and would wipe counters gathered above.
        // Snapshot query_db perf here so `CompilePerfReport` / `--perf-json` stay aligned with
        // this compile's flatten+inline work (tier checks, cache_scope_stage_*, wall timers).
        let qperf_after_flatten_inline = crate::query_db::perf_snapshot();
        if perf_trace {
            eprintln!(
                "[perf] compile_phase.flatten_inline_ms={}",
                perf_report.flatten_inline_ms
            );
        }
        let lib_paths_heavy: Vec<std::path::PathBuf> = compiler
            .options
            .external_libs
            .iter()
            .map(std::path::PathBuf::from)
            .collect();
        if crate::cache::warmup::trigger_warmup_if_heavy(
            perf_report.flatten_inline_ms,
            model_name,
            &compiler.loader.loaded_source_paths(),
            &lib_paths_heavy,
        ) {
            perf_report.warmup_auto_enqueued = true;
        }
        {
            let condenser_t0 = Instant::now();
            let mut condenser_reg = crate::condenser::CondenserRegistry::new();
            condenser_reg.register(Box::new(crate::condenser::FlattenCondenser));
            let mut condenser_ctx = crate::condenser::CondenserContext::new(
                model_name,
                crate::condenser::CondenserPhase::FirstRun,
            )
            .with_quiet(compiler.options.quiet);
            if let Some(ref profile) = leyden_profile {
                condenser_ctx = condenser_ctx.with_profile(profile.clone());
            }
            let condenser_results = condenser_reg.run_applicable(&mut condenser_ctx);
            for result in &condenser_results {
                match result {
                    Ok(out) => {
                        if out.cache_hits > 0 {
                            perf_report.condenser_cache_hits += out.cache_hits;
                        }
                        perf_report.condenser_artifacts_written += out.artifacts_written;
                    }
                    Err(_) => {
                        perf_report.condenser_errors += 1;
                    }
                }
            }
            perf_report.condenser_total_elapsed_us += condenser_t0.elapsed().as_micros() as u64;
        }
        apply_flatten_qperf(
            &mut perf_report,
            &qperf_after_flatten_inline,
            model_name,
            perf_trace,
        );
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

        let analyze_out = compile_analyze(
            compiler,
            model_name,
            &flat_model,
            perf_report,
            perf_trace,
            stage_trace,
            &leyden_profile,
            total_equations,
            total_declarations,
            &model_file_path,
        )?;
        let ast = match analyze_out {
            CompileAnalyzeOutcome::Early(o) => return Ok(o),
            CompileAnalyzeOutcome::Continue(s) => s,
        };
        let perf_report = ast.perf_report;
        let ext = compile_dae_external(
            compiler,
            model_name,
            &flat_model,
            perf_report,
            perf_trace,
            ast.flat_h_pipeline,
            &ast.state_vars_sorted,
            &ast.discrete_vars_sorted,
            &ast.param_vars,
            &ast.output_vars,
            &ast.input_var_names,
            &ast.alg_equations,
            &ast.diff_equations,
            &ast.algorithms,
            ast.states.len(),
            ast.initial_variable_count,
            ast.when_equation_count,
            ast.differential_index,
            ast.constraint_equation_count,
            &ast.block_causality,
            &ast.external_sites_pre_fold,
            ast.symbolic_ode_jacobian_matrix.is_some(),
        )?;
        let mut perf_report = ext.perf_report;
        let stub_out = compile_user_stubs(
            compiler,
            &ast.alg_equations,
            &ast.diff_equations,
            &ast.algorithms,
            &ext.external_list,
            &mut perf_report,
        )?;
        let emit_out = compile_emit_and_partitions(
            compiler,
            model_name,
            &flat_model,
            perf_report,
            perf_trace,
            ast.differential_index,
            total_equations,
            total_declarations,
            &ast.state_vars_sorted,
            &ast.discrete_vars_sorted,
            &ast.param_vars,
            &ast.output_vars,
            &ast.alg_equations,
            &ast.diff_equations,
            &ast.algorithms,
            ext.strong_component_jacobians,
            ext.symbolic_ode_jacobian,
            ast.numeric_ode_jacobian,
            &ast.symbolic_ode_jacobian_matrix,
            &ast.ode_jacobian_sparse,
            &ext.simulation_dae,
            ast.blt_degrade_guard_triggered,
            ast.blt_degrade_guard_limit,
            ast.blt_degrade_guard_equation_count,
            ast.symbolic_index_signal_count,
            ast.implicit_derivative_constraint_count,
            &ast.state_var_index,
            &ast.output_var_index,
            &ast.param_var_index,
            &ext.external_list,
            &stub_out.user_function_bodies,
            &ext.backend_clock_partitions,
        )?;
        let perf_report = emit_out.perf_report;
        let external_names: HashSet<String> = ext
            .external_list
            .iter()
            .map(|(n, _, _)| n.clone())
            .collect();
        return compile_finish(
            compiler,
            model_name,
            CompileFinishState {
                perf_report,
                perf_trace,
                external_list: ext.external_list,
                user_stub_ptrs: stub_out.user_stub_ptrs,
                external_names,
                external_t0: ext.external_t0,
                model_file_path,
                state_vars_sorted: ast.state_vars_sorted,
                discrete_vars_sorted: ast.discrete_vars_sorted,
                param_vars: ast.param_vars,
                output_vars: ast.output_vars,
                array_info: ast.array_info,
                params: ast.params,
                flat_model,
                array_size_policy,
                user_stub_jits: stub_out.user_stub_jits,
                leyden_profile,
                alg_equations: ast.alg_equations,
                diff_equations: ast.diff_equations,
                when_equation_count: ast.when_equation_count,
                adaptive_fold: ast.adaptive_fold,
                flat_pipeline_hex: ast.flat_pipeline_hex,
                fold_record_at_start: ast.fold_record_at_start,
                fold_cooldown_at_start: ast.fold_cooldown_at_start,
                states: ast.states,
                discrete_vals: ast.discrete_vals,
                output_start_vals: ast.output_start_vals,
                state_var_index: ast.state_var_index,
                backend_clock_partitions: ext.backend_clock_partitions,
                clock_partition_schedule: emit_out.clock_partition_schedule,
                numeric_ode_jacobian: ast.numeric_ode_jacobian,
                symbolic_ode_jacobian_matrix: ast.symbolic_ode_jacobian_matrix,
                newton_tearing_var_names: emit_out.newton_tearing_var_names,
                dae_differential_index: ext.dae_differential_index,
                ida_component_id: ext.ida_component_id,
                algorithms: ast.algorithms,
                const_fold_param_pairs: ast.const_fold_param_pairs,
                root_model,
            },
        );
}
