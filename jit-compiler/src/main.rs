mod analysis;
mod ast;
mod backend_dae;
mod compiler;
mod diag;
mod equation_graph;
mod error;
mod expr_eval;
mod flatten;
mod fmi;
mod i18n;
mod jit;
mod loader_compat;
mod loader;
mod parser;
mod script;
mod simulation;
mod solver;
mod sparse_solve;
mod string_intern;

use compiler::{CompileOutput, Compiler};
use simulation::{run_simulation, run_simulation_collect};
use std::env;
use std::io::Read;
use std::process::ExitCode;
use std::thread;

type RunError = error::AppError;

fn emit_validate_json(
    success: bool,
    warnings: &[diag::WarningInfo],
    errors: &[String],
    state_vars: &[String],
    output_vars: &[String],
) {
    let warnings_json: Vec<serde_json::Value> = warnings
        .iter()
        .map(|w| {
            serde_json::json!({
                "path": w.path,
                "line": w.line,
                "column": w.column,
                "message": w.message
            })
        })
        .collect();
    let out = serde_json::json!({
        "success": success,
        "warnings": warnings_json,
        "errors": errors,
        "state_vars": state_vars,
        "output_vars": output_vars
    });
    println!("{}", serde_json::to_string(&out).unwrap_or_default());
}

