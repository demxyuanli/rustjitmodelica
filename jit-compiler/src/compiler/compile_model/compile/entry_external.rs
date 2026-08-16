use std::collections::HashSet;
use std::time::Instant;

use crate::analysis::BlockCausalityInfo;
use crate::ast::{AlgorithmStatement, Equation};
use crate::backend_dae::{
    build_simulation_dae, ida_component_id_for_states, ClockPartition as BackendClockPartition,
    SimulationDae,
};
use crate::cache::external_resolve_cache;
use crate::compiler::{collect_external_calls, collect_external_raw_call_sites, CompilePerfReport, Compiler};
use crate::flatten::{flatten_cache, FlattenedModel};

use super::super::disk_cache::{
    backend_dae_disk_cache_enabled, backend_dae_disk_key, try_read_backend_dae_disk,
    try_write_backend_dae_disk,
};

pub(crate) struct CompileExternalState {
    pub(crate) perf_report: CompilePerfReport,
    pub(crate) backend_clock_partitions: Vec<BackendClockPartition>,
    pub(crate) simulation_dae: SimulationDae,
    pub(crate) ida_component_id: Vec<f64>,
    pub(crate) dae_differential_index: u32,
    pub(crate) external_t0: Instant,
    pub(crate) external_list: Vec<(String, String, Option<String>)>,
    pub(crate) symbolic_ode_jacobian: bool,
    pub(crate) strong_component_jacobians: bool,
}

pub(crate) fn compile_dae_external(
    compiler: &mut Compiler,
    model_name: &str,
    flat_model: &FlattenedModel,
    mut perf_report: CompilePerfReport,
    perf_trace: bool,
    flat_h_pipeline: u64,
    state_vars_sorted: &[String],
    discrete_vars_sorted: &[String],
    param_vars: &[String],
    output_vars: &[String],
    input_var_names: &[String],
    alg_equations: &[Equation],
    diff_equations: &[Equation],
    algorithms: &[AlgorithmStatement],
    states_len: usize,
    initial_variable_count: usize,
    when_equation_count: usize,
    differential_index: u32,
    constraint_equation_count: usize,
    block_causality: &[BlockCausalityInfo],
    external_sites_pre_fold: &[String],
    symbolic_ode_jacobian_matrix_is_some: bool,
) -> Result<CompileExternalState, Box<dyn std::error::Error + Send + Sync>> {
    let opts = &compiler.options;
    let symbolic_ode_jacobian = symbolic_ode_jacobian_matrix_is_some;
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
        let ida_component_id = ida_component_id_for_states(&simulation_dae, states_len);
        let dae_differential_index = simulation_dae.dae.differential_index;
        perf_report.backend_dae_ms = dae_t0.elapsed().as_millis() as u64;
        if perf_trace {
            eprintln!(
                "[perf] compile_phase.backend_dae_ms={}",
                perf_report.backend_dae_ms
            );
        }

        let external_t0 = Instant::now();
        let t_ext_gather = Instant::now();
        let _post_external_names: HashSet<String> =
            collect_external_raw_call_sites(&alg_equations, &diff_equations, &algorithms)
                .into_iter()
                .collect();
        perf_report.external_resolve_gather_us = t_ext_gather.elapsed().as_micros() as u64;
        let external_list = if external_resolve_cache::external_resolve_cache_enabled() {
            let key = external_resolve_cache::compute_external_resolve_key(
                model_name,
                &compiler.loader,
                &external_sites_pre_fold,
                &opts.external_libs,
            );
            let t_lookup = Instant::now();
            if let Some(cached) =
                external_resolve_cache::try_load(model_name, &compiler.loader, &key)
            {
                perf_report.external_resolve_lookup_us = t_lookup.elapsed().as_micros() as u64;
                perf_report.external_resolve_cache_status = "hit".to_string();
                cached
            } else {
                perf_report.external_resolve_lookup_us = t_lookup.elapsed().as_micros() as u64;
                let miss_detail =
                    external_resolve_cache::diagnose_miss(model_name, &compiler.loader, &key);
                perf_report.external_resolve_miss_detail = Some(miss_detail.clone());
                perf_report.cache_miss_events.push(crate::compiler::CacheMissEvent {
                    layer: "external_resolve".to_string(),
                    reason: "cache_miss".to_string(),
                    detail: Some(miss_detail),
                });
                let t_compute = Instant::now();
                let computed = collect_external_calls(
                    &mut compiler.loader,
                    &alg_equations,
                    &diff_equations,
                    &algorithms,
                );
                perf_report.external_resolve_compute_us = t_compute.elapsed().as_micros() as u64;
                let t_store = Instant::now();
                perf_report.external_resolve_cache_status =
                    if external_resolve_cache::try_store(model_name, &compiler.loader, &key, &computed)
                        .is_ok()
                    {
                        perf_report.external_resolve_store_us =
                            t_store.elapsed().as_micros() as u64;
                        "put".to_string()
                    } else {
                        perf_report.external_resolve_store_us =
                            t_store.elapsed().as_micros() as u64;
                        "miss_compute".to_string()
                    };
                computed
            }
        } else {
            perf_report.external_resolve_cache_status = "disabled".to_string();
            let t_compute = Instant::now();
            let out = collect_external_calls(
                &mut compiler.loader,
                &alg_equations,
                &diff_equations,
                &algorithms,
            );
            perf_report.external_resolve_compute_us = t_compute.elapsed().as_micros() as u64;
            out
        };
        Ok(CompileExternalState {
            perf_report,
            backend_clock_partitions,
            simulation_dae,
            ida_component_id,
            dae_differential_index,
            external_t0,
            external_list,
            symbolic_ode_jacobian,
            strong_component_jacobians,
        })
}
