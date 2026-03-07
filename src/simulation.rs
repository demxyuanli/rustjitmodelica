// RT1-1: DAE/ODE solver with events. Adaptive RK45 when no when/zero-crossing; fixed-step with
// event detection and reinit when when/zero-crossing present. Event iteration at each time step.
use std::collections::HashMap;
use std::io::{self, Write};
use std::fs::File;
use crate::jit::{CalcDerivsFunc, native};
use crate::i18n;
use crate::solver::{Solver, System, RungeKutta4Solver, AdaptiveRK45Solver, BackwardEulerSolver};
use crate::ast::Expression;

/// Serializable simulation time series for IDE/Plotly (time + series per variable).
#[derive(Debug, Clone, serde::Serialize)]
pub struct SimulationResult {
    pub time: Vec<f64>,
    pub series: HashMap<String, Vec<f64>>,
}

/// Row collector for run_simulation when collecting in-memory (time, states, discrete, outputs).
pub type ResultCollector = Vec<(f64, Vec<f64>, Vec<f64>, Vec<f64>)>;

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
    state_var_index: &HashMap<String, usize>,
    t_end: f64,
    dt: f64,
    numeric_ode_jacobian: bool,
    symbolic_ode_jacobian: Option<&Vec<Vec<Expression>>>,
    newton_tearing_var_names: &[String],
    atol: f64,
    rtol: f64,
    solver: &str,
    output_interval: f64,
    result_file: Option<&str>,
    mut result_collector: Option<&mut ResultCollector>,
) -> Result<(), String> {
    let mut time = 0.0;
    let mut derivs = vec![0.0; states.len()];
    let mut outputs = vec![0.0; output_vars.len()];
    if states.is_empty() && !output_vars.is_empty() {
        for o in outputs.iter_mut() {
            *o = 1.0;
        }
    }
    let mut when_states = vec![0.0; when_count * 2];
    let mut crossings = vec![0.0; crossings_count];
    let mut pre_states = vec![0.0; states.len()]; 
    let mut pre_discrete_vals = vec![0.0; discrete_vals.len()];
    
    // RT1-3: Use adaptive RK45 only when solver is rk45 and no when/zero-crossing.
    let use_adaptive = solver == "rk45" && when_count == 0 && crossings_count == 0;
    let use_implicit = solver == "implicit";
    let mut rk4_solver = RungeKutta4Solver::new(states.len());
    let mut rk45_solver = AdaptiveRK45Solver::new(states.len(), atol, rtol);
    let mut backward_euler_solver = BackwardEulerSolver::new(states.len());

    let mut out: Box<dyn Write> = if result_collector.is_some() {
        Box::new(io::sink())
    } else if let Some(path) = result_file {
        let f = File::create(path).map_err(|e| format!("Failed to create result file {}: {}", path, e))?;
        Box::new(std::io::BufWriter::new(f))
    } else {
        Box::new(io::stdout())
    };

    let w = &mut out;
    let mut write_row = |line: &str| -> Result<(), String> {
        w.write_all(line.as_bytes()).map_err(|e| e.to_string())?;
        w.write_all(b"\n").map_err(|e| e.to_string())?;
        w.flush().map_err(|e| e.to_string())?;
        Ok(())
    };

    let mut header = i18n::msg0("time").to_string();
    for var in state_vars { header.push_str(&format!(", {}", var)); }
    for var in discrete_vars { header.push_str(&format!(", {}", var)); }
    for var in output_vars { header.push_str(&format!(", {}", var)); }
    write_row(&header)?;

    let print_interval = output_interval;
    let mut next_print = 0.0;
    let epsilon = 1e-5;
    let mut adaptive_step_count: u64 = 0;

    native::reset_terminate_flag();

    while time <= t_end + epsilon {
        // 1. Event Iteration Loop (Handle events at current time)
        // Capture pre-states (left limit) before event iteration
        pre_states.copy_from_slice(&states);
        pre_discrete_vals.copy_from_slice(&discrete_vals);
        
        let mut event_iter_count = 0;
        let (mut diag_residual, mut diag_x) = (0.0_f64, 0.0_f64);
        let (diag_res_ptr, diag_x_ptr) = if newton_tearing_var_names.is_empty() {
            (std::ptr::null_mut(), std::ptr::null_mut())
        } else {
            (&mut diag_residual as *mut f64, &mut diag_x as *mut f64)
        };
        let mut diag_call_index = 0u32;
        let mut diag_time = 0.0_f64;
        let mut diag_state = vec![0.0_f64; states.len()];
        let (eval_call_index_ptr, last_eval_time_ptr, last_eval_state_ptr, last_eval_state_len) =
            if newton_tearing_var_names.is_empty() {
                (std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), 0)
            } else {
                (
                    &mut diag_call_index as *mut u32,
                    &mut diag_time as *mut f64,
                    diag_state.as_mut_ptr(),
                    diag_state.len(),
                )
            };
        let mut scratch_outputs_for_step = vec![0.0_f64; output_vars.len()];

        const ALG_FIXED_POINT_MAX: u32 = 15;
        let do_alg_iter = states.is_empty() && !output_vars.is_empty() && newton_tearing_var_names.len() > 0;
        let mut alg_iter = 0u32;
        let mut prev_outputs = vec![0.0; output_vars.len()];

        loop {
            unsafe {
                let status = (calc_derivs)(
                    time,
                    states.as_mut_ptr(),
                    discrete_vals.as_mut_ptr(),
                    derivs.as_mut_ptr(),
                    params.as_ptr(),
                    outputs.as_mut_ptr(),
                    when_states.as_mut_ptr(),
                    crossings.as_mut_ptr(),
                    pre_states.as_ptr(),
                    pre_discrete_vals.as_ptr(),
                    t_end,
                    diag_res_ptr,
                    diag_x_ptr,
                );
                if status != 0 {
                    let t_fmt = format!("{:.4}", time);
                    eprintln!("{}", i18n::msg("simulation_failed_at", &[&t_fmt as &dyn std::fmt::Display, &status]));
                    if status == 2 {
                        eprintln!("{}", i18n::msg0("newton_failure"));
                        if !newton_tearing_var_names.is_empty() {
                            let names = newton_tearing_var_names.join(", ");
                            let res_fmt = format!("{:.6e}", diag_residual);
                            let val_fmt = format!("{:.6e}", diag_x);
                            eprintln!("{}", i18n::msg("tearing_vars_residual", &[&names as &dyn std::fmt::Display, &res_fmt as &dyn std::fmt::Display, &val_fmt as &dyn std::fmt::Display]));
                        }
                    }
                    return Err(format!("Simulation failed at t={:.4} with status {}", time, status));
                }
            }

            if do_alg_iter && alg_iter < ALG_FIXED_POINT_MAX {
                let max_diff = if alg_iter == 0 {
                    1.0
                } else {
                    prev_outputs.iter().zip(outputs.iter())
                        .map(|(a, b)| (a - b).abs()).fold(0.0_f64, f64::max)
                };
                if alg_iter > 0 && max_diff < 1e-10 {
                    break;
                }
                prev_outputs.copy_from_slice(&outputs);
                alg_iter += 1;
                if alg_iter < ALG_FIXED_POINT_MAX {
                    continue;
                }
            }

            if native::terminate_requested() {
                println!("{}", i18n::msg("simulation_terminated", &[&format!("{:.4}", time) as &dyn std::fmt::Display]));
                return Ok(());
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
                eprintln!("{}", i18n::msg("event_loop_no_converge", &[&time]));
                break;
            }
        }
        
        // Optionally compute and print numeric (and symbolic) ODE Jacobian at the start of simulation.
        if numeric_ode_jacobian && (time - 0.0).abs() < epsilon {
            let n = states.len();
            if n > 0 {
                let mut system = System {
                    calc_derivs,
                    params: &params,
                    discrete: &mut discrete_vals,
                    outputs: &mut outputs,
                    when_states: &mut when_states,
                    crossings: &mut crossings,
                    pre_states: &pre_states,
                    pre_discrete: &pre_discrete_vals,
                    t_end,
                    diag_residual: diag_res_ptr,
                    diag_x: diag_x_ptr,
                    eval_call_index: std::ptr::null_mut(),
                    last_eval_time: std::ptr::null_mut(),
                    last_eval_state: std::ptr::null_mut(),
                    last_eval_state_len: 0,
                    scratch_outputs: None,
                };
                let mut jac = vec![0.0_f64; n * n];
                if let Err(code) = system.compute_ode_jacobian_numeric(time, &states, &mut jac, 1e-6) {
                    eprintln!(
                        "Warning: numeric ODE Jacobian computation failed at t={:.4} with status {}",
                        time, code
                    );
                } else {
                    println!("Numeric ODE Jacobian at t={:.4} (size {} x {}):", time, n, n);
                    for i in 0..n {
                        print!("  row {}:", i);
                        for j in 0..n {
                            let v = jac[i * n + j];
                            print!(" {:+.4}", v);
                        }
                        println!();
                    }

                    if let Some(jac_exprs) = symbolic_ode_jacobian {
                        // Evaluate symbolic Jacobian at current state and compare.
                        if jac_exprs.len() == n && jac_exprs.iter().all(|row| row.len() == n) {
                            let mut max_diff = 0.0_f64;
                            for i in 0..n {
                                for j in 0..n {
                                    let expr = &jac_exprs[i][j];
                                    let v_sym = eval_jac_expr_at_state(expr, state_var_index, &states);
                                    let v_num = jac[i * n + j];
                                    let diff = (v_sym - v_num).abs();
                                    if diff > max_diff {
                                        max_diff = diff;
                                    }
                                }
                            }
                            println!(
                                "Max difference between symbolic and numeric ODE Jacobian at t={:.4}: {:.6}",
                                time, max_diff
                            );
                        } else {
                            eprintln!(
                                "Warning: symbolic ODE Jacobian matrix size ({:?}x?) does not match state dimension {}",
                                jac_exprs.len(),
                                n
                            );
                        }
                    }
                }
            }
        }

        if time >= next_print - epsilon {
            let mut row = format!("{:.4}", time);
            for val in &states { row.push_str(&format!(", {:.4}", val)); }
            for val in &discrete_vals { row.push_str(&format!(", {:.4}", val)); }
            for val in &outputs { row.push_str(&format!(", {:.4}", val)); }
            write_row(&row)?;
            if let Some(ref mut c) = result_collector {
                c.push((time, states.clone(), discrete_vals.clone(), outputs.clone()));
            }
            next_print += print_interval;
        }

        // 2. Integration Step (Variable Step for Zero-Crossing)
        
        // Save current state
        let state_at_t = states.clone();
        let discrete_at_t = discrete_vals.clone();
        let crossings_at_t = crossings.clone();
        scratch_outputs_for_step.copy_from_slice(&outputs);

        // Trial Step
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
                t_end,
                diag_residual: diag_res_ptr,
                diag_x: diag_x_ptr,
                eval_call_index: eval_call_index_ptr,
                last_eval_time: last_eval_time_ptr,
                last_eval_state: last_eval_state_ptr,
                last_eval_state_len,
                scratch_outputs: Some(&mut scratch_outputs_for_step),
            };
            let step_res = if use_adaptive {
                let r = rk45_solver.step(&mut system, time, dt, &mut states);
                if r.is_ok() {
                    adaptive_step_count += 1;
                }
                r
            } else if use_implicit {
                backward_euler_solver.step(&mut system, time, dt, &mut states)
            } else {
                rk4_solver.step(&mut system, time, dt, &mut states)
            };
            if let Err(status) = step_res {
                eprintln!("{}", i18n::msg("simulation_failed_at", &[&format!("{:.4}", time) as &dyn std::fmt::Display, &status]));
                if status == 2 {
                    eprintln!("{}", i18n::msg0("newton_failure"));
                    let state_display = if newton_tearing_var_names.is_empty() {
                        format!("{:?}", states)
                    } else {
                        format!("{:?}", diag_state)
                    };
                    eprintln!(
                        "[step] calc_derivs call #{} at time={:.6}, state={}, diag_residual={:.6e}, diag_x={:.6e}",
                        diag_call_index, diag_time, state_display, diag_residual, diag_x
                    );
                    if !newton_tearing_var_names.is_empty() {
                        let names = newton_tearing_var_names.join(", ");
                        let res_fmt = format!("{:.6e}", diag_residual);
                        let val_fmt = format!("{:.6e}", diag_x);
                        eprintln!("{}", i18n::msg("tearing_vars_residual", &[&names as &dyn std::fmt::Display, &res_fmt as &dyn std::fmt::Display, &val_fmt as &dyn std::fmt::Display]));
                    }
                }
                return Err(format!("Solver step failed with status {}", status));
            }
        }
        let t_trial = time + dt;
        
        // Evaluate at trial point
        unsafe {
            let status = (calc_derivs)(
                t_trial,
                states.as_mut_ptr(),
                discrete_vals.as_mut_ptr(),
                derivs.as_mut_ptr(),
                params.as_ptr(),
                outputs.as_mut_ptr(),
                when_states.as_mut_ptr(),
                crossings.as_mut_ptr(),
                pre_states.as_ptr(),
                pre_discrete_vals.as_ptr(),
                t_end,
                diag_res_ptr,
                diag_x_ptr,
            );
            if status != 0 {
                let t_fmt = format!("{:.4}", t_trial);
                eprintln!("{}", i18n::msg("simulation_failed_trial", &[&t_fmt as &dyn std::fmt::Display, &status]));
                if status == 2 {
                    eprintln!("{}", i18n::msg0("newton_failure"));
                    eprintln!(
                        "[trial] time={:.6}, state={:?}, diag_residual={:.6e}, diag_x={:.6e}",
                        t_trial, states, diag_residual, diag_x
                    );
                    if !newton_tearing_var_names.is_empty() {
                        let names = newton_tearing_var_names.join(", ");
                        let res_fmt = format!("{:.6e}", diag_residual);
                        let val_fmt = format!("{:.6e}", diag_x);
                        eprintln!("{}", i18n::msg("tearing_vars_residual", &[&names as &dyn std::fmt::Display, &res_fmt as &dyn std::fmt::Display, &val_fmt as &dyn std::fmt::Display]));
                    }
                }
                return Err(format!("Simulation failed at t={:.4} (trial step) with status {}", t_trial, status));
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
                scratch_outputs_for_step.copy_from_slice(&outputs);

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
                        t_end,
                        diag_residual: diag_res_ptr,
                        diag_x: diag_x_ptr,
                        eval_call_index: eval_call_index_ptr,
                        last_eval_time: last_eval_time_ptr,
                        last_eval_state: last_eval_state_ptr,
                        last_eval_state_len,
                        scratch_outputs: Some(&mut scratch_outputs_for_step),
                    };
                    let step_res = if use_adaptive {
                        let r = rk45_solver.step(&mut system, time, dt_event, &mut states);
                        if r.is_ok() {
                            adaptive_step_count += 1;
                        }
                        r
                    } else if use_implicit {
                        backward_euler_solver.step(&mut system, time, dt_event, &mut states)
                    } else {
                        rk4_solver.step(&mut system, time, dt_event, &mut states)
                    };
                    if let Err(status) = step_res {
                        eprintln!("{}", i18n::msg("simulation_failed_at", &[&format!("{:.4}", time) as &dyn std::fmt::Display, &status]));
                        if status == 2 {
                            eprintln!("{}", i18n::msg0("newton_failure"));
                            let state_display = if newton_tearing_var_names.is_empty() {
                                format!("{:?}", states)
                            } else {
                                format!("{:?}", diag_state)
                            };
                            eprintln!(
                                "[step] calc_derivs call #{} at time={:.6}, state={}, diag_residual={:.6e}, diag_x={:.6e}",
                                diag_call_index, diag_time, state_display, diag_residual, diag_x
                            );
                            if !newton_tearing_var_names.is_empty() {
                                let names = newton_tearing_var_names.join(", ");
                                let res_fmt = format!("{:.6e}", diag_residual);
                                let val_fmt = format!("{:.6e}", diag_x);
                                eprintln!("{}", i18n::msg("tearing_vars_residual", &[&names as &dyn std::fmt::Display, &res_fmt as &dyn std::fmt::Display, &val_fmt as &dyn std::fmt::Display]));
                            }
                        }
                        return Err(format!("Solver step failed with status {}", status));
                    }
                }
                time += dt_event;
                
                // Don't print, next loop will handle event
        } else {
            // Accept full step
            time = t_trial;
        }
    }
    if use_adaptive {
        println!("{}", i18n::msg("adaptive_rk45_steps", &[&adaptive_step_count]));
    }
    Ok(())
}

