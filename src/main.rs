mod ast;
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
use compiler::Compiler;
use simulation::run_simulation;

fn run(args: Vec<String>) {
    let mut backend_dae_info = false;
    let mut index_reduction_method = "none".to_string();
    let mut generate_dynamic_jacobian = "none".to_string();
    let mut model_name = None;
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if a == "--backend-dae-info" {
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
        } else if let Some(v) = a.strip_prefix("--generate-dynamic-jacobian=") {
            generate_dynamic_jacobian = v.to_string();
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
        eprintln!("Unknown or extra arguments after model name.");
        process::exit(1);
    }
    let model_name = match model_name {
        Some(n) => n,
        None => {
            eprintln!("Usage: {} [options] <model_name>", args[0]);
            eprintln!("  --backend-dae-info, -d=backenddaeinfo  print backend DAE statistics (OpenModelica-style)");
            eprintln!("  --index-reduction-method=<none|uode|dynamicStateSelection|dummyDerivatives>  (default: none)");
            eprintln!("  --generate-dynamic-jacobian=<none|numeric|symbolic>  (default: none)");
            process::exit(1);
        }
    };
    let mut compiler = Compiler::new();
    compiler.options.backend_dae_info = backend_dae_info;
    compiler.options.index_reduction_method = index_reduction_method;
    compiler.options.generate_dynamic_jacobian = generate_dynamic_jacobian;
    compiler.loader.add_path(".".into());
    compiler.loader.add_path("StandardLib".into());
    compiler.loader.add_path("TestLib".into());
    println!("Compiling {}...", &model_name);
    let artifacts = match compiler.compile(&model_name) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Compilation failed: {}", e);
            process::exit(1);
        }
    };
    println!("Compilation successful!");
    println!("  States: {}", artifacts.states.len());
    println!("  Discrete Vars: {}", artifacts.discrete_vars.len());
    println!("  Parameters: {}", artifacts.params.len());
    println!("  Outputs: {}", artifacts.output_vars.len());
    println!("  When Statements: {}", artifacts.when_count);
    println!("  Zero-Crossings: {}", artifacts.crossings_count);
    println!("Starting simulation...");
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
        artifacts.t_end,
        artifacts.dt,
    );
    println!("Simulation completed.");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    const STACK_SIZE: usize = 8 * 1024 * 1024;
    let child = thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(move || run(args))
        .expect("failed to spawn thread");
    child.join().expect("thread panicked");
}
