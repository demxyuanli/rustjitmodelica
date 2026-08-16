use std::collections::HashSet;
use std::time::Instant;

use crate::analysis::analyze_initial_equations;
use crate::i18n;
use crate::ast::Equation;
use crate::cache::fold_benefit_record;
use crate::cache::fold_benefit_record::FoldBenefitRecord;
use crate::compiler::jacobian;
use crate::compiler::pipeline::{
    analyze_equations, build_runtime_algorithms, classify_variables,
};
use crate::compiler::{
    adaptive::{AdaptiveParameterEngine, ModelStats},
    collect_external_raw_call_sites,
    CompileOutput, CompilePerfReport, CompileStopPhase, Compiler, ValidationAnalyzedSummary,
};
use crate::condenser::profile_data::ModelProfile;
use crate::diag::WarningInfo;
use crate::flatten::ValidationMode;
use crate::flatten::{flatten_cache, FlattenedModel};
use crate::jit::ArrayInfo;
use crate::analysis::BlockCausalityInfo;
use crate::ast::Expression;
use crate::ast::AlgorithmStatement;

use super::super::aot_coverage::maybe_coverage_target_warning_message;
use super::super::constfold_opt::optimize_equations_for_constfold_dce;
use super::super::disk_cache::{
    analyze_cache, analysis_summary_disk_cache_enabled, analysis_summary_disk_key,
    flat_model_hash, pipeline_analysis_disk_cache_enabled, pipeline_analysis_disk_key,
    try_read_pipeline_analysis_disk, try_write_analysis_summary_disk,
    try_write_pipeline_analysis_disk, AnalyzeCacheEntry,
};
use super::entry_finish::apply_fallback_snapshot;

pub(crate) struct CompileAnalyzeState {
    pub(crate) perf_report: CompilePerfReport,
    pub(crate) flat_h_pipeline: u64,
    pub(crate) flat_pipeline_hex: String,
    pub(crate) state_vars_sorted: Vec<String>,
    pub(crate) discrete_vars_sorted: Vec<String>,
    pub(crate) param_vars: Vec<String>,
    pub(crate) input_var_names: Vec<String>,
    pub(crate) output_vars: Vec<String>,
    pub(crate) output_start_vals: Vec<f64>,
    pub(crate) output_var_index: std::collections::HashMap<String, usize>,
    pub(crate) state_var_index: std::collections::HashMap<String, usize>,
    pub(crate) param_var_index: std::collections::HashMap<String, usize>,
    pub(crate) array_info: std::collections::HashMap<String, ArrayInfo>,
    pub(crate) states: Vec<f64>,
    pub(crate) discrete_vals: Vec<f64>,
    pub(crate) params: Vec<f64>,
    pub(crate) alg_equations: Vec<Equation>,
    pub(crate) diff_equations: Vec<Equation>,
    pub(crate) algorithms: Vec<AlgorithmStatement>,
    pub(crate) external_sites_pre_fold: Vec<String>,
    pub(crate) adaptive_fold: bool,
    pub(crate) fold_record_at_start: Option<FoldBenefitRecord>,
    pub(crate) fold_cooldown_at_start: bool,
    pub(crate) const_fold_param_pairs: Vec<(String, f64)>,
    pub(crate) block_causality: Vec<BlockCausalityInfo>,
    pub(crate) differential_index: u32,
    pub(crate) constraint_equation_count: usize,
    pub(crate) blt_degrade_guard_triggered: bool,
    pub(crate) blt_degrade_guard_limit: Option<usize>,
    pub(crate) blt_degrade_guard_equation_count: Option<usize>,
    pub(crate) symbolic_index_signal_count: usize,
    pub(crate) implicit_derivative_constraint_count: usize,
    pub(crate) numeric_ode_jacobian: bool,
    pub(crate) ode_jacobian_sparse: Option<jacobian::SparseOdeJacobian>,
    pub(crate) symbolic_ode_jacobian_matrix: Option<Vec<Vec<Expression>>>,
    pub(crate) initial_variable_count: usize,
    pub(crate) when_equation_count: usize,
}

