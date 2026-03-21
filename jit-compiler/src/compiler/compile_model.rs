use std::collections::{HashMap, HashSet};

use crate::analysis::analyze_initial_equations;
use crate::ast::{Equation, Expression};
use crate::backend_dae::{
    build_simulation_dae, ClockPartition as BackendClockPartition, SimulationDae,
};
use crate::diag::WarningInfo;
use crate::i18n;
use crate::jit::native::builtin_jit_symbol_names;
use crate::jit::Jit;

use super::{
    c_codegen, collect_all_called_names, collect_external_calls, inline, jacobian,
    Artifacts, CompileOutput, Compiler,
};
use super::pipeline::{
    analyze_equations, build_runtime_algorithms, classify_variables,
    collect_newton_tearing_var_names, flatten_and_inline, stage_trace_enabled,
};

pub(super) fn compile(
    compiler: &mut Compiler,
    model_name: &str,
) -> Result<CompileOutput, Box<dyn std::error::Error + Send + Sync>> {
        let stage_trace = stage_trace_enabled();

        compiler.warnings.clear();
        compiler.loader.set_quiet(compiler.options.quiet);
        let opts = &compiler.options;
        let model_file_path = format!("{}.mo", model_name.replace('.', "/"));
        if !compiler.options.quiet {
            println!(
                "{}",
                i18n::msg("loading_model", &[&model_name as &dyn std::fmt::Display])
            );
        }
        let mut root_model = compiler
            .loader
            .load_model(model_name)
            .map_err(|e| -> Box<dyn std::error::Error + Send + Sync> { Box::new(e) })?;

        if root_model.as_ref().is_function {
            if !compiler.options.quiet {
                if compiler.options.function_args.is_some() {
                    println!("{}", i18n::msg0("evaluating_function_args"));
                } else {
                    println!("{}", i18n::msg0("evaluating_function_default"));
                }
            }
            let value = compiler.run_function_once(model_name)?;
            return Ok(CompileOutput::FunctionRun(value));
        }

        if !compiler.options.quiet {
            println!("{}", i18n::msg0("flattening_model"));
        }
        let frontend = flatten_and_inline(
            &mut root_model,
            model_name,
            &mut compiler.loader,
            compiler.options.quiet,
            stage_trace,
        )?;
        let flat_model = frontend.flat_model;
        let total_equations = frontend.total_equations;
        let total_declarations = frontend.total_declarations;
        if !compiler.options.quiet {
            println!("{}", i18n::msg("flattened_equations", &[&total_equations]));
            println!(
                "{}",
                i18n::msg("flattened_declarations", &[&total_declarations])
            );
            println!("{}", i18n::msg0("analyzing_variables"));
        }

        let mut variable_layout = classify_variables(&flat_model, opts.quiet, stage_trace);
        if !compiler.options.quiet {
            println!("{}", i18n::msg0("normalizing_derivatives"));
            println!("{}", i18n::msg0("performing_structure_analysis"));
        }
        let analysis_stage = analyze_equations(&flat_model, &mut variable_layout, opts, stage_trace);
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
        let alg_equations = analysis_stage.alg_equations;
        let diff_equations = analysis_stage.diff_equations;
        let differential_index = analysis_stage.differential_index;
        let constraint_equation_count = analysis_stage.constraint_equation_count;
        let constant_conflict_count = analysis_stage.constant_conflict_count;
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

        let mut known_at_initial = HashSet::new();
        known_at_initial.insert("time".to_string());
        for p in &param_vars {
            known_at_initial.insert(p.clone());
        }
        let initial_info =
            analyze_initial_equations(&flat_model.initial_equations, &known_at_initial);
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

        let symbolic_ode_jacobian = symbolic_ode_jacobian_matrix.is_some();
        let strong_component_jacobians = false;

        let when_equation_count = flat_model
            .equations
            .iter()
            .filter(|e| matches!(e, Equation::When(_, _, _)))
            .count();
        let backend_clock_partitions: Vec<BackendClockPartition> = flat_model
            .clock_partitions
            .iter()
            .map(|p| BackendClockPartition {
                id: p.id.clone(),
                var_names: p.var_names.clone(),
            })
            .collect();
        let simulation_dae: SimulationDae = build_simulation_dae(
            &state_vars_sorted,
            &discrete_vars_sorted,
            &param_vars,
            &output_vars,
            &input_var_names,
            diff_equations.len(),
            &alg_equations,
            flat_model.initial_equations.len(),
            initial_info.variable_count,
            when_equation_count,
            differential_index,
            constraint_equation_count,
            &backend_clock_partitions,
        );

        let external_list = collect_external_calls(
            &mut compiler.loader,
            &alg_equations,
            &diff_equations,
            &flat_model.algorithms,
        );

        let all_called =
            collect_all_called_names(&alg_equations, &diff_equations, &flat_model.algorithms);
        let external_names: HashSet<String> =
            external_list.iter().map(|(n, _, _)| n.clone()).collect();
        let stub_cap = all_called.len().saturating_sub(external_names.len());
        let mut user_stub_jits: Vec<Jit> = Vec::new();
        let mut user_stub_ptrs: HashMap<String, *const u8> = HashMap::with_capacity(stub_cap);
        let mut user_function_bodies: HashMap<String, (Vec<String>, Expression)> =
            HashMap::with_capacity(stub_cap);
        for name in &all_called {
            if inline::is_builtin_function(name) || external_names.contains(name) {
                continue;
            }
            // MSL Fluid: valveCharacteristic is usually provided via replaceable function
            // bound to BaseClasses.ValveCharacteristics.linear/one/...; our current
            // frontend does not track the specific binding here, so we fall back to
            // the default linear characteristic for JIT stubs.
            let load_name = if name == "valveCharacteristic" {
                "Modelica.Fluid.Valves.BaseClasses.ValveCharacteristics.linear".to_string()
            } else if name.starts_with("world.") {
                format!("Modelica.Mechanics.MultiBody.World.{}", name.trim_start_matches("world."))
            } else if name.starts_with("BaseClasses.") {
                format!("Modelica.Fluid.Utilities.{}", name)
            } else if name.starts_with("Machines.") {
                format!("Modelica.Electrical.{}", name)
            } else if name.starts_with("Mechanics.") {
                format!("Modelica.{}", name)
            } else if name == "Cv" {
                "Modelica.Units.Conversions".to_string()
            } else if let Some(rest) = name.strip_prefix("Cv.") {
                format!("Modelica.Units.Conversions.{}", rest)
            } else {
                name.to_string()
            };
            let func_model = match compiler.loader.load_model(&load_name) {
                Ok(m) => m,
                Err(_) => {
                    // Some collected call-like identifiers are not real Modelica
                    // functions in current frontend coverage; skip hard failure.
                    continue;
                }
            };
            if func_model.external_info.is_some() {
                continue;
            }
            let Some((input_names, outputs)) = inline::get_function_body(func_model.as_ref()) else {
                // Keep compilation progressing for library test wrappers that call
                // non-inlinable functions; expr translator provides a placeholder path.
                continue;
            };
            if outputs.len() != 1 {
                return Err(format!(
                    "Function '{}' has {} outputs; JIT callable supports single-output only (FUNC-2).",
                    name, outputs.len()
                ).into());
            }
            let mut stub_jit = Jit::new();
            let ptr = stub_jit
                .compile_user_function_stub(name, &input_names, &outputs[0].1)
                .map_err(|e| format!("JIT stub for '{}': {}", name, e))?;
            user_stub_ptrs.insert(name.clone(), ptr);
            user_stub_jits.push(stub_jit);
            user_function_bodies.insert(name.clone(), (input_names.clone(), outputs[0].1.clone()));
        }
        let mut all_symbols = compiler.external_symbol_ptrs.clone();
        for (k, v) in user_stub_ptrs {
            all_symbols.insert(k, v);
        }

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

        let algorithms = build_runtime_algorithms(&flat_model, stage_trace);
        let newton_tearing_var_names = collect_newton_tearing_var_names(&alg_equations);

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

        let t_end = compiler.options.t_end;
        let dt = compiler.options.dt;
        let mut jit = if all_symbols.is_empty() {
            Jit::new()
        } else {
            Jit::new_with_extra_symbols(Some(&all_symbols))
        };
        let res = jit.compile(
            &state_vars_sorted,
            &discrete_vars_sorted,
            &param_vars,
            &output_vars,
            &array_info,
            &alg_equations,
            &diff_equations,
            &algorithms,
            t_end,
            &newton_tearing_var_names,
        );

        match res {
            Ok((calc_derivs, when_count, crossings_count)) => {
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
                    when_count,
                    crossings_count,
                    t_end,
                    dt,
                    numeric_ode_jacobian,
                    symbolic_ode_jacobian: symbolic_ode_jacobian_matrix,
                    newton_tearing_var_names,
                    atol: compiler.options.atol,
                    rtol: compiler.options.rtol,
                    solver: compiler.options.solver.clone(),
                    output_interval: compiler.options.output_interval,
                    result_file: compiler.options.result_file.clone(),
                    user_stub_jits,
                }))
            }
            Err(e) => Err(format!(
                "JIT compilation failed: {}{}",
                e,
                compiler.source_loc_suffix(model_name)
            )
            .into()),
        }
}