/// INT-1: REPL loop. Commands: <var_name> (print value), simulate, list, quit/exit.
fn run_repl_loop(artifacts: compiler::Artifacts) -> Result<(), RunError> {
    use std::io::{self, BufRead, Write};
    println!(
        "REPL: type variable name to inspect, 'simulate' to run, 'list' for vars, 'quit' to exit."
    );
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    loop {
        write!(stdout, "> ").ok();
        stdout.flush().ok();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_err() || line.is_empty() {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_lowercase();
        if lower == "quit" || lower == "exit" {
            break;
        }
        if lower == "simulate" {
            println!("{}", i18n::msg0("starting_simulation"));
            run_simulation(
                artifacts.calc_derivs,
                artifacts.when_count,
                artifacts.crossings_count,
                artifacts.states.clone(),
                artifacts.discrete_vals.clone(),
                artifacts.params.clone(),
                &artifacts.state_vars,
                &artifacts.discrete_vars,
                &artifacts.output_vars,
                &artifacts.output_start_vals,
                &artifacts.state_var_index,
                artifacts.t_end,
                artifacts.dt,
                artifacts.numeric_ode_jacobian,
                artifacts.symbolic_ode_jacobian.as_ref(),
                &artifacts.newton_tearing_var_names,
                artifacts.atol,
                artifacts.rtol,
                &artifacts.solver,
                artifacts.output_interval,
                artifacts.result_file.as_deref(),
                None,
            )?;
            println!("{}", i18n::msg0("simulation_completed"));
            continue;
        }
        if lower == "list" || lower == "vars" {
            for n in &artifacts.state_vars {
                println!("  state {}", n);
            }
            for n in &artifacts.param_vars {
                println!("  param {}", n);
            }
            for n in &artifacts.discrete_vars {
                println!("  discrete {}", n);
            }
            continue;
        }
        let name = line;
        if let Some(&i) = artifacts.state_var_index.get(name) {
            if i < artifacts.states.len() {
                println!("{}", artifacts.states[i]);
                continue;
            }
        }
        if let Some((i, _)) = artifacts
            .param_vars
            .iter()
            .enumerate()
            .find(|(_, s)| *s == name)
        {
            if i < artifacts.params.len() {
                println!("{}", artifacts.params[i]);
                continue;
            }
        }
        if let Some((i, _)) = artifacts
            .discrete_vars
            .iter()
            .enumerate()
            .find(|(_, s)| *s == name)
        {
            if i < artifacts.discrete_vals.len() {
                println!("{}", artifacts.discrete_vals[i]);
                continue;
            }
        }
        println!("unknown variable: {}", name);
    }
    Ok(())
}

fn run(args: Vec<String>) -> Result<(), RunError> {
    let mut backend_dae_info = false;
    let mut index_reduction_method = "none".to_string();
    let mut tearing_method = "first".to_string();
    let mut generate_dynamic_jacobian = "none".to_string();
    let mut t_end = 10.0_f64;
    let mut dt = 0.01_f64;
    let mut atol = 1e-6_f64;
    let mut rtol = 1e-3_f64;
    let mut function_args: Option<Vec<f64>> = None;
    let mut solver = "rk45".to_string();
    let mut output_interval = 0.05_f64;
    let mut result_file: Option<String> = None;
    let mut warnings_level = "all".to_string();
    let mut emit_c_dir: Option<String> = None;
    let mut emit_fmu_dir: Option<String> = None;
    let mut emit_fmu_me_dir: Option<String> = None;
    let mut external_libs: Vec<String> = Vec::new();
    let mut lib_paths: Vec<String> = Vec::new();
    let mut repl = false;
    let mut script_path: Option<String> = None;
    let mut model_name = None;
    let mut validate_only = false;
    let mut output_format: Option<String> = None;
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if let Some(lang) = a.strip_prefix("--lang=") {
            let _ = std::env::set_var("RUSTMODLICA_LANG", lang);
            i += 1;
        } else if a == "--backend-dae-info" {
            backend_dae_info = true;
            i += 1;
        } else if a == "-d" && i + 1 < args.len() {
            if args[i + 1] == "backenddaeinfo" {
                backend_dae_info = true;
            }
            i += 2;
        } else if a.starts_with("-d=")
            && a.strip_prefix("-d=")
                .map(|s| s == "backenddaeinfo")
                .unwrap_or(false)
        {
            backend_dae_info = true;
            i += 1;
        } else if let Some(v) = a.strip_prefix("--index-reduction-method=") {
            index_reduction_method = v.to_string();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--tearing-method=") {
            tearing_method = v.to_string();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--generate-dynamic-jacobian=") {
            generate_dynamic_jacobian = v.to_string();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--t-end=") {
            t_end = v.parse().unwrap_or(10.0);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--dt=") {
            dt = v.parse().unwrap_or(0.01);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--atol=") {
            atol = v.parse().unwrap_or(1e-6);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--rtol=") {
            rtol = v.parse().unwrap_or(1e-3);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--function-args=") {
            let parsed: Result<Vec<f64>, _> =
                v.split(',').map(|s| s.trim().parse::<f64>()).collect();
            function_args = parsed.ok();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--solver=") {
            solver = v.to_string();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--output-interval=") {
            output_interval = v.parse().unwrap_or(0.05);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--result-file=") {
            result_file = Some(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--warnings=") {
            warnings_level = v.to_string();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--emit-c=") {
            emit_c_dir = Some(v.to_string());
            i += 1;
        } else if a == "--repl" {
            repl = true;
            i += 1;
        } else if let Some(v) = a.strip_prefix("--script=") {
            script_path = Some(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--emit-fmu=") {
            emit_fmu_dir = Some(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--emit-fmu-me=") {
            emit_fmu_me_dir = Some(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--external-lib=") {
            external_libs.push(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--lib-path=") {
            if !v.trim().is_empty() {
                lib_paths.push(v.to_string());
            }
            i += 1;
        } else if let Some(v) = a.strip_prefix("--modelica-stdlib=") {
            if !v.trim().is_empty() {
                lib_paths.push(v.to_string());
            }
            i += 1;
        } else if a == "--validate" {
            validate_only = true;
            i += 1;
        } else if a == "-" {
            model_name = Some("-".to_string());
            i += 1;
            break;
        } else if let Some(v) = a.strip_prefix("--output-format=") {
            output_format = Some(v.to_string());
            i += 1;
        } else if !a.starts_with('-') {
            model_name = Some(a.clone());
            i += 1;
            break;
        } else {
            i += 1;
        }
    }
    if i < args.len() {
        return Err("Unknown or extra arguments after model name.".into());
    }
    if let Some(ref path) = script_path {
        let mut compiler = Compiler::new();
        compiler.options.backend_dae_info = backend_dae_info;
        compiler.options.index_reduction_method = index_reduction_method;
        compiler.options.tearing_method = tearing_method;
        compiler.options.generate_dynamic_jacobian = generate_dynamic_jacobian;
        compiler.options.t_end = t_end;
        compiler.options.dt = dt;
        compiler.options.atol = atol;
        compiler.options.rtol = rtol;
        compiler.options.function_args = function_args;
        compiler.options.solver = solver;
        compiler.options.output_interval = output_interval;
        compiler.options.result_file = result_file;
        compiler.options.warnings_level = warnings_level;
        compiler.options.emit_c_dir = emit_c_dir.clone();
        compiler.options.external_libs = external_libs;
        for p in &lib_paths {
            compiler.loader.add_path(p.into());
        }
        compiler.loader.add_path(".".into());
        compiler.loader.add_path("StandardLib".into());
        compiler.loader.add_path("TestLib".into());
        let mut runner = script::ScriptRunner::new(compiler);
        if path == "-" {
            runner.run_script(std::io::stdin())?;
        } else {
            let f = std::fs::File::open(path)
                .map_err(|e| format!("Cannot open script file {}: {}", path, e))?;
            runner.run_script(f)?;
        }
        return Ok(());
    }
    let model_name = match model_name {
        Some(n) => n,
        None => {
            let msg = format!(
                "Usage: {} [options] <model_name>\n  --lang=en|zh  message language\n  --validate  compile only, output JSON to stdout\n  --output-format=json  simulation: output time series as JSON to stdout\n  --solver=rk4|rk45|implicit  (default: rk45)\n  --warnings=all|none|error  (default: all)\n  --backend-dae-info  print backend DAE statistics\n  --index-reduction-method=<none|dummyDerivative|debugPrint>\n  --t-end=<float>  --dt=<float>  --atol=<float>  --rtol=<float>\n  --output-interval=<float>  (default 0.05)\n  --result-file=<path>  write CSV time series to file\n  --emit-c=<dir>  emit C source (model.c, model.h) to directory\n  --repl  after compile, enter REPL (inspect vars, simulate, quit)\n  --script=<path>  run script file (load, setParameter, simulate, quit); use - for stdin\n  --emit-fmu=<dir>  emit C + modelDescription.xml + fmi2_cs.c for FMI 2.0 CS\n  --emit-fmu-me=<dir>  emit C + modelDescription.xml + fmi2_me.c for FMI 2.0 ME\n  --external-lib=<path>  load shared library for external function symbols (EXT-1; repeatable)\n  --lib-path=<dir>  add a Modelica library root to the loader search path (repeatable)\n  --modelica-stdlib=<dir>  alias of --lib-path\n  --function-args=<f1,f2,...>  function input values\n  Use model_name '-' to read Modelica source from stdin.",
                args[0]
            );
            return Err(msg.into());
        }
    };
    let mut compiler = Compiler::new();
    compiler.options.backend_dae_info = backend_dae_info;
    compiler.options.index_reduction_method = index_reduction_method;
    compiler.options.tearing_method = tearing_method;
    compiler.options.generate_dynamic_jacobian = generate_dynamic_jacobian;
    compiler.options.t_end = t_end;
    compiler.options.dt = dt;
    compiler.options.atol = atol;
    compiler.options.rtol = rtol;
    compiler.options.function_args = function_args;
    compiler.options.solver = solver;
    compiler.options.output_interval = output_interval;
    compiler.options.result_file = result_file;
    compiler.options.warnings_level = warnings_level;
    if emit_fmu_dir.is_some() && emit_c_dir.is_none() {
        emit_c_dir = emit_fmu_dir.clone();
    }
    if emit_fmu_me_dir.is_some() && emit_c_dir.is_none() {
        emit_c_dir = emit_fmu_me_dir.clone();
    }
    compiler.options.emit_c_dir = emit_c_dir;
    compiler.options.external_libs = external_libs;
    let run_repl = repl;
    let json_mode = validate_only || output_format.as_deref() == Some("json");
    if validate_only {
        compiler.options.quiet = true;
    }
    for p in &lib_paths {
        compiler.loader.add_path(p.into());
    }
    compiler.loader.add_path(".".into());
    compiler.loader.add_path("StandardLib".into());
    compiler.loader.add_path("TestLib".into());

    let effective_model = if model_name == "-" {
        let mut code = String::new();
        std::io::stdin()
            .read_to_string(&mut code)
            .map_err(|e| format!("Failed to read stdin: {}", e))?;
        compiler
            .loader
            .load_model_from_source("<stdin>", &code)
            .map_err(|e| format!("{}", e))?;
        "<stdin>".to_string()
    } else {
        model_name.clone()
    };

    if !json_mode {
        println!(
            "{}",
            i18n::msg("compiling", &[&effective_model as &dyn std::fmt::Display])
        );
    }
    let out = match compiler.compile(&effective_model) {
        Ok(o) => o,
        Err(e) => {
            let warnings = compiler.take_warnings();
            if validate_only {
                emit_validate_json(false, &warnings, &[e.to_string()], &[], &[]);
                return Ok(());
            }
            return Err(e.into());
        }
    };
    let warnings = compiler.take_warnings();
    let warn_level = compiler.options.warnings_level.as_str();
    if validate_only {
        match &out {
            CompileOutput::FunctionRun(_) => {
                emit_validate_json(true, &warnings, &[], &[], &[]);
            }
            CompileOutput::Simulation(artifacts) => {
                emit_validate_json(
                    true,
                    &warnings,
                    &[],
                    &artifacts.state_vars,
                    &artifacts.output_vars,
                );
            }
        }
        return Ok(());
    }
    if warn_level != "none" {
        for w in &warnings {
            if warn_level == "error" {
                return Err(w.to_string().into());
            }
            eprintln!("{}", w);
        }
        if !warnings.is_empty() && warn_level == "all" {
            eprintln!("{}", i18n::msg("warnings_generated", &[&warnings.len()]));
        }
    }
    match out {
        CompileOutput::FunctionRun(value) => {
            if !json_mode {
                println!("{}", i18n::msg("result", &[&value]));
            }
            return Ok(());
        }
        CompileOutput::Simulation(artifacts) => {
            println!("{}", i18n::msg0("compilation_successful"));
            println!("{}", i18n::msg("states", &[&artifacts.states.len()]));
            println!(
                "{}",
                i18n::msg("discrete_vars", &[&artifacts.discrete_vars.len()])
            );
            println!("{}", i18n::msg("parameters", &[&artifacts.params.len()]));
            println!("{}", i18n::msg("outputs", &[&artifacts.output_vars.len()]));
            println!("{}", i18n::msg("when_statements", &[&artifacts.when_count]));
            println!(
                "{}",
                i18n::msg("zero_crossings", &[&artifacts.crossings_count])
            );
            if let Some(ref dir) = emit_fmu_dir {
                let path = std::path::Path::new(dir);
                match fmi::emit_fmu_artifacts(
                    path,
                    &effective_model,
                    &artifacts.state_vars,
                    &artifacts.param_vars,
                    &artifacts.output_vars,
                    0.0,
                    artifacts.t_end,
                    artifacts.dt,
                ) {
                    Ok(files) => {
                        let paths: Vec<String> =
                            files.iter().map(|p| p.display().to_string()).collect();
                        println!("FMI CS: emitted {}", paths.join(", "));
                    }
                    Err(e) => return Err(format!("FMI CS emit failed: {}", e).into()),
                }
            }
            if let Some(ref dir) = emit_fmu_me_dir {
                let path = std::path::Path::new(dir);
                match fmi::emit_fmu_me_artifacts(
                    path,
                    &effective_model,
                    &artifacts.state_vars,
                    &artifacts.param_vars,
                    &artifacts.output_vars,
                    0.0,
                    artifacts.t_end,
                    artifacts.dt,
                ) {
                    Ok(files) => {
                        let paths: Vec<String> =
                            files.iter().map(|p| p.display().to_string()).collect();
                        println!("FMI ME: emitted {}", paths.join(", "));
                    }
                    Err(e) => return Err(format!("FMI ME emit failed: {}", e).into()),
                }
            }
            if run_repl {
                run_repl_loop(artifacts)?;
                return Ok(());
            }
            if emit_fmu_dir.is_some() || emit_fmu_me_dir.is_some() {
                return Ok(());
            }
            if output_format.as_deref() == Some("json") {
                let result = run_simulation_collect(
                    artifacts.calc_derivs,
                    artifacts.when_count,
                    artifacts.crossings_count,
                    artifacts.states,
                    artifacts.discrete_vals,
                    artifacts.params,
                    &artifacts.state_vars,
                    &artifacts.discrete_vars,
                    &artifacts.output_vars,
                    &artifacts.output_start_vals,
                    &artifacts.state_var_index,
                    artifacts.t_end,
                    artifacts.dt,
                    artifacts.numeric_ode_jacobian,
                    artifacts.symbolic_ode_jacobian.as_ref(),
                    &artifacts.newton_tearing_var_names,
                    artifacts.atol,
                    artifacts.rtol,
                    &artifacts.solver,
                    artifacts.output_interval,
                )?;
                println!("{}", serde_json::to_string(&result).unwrap_or_default());
                return Ok(());
            }
            if !json_mode {
                println!("{}", i18n::msg0("starting_simulation"));
            }
            run_simulation(
                artifacts.calc_derivs,
                artifacts.when_count,
                artifacts.crossings_count,
                artifacts.states,
                artifacts.discrete_vals,
                artifacts.params,
                &artifacts.state_vars,
                &artifacts.discrete_vars,
                &artifacts.output_vars,
                &artifacts.output_start_vals,
                &artifacts.state_var_index,
                artifacts.t_end,
                artifacts.dt,
                artifacts.numeric_ode_jacobian,
                artifacts.symbolic_ode_jacobian.as_ref(),
                &artifacts.newton_tearing_var_names,
                artifacts.atol,
                artifacts.rtol,
                &artifacts.solver,
                artifacts.output_interval,
                artifacts.result_file.as_deref(),
                None,
            )?;
            if !json_mode {
                println!("{}", i18n::msg0("simulation_completed"));
            }
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    const STACK_SIZE: usize = 8 * 1024 * 1024;
    let child = thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(move || run(args))
        .map_err(|e| error::AppError::ThreadSpawn(e.to_string()));
    let child = match child {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(1);
        }
    };
    match child.join() {
        Err(_) => {
            eprintln!("{}", error::AppError::ThreadPanic);
            ExitCode::from(1)
        }
        Ok(Err(e)) => {
            eprintln!("{}", e);
            ExitCode::from(1)
        }
        Ok(Ok(())) => ExitCode::from(0),
    }
}