/// Run simulation and return time series in memory (for IDE/Plotly). Does not write to file/stdout.
pub fn run_simulation_collect(
    calc_derivs: CalcDerivsFunc,
    when_count: usize,
    crossings_count: usize,
    states: Vec<f64>,
    discrete_vals: Vec<f64>,
    params: Vec<f64>,
    state_vars: &[String],
    discrete_vars: &[String],
    output_vars: &[String],
    state_var_index: &HashMap<String, usize>,
    t_end: f64,
    dt: f64,
    numeric_ode_jacobian: bool,
    symbolic_ode_jacobian: Option<&Vec<Vec<Expression>>>,
    newton_tearing_var_names: &[String],
    atol: f64,
    rtol: f64,
    solver: &str,
    output_interval: f64,
) -> Result<SimulationResult, String> {
    let mut collector = ResultCollector::new();
    run_simulation(
        calc_derivs,
        when_count,
        crossings_count,
        states,
        discrete_vals,
        params,
        state_vars,
        discrete_vars,
        output_vars,
        state_var_index,
        t_end,
        dt,
        numeric_ode_jacobian,
        symbolic_ode_jacobian,
        newton_tearing_var_names,
        atol,
        rtol,
        solver,
        output_interval,
        None,
        Some(&mut collector),
    )?;
    let mut time = Vec::with_capacity(collector.len());
    let mut series: HashMap<String, Vec<f64>> = HashMap::new();
    series.insert("time".to_string(), Vec::with_capacity(collector.len()));
    for name in state_vars {
        series.insert(name.clone(), Vec::with_capacity(collector.len()));
    }
    for name in discrete_vars {
        series.insert(name.clone(), Vec::with_capacity(collector.len()));
    }
    for name in output_vars {
        series.insert(name.clone(), Vec::with_capacity(collector.len()));
    }
    for (t, st, disc, out) in collector {
        time.push(t);
        series.get_mut("time").unwrap().push(t);
        for (i, name) in state_vars.iter().enumerate() {
            let v = st.get(i).copied().unwrap_or(0.0);
            series.get_mut(name).unwrap().push(v);
        }
        for (i, name) in discrete_vars.iter().enumerate() {
            let v = disc.get(i).copied().unwrap_or(0.0);
            series.get_mut(name).unwrap().push(v);
        }
        for (i, name) in output_vars.iter().enumerate() {
            let v = out.get(i).copied().unwrap_or(0.0);
            series.get_mut(name).unwrap().push(v);
        }
    }
    Ok(SimulationResult { time, series })
}

