use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::time::Instant;

use crate::analysis::analyze_initial_equations;
use crate::ast::{Equation, Expression};
use crate::backend_dae::{
    build_simulation_dae, ida_component_id_for_states, ClockPartition as BackendClockPartition,
    SimulationDae,
};
use crate::diag::WarningInfo;
use crate::i18n;
use crate::jit::native::builtin_jit_symbol_names;
use crate::jit::Jit;

use super::{
    c_codegen, collect_all_called_names, collect_external_calls, inline, jacobian,
    solvable_scale_warn, Artifacts, ClockPartitionScheduleEntry, ClockPartitionTrigger,
    CompileOutput, CompilePerfReport, Compiler,
};
use super::pipeline::{
    analyze_equations, build_runtime_algorithms, classify_variables,
    collect_newton_tearing_var_names, flatten_and_inline, stage_trace_enabled,
};

#[derive(Debug, Clone, Copy)]
enum AotCacheStatus {
    DisabledNoEnv,
    DisabledEmptyDir,
    Hit,
    Store,
    WriteFailed,
    MkdirFailed,
}

impl AotCacheStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::DisabledNoEnv => "disabled_no_env",
            Self::DisabledEmptyDir => "disabled_empty_dir",
            Self::Hit => "hit",
            Self::Store => "store",
            Self::WriteFailed => "write_failed",
            Self::MkdirFailed => "mkdir_failed",
        }
    }
}

