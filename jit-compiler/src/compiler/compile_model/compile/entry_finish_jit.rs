use std::collections::HashMap;
use std::time::Instant;

use xxhash_rust::xxh64::Xxh64;

use crate::cache::artifact_bundle::CompiledArtifactBundle;
use crate::cache::fold_benefit_record;
use crate::cache::{artifact_cache, artifact_key};
use crate::compiler::{
    Artifacts, CompileOutput, CompilePerfReport, CompileStopPhase, Compiler, DualCompileErrorReport,
};
use crate::flatten::ValidationMode;
use crate::flatten::flatten_cache;
use crate::jit::native::builtin_jit_symbol_ptrs;
use crate::jit::Jit;

use super::super::env_perf::env_flag;
use super::entry_finish::{apply_fallback_snapshot, FinishLayout};
use super::entry_finish_bundle::FinishBundleKeys;

pub(crate) fn compile_finish_jit(
    compiler: &mut Compiler,
    model_name: &str,
    layout: FinishLayout,
    keys: FinishBundleKeys,
    all_symbols: HashMap<String, *const u8>,
    lib_paths: Vec<std::path::PathBuf>,
    mut perf_report: CompilePerfReport,
    perf_trace: bool,
    adaptive_fold: bool,
    flat_pipeline_hex: String,
    fold_record_at_start: Option<crate::cache::fold_benefit_record::FoldBenefitRecord>,
    fold_cooldown_at_start: bool,
    leyden_profile: Option<crate::condenser::profile_data::ModelProfile>,
) -> Result<CompileOutput, Box<dyn std::error::Error + Send + Sync>> {
    let FinishLayout {
        state_vars_sorted,
        discrete_vars_sorted,
        param_vars,
        output_vars,
        array_info,
        params,
        flat_model,
        user_stub_jits,
        alg_equations,
        diff_equations,
        when_equation_count,
        states,
        discrete_vals,
        output_start_vals,
        state_var_index,
        backend_clock_partitions,
        clock_partition_schedule,
        numeric_ode_jacobian,
        symbolic_ode_jacobian_matrix,
        newton_tearing_var_names,
        dae_differential_index,
        ida_component_id,
        algorithms,
        const_fold_param_pairs,
        root_model,
        external_names,
    } = layout;
    let FinishBundleKeys {
        t_end,
        dt,
        compile_stop_s,
        layout_fp,
        connector_connection_degree,
        codegen_ck,
        codegen_ck_hash,
        sim_bundle_key,
    } = keys;
    let opts = &compiler.options;
        let aot_archive_hit = {
            if !crate::jit::aot_archive::aot_default_archive_native_load_enabled() {
                false
            } else if let Some(archives) = crate::jit::aot_archive::AotArchiveSet::load_tiered() {
                let found = archives.model_names().iter().any(|n| *n == model_name);
                if found && perf_trace {
                    eprintln!("[perf] aot_archive=hit model={}", model_name);
                }
                found
            } else {
                false
            }
        };
        if let Some(ref profile) = leyden_profile {
            crate::jit::speculation::init_global_registry(profile);
            if let Ok(reg) = crate::jit::speculation::global_registry().read() {
                perf_report.speculation_guard_count = reg.total_guard_count() as u32;
            }
        }
        let eq_count = alg_equations.len() + diff_equations.len();
        let tiering_policy = crate::jit::tiered::TieringPolicy::for_equation_count(eq_count);
        let selected_tier = tiering_policy.select_initial_tier(eq_count);
        perf_report.tierup_step_threshold = tiering_policy.tierup_step_threshold;
        perf_report.newton_sparse_policy = match crate::solvable_limits::newton_sparse_policy_from_env()
        {
            crate::solvable_limits::NewtonSparsePolicy::Dense => "dense".to_string(),
            crate::solvable_limits::NewtonSparsePolicy::Sparse => "sparse".to_string(),
            crate::solvable_limits::NewtonSparsePolicy::Auto => "auto".to_string(),
        };
        // Prefer NEWTON_PATH when set (same as JIT path preference).
        if let Ok(raw) = std::env::var("RUSTMODLICA_NEWTON_PATH") {
            let s = raw.trim().to_ascii_lowercase();
            if matches!(s.as_str(), "dense" | "dense_only") {
                perf_report.newton_sparse_policy = "dense".to_string();
            } else if matches!(s.as_str(), "sparse" | "csr" | "sparse_only") {
                perf_report.newton_sparse_policy = "sparse".to_string();
            } else if !s.is_empty() {
                perf_report.newton_sparse_policy = "auto".to_string();
            }
        }
        if selected_tier == crate::jit::tiered::CompileTier::Interpreter
            && crate::jit::interpreter::EquationInterpreter::is_interpretable(
                alg_equations.len(),
                diff_equations.len(),
                state_vars_sorted.len(),
            )
        {
            crate::jit::interpreter::install_interpreter_context(
                model_name.to_string(),
                state_vars_sorted.clone(),
                param_vars.clone(),
                diff_equations.clone(),
                alg_equations.to_vec(),
            );
        }
        perf_report.compile_tier = match selected_tier {
            crate::jit::tiered::CompileTier::Interpreter => "interpreter".to_string(),
            crate::jit::tiered::CompileTier::FastJit => "fast_jit".to_string(),
            crate::jit::tiered::CompileTier::OptimizedJit => "optimized_jit".to_string(),
            crate::jit::tiered::CompileTier::ProfileGuided => "profile_guided".to_string(),
        };
        if aot_archive_hit {
            perf_report.compile_tier = "aot_cached".to_string();
        }

        let mut aot_native_blob: Option<Vec<u8>> = None;
        let mut aot_native_cf: Option<crate::jit::codegen_cache::CachedFunction> = None;
        let mut aot_native_when_cross: Option<(usize, usize)> = None;
        perf_report.aot_native_load_status =
            CompilePerfReport::AOT_NATIVE_STATUS_NO_MATCHING_ENTRY.to_string();
        perf_report.aot_native_load_detail = None;

        if !crate::jit::aot_archive::aot_default_archive_native_load_enabled() {
            perf_report.aot_native_load_status =
                CompilePerfReport::AOT_NATIVE_STATUS_DISABLED_BY_ENV.to_string();
            perf_report.aot_native_load_detail =
                Some("RUSTMODLICA_AOT_NATIVE_LOAD=0".to_string());
        } else if let Some(archives) = crate::jit::aot_archive::AotArchiveSet::load_tiered() {
                    let key_h = codegen_ck.stable_hash();
                    if let Some((blob, ent)) =
                        archives.lookup_entry_with_blob(model_name, &key_h)
                    {
                        if ent.when_count as usize != when_equation_count {
                            perf_report.aot_native_load_status =
                                CompilePerfReport::AOT_NATIVE_STATUS_WRONG_KEY.to_string();
                            perf_report.aot_native_load_detail =
                                Some("toc_when_count_mismatch_vs_flatten".to_string());
                        } else if blob.is_empty() {
                            perf_report.aot_native_load_status =
                                CompilePerfReport::AOT_NATIVE_STATUS_NO_BLOB.to_string();
                        } else {
                            #[cfg(any(windows, target_os = "linux"))]
                            {
                                let mut runtime_symbols = builtin_jit_symbol_ptrs();
                                for (k, v) in &all_symbols {
                                    runtime_symbols.insert(k.clone(), *v);
                                }
                                let wc = ent.when_count as usize;
                                let cc = ent.crossings_count as usize;
                                if let Some(cf) = crate::jit::codegen_cache::load_aot_code_blob(
                                    blob,
                                    &ent.import_symbols,
                                    &runtime_symbols,
                                    wc,
                                    cc,
                                ) {
                                    aot_native_blob = Some(blob.to_vec());
                                    aot_native_cf = Some(cf);
                                    aot_native_when_cross = Some((wc, cc));
                                    perf_report.aot_native_load_status =
                                        CompilePerfReport::AOT_NATIVE_STATUS_LOADED.to_string();
                                    perf_report.aot_native_load_detail = Some(
                                        CompilePerfReport::AOT_NATIVE_DETAIL_TOC_MATCH.to_string(),
                                    );
                                    perf_report.compile_tier = "aot_native_loaded".to_string();
                                    perf_report.structural_cache_hit = true;
                                    perf_report.cache_warm_ratio =
                                        perf_report.compute_warm_ratio(true);
                                    if perf_trace {
                                        eprintln!(
                                            "[perf] aot_native=loaded model={} bytes={}",
                                            model_name,
                                            blob.len()
                                        );
                                    }
                                } else {
                                    perf_report.aot_native_load_status =
                                        CompilePerfReport::AOT_NATIVE_STATUS_LOAD_FAILED.to_string();
                                    perf_report.aot_native_load_detail = Some(
                                        CompilePerfReport::AOT_NATIVE_DETAIL_TOC_MATCH.to_string(),
                                    );
                                }
                            }
                            #[cfg(not(any(windows, target_os = "linux")))]
                            {
                                perf_report.aot_native_load_status =
                                    CompilePerfReport::AOT_NATIVE_STATUS_UNSUPPORTED_OS.to_string();
                            }
                        }
                    } else if archives.model_names().iter().any(|n| *n == model_name) {
                        perf_report.aot_native_load_status =
                            CompilePerfReport::AOT_NATIVE_STATUS_WRONG_KEY.to_string();
                    } else {
                        perf_report.aot_native_load_status =
                            CompilePerfReport::AOT_NATIVE_STATUS_MODEL_NOT_IN_ARCHIVE.to_string();
                    }
        } else {
            perf_report.aot_native_load_status =
                CompilePerfReport::AOT_NATIVE_STATUS_NO_ARCHIVE_FILE.to_string();
        }

        let mut aot_jit_codegen_keepalive: Option<Box<crate::jit::codegen_cache::CachedFunction>> =
            None;

        let mut jit = if all_symbols.is_empty() {
            Jit::new()
        } else {
            Jit::new_with_extra_symbols(Some(&all_symbols))
        };
        jit.set_active_model_name(model_name);
        let tier0_max_eq: usize = std::env::var("RUSTMODLICA_JIT_TIER0_BYPASS_MAX_EQ")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(150);
        let tier0_bypass = env_flag("RUSTMODLICA_JIT_TIER0_BYPASS", false)
            && aot_native_cf.is_none()
            && perf_report.flat_full_cache_hits > 0
            && when_equation_count == 0
            && eq_count < tier0_max_eq
            && crate::jit::interpreter::EquationInterpreter::is_interpretable(
                alg_equations.len(),
                diff_equations.len(),
                state_vars_sorted.len(),
            );
        if tier0_bypass {
            crate::jit::interpreter::install_interpreter_context(
                model_name.to_string(),
                state_vars_sorted.clone(),
                param_vars.clone(),
                diff_equations.clone(),
                alg_equations.to_vec(),
            );
        }
        let jit_t0 = Instant::now();
        let res = if let Some(cf) = aot_native_cf {
            let (w, c) = aot_native_when_cross.unwrap_or((0, 0));
            let f = cf.func;
            aot_jit_codegen_keepalive = Some(Box::new(cf));
            Ok((f, w, c))
        } else if tier0_bypass {
            perf_report.jit_bypassed_tier0 = true;
            Ok((
                crate::jit::interpreter::interpreter_trampoline
                    as crate::jit::types::CalcDerivsFunc,
                when_equation_count,
                0usize,
            ))
        } else {
            jit.compile(
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
                &root_model.enumerations,
            )
        };
        // Background tier-up: leak the Jit so its JITModule's mmap region
        // (which backs calc_derivs pointer) survives; without this the fn
        // pointer dangles after the Jit drops (J7 tier-up UAF).
        //
        // In the normal path, move the Jit into the returned Artifacts
        // instead of dropping it here: a jit-codegen disk cache HIT stores
        // its relocated ExecCodeBuffer in `jit.disk_cache_live`, and dropping
        // the Jit frees that executable memory while `calc_derivs` still
        // points into it (warm-cache STATUS_ACCESS_VIOLATION, J12).
        let mut jit_keepalive: Option<Box<crate::jit::Jit>> = None;
        if compiler.options.jit_leak {
            let _: &'static mut _ = Box::leak(Box::new(jit));
        } else {
            jit_keepalive = Some(Box::new(jit));
        }
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
        {
            let qperf_jit = crate::query_db::perf_snapshot();
            perf_report.newton_sparse_blocks =
                *qperf_jit.get("newton_sparse_blocks").unwrap_or(&0);
            perf_report.newton_dense_blocks = *qperf_jit.get("newton_dense_blocks").unwrap_or(&0);
            perf_report.newton_sparse_nnz_total =
                *qperf_jit.get("newton_sparse_nnz_total").unwrap_or(&0);
        }
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
                let aot_native_used_here = aot_jit_codegen_keepalive.is_some();
                perf_report.full_recompile_reason = if aot_native_used_here {
                    Some("aot_native_blob".to_string())
                } else {
                    Some("jit_full_compile".to_string())
                };
                perf_report.cache_full_recompile_count += 1;
                if !aot_native_used_here {
                    perf_report.cache_warm_ratio = perf_report.compute_warm_ratio(false);
                }
                if matches!(compiler.options.compile_stop, CompileStopPhase::Full) {
                    if let Some(_cache_root) = flatten_cache::flatten_cache_dir() {
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
                            model_name,
                            &compiler.loader,
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
                                    model_name: model_name.to_string(),
                                    key: artifact_key_s,
                                    bundle,
                                },
                            );
                            perf_report.artifact_bundle_cache_status = "deferred".to_string();
                        } else {
                            let _ = artifact_cache::put(
                                &_cache_root,
                                &artifact_key_s,
                                &bundle,
                            );
                            perf_report.artifact_bundle_cache_status = "put".to_string();
                        }
                    }
                }
                {
                    let condenser_t0 = Instant::now();
                    let mut condenser_reg = crate::condenser::CondenserRegistry::new();
                    condenser_reg.register(Box::new(crate::condenser::CodegenCondenser));
                    let mut condenser_ctx = crate::condenser::CondenserContext::new(
                        model_name,
                        crate::condenser::CondenserPhase::Warmup,
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
                                perf_report.condenser_artifacts_written +=
                                    out.artifacts_written;
                            }
                            Err(_) => {
                                perf_report.condenser_errors += 1;
                            }
                        }
                    }
                    perf_report.condenser_total_elapsed_us +=
                        condenser_t0.elapsed().as_micros() as u64;
                }
                {
                    let mut builder = crate::jit::aot_archive::AotArchive::new_builder();
                    let code_bytes = aot_native_blob.take().unwrap_or_else(|| {
                        crate::jit::codegen_cache::CodegenCache::new()
                            .try_read_object_bytes(&codegen_ck)
                            .unwrap_or_default()
                    });
                    let import_symbols =
                        crate::jit::codegen_cache::undefined_import_names_from_object(&code_bytes);
                    builder.add_model(
                        model_name,
                        &codegen_ck.stable_hash(),
                        code_bytes,
                        import_symbols,
                        when_count as u32,
                        crossings_count as u32,
                    );
                    match builder.write_merged_for_scope(codegen_ck.cache_scope) {
                        Ok(()) => {
                            if perf_trace {
                                eprintln!(
                                    "[perf] aot_archive=merged model={}",
                                    model_name,
                                );
                            }
                        }
                        Err(e) => {
                            if perf_trace {
                                eprintln!("[perf] aot_archive=merge_failed err={}", e);
                            }
                        }
                    }
                }
                if let Ok(reg) = crate::jit::speculation::global_registry().read() {
                    perf_report.speculation_invalidation_count = reg.invalidation_count() as u32;
                }

                let (final_calc_derivs, dual_generic, dual_keepalive) = if compiler.options.dual_compile
                    && aot_jit_codegen_keepalive.is_none()
                {
                    let has_guards = crate::jit::speculation::global_registry()
                        .read()
                        .map(|r| r.active_guard_count() > 0)
                        .unwrap_or(false);
                    if has_guards {
                        perf_report.dual_compile_status =
                            CompilePerfReport::DUAL_COMPILE_STATUS_ATTEMPTED.to_string();
                        let extra_sym = if all_symbols.is_empty() { None } else { Some(&all_symbols) };
                        match crate::jit::deopt::dual_compile(
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
                            extra_sym,
                        ) {
                            Ok(dual) => {
                                let spec = dual.speculative;
                                let gen = dual.generic;
                                perf_report.dual_compile_ok = true;
                                perf_report.dual_compile_speculation_count =
                                    dual.speculation_count as u32;
                                perf_report.dual_compile_status =
                                    CompilePerfReport::DUAL_COMPILE_STATUS_OK.to_string();
                                perf_report.dual_compile_error = None;
                                if perf_trace {
                                    eprintln!(
                                        "[perf] dual_compile=ok speculations={}",
                                        dual.speculation_count
                                    );
                                }
                                perf_report.compile_tier = "dual_speculative".to_string();
                                (spec, Some(gen), Some(Box::new(dual)))
                            }
                            Err(e) => {
                                perf_report.dual_compile_ok = false;
                                perf_report.dual_compile_status =
                                    CompilePerfReport::DUAL_COMPILE_STATUS_FAILED.to_string();
                                perf_report.dual_compile_error = Some(DualCompileErrorReport {
                                    code: e.code().to_string(),
                                    registry_poisoned: e.registry_poisoned(),
                                    cranelift_phase: e
                                        .cranelift_phase()
                                        .map(|s| s.to_string()),
                                    symbol_name: e.symbol_name().map(|s| s.to_string()),
                                    detail: CompilePerfReport::truncate_dual_compile_error_detail(
                                        &e.stable_detail(),
                                    ),
                                });
                                if perf_trace {
                                    eprintln!("[perf] dual_compile=failed err={}", e);
                                }
                                (calc_derivs, None, None)
                            }
                        }
                    } else {
                        perf_report.dual_compile_ok = false;
                        perf_report.dual_compile_status =
                            CompilePerfReport::DUAL_COMPILE_STATUS_SKIPPED_NO_GUARDS.to_string();
                        perf_report.dual_compile_error = None;
                        (calc_derivs, None, None)
                    }
                } else {
                    if compiler.options.dual_compile && aot_jit_codegen_keepalive.is_some() {
                        perf_report.dual_compile_ok = false;
                        perf_report.dual_compile_status =
                            CompilePerfReport::DUAL_COMPILE_STATUS_SKIPPED_AOT_NATIVE.to_string();
                        perf_report.dual_compile_error = None;
                    }
                    (calc_derivs, None, None)
                };

                perf_report.jit_compile_ok = true;
                perf_report.jit_error = None;
                fold_benefit_record::persist_and_tierup_flags(
                    adaptive_fold,
                    flatten_cache::flatten_cache_dir().as_deref(),
                    &flat_pipeline_hex,
                    fold_record_at_start.clone(),
                    fold_cooldown_at_start,
                    perf_report.const_fold_skipped_by_policy,
                    perf_report.const_fold_cooldown_active,
                    perf_report.const_fold_count,
                    perf_report.eq_dce_removed,
                    perf_report.alg_eq_count,
                    perf_report.diff_eq_count,
                    perf_report.external_resolve_ms,
                );
                apply_fallback_snapshot(&mut perf_report);
                compiler.last_compile_perf = Some(perf_report);
                if user_stub_jits.is_empty()
                    && crate::cache::sim_bundle_cache::sim_bundle_cache_enabled()
                {
                    if let Some(ref sk) = sim_bundle_key {
                        let bundle = crate::cache::sim_bundle_cache::CompiledSimBundle {
                            schema_ver: 1,
                            codegen_key: codegen_ck.clone(),
                            when_count,
                            crossings_count,
                            var_layout_fingerprint: layout_fp.clone(),
                        };
                        let cache_root = crate::flatten::flatten_cache_dir()
                            .unwrap_or_else(|| std::path::PathBuf::from("."));
                        let _ = crate::cache::sim_bundle_cache::try_store(
                            &cache_root,
                            sk,
                            &bundle,
                        );
                    }
                }
                crate::query_db::salsa_session::record_codegen_stable_hash(
                    &compiler.loader,
                    model_name,
                    compiler.options.coarse_constrainedby_only,
                    compile_stop_s,
                    ValidationMode::parse(compiler.options.validation_mode.as_str()),
                    codegen_ck_hash,
                );
                let loaded_paths = compiler.loader.loaded_source_paths();
                crate::cache::warmup::trigger_warmup(model_name, &loaded_paths, &lib_paths);
                Ok(CompileOutput::Simulation(Artifacts {
                    calc_derivs: final_calc_derivs,
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
                    calc_derivs_codegen_keepalive: aot_jit_codegen_keepalive.take(),
                    jit_module_keepalive: jit_keepalive.take(),
                    param_only_update: false,
                    dual_compile_generic: dual_generic,
                    dual_compile_keepalive: dual_keepalive,
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
