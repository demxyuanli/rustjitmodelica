use rustmodlica::{run_simulation, Artifacts};
use rustmodlica::i18n;

use super::RunError;

pub(crate) fn run_repl_loop(artifacts: Artifacts, model_name: &str, lib_paths: Vec<std::path::PathBuf>) -> Result<(), RunError> {
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
                artifacts.differential_index,
                artifacts.ida_component_id.as_slice(),
                &artifacts.solver,
                artifacts.output_interval,
                artifacts.result_file.as_deref(),
                &artifacts.clock_partition_schedule,
                None,
                None,
                model_name,
                lib_paths.clone(),
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
