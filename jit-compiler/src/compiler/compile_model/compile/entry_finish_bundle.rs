use std::collections::HashMap;

use xxhash_rust::xxh64::Xxh64;

use crate::cache::fold_benefit_record;
use crate::cache::fold_benefit_record::FoldBenefitRecord;
use crate::compiler::{
    solvable_scale_warn, Artifacts, CompileOutput, CompilePerfReport, CompileStopPhase, Compiler,
};
use crate::flatten::ArraySizePolicy;
use crate::flatten::ValidationMode;
use crate::flatten::flatten_cache;

use super::entry_finish::{apply_fallback_snapshot, FinishLayout};

pub(crate) struct FinishBundleKeys {
    pub t_end: f64,
    pub dt: f64,
    pub compile_stop_s: &'static str,
    pub layout_fp: String,
    pub connector_connection_degree: HashMap<String, usize>,
    pub codegen_ck: crate::jit::codegen_cache::CodegenCacheKey,
    pub codegen_ck_hash: String,
    pub sim_bundle_key: Option<String>,
}

pub(crate) enum FinishBundleOutcome {
    Cached(CompileOutput),
    Continue {
        keys: FinishBundleKeys,
        layout: FinishLayout,
        perf_report: CompilePerfReport,
    },
}

pub(crate) fn try_finish_sim_bundle(
    compiler: &mut Compiler,
    model_name: &str,
    all_symbols: &HashMap<String, *const u8>,
    mut perf_report: CompilePerfReport,
    _perf_trace: bool,
    model_file_path: &str,
    adaptive_fold: bool,
    flat_pipeline_hex: &str,
    fold_record_at_start: &Option<FoldBenefitRecord>,
    fold_cooldown_at_start: bool,
    array_size_policy: ArraySizePolicy,
    layout: FinishLayout,
) -> Result<FinishBundleOutcome, Box<dyn std::error::Error + Send + Sync>> {
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
    let array_sizes_path = compiler
        .options
        .array_sizes_json
        .as_deref()
        .map(std::path::Path::new);
        let t_end = compiler.options.t_end;
        let dt = compiler.options.dt;
        solvable_scale_warn::push_dense_newton_scale_warnings(
            &alg_equations,
            &mut compiler.warnings,
            model_file_path.to_string(),
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
        let codegen_ck_hash = codegen_ck.stable_hash();
        let salsa_codegen_hint = crate::query_db::salsa_session::take_salsa_codegen_stable_hash();
        perf_report.salsa_codegen_reuse_eligible =
            salsa_codegen_hint.as_deref() == Some(codegen_ck_hash.as_str());
        let sim_bundle_key = flatten_cache::flatten_cache_dir().map(|_| {
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
            crate::cache::sim_bundle_cache::storage_key(&flat_full_key, model_name)
        });
        if !crate::cache::sim_bundle_cache::sim_bundle_cache_enabled() {
            perf_report.artifact_bundle_cache_status = "disabled".to_string();
        } else if !user_stub_jits.is_empty() {
            perf_report.artifact_bundle_cache_status = "skipped_stubs".to_string();
        } else if !crate::jit::codegen_cache::codegen_cache_enabled() {
            perf_report.artifact_bundle_cache_status = "skipped_codegen_disk".to_string();
        } else {
            perf_report.artifact_bundle_cache_status = "miss".to_string();
            if let Some(ref sk) = sim_bundle_key {
                let cache_root_sb = crate::flatten::flatten_cache_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                if let Some(bundle) =
                    crate::cache::sim_bundle_cache::try_load(&cache_root_sb, sk)
                {
                    if bundle.var_layout_fingerprint == layout_fp
                        && bundle.codegen_key.stable_hash() == codegen_ck.stable_hash()
                    {
                        let cc = crate::jit::codegen_cache::CodegenCache::new();
                        let param_map =
                            crate::jit::codegen_cache::param_values_by_name(&param_vars, &params);
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
                                                        &all_symbols,
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
                                                        &all_symbols,
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
                            cached_fn = cc.get(&bundle.codegen_key, &all_symbols, Some(&param_map));
                        }
                        if let Some(cached) = cached_fn {
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
                            let param_sig_delta = bundle.codegen_key.param_signature
                                != codegen_ck.param_signature
                                || bundle.codegen_key.type_profile_hash
                                    != codegen_ck.type_profile_hash;
                            if param_sig_delta {
                                perf_report.param_only_update = true;
                            }
                            perf_report.jit_cache_partial_recompile =
                                perf_report.jit_incremental_enabled || param_sig_delta;
                            perf_report.jit_cache_recompiled_functions = 0;
                            perf_report.jit_cache_skipped_functions = 0;
                            perf_report.jit_ms = 0;
                            perf_report.codegen_wall_us = 0;
                            perf_report.codegen_wall_ms = 0;
                            perf_report.artifact_bundle_cache_status = "hit".to_string();
                            perf_report.structural_cache_hit = true;
                            perf_report.cache_structural_hit_count += 1;
                            perf_report.cache_warm_ratio =
                                perf_report.compute_warm_ratio(true);
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
                            crate::query_db::salsa_session::record_codegen_stable_hash(
                                &compiler.loader,
                                model_name,
                                compiler.options.coarse_constrainedby_only,
                                compile_stop_s,
                                ValidationMode::parse(compiler.options.validation_mode.as_str()),
                                codegen_ck_hash,
                            );
                            let cf = Box::new(cached);
                            let param_only = param_sig_delta;
                            return Ok(FinishBundleOutcome::Cached(CompileOutput::Simulation(Artifacts {
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
                                jit_module_keepalive: None,
                                param_only_update: param_only,
                                dual_compile_generic: None,
                                dual_compile_keepalive: None,
                            })));
                        } else {
                            perf_report.cache_miss_reason =
                                Some("sim_bundle_codegen_miss".to_string());
                            perf_report.cache_miss_events.push(crate::compiler::CacheMissEvent {
                                layer: "sim_bundle".to_string(),
                                reason: "codegen_miss".to_string(),
                                detail: None,
                            });
                        }
                    }
                }
            }
        }
        let keys = FinishBundleKeys {
            t_end,
            dt,
            compile_stop_s,
            layout_fp,
            connector_connection_degree,
            codegen_ck,
            codegen_ck_hash,
            sim_bundle_key,
        };
        Ok(FinishBundleOutcome::Continue {
            keys,
            layout: FinishLayout {
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
            },
            perf_report,
        })
}
