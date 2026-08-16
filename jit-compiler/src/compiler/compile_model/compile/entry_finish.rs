use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use crate::ast::{AlgorithmStatement, Equation, Expression, Model};
use crate::backend_dae::ClockPartition as BackendClockPartition;
use crate::cache::fold_benefit_record::FoldBenefitRecord;
use crate::condenser::profile_data::ModelProfile;
use crate::diag::fallback_counter;
use crate::diag::fallback_counter::FallbackCounterSnapshot;
use crate::flatten::ArraySizePolicy;
use crate::flatten::FlattenedModel;
use crate::jit::ArrayInfo;
use crate::jit::Jit;

use crate::compiler::{
    ClockPartitionScheduleEntry, CompileOutput, CompilePerfReport, Compiler,
};

use super::entry_finish_bundle::{try_finish_sim_bundle, FinishBundleOutcome};
use super::entry_finish_jit::compile_finish_jit;
use super::entry_finish_libs::load_finish_libs;

pub(crate) fn apply_fallback_snapshot(report: &mut CompilePerfReport) {
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
}

pub(crate) struct CompileFinishState {
    pub(crate) perf_report: CompilePerfReport,
    pub(crate) perf_trace: bool,
    pub(crate) external_list: Vec<(String, String, Option<String>)>,
    pub(crate) user_stub_ptrs: HashMap<String, *const u8>,
    pub(crate) external_names: HashSet<String>,
    pub(crate) external_t0: Instant,
    pub(crate) model_file_path: String,
    pub(crate) state_vars_sorted: Vec<String>,
    pub(crate) discrete_vars_sorted: Vec<String>,
    pub(crate) param_vars: Vec<String>,
    pub(crate) output_vars: Vec<String>,
    pub(crate) array_info: HashMap<String, ArrayInfo>,
    pub(crate) params: Vec<f64>,
    pub(crate) flat_model: FlattenedModel,
    pub(crate) array_size_policy: ArraySizePolicy,
    pub(crate) user_stub_jits: Vec<Jit>,
    pub(crate) leyden_profile: Option<ModelProfile>,
    pub(crate) alg_equations: Vec<Equation>,
    pub(crate) diff_equations: Vec<Equation>,
    pub(crate) when_equation_count: usize,
    pub(crate) adaptive_fold: bool,
    pub(crate) flat_pipeline_hex: String,
    pub(crate) fold_record_at_start: Option<FoldBenefitRecord>,
    pub(crate) fold_cooldown_at_start: bool,
    pub(crate) states: Vec<f64>,
    pub(crate) discrete_vals: Vec<f64>,
    pub(crate) output_start_vals: Vec<f64>,
    pub(crate) state_var_index: HashMap<String, usize>,
    pub(crate) backend_clock_partitions: Vec<BackendClockPartition>,
    pub(crate) clock_partition_schedule: Vec<ClockPartitionScheduleEntry>,
    pub(crate) numeric_ode_jacobian: bool,
    pub(crate) symbolic_ode_jacobian_matrix: Option<Vec<Vec<Expression>>>,
    pub(crate) newton_tearing_var_names: Vec<String>,
    pub(crate) dae_differential_index: u32,
    pub(crate) ida_component_id: Vec<f64>,
    pub(crate) algorithms: Vec<AlgorithmStatement>,
    pub(crate) const_fold_param_pairs: Vec<(String, f64)>,
    pub(crate) root_model: Arc<Model>,
}

pub(crate) struct FinishLayout {
    pub(crate) state_vars_sorted: Vec<String>,
    pub(crate) discrete_vars_sorted: Vec<String>,
    pub(crate) param_vars: Vec<String>,
    pub(crate) output_vars: Vec<String>,
    pub(crate) array_info: HashMap<String, ArrayInfo>,
    pub(crate) params: Vec<f64>,
    pub(crate) flat_model: FlattenedModel,
    pub(crate) user_stub_jits: Vec<Jit>,
    pub(crate) alg_equations: Vec<Equation>,
    pub(crate) diff_equations: Vec<Equation>,
    pub(crate) when_equation_count: usize,
    pub(crate) states: Vec<f64>,
    pub(crate) discrete_vals: Vec<f64>,
    pub(crate) output_start_vals: Vec<f64>,
    pub(crate) state_var_index: HashMap<String, usize>,
    pub(crate) backend_clock_partitions: Vec<BackendClockPartition>,
    pub(crate) clock_partition_schedule: Vec<ClockPartitionScheduleEntry>,
    pub(crate) numeric_ode_jacobian: bool,
    pub(crate) symbolic_ode_jacobian_matrix: Option<Vec<Vec<Expression>>>,
    pub(crate) newton_tearing_var_names: Vec<String>,
    pub(crate) dae_differential_index: u32,
    pub(crate) ida_component_id: Vec<f64>,
    pub(crate) algorithms: Vec<AlgorithmStatement>,
    pub(crate) const_fold_param_pairs: Vec<(String, f64)>,
    pub(crate) root_model: Arc<Model>,
    pub(crate) external_names: HashSet<String>,
}

pub(crate) fn compile_finish(
    compiler: &mut Compiler,
    model_name: &str,
    state: CompileFinishState,
) -> Result<CompileOutput, Box<dyn std::error::Error + Send + Sync>> {
    let CompileFinishState {
        mut perf_report,
        perf_trace,
        external_list,
        user_stub_ptrs,
        external_names,
        external_t0,
        model_file_path,
        state_vars_sorted,
        discrete_vars_sorted,
        param_vars,
        output_vars,
        array_info,
        params,
        flat_model,
        array_size_policy,
        user_stub_jits,
        leyden_profile,
        alg_equations,
        diff_equations,
        when_equation_count,
        adaptive_fold,
        flat_pipeline_hex,
        fold_record_at_start,
        fold_cooldown_at_start,
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
    } = state;

    let (lib_paths, all_symbols) = load_finish_libs(
        compiler,
        &external_list,
        user_stub_ptrs,
        &external_names,
        external_t0,
        perf_trace,
        &mut perf_report,
    )?;

    let layout = FinishLayout {
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
    };

    let bundle_out = try_finish_sim_bundle(
        compiler,
        model_name,
        &all_symbols,
        perf_report,
        perf_trace,
        &model_file_path,
        adaptive_fold,
        &flat_pipeline_hex,
        &fold_record_at_start,
        fold_cooldown_at_start,
        array_size_policy,
        layout,
    )?;
    let (keys, layout, perf_report) = match bundle_out {
        FinishBundleOutcome::Cached(out) => return Ok(out),
        FinishBundleOutcome::Continue {
            keys,
            layout,
            perf_report,
        } => (keys, layout, perf_report),
    };

    compile_finish_jit(
        compiler,
        model_name,
        layout,
        keys,
        all_symbols,
        lib_paths,
        perf_report,
        perf_trace,
        adaptive_fold,
        flat_pipeline_hex,
        fold_record_at_start,
        fold_cooldown_at_start,
        leyden_profile,
    )
}
