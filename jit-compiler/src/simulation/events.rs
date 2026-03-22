//! Fixed-point event iteration at the current simulation time (when clauses, algebraic refinement).

use crate::i18n;
use crate::jit::native;
use crate::jit::CalcDerivsFunc;

use super::newton_recovery::{
    allow_algebraic_newton_fallback, allow_zero_residual_newton, fail_if_assert_storm,
    print_newton_diag, recover_newton_at_t0,
};
use super::sim_io::flush_writer;

pub(crate) enum EventIterationOutcome {
    Completed,
    TerminatedOk,
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_event_iteration_at_time(
    time: f64,
    t_end: f64,
    calc_derivs: CalcDerivsFunc,
    when_count: usize,
    states: &mut [f64],
    discrete_vals: &mut [f64],
    derivs: &mut [f64],
    params: &[f64],
    outputs: &mut [f64],
    when_states: &mut [f64],
    crossings: &mut [f64],
    pre_states: &[f64],
    pre_discrete_vals: &[f64],
    homotopy_lambda: &mut f64,
    homotopy_lambda_ptr: *const f64,
    newton_tearing_var_names: &[String],
    output_start_vals: &[f64],
    output_vars: &[String],
    diag_residual: &mut f64,
    diag_x: &mut f64,
    diag_res_ptr: *mut f64,
    diag_x_ptr: *mut f64,
    diag_call_index: &mut u32,
    diag_time: &mut f64,
    prev_outputs: &mut [f64],
    w: &mut dyn std::io::Write,
) -> Result<EventIterationOutcome, String> {
    let mut event_iter_count = 0;
    const ALG_FIXED_POINT_MAX: u32 = 15;
    let do_alg_iter =
        states.is_empty() && !output_vars.is_empty() && !newton_tearing_var_names.is_empty();
    let mut alg_iter = 0u32;
    prev_outputs.fill(0.0);

    loop {
        unsafe {
            native::suppress_assert_begin();
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
                homotopy_lambda_ptr,
            );
            native::suppress_assert_end();
            if status != 0 {
                if allow_zero_residual_newton(status, *diag_residual) {
                    break;
                }
                if time == 0.0 && status == 2 && event_iter_count == 0 {
                    let recovered = recover_newton_at_t0(
                        calc_derivs,
                        time,
                        t_end,
                        params,
                        pre_states,
                        pre_discrete_vals,
                        output_start_vals,
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
                    if recovered || !states.is_empty() {
                        if !recovered && !states.is_empty() {
                            eprintln!(
                                "[newton-init] accepting t=0 Newton non-convergence (residual={:.6e}), continuing with current values",
                                *diag_residual
                            );
                        }
                        break;
                    }
                }
                if allow_algebraic_newton_fallback(status, states.len()) {
                    eprintln!(
                        "[newton-diag] phase=event-iteration-fallback eval_calls={} last_eval_time={:.6} diag_residual={:.6e} diag_x={:.6e} (algebraic Newton fallback accepted)",
                        *diag_call_index, *diag_time, *diag_residual, *diag_x,
                    );
                    break;
                }
                let t_fmt = format!("{:.4}", time);
                eprintln!(
                    "{}",
                    i18n::msg(
                        "simulation_failed_at",
                        &[&t_fmt as &dyn std::fmt::Display, &status]
                    )
                );
                if status == 2 {
                    eprintln!("{}", i18n::msg0("newton_failure"));
                    print_newton_diag(
                        "event-iteration",
                        *diag_call_index,
                        *diag_time,
                        *diag_residual,
                        *diag_x,
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
                return Err(format!(
                    "Simulation failed at t={:.4} with status {}",
                    time, status
                ));
            }
        }
        fail_if_assert_storm("event-iteration", time)?;

        if do_alg_iter && alg_iter < ALG_FIXED_POINT_MAX {
            let max_diff = if alg_iter == 0 {
                1.0
            } else {
                let mut m = 0.0_f64;
                for i in 0..outputs.len() {
                    let d = (prev_outputs[i] - outputs[i]).abs();
                    if d > m {
                        m = d;
                    }
                }
                m
            };
            if alg_iter > 0 && max_diff < 1e-10 {
                break;
            }
            prev_outputs.copy_from_slice(outputs);
            alg_iter += 1;
            if alg_iter < ALG_FIXED_POINT_MAX {
                continue;
            }
        }

        if native::terminate_requested() {
            println!(
                "{}",
                i18n::msg(
                    "simulation_terminated",
                    &[&format!("{:.4}", time) as &dyn std::fmt::Display]
                )
            );
            flush_writer(w)?;
            return Ok(EventIterationOutcome::TerminatedOk);
        }

        let mut converged = true;
        if when_count > 0 {
            for i in 0..when_count {
                let idx_pre = i * 2;
                let idx_new = i * 2 + 1;
                let pre_val = when_states[idx_pre];
                let new_val = when_states[idx_new];

                if pre_val != new_val {
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

    Ok(EventIterationOutcome::Completed)
}
