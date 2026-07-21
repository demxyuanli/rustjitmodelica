//! Zero-crossing localize + event-step retry after trial-eval.

use crate::i18n;
use crate::jit::CalcDerivsFunc;
use crate::solver::{AdaptiveRK45Solver, BackwardEulerSolver, RungeKutta4Solver, Solver, System};
use std::io::Write;

use super::super::newton_recovery::{
    fail_if_assert_storm, is_ideal_switch_complementarity_name, prepare_event_step_algebraic_guess,
    print_newton_diag, recover_newton_ideal_switch, warm_start_ideal_switch_outputs,
};
use super::super::sim_io::flush_writer;
use super::super::types::{EventQueue, QueuedEvent, QueuedEventKind};

/// Check zero-crossings, localize event step, and accept or reject the trial step.
///
/// On success, updates `time` and `switch_discontinuity_nudges` (verbatim driver logic).
#[allow(clippy::too_many_arguments)]
pub(super) fn handle_zero_crossings(
    crossings_count: usize,
    save_crossings: &[f64],
    crossings: &mut [f64],
    profile_collector: &mut Option<crate::condenser::profile_data::ProfileCollector>,
    t_trial: f64,
    event_queue: &mut EventQueue,
    dt_step: f64,
    dt: f64,
    time: &mut f64,
    switch_discontinuity_nudges: &mut u32,
    states: &mut [f64],
    save_states: &[f64],
    discrete_vals: &mut [f64],
    save_discrete: &[f64],
    outputs: &mut [f64],
    save_outputs: &[f64],
    scratch_outputs_for_step: &mut [f64],
    output_vars: &[String],
    active_calc_derivs: CalcDerivsFunc,
    params: &[f64],
    when_states: &mut [f64],
    pre_states: &[f64],
    pre_discrete_vals: &[f64],
    t_end: f64,
    diag_res_ptr: *mut f64,
    diag_x_ptr: *mut f64,
    eval_call_index_ptr: *mut u32,
    last_eval_time_ptr: *mut f64,
    last_eval_state_ptr: *mut f64,
    last_eval_state_len: usize,
    use_scratch_outputs_for_solver: bool,
    homotopy_lambda: &mut f64,
    homotopy_lambda_ptr: *const f64,
    step_buf_discrete: &mut Vec<f64>,
    step_buf_when: &mut Vec<f64>,
    step_buf_crossings: &mut Vec<f64>,
    step_buf_outputs: &mut Vec<f64>,
    step_buf_guess: &mut Vec<f64>,
    hotspot_threshold: u64,
    stack_scratch_enabled: bool,
    use_adaptive: bool,
    prefer_implicit_near_switch: bool,
    use_implicit: bool,
    use_radau: bool,
    use_qss: bool,
    rk45_solver: &mut AdaptiveRK45Solver,
    backward_euler_solver: &mut BackwardEulerSolver,
    radau_solver: &mut crate::solver::radau::RadauSolver,
    qss_solver: &mut crate::solver::qss::QssSolver,
    rk4_solver: &mut RungeKutta4Solver,
    adaptive_step_count: &mut u64,
    diag_residual: &mut f64,
    diag_x: &mut f64,
    diag_call_index: u32,
    diag_time: f64,
    diag_state: &[f64],
    newton_tearing_var_names: &[String],
    derivs: &mut [f64],
    w: &mut dyn Write,
) -> Result<(), String> {
    // Check for Zero-Crossings
    let mut min_alpha = 1.0;
    let mut event_found = false;

    for i in 0..crossings_count {
        let c_prev = save_crossings[i];
        let c_curr = crossings[i];

        if c_prev * c_curr < 0.0 {
            event_found = true;
            if let Some(ref mut collector) = profile_collector {
                collector.record_zero_crossing(i, t_trial);
            }
            event_queue.push_unique(QueuedEvent {
                time: t_trial,
                kind: QueuedEventKind::ZeroCrossing(i),
            });
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
        let mut dt_event = dt_step * min_alpha;
        // Infinitesimal localize loops forever at PWM edges (time += 0). Floor the
        // event step so we always make progress past the crossing.
        let dt_event_floor = (dt * 1e-4).clamp(1e-12, dt * 0.01);
        if dt_event < dt_event_floor {
            eprintln!(
                "[newton-switch-recover] event-step dt floor at t={:.6}: {:.3e} -> {:.3e}",
                time, dt_event, dt_event_floor
            );
            dt_event = dt_event_floor;
        }

        states.copy_from_slice(save_states);
        discrete_vals.copy_from_slice(save_discrete);
        // Trial-eval at t_trial writes post-crossing algebraics into `outputs` while we
        // rewind continuous/discrete state; restore the pre-step guess and clamp switch s.
        prepare_event_step_algebraic_guess(
            output_vars,
            save_outputs,
            outputs,
            scratch_outputs_for_step,
        );

        {
            let mut system = System {
                calc_derivs: active_calc_derivs,
                params,
                discrete: discrete_vals,
                outputs,
                when_states,
                crossings,
                pre_states,
                pre_discrete: pre_discrete_vals,
                t_end,
                diag_residual: diag_res_ptr,
                diag_x: diag_x_ptr,
                eval_call_index: eval_call_index_ptr,
                last_eval_time: last_eval_time_ptr,
                last_eval_state: last_eval_state_ptr,
                last_eval_state_len,
                scratch_outputs: if use_scratch_outputs_for_solver {
                    Some(scratch_outputs_for_step)
                } else {
                    None
                },
                homotopy_lambda_ptr,
                buf_discrete: step_buf_discrete,
                buf_when: step_buf_when,
                buf_crossings: step_buf_crossings,
                buf_outputs: step_buf_outputs,
                buf_guess: step_buf_guess,
                eval_count: 0,
                hotspot_threshold,
                simd_step_hits: 0,
                simd_step_fallbacks: 0,
                stack_scratch_enabled,
            };
            let use_be_event = use_implicit || prefer_implicit_near_switch;
            let step_res = if use_adaptive {
                let r = rk45_solver.step(&mut system, *time, dt_event, states);
                if r.is_ok() {
                    *adaptive_step_count += 1;
                }
                r
            } else if use_be_event {
                backward_euler_solver.step(&mut system, *time, dt_event, states)
            } else if use_radau {
                radau_solver.step(&mut system, *time, dt_event, states)
            } else if use_qss {
                qss_solver.step(&mut system, *time, dt_event, states)
            } else {
                rk4_solver.step(&mut system, *time, dt_event, states)
            };
            let step_res = if let Err(status) = step_res {
                if status == 2 {
                    drop(system);
                    states.copy_from_slice(save_states);
                    discrete_vals.copy_from_slice(save_discrete);
                    prepare_event_step_algebraic_guess(
                        output_vars,
                        save_outputs,
                        outputs,
                        scratch_outputs_for_step,
                    );
                    let recovered = recover_newton_ideal_switch(
                        active_calc_derivs,
                        *time,
                        t_end,
                        params,
                        pre_states,
                        pre_discrete_vals,
                        output_vars,
                        states,
                        discrete_vals,
                        derivs,
                        outputs,
                        when_states,
                        crossings,
                        diag_res_ptr,
                        diag_x_ptr,
                        diag_residual,
                        homotopy_lambda,
                        homotopy_lambda_ptr,
                    );
                    states.copy_from_slice(save_states);
                    discrete_vals.copy_from_slice(save_discrete);
                    let _ = warm_start_ideal_switch_outputs(output_vars, outputs);
                    scratch_outputs_for_step.copy_from_slice(outputs);
                    if recovered
                        || output_vars
                            .iter()
                            .any(|n| is_ideal_switch_complementarity_name(n))
                    {
                        let mut system = System {
                            calc_derivs: active_calc_derivs,
                            params,
                            discrete: discrete_vals,
                            outputs,
                            when_states,
                            crossings,
                            pre_states,
                            pre_discrete: pre_discrete_vals,
                            t_end,
                            diag_residual: diag_res_ptr,
                            diag_x: diag_x_ptr,
                            eval_call_index: eval_call_index_ptr,
                            last_eval_time: last_eval_time_ptr,
                            last_eval_state: last_eval_state_ptr,
                            last_eval_state_len,
                            scratch_outputs: if use_scratch_outputs_for_solver {
                                Some(scratch_outputs_for_step)
                            } else {
                                None
                            },
                            homotopy_lambda_ptr,
                            buf_discrete: step_buf_discrete,
                            buf_when: step_buf_when,
                            buf_crossings: step_buf_crossings,
                            buf_outputs: step_buf_outputs,
                            buf_guess: step_buf_guess,
                            eval_count: 0,
                            hotspot_threshold,
                            simd_step_hits: 0,
                            simd_step_fallbacks: 0,
                            stack_scratch_enabled,
                        };
                        let retry = if use_adaptive {
                            rk45_solver.step(&mut system, *time, dt_event, states)
                        } else if use_be_event {
                            backward_euler_solver.step(&mut system, *time, dt_event, states)
                        } else if use_radau {
                            radau_solver.step(&mut system, *time, dt_event, states)
                        } else if use_qss {
                            qss_solver.step(&mut system, *time, dt_event, states)
                        } else {
                            rk4_solver.step(&mut system, *time, dt_event, states)
                        };
                        if retry.is_ok() {
                            eprintln!(
                                "[newton-switch-recover] event-step retry succeeded at t={:.6}",
                                time
                            );
                        }
                        retry
                    } else {
                        Err(status)
                    }
                } else {
                    Err(status)
                }
            } else {
                Ok(())
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
                        *diag_residual,
                        *diag_x,
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
                        let res_fmt = format!("{:.6e}", *diag_residual);
                        let val_fmt = format!("{:.6e}", *diag_x);
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
            fail_if_assert_storm("event-step", *time)?;
        }
        *time += dt_event;
        *switch_discontinuity_nudges = 0;

        // Don't print, next loop will handle event
    } else {
        // Accept full step
        *time = t_trial;
        *switch_discontinuity_nudges = 0;
    }
    Ok(())
}