pub(crate) enum CompileAnalyzeOutcome {
    Early(CompileOutput),
    Continue(CompileAnalyzeState),
}

pub(crate) fn compile_analyze(
    compiler: &mut Compiler,
    model_name: &str,
    flat_model: &FlattenedModel,
    mut perf_report: CompilePerfReport,
    perf_trace: bool,
    stage_trace: bool,
    leyden_profile: &Option<ModelProfile>,
    total_equations: usize,
    total_declarations: usize,
    model_file_path: &String,
) -> Result<CompileAnalyzeOutcome, Box<dyn std::error::Error + Send + Sync>> {
    let opts = &compiler.options;
        let analyze_t0 = Instant::now();
        let flat_h_pipeline = flat_model_hash(&flat_model);
        let flat_pipeline_hex = format!("{:016x}", flat_h_pipeline);
        let adaptive_fold = fold_benefit_record::adaptive_fold_policy_enabled();
        let fold_record_at_start = if adaptive_fold {
            flatten_cache::flatten_cache_dir().and_then(|root| {
                fold_benefit_record::try_load(root.as_path(), &flat_pipeline_hex)
            })
        } else {
            None
        };
        let fold_cooldown_at_start = fold_record_at_start
            .as_ref()
            .map(|r| r.cooldown_remaining > 0)
            .unwrap_or(false);
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
        {
            let condenser_t0 = Instant::now();
            let mut condenser_reg = crate::condenser::CondenserRegistry::new();
            condenser_reg.register(Box::new(crate::condenser::AnalysisCondenser));
            condenser_reg.register(Box::new(crate::condenser::SymbolicCondenser));
            let condenser_phase = if leyden_profile.is_some() {
                crate::condenser::CondenserPhase::FirstRun
            } else {
                crate::condenser::CondenserPhase::FirstRun
            };
            let mut condenser_ctx = crate::condenser::CondenserContext::new(
                model_name,
                condenser_phase,
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
        let algorithms = build_runtime_algorithms(&flat_model, &discrete_vars_sorted, stage_trace);
        let mut external_sites_pre_fold: Vec<String> =
            collect_external_raw_call_sites(&alg_equations, &diff_equations, &algorithms)
                .into_iter()
                .collect();
        external_sites_pre_fold.sort_unstable();
        if adaptive_fold {
            if let Some(ref r) = fold_record_at_start {
                if r.cooldown_remaining > 0 {
                    perf_report.const_fold_cooldown_active = true;
                    perf_report.const_fold_enabled = false;
                    perf_report.eq_dce_enabled = false;
                } else if r.last_fold_count == 0 && r.last_dce_removed == 0 {
                    perf_report.const_fold_skipped_by_policy = true;
                    perf_report.eq_dce_skipped_by_policy = true;
                    perf_report.const_fold_enabled = false;
                    perf_report.eq_dce_enabled = false;
                }
            }
        }
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
            return Ok(CompileAnalyzeOutcome::Early(CompileOutput::ValidationAnalyzed(
                summary,
            )));
        }
        Ok(CompileAnalyzeOutcome::Continue(CompileAnalyzeState {
            perf_report,
            flat_h_pipeline,
            flat_pipeline_hex,
            state_vars_sorted,
            discrete_vars_sorted,
            param_vars,
            input_var_names,
            output_vars,
            output_start_vals,
            output_var_index,
            state_var_index,
            param_var_index,
            array_info,
            states,
            discrete_vals,
            params,
            alg_equations,
            diff_equations,
            algorithms,
            external_sites_pre_fold,
            adaptive_fold,
            fold_record_at_start,
            fold_cooldown_at_start,
            const_fold_param_pairs,
            block_causality,
            differential_index,
            constraint_equation_count,
            blt_degrade_guard_triggered,
            blt_degrade_guard_limit,
            blt_degrade_guard_equation_count,
            symbolic_index_signal_count,
            implicit_derivative_constraint_count,
            numeric_ode_jacobian,
            ode_jacobian_sparse,
            symbolic_ode_jacobian_matrix,
            initial_variable_count,
            when_equation_count,
        }))
}