fn eval_jac_expr_at_state(expr: &Expression, state_var_index: &HashMap<String, usize>, states: &[f64]) -> f64 {
    match expr {
        Expression::Number(n) => *n,
        Expression::Variable(name) => {
            if let Some(&idx) = state_var_index.get(name) {
                if idx < states.len() {
                    return states[idx];
                }
            }
            0.0
        }
        Expression::BinaryOp(lhs, op, rhs) => {
            let l = eval_jac_expr_at_state(lhs, state_var_index, states);
            let r = eval_jac_expr_at_state(rhs, state_var_index, states);
            use crate::ast::Operator;
            match op {
                Operator::Add => l + r,
                Operator::Sub => l - r,
                Operator::Mul => l * r,
                Operator::Div => l / r,
                _ => 0.0,
            }
        }
        Expression::If(c, t, f) => {
            let cv = eval_jac_expr_at_state(c, state_var_index, states);
            if cv != 0.0 {
                eval_jac_expr_at_state(t, state_var_index, states)
            } else {
                eval_jac_expr_at_state(f, state_var_index, states)
            }
        }
        Expression::ArrayLiteral(items) => {
            if let Some(first) = items.first() {
                eval_jac_expr_at_state(first, state_var_index, states)
            } else {
                0.0
            }
        }
        Expression::Der(inner) => eval_jac_expr_at_state(inner, state_var_index, states),
        Expression::ArrayAccess(base, _idx) => eval_jac_expr_at_state(base, state_var_index, states),
        Expression::Dot(base, _member) => eval_jac_expr_at_state(base, state_var_index, states),
        Expression::Range(_, _, _) => 0.0,
        Expression::Call(_, _) => 0.0,
        Expression::Sample(inner) => eval_jac_expr_at_state(inner, state_var_index, states),
        Expression::Interval(inner) => eval_jac_expr_at_state(inner, state_var_index, states),
        Expression::Hold(inner) => eval_jac_expr_at_state(inner, state_var_index, states),
        Expression::Previous(inner) => eval_jac_expr_at_state(inner, state_var_index, states),
        Expression::SubSample(c, _) | Expression::SuperSample(c, _) | Expression::ShiftSample(c, _) => {
            eval_jac_expr_at_state(c, state_var_index, states)
        }
        Expression::StringLiteral(_) => 0.0,
    }
}
