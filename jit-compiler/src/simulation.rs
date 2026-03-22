// RT1-1: DAE/ODE solver with events. Adaptive RK45 when no when/zero-crossing; fixed-step with
// event detection and reinit when when/zero-crossing present. Event iteration at each time step.
use crate::ast::Expression;
use crate::i18n;
use crate::jit::{native, CalcDerivsFunc};
use crate::solver::{AdaptiveRK45Solver, BackwardEulerSolver, RungeKutta4Solver, Solver, System};
use std::collections::HashMap;
use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io::{self, Write};

mod events;
mod jacobian;
mod newton_recovery;
mod sim_io;
mod step;
mod types;

pub use self::types::SimulationResult;
pub use self::types::run_simulation_collect;
use self::events::{run_event_iteration_at_time, EventIterationOutcome};
use self::newton_recovery::{allow_zero_residual_newton, fail_if_assert_storm, print_newton_diag};
use self::sim_io::{flush_writer, write_csv_line};
use self::step::maybe_print_numeric_jacobian;
use self::types::ResultCollector;

const CSV_ROWS_PER_FLUSH: u32 = 64;

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
    output_start_vals: &[f64],
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
    let mut outputs = if output_start_vals.len() == output_vars.len() {
        output_start_vals.to_vec()
    } else {
        vec![0.0; output_vars.len()]
    };
    let mut when_states = vec![0.0; when_count * 2];
    let mut crossings = vec![0.0; crossings_count];
    let mut pre_states = vec![0.0; states.len()];
    let mut pre_discrete_vals = vec![0.0; discrete_vals.len()];
    let mut homotopy_lambda: f64 = 1.0;
    let homotopy_lambda_ptr: *const f64 = &homotopy_lambda;

    // RT1-3: Use adaptive RK45 only when solver is rk45 and no when/zero-crossing.
    let use_adaptive = solver == "rk45" && when_count == 0 && crossings_count == 0;
    let use_implicit = solver == "implicit";
    let mut rk4_solver = RungeKutta4Solver::new(states.len());
    let mut rk45_solver = AdaptiveRK45Solver::new(states.len(), atol, rtol);
    let mut backward_euler_solver = BackwardEulerSolver::new(states.len());

    let mut out: Box<dyn Write> = if result_collector.is_some() {
        Box::new(io::sink())
    } else if let Some(path) = result_file {
        let f = File::create(path)
            .map_err(|e| format!("Failed to create result file {}: {}", path, e))?;
        Box::new(std::io::BufWriter::new(f))
    } else {
        Box::new(std::io::BufWriter::new(io::stdout()))
    };

    let w = &mut out;

    let est_cols = 1 + state_vars.len() + discrete_vars.len() + output_vars.len();
    let mut header = String::with_capacity(est_cols * 16);
    header.push_str(i18n::msg0("time"));
    for var in state_vars {
        write!(&mut header, ", {}", var).map_err(|e| e.to_string())?;
    }
    for var in discrete_vars {
        write!(&mut header, ", {}", var).map_err(|e| e.to_string())?;
    }
    for var in output_vars {
        write!(&mut header, ", {}", var).map_err(|e| e.to_string())?;
    }
    write_csv_line(w, &header)?;
    flush_writer(w)?;

    let mut csv_row = String::with_capacity(est_cols * 24);
    let mut rows_since_flush: u32 = 0;

    let print_interval = output_interval;
    let mut next_print = 0.0;
    let epsilon = 1e-5;
    let mut adaptive_step_count: u64 = 0;
    let mut diag_state = vec![0.0_f64; states.len()];
    let mut scratch_outputs_for_step = vec![0.0_f64; output_vars.len()];
    let mut prev_outputs = vec![0.0; output_vars.len()];
    let mut save_states = vec![0.0_f64; states.len()];
    let mut save_discrete = vec![0.0_f64; discrete_vals.len()];
    let mut save_crossings = vec![0.0_f64; crossings_count];
    let mut trial_discrete = vec![0.0_f64; discrete_vals.len()];
    let mut trial_when_states = vec![0.0_f64; when_count * 2];

    native::reset_terminate_flag();
    native::reset_assert_counter();

    while time <= t_end + epsilon {
        native::reset_assert_counter();
        // 1. Event Iteration Loop (Handle events at current time)
        // Capture pre-states (left limit) before event iteration
        pre_states.copy_from_slice(&states);
        pre_discrete_vals.copy_from_slice(&discrete_vals);

        let (mut diag_residual, mut diag_x) = (0.0_f64, 0.0_f64);
        let (diag_res_ptr, diag_x_ptr) = if newton_tearing_var_names.is_empty() {
            (std::ptr::null_mut(), std::ptr::null_mut())
        } else {
            (&mut diag_residual as *mut f64, &mut diag_x as *mut f64)
        };
        let mut diag_call_index = 0u32;
        let mut diag_time = 0.0_f64;
        diag_state.fill(0.0);
        let (eval_call_index_ptr, last_eval_time_ptr, last_eval_state_ptr, last_eval_state_len) =
            if newton_tearing_var_names.is_empty() {
                (
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    0,
                )
            } else {
                (
                    &mut diag_call_index as *mut u32,
                    &mut diag_time as *mut f64,
                    diag_state.as_mut_ptr(),
                    diag_state.len(),
                )
            };

        match run_event_iteration_at_time(
            time,
            t_end,
            calc_derivs,
            when_count,
            &mut states,
            &mut discrete_vals,
            &mut derivs,
            &params,
            &mut outputs,
            &mut when_states,
            &mut crossings,
            &pre_states,
            &pre_discrete_vals,
            &mut homotopy_lambda,
            homotopy_lambda_ptr,
            newton_tearing_var_names,
            output_start_vals,
            output_vars,
            &mut diag_residual,
            &mut diag_x,
            diag_res_ptr,
            diag_x_ptr,
            &mut diag_call_index,
            &mut diag_time,
            &mut prev_outputs,
            &mut **w,
        )? {
            EventIterationOutcome::TerminatedOk => return Ok(()),
            EventIterationOutcome::Completed => {}
        }

        maybe_print_numeric_jacobian(
            numeric_ode_jacobian,
            time,
            epsilon,
            &states,
            calc_derivs,
            &params,
            &mut discrete_vals,
            &mut outputs,
            &mut when_states,
            &mut crossings,
            &pre_states,
            &pre_discrete_vals,
            t_end,
            symbolic_ode_jacobian,
            state_var_index,
            homotopy_lambda_ptr,
        );

        if time >= next_print - epsilon {
            csv_row.clear();
            write!(&mut csv_row, "{:.4}", time).map_err(|e| e.to_string())?;
            for val in &states {
                write!(&mut csv_row, ", {:.4}", val).map_err(|e| e.to_string())?;
            }
            for val in &discrete_vals {
                write!(&mut csv_row, ", {:.4}", val).map_err(|e| e.to_string())?;
            }
            for val in &outputs {
                write!(&mut csv_row, ", {:.4}", val).map_err(|e| e.to_string())?;
            }
            write_csv_line(w, &csv_row)?;
            rows_since_flush = rows_since_flush.saturating_add(1);
            if rows_since_flush >= CSV_ROWS_PER_FLUSH {
                flush_writer(w)?;
                rows_since_flush = 0;
            }
            if let Some(ref mut c) = result_collector {
                c.push((time, states.clone(), discrete_vals.clone(), outputs.clone()));
            }
            next_print += print_interval;
        }

        // 2. Integration Step (Variable Step for Zero-Crossing)
        if states.is_empty() {
            time += dt;
            continue;
        }

        save_states.copy_from_slice(&states);
        save_discrete.copy_from_slice(&discrete_vals);
        save_crossings.copy_from_slice(&crossings);
        scratch_outputs_for_step.copy_from_slice(&outputs);

        // Trial Step -- suppress assertions during intermediate solver evaluations;
        // assertions are only semantically valid at accepted time points (event iteration).
        native::suppress_assert_begin();
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
                homotopy_lambda_ptr,
                buf_discrete: Vec::new(),
                buf_when: Vec::new(),
                buf_crossings: Vec::new(),
                buf_outputs: Vec::new(),
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
                eprintln!(
                    "{}",
                    i18n::msg(
                        "simulation_failed_at",
                        &[&format!("{:.4}", time) as &dyn std::fmt::Display, &status]
                    )
                );
                if status == 2 {
                    eprintln!("{}", i18n::msg0("newton_failure"));
                    print_newton_diag("solver-step", diag_call_index, diag_time, diag_residual, diag_x);
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
                        eprintln!(
                            "{}",
                            i18n::msg(
                                "tearing_vars_residual",
                                &[
                                    &names as &dyn std::fmt::Display,
                                    &res_fmt as &dyn std::fmt::Display,
                                    &val_fmt as &dyn std::fmt::Display
                                ]
                            )
                        );
                    }
                }
                let _ = flush_writer(w);
                native::suppress_assert_end();
                return Err(format!("Solver step failed with status {}", status));
            }
        }
        let t_trial = time + dt;

        trial_discrete.copy_from_slice(&discrete_vals);
        trial_when_states.copy_from_slice(&when_states);
        unsafe {
            let status = (calc_derivs)(
                t_trial,
                states.as_mut_ptr(),
                trial_discrete.as_mut_ptr(),
                derivs.as_mut_ptr(),
                params.as_ptr(),
                outputs.as_mut_ptr(),
                trial_when_states.as_mut_ptr(),
                crossings.as_mut_ptr(),
                pre_states.as_ptr(),
                pre_discrete_vals.as_ptr(),
                t_end,
                diag_res_ptr,
                diag_x_ptr,
                homotopy_lambda_ptr,
            );
            if status != 0 {
                if allow_zero_residual_newton(status, diag_residual) {
                    native::suppress_assert_end();
                    continue;
                }
                let t_fmt = format!("{:.4}", t_trial);
                eprintln!(
                    "{}",
                    i18n::msg(
                        "simulation_failed_trial",
                        &[&t_fmt as &dyn std::fmt::Display, &status]
                    )
                );
                if status == 2 {
                    eprintln!("{}", i18n::msg0("newton_failure"));
                    print_newton_diag("trial-eval", diag_call_index, diag_time, diag_residual, diag_x);
                    eprintln!(
                        "[trial] time={:.6}, state={:?}, diag_residual={:.6e}, diag_x={:.6e}",
                        t_trial, states, diag_residual, diag_x
                    );
                    if !newton_tearing_var_names.is_empty() {
                        let names = newton_tearing_var_names.join(", ");
                        let res_fmt = format!("{:.6e}", diag_residual);
                        let val_fmt = format!("{:.6e}", diag_x);
                        eprintln!(
                            "{}",
                            i18n::msg(
                                "tearing_vars_residual",
                                &[
                                    &names as &dyn std::fmt::Display,
                                    &res_fmt as &dyn std::fmt::Display,
                                    &val_fmt as &dyn std::fmt::Display
                                ]
                            )
                        );
                    }
                }
                let _ = flush_writer(w);
                native::suppress_assert_end();
                return Err(format!(
                    "Simulation failed at t={:.4} (trial step) with status {}",
                    t_trial, status
                ));
            }
        }
        native::suppress_assert_end();

        // Check for Zero-Crossings
        let mut min_alpha = 1.0;
        let mut event_found = false;

        for i in 0..crossings_count {
            let c_prev = save_crossings[i];
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

            states.copy_from_slice(&save_states);
            discrete_vals.copy_from_slice(&save_discrete);
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
                    homotopy_lambda_ptr,
                    buf_discrete: Vec::new(),
                    buf_when: Vec::new(),
                    buf_crossings: Vec::new(),
                    buf_outputs: Vec::new(),
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
                    eprintln!(
                        "{}",
                        i18n::msg(
                            "simulation_failed_at",
                            &[&format!("{:.4}", time) as &dyn std::fmt::Display, &status]
                        )
                    );
                    if status == 2 {
                        eprintln!("{}", i18n::msg0("newton_failure"));
                        print_newton_diag(
                            "event-step",
                            diag_call_index,
                            diag_time,
                            diag_residual,
                            diag_x,
                        );
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
                            eprintln!(
                                "{}",
                                i18n::msg(
                                    "tearing_vars_residual",
                                    &[
                                        &names as &dyn std::fmt::Display,
                                        &res_fmt as &dyn std::fmt::Display,
                                        &val_fmt as &dyn std::fmt::Display
                                    ]
                                )
                            );
                        }
                    }
                    let _ = flush_writer(w);
                    return Err(format!("Solver step failed with status {}", status));
                }
                fail_if_assert_storm("event-step", time)?;
            }
            time += dt_event;

            // Don't print, next loop will handle event
        } else {
            // Accept full step
            time = t_trial;
        }
    }
    if use_adaptive {
        println!(
            "{}",
            i18n::msg("adaptive_rk45_steps", &[&adaptive_step_count])
        );
    }
    flush_writer(w)?;
    Ok(())
}

