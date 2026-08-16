use std::collections::{HashMap, HashSet};
use std::time::Instant;

use rayon::prelude::*;

use crate::ast::{AlgorithmStatement, Equation, Expression};
use crate::backend_dae::{ClockPartition as BackendClockPartition, SimulationDae};
use crate::compiler::jacobian;
use crate::compiler::pipeline::collect_newton_tearing_var_names;
use crate::compiler::{c_codegen, ClockPartitionScheduleEntry, Compiler, CompilePerfReport};
use crate::flatten::FlattenedModel;
use crate::i18n;

use super::super::aot_coverage::maybe_write_aot_cache_marker;
use super::super::clock_partition_parse::parse_clock_partition_trigger;
use super::super::env_perf::jit_partition_scan_parallel_enabled;

pub(crate) struct CompileEmitState {
    pub(crate) perf_report: CompilePerfReport,
    pub(crate) newton_tearing_var_names: Vec<String>,
    pub(crate) clock_partition_schedule: Vec<ClockPartitionScheduleEntry>,
}

pub(crate) fn compile_emit_and_partitions(
    compiler: &mut Compiler,
    model_name: &str,
    flat_model: &FlattenedModel,
    mut perf_report: CompilePerfReport,
    perf_trace: bool,
    differential_index: u32,
    total_equations: usize,
    total_declarations: usize,
    state_vars_sorted: &[String],
    discrete_vars_sorted: &[String],
    param_vars: &[String],
    output_vars: &[String],
    alg_equations: &[Equation],
    diff_equations: &[Equation],
    algorithms: &[AlgorithmStatement],
    strong_component_jacobians: bool,
    symbolic_ode_jacobian: bool,
    numeric_ode_jacobian: bool,
    symbolic_ode_jacobian_matrix: &Option<Vec<Vec<Expression>>>,
    ode_jacobian_sparse: &Option<jacobian::SparseOdeJacobian>,
    simulation_dae: &SimulationDae,
    blt_degrade_guard_triggered: bool,
    blt_degrade_guard_limit: Option<usize>,
    blt_degrade_guard_equation_count: Option<usize>,
    symbolic_index_signal_count: usize,
    implicit_derivative_constraint_count: usize,
    state_var_index: &HashMap<String, usize>,
    output_var_index: &HashMap<String, usize>,
    param_var_index: &HashMap<String, usize>,
    external_list: &[(String, String, Option<String>)],
    user_function_bodies: &HashMap<String, (Vec<String>, Expression)>,
    backend_clock_partitions: &[BackendClockPartition],
) -> Result<CompileEmitState, Box<dyn std::error::Error + Send + Sync>> {
    let opts = &compiler.options;
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
                Some(user_function_bodies)
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
                    let mut paths: Vec<String> =
                        files.iter().map(|p| p.display().to_string()).collect();
                    if compiler.options.emit_coder_package {
                        let info = crate::compiler::CoderPackageInfo {
                            model_name: model_name.to_string(),
                            state_vars: state_vars_sorted.to_vec(),
                            param_vars: param_vars.to_vec(),
                            output_vars: output_vars.to_vec(),
                            sample_time: compiler.options.output_interval.max(compiler.options.dt),
                            stop_time: compiler.options.t_end,
                            has_jacobian: jac.is_some(),
                        };
                        match crate::compiler::write_coder_package(path, &info) {
                            Ok(extra) => {
                                paths.extend(extra.iter().map(|p| p.display().to_string()));
                            }
                            Err(e) => {
                                return Err(format!(
                                    "Coder package failed: {}{}",
                                    e,
                                    compiler.source_loc_suffix(model_name)
                                )
                                .into());
                            }
                        }
                    }
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

        Ok(CompileEmitState {
            perf_report,
            newton_tearing_var_names,
            clock_partition_schedule,
        })
}