fn perf_trace_enabled() -> bool {
    std::env::var("RUSTMODLICA_PERF_TRACE")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

fn maybe_write_aot_cache_marker(
    model_name: &str,
    alg_equations: &[Equation],
    diff_equations: &[Equation],
    options: &crate::compiler::CompilerOptions,
) -> AotCacheStatus {
    let Ok(cache_dir) = std::env::var("RUSTMODLICA_AOT_CACHE_DIR") else {
        return AotCacheStatus::DisabledNoEnv;
    };
    if cache_dir.trim().is_empty() {
        return AotCacheStatus::DisabledEmptyDir;
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    model_name.hash(&mut hasher);
    options.solver.hash(&mut hasher);
    options.index_reduction_method.hash(&mut hasher);
    options.tearing_method.hash(&mut hasher);
    options.generate_dynamic_jacobian.hash(&mut hasher);
    alg_equations.len().hash(&mut hasher);
    diff_equations.len().hash(&mut hasher);
    for eq in alg_equations.iter().chain(diff_equations.iter()) {
        format!("{:?}", eq).hash(&mut hasher);
    }
    let key = format!("{:016x}", hasher.finish());
    let cache_root = std::path::PathBuf::from(cache_dir);
    if std::fs::create_dir_all(&cache_root).is_err() {
        return AotCacheStatus::MkdirFailed;
    }
    let path = cache_root.join(format!("{}.aot-marker", key));
    if path.exists() {
        eprintln!("[aot-cache] hit {}", path.display());
        if perf_trace_enabled() {
            eprintln!("[perf] aot_cache=hit key={}", key);
        }
        return AotCacheStatus::Hit;
    }
    let payload = format!(
        "model={}\nkey={}\nsolver={}\nalg_eqs={}\ndiff_eqs={}\n",
        model_name,
        key,
        options.solver,
        alg_equations.len(),
        diff_equations.len()
    );
    if std::fs::write(&path, payload).is_ok() {
        eprintln!("[aot-cache] store {}", path.display());
        if perf_trace_enabled() {
            eprintln!("[perf] aot_cache=store key={}", key);
        }
        return AotCacheStatus::Store;
    }
    if perf_trace_enabled() {
        eprintln!("[perf] aot_cache=write_failed key={}", key);
    }
    AotCacheStatus::WriteFailed
}

fn parse_clock_partition_trigger(id: &str) -> ClockPartitionTrigger {
    fn parse_clock_factor(token: &str) -> Option<f64> {
        if let Ok(v) = token.parse::<f64>() {
            if v <= 0.0 {
                return Some(1.0);
            }
            return Some(v);
        }
        if let Some(inner) = token
            .strip_prefix("Number(")
            .and_then(|s| s.strip_suffix(')'))
        {
            if let Ok(v) = inner.parse::<f64>() {
                if v <= 0.0 {
                    return Some(1.0);
                }
                return Some(v);
            }
            return None;
        }
        None
    }

    fn parse_sample_key(key: &str) -> Option<(f64, f64)> {
        let rest = key.strip_prefix("sample_")?;
        let mut parts = rest.splitn(2, '_');
        let interval = parts.next()?.parse::<f64>().ok()?;
        if interval <= 0.0 {
            return None;
        }
        let start = parts
            .next()
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);
        Some((start, interval))
    }

    fn parse_derived_key(key: &str) -> Option<(f64, f64)> {
        let (op, rest) = if let Some(r) = key.strip_prefix("subSample_") {
            ("sub", r)
        } else if let Some(r) = key.strip_prefix("superSample_") {
            ("super", r)
        } else if let Some(r) = key.strip_prefix("shiftSample_") {
            ("shift", r)
        } else if let Some(r) = key.strip_prefix("backSample_") {
            ("back", r)
        } else {
            return None;
        };
        let split = rest.rfind('_')?;
        let (base_key, factor_token) = rest.split_at(split);
        let factor = parse_clock_factor(factor_token.trim_start_matches('_'))?;
        if factor == 0.0 {
            return None;
        }
        let (base_start, base_interval) = parse_clock_key(base_key)?;
        match op {
            "sub" => Some((base_start, base_interval * factor)),
            "super" => Some((base_start, base_interval / factor)),
            "shift" => Some((base_start + factor * base_interval, base_interval)),
            "back" => Some((
                base_start + (factor - 1.0) * base_interval,
                base_interval * factor,
            )),
            _ => None,
        }
    }

    fn parse_clock_key(key: &str) -> Option<(f64, f64)> {
        parse_sample_key(key).or_else(|| parse_derived_key(key))
    }

    if let Some((start, interval)) = parse_clock_key(id) {
        if interval > 0.0 {
            return ClockPartitionTrigger::Sample { start, interval };
        }
    }
    ClockPartitionTrigger::Always
}

fn parse_coverage_status() -> Option<(f64, f64, f64, f64, Vec<String>)> {
    let candidate_paths = [
        Path::new("scripts/coverage_status.json"),
        Path::new("jit-compiler/scripts/coverage_status.json"),
    ];
    let path = candidate_paths.iter().find(|p| p.exists())?;
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let sem_target = v.get("semantic_target_percent")?.as_f64()?;
    let sem_current = v.get("semantic_current_percent")?.as_f64()?;
    let m34_target = v.get("modelica34_target_percent")?.as_f64()?;
    let m34_current = v.get("modelica34_current_percent")?.as_f64()?;
    let gaps = v
        .get("gaps")
        .and_then(|g| g.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(|s| s.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Some((sem_target, sem_current, m34_target, m34_current, gaps))
}

fn maybe_coverage_target_warning_message() -> Result<Option<String>, Box<dyn std::error::Error + Send + Sync>> {
    let Some((sem_target, sem_current, m34_target, m34_current, gaps)) = parse_coverage_status() else {
        return Ok(None);
    };
    let sem_ok = sem_current + f64::EPSILON >= sem_target;
    let m34_ok = m34_current + f64::EPSILON >= m34_target;
    if sem_ok && m34_ok {
        return Ok(None);
    }
    let gap_text = if gaps.is_empty() {
        "none listed".to_string()
    } else {
        gaps.join(", ")
    };
    let msg = format!(
        "coverage target not met: semantic {:.2}% / target {:.2}%, Modelica 3.4 {:.2}% / target {:.2}%. gaps: {}. Run `powershell -ExecutionPolicy Bypass -File scripts/run_mos_regression.ps1` and refresh `scripts/coverage_status.json`.",
        sem_current, sem_target, m34_current, m34_target, gap_text
    );
    let strict = std::env::var("RUSTMODLICA_COVERAGE_STRICT")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE"))
        .unwrap_or(false);
    if strict {
        return Err(msg.into());
    }
    Ok(Some(msg))
}

pub(super) fn compile(
    compiler: &mut Compiler,
    model_name: &str,
) -> Result<CompileOutput, Box<dyn std::error::Error + Send + Sync>> {
        let stage_trace = stage_trace_enabled();
        let perf_trace = perf_trace_enabled();
        compiler.last_compile_perf = None;

        compiler.warnings.clear();
        compiler.loader.set_quiet(compiler.options.quiet);
        let opts = &compiler.options;
        let model_file_path = format!("{}.mo", model_name.replace('.', "/"));
        let mut perf_report = CompilePerfReport {
            model_name: model_name.to_string(),
            ..Default::default()
        };
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

        if root_model.as_ref().is_function {
            if !compiler.options.quiet {
                if compiler.options.function_args.is_some() {
                    println!("{}", i18n::msg0("evaluating_function_args"));
                } else {
                    println!("{}", i18n::msg0("evaluating_function_default"));
                }
            }
            let value = compiler.run_function_once(model_name)?;
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
        let flatten_t0 = Instant::now();
        let frontend = flatten_and_inline(
            &mut root_model,
            model_name,
            &mut compiler.loader,
            compiler.options.quiet,
            stage_trace,
            snap_path,
            compiler.options.coarse_constrainedby_only,
        )?;
        perf_report.flatten_inline_ms = flatten_t0.elapsed().as_millis() as u64;
        if perf_trace {
            eprintln!(
                "[perf] compile_phase.flatten_inline_ms={}",
                perf_report.flatten_inline_ms
            );
        }
        let flat_model = frontend.flat_model;
        if compiler.options.flat_snapshot_only {
            compiler.last_compile_perf = Some(perf_report);
            return Ok(CompileOutput::FlatSnapshotDone);
        }
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

        let analyze_t0 = Instant::now();
        let mut variable_layout = classify_variables(&flat_model, opts.quiet, stage_trace);
        if !compiler.options.quiet {
            println!("{}", i18n::msg0("normalizing_derivatives"));
            println!("{}", i18n::msg0("performing_structure_analysis"));
        }
        let analysis_stage = analyze_equations(&flat_model, &mut variable_layout, opts, stage_trace);
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
        let alg_equations = analysis_stage.alg_equations;
        let diff_equations = analysis_stage.diff_equations;
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
        perf_report.blt_degrade_guard_triggered = blt_degrade_guard_triggered;
        perf_report.blt_degrade_guard_limit = blt_degrade_guard_limit;
        perf_report.blt_degrade_guard_equation_count = blt_degrade_guard_equation_count;
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
        let dae_t0 = Instant::now();
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
        let ida_component_id = ida_component_id_for_states(&simulation_dae, states.len());
        let dae_differential_index = simulation_dae.dae.differential_index;
        perf_report.backend_dae_ms = dae_t0.elapsed().as_millis() as u64;
        if perf_trace {
            eprintln!(
                "[perf] compile_phase.backend_dae_ms={}",
                perf_report.backend_dae_ms
            );
        }

        let external_t0 = Instant::now();
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
                blt_degrade_guard_triggered,
                blt_degrade_guard_limit,
                blt_degrade_guard_equation_count,
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
        let aot_cache_status =
            maybe_write_aot_cache_marker(model_name, &alg_equations, &diff_equations, opts);
        perf_report.aot_cache_status = aot_cache_status.as_str().to_string();
        if perf_trace {
            eprintln!("[perf] aot_cache_status={}", aot_cache_status.as_str());
        }
        let mut clock_partition_schedule: Vec<ClockPartitionScheduleEntry> = Vec::new();
        for part in &backend_clock_partitions {
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
                crate::jit::analysis::collect_modified_equations(
                    std::slice::from_ref(eq),
                    &mut modified,
                );
                if modified.iter().any(|v| var_set.contains(v)) {
                    alg_equation_indices.push(idx);
                }
            }

            let mut diff_equation_indices = Vec::new();
            for (idx, eq) in diff_equations.iter().enumerate() {
                let mut modified = HashSet::new();
                crate::jit::analysis::collect_modified_equations(
                    std::slice::from_ref(eq),
                    &mut modified,
                );
                if modified.iter().any(|v| var_set.contains(v)) {
                    diff_equation_indices.push(idx);
                }
            }

            clock_partition_schedule.push(ClockPartitionScheduleEntry {
                id: part.id.clone(),
                trigger: parse_clock_partition_trigger(&part.id),
                var_names,
                algorithm_indices,
                alg_equation_indices,
                diff_equation_indices,
            });
        }

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
            &newton_tearing_var_names,
        );
        perf_report.jit_ms = jit_t0.elapsed().as_millis() as u64;
        if perf_trace {
            eprintln!("[perf] compile_phase.jit_ms={}", perf_report.jit_ms);
        }

        match res {
            Ok((calc_derivs, when_count, crossings_count)) => {
                perf_report.jit_compile_ok = true;
                perf_report.jit_error = None;
                compiler.last_compile_perf = Some(perf_report);
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
                }))
            }
            Err(e) => {
                let err_text = e.to_string();
                perf_report.jit_compile_ok = false;
                perf_report.jit_error = Some(err_text.clone());
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
