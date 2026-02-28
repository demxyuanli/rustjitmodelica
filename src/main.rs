mod ast;
mod backend_dae;
mod expr_eval;
mod i18n;
mod diag;
mod parser;
mod loader;
mod flatten;
mod analysis;
mod jit;
mod simulation;
mod compiler;
mod solver;

use std::env;
use std::process;
use std::thread;
use compiler::{Compiler, CompileOutput};
use simulation::run_simulation;

type RunError = Box<dyn std::error::Error + Send + Sync>;

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
    let mut warnings_level = "all".to_string();
    let mut model_name = None;
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
        } else if a.starts_with("-d=") && a.strip_prefix("-d=").map(|s| s == "backenddaeinfo").unwrap_or(false) {
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
            let parsed: Result<Vec<f64>, _> = v.split(',').map(|s| s.trim().parse::<f64>()).collect();
            function_args = parsed.ok();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--solver=") {
            solver = v.to_string();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--warnings=") {
            warnings_level = v.to_string();
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
    let model_name = match model_name {
        Some(n) => n,
        None => {
            let msg = format!(
                "Usage: {} [options] <model_name>\n  --lang=en|zh  message language\n  --solver=rk4|rk45  (default: rk45)\n  --warnings=all|none|error  (default: all)\n  --backend-dae-info  print backend DAE statistics\n  --index-reduction-method=<none|dummyDerivative|debugPrint>\n  --t-end=<float>  --dt=<float>  --atol=<float>  --rtol=<float>\n  --function-args=<f1,f2,...>  function input values",
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
    compiler.options.warnings_level = warnings_level;
    compiler.loader.add_path(".".into());
    compiler.loader.add_path("StandardLib".into());
    compiler.loader.add_path("TestLib".into());
    println!("{}", i18n::msg("compiling", &[&model_name as &dyn std::fmt::Display]));
    let out = compiler.compile(&model_name)?;
    let warnings = compiler.take_warnings();
    let warn_level = compiler.options.warnings_level.as_str();
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
            println!("{}", i18n::msg("result", &[&value]));
            return Ok(());
        }
        CompileOutput::Simulation(artifacts) => {
            println!("{}", i18n::msg0("compilation_successful"));
            println!("{}", i18n::msg("states", &[&artifacts.states.len()]));
            println!("{}", i18n::msg("discrete_vars", &[&artifacts.discrete_vars.len()]));
            println!("{}", i18n::msg("parameters", &[&artifacts.params.len()]));
            println!("{}", i18n::msg("outputs", &[&artifacts.output_vars.len()]));
            println!("{}", i18n::msg("when_statements", &[&artifacts.when_count]));
            println!("{}", i18n::msg("zero_crossings", &[&artifacts.crossings_count]));
            println!("{}", i18n::msg0("starting_simulation"));
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
                &artifacts.state_var_index,
                artifacts.t_end,
                artifacts.dt,
                artifacts.numeric_ode_jacobian,
                artifacts.symbolic_ode_jacobian.as_ref(),
                &artifacts.newton_tearing_var_names,
                artifacts.atol,
                artifacts.rtol,
                &artifacts.solver,
            )?;
            println!("{}", i18n::msg0("simulation_completed"));
        }
    }
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    const STACK_SIZE: usize = 8 * 1024 * 1024;
    let child = thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(move || run(args))
        .expect("failed to spawn thread");
    let status = child.join().expect("thread panicked");
    if let Err(e) = status {
        eprintln!("{}", e);
        process::exit(1);
    }
}
