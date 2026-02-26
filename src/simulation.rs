use std::process;
use crate::jit::CalcDerivsFunc;
use crate::solver::{Solver, System, RungeKutta4Solver};

pub fn run_simulation(
    calc_derivs: CalcDerivsFunc,
    when_count: usize,
    crossings_count: usize,
    mut states: Vec<f64>,
    mut discrete_vals: Vec<f64>,
    params: Vec<f64>,
    state_vars: &[String],
    discrete_vars: &[String],
    output_vars: &[String],
    t_end: f64,
    dt: f64,
) {
    let mut time = 0.0;
    let mut derivs = vec![0.0; states.len()];
    let mut outputs = vec![0.0; output_vars.len()];
    let mut when_states = vec![0.0; when_count * 2];
    let mut crossings = vec![0.0; crossings_count];
    let mut pre_states = vec![0.0; states.len()]; 
    let mut pre_discrete_vals = vec![0.0; discrete_vals.len()];
    
    // Initialize Solver
    let mut solver = RungeKutta4Solver::new(states.len());

    // Header
    print!("Time");
    for var in state_vars { print!(", {}", var); }
    for var in discrete_vars { print!(", {}", var); }
    for var in output_vars { print!(", {}", var); }
    println!();

    let print_interval = 0.05; 
    let mut next_print = 0.0;
    let epsilon = 1e-5;

    while time <= t_end + epsilon {
        // 1. Event Iteration Loop (Handle events at current time)
        // Capture pre-states (left limit) before event iteration
        pre_states.copy_from_slice(&states);
        pre_discrete_vals.copy_from_slice(&discrete_vals);
        
        let mut event_iter_count = 0;
        loop {
            unsafe {
                // Call JIT function
                let status = (calc_derivs)(time, states.as_mut_ptr(), discrete_vals.as_mut_ptr(), derivs.as_mut_ptr(), params.as_ptr(), outputs.as_mut_ptr(), when_states.as_mut_ptr(), crossings.as_mut_ptr(), pre_states.as_ptr(), pre_discrete_vals.as_ptr());
                if status != 0 {
                    eprintln!("Error: Simulation failed at time {:.4} with status code {}", time, status);
                    if status == 2 {
                        eprintln!("  (Newton-Raphson Divergence)");
                    }
                    process::exit(1);
                }
            }

            let mut converged = true;
            if when_count > 0 {
                for i in 0..when_count {
                    let idx_pre = i * 2;
                    let idx_new = i * 2 + 1;
                    let pre_val = when_states[idx_pre];
                    let new_val = when_states[idx_new];

                    // Check if value changed
                    if pre_val != new_val {
                        // Event detected! Update pre value for next iteration
                        when_states[idx_pre] = new_val;
                        converged = false;
                    }
                }
            }
            
            if converged {
                break;
            }

            event_iter_count += 1;
            if event_iter_count > 100 {
                eprintln!("Warning: Event loop did not converge at time {}", time);
                break;
            }
        }
        
        // Print Output
        if time >= next_print - epsilon {
            print!("{:.4}", time);
            for val in &states { print!(", {:.4}", val); }
            for val in &discrete_vals { print!(", {:.4}", val); }
            for val in &outputs { print!(", {:.4}", val); }
            println!();
            next_print += print_interval;
        }

        // 2. Integration Step (Variable Step for Zero-Crossing)
        
        // Save current state
        let state_at_t = states.clone();
        let discrete_at_t = discrete_vals.clone();
        let crossings_at_t = crossings.clone();
        // let derivs_at_t = derivs.clone(); // Not needed for solver
        
        // Trial Step (RK4)
        {
            let mut system = System {
                calc_derivs,
                params: &params,
                discrete: &mut discrete_vals,
                outputs: &mut outputs,
                when_states: &mut when_states,
                crossings: &mut crossings,
                pre_states: &pre_states,
                pre_discrete: &pre_discrete_vals,
            };
            // Note: system.evaluate uses `derivs` buffer if needed, but solver might use internal buffers.
            // EulerSolver uses passed buffer. RK4 uses internal k1..k4.
            // We can pass `derivs` as scratch space if needed, but `solver.step` signature doesn't take it explicitly?
            // Ah, `step` takes `system`. `system.evaluate` takes `derivs`.
            // My `Solver::step` signature: `fn step(..., states: &mut [f64]) -> Result`.
            // It does NOT take `derivs` buffer.
            // Wait, `EulerSolver` has internal `derivs`.
            // `System::evaluate` signature: `fn evaluate(..., derivs: &mut [f64])`.
            // So `EulerSolver` calls `system.evaluate(..., &mut self.derivs)`.
            // `RK4` calls `system.evaluate_scratch(..., &mut self.k1)`.
            // So `derivs` buffer in `run_simulation` is NOT used by `solver.step`.
            // It is only used by `calc_derivs` call in event loop and trial check.
            solver.step(&mut system, time, dt, &mut states).unwrap();
        }
        let t_trial = time + dt;
        
        // Evaluate at trial point
        unsafe {
            let status = (calc_derivs)(t_trial, states.as_mut_ptr(), discrete_vals.as_mut_ptr(), derivs.as_mut_ptr(), params.as_ptr(), outputs.as_mut_ptr(), when_states.as_mut_ptr(), crossings.as_mut_ptr(), pre_states.as_ptr(), pre_discrete_vals.as_ptr());
            if status != 0 {
                eprintln!("Error: Simulation failed at time {:.4} (trial step) with status code {}", t_trial, status);
                process::exit(1);
            }
        }
        
        // Check for Zero-Crossings
        let mut min_alpha = 1.0;
        let mut event_found = false;
        
        for i in 0..crossings_count {
            let c_prev = crossings_at_t[i];
            let c_curr = crossings[i];
            
            if c_prev * c_curr < 0.0 {
                event_found = true;
                let diff = c_curr - c_prev;
                if diff.abs() > 1e-12 {
                    // Linear Interpolation: 0 = prev + alpha * diff
                    let alpha = -c_prev / diff;
                    if alpha > 0.0 && alpha < min_alpha {
                        min_alpha = alpha;
                    }
                }
            }
        }
        
        if event_found {
                let dt_event = dt * min_alpha;
                // Ensure dt_event is not too small (infinite loop)
                if dt_event < 1e-10 {
                    // Force a small step to cross?
                }
                
                // Restore and advance
                states = state_at_t;
                discrete_vals = discrete_at_t; // Discrete vars constant during step
                
                {
                    let mut system = System {
                        calc_derivs,
                        params: &params,
                        discrete: &mut discrete_vals,
                        outputs: &mut outputs,
                        when_states: &mut when_states,
                        crossings: &mut crossings,
                        pre_states: &pre_states,
                        pre_discrete: &pre_discrete_vals,
                    };
                    solver.step(&mut system, time, dt_event, &mut states).unwrap();
                }
                time += dt_event;
                
                // Don't print, next loop will handle event
        } else {
            // Accept full step
            time = t_trial;
        }
    }
}
