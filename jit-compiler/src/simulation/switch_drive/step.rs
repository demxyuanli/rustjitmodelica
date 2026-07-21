use crate::compiler::ClockPartitionScheduleEntry;
use crate::i18n;
use crate::jit::{native, CalcDerivsFunc};
use crate::solver::{BackwardEulerSolver, RungeKutta4Solver, Solver, System};
use std::io::Write;

use super::super::events::{run_event_iteration_at_time, EventIterationOutcome};
use super::super::newton_recovery::{
    is_ideal_switch_complementarity_name, print_newton_diag, recover_newton_ideal_switch,
    warm_start_ideal_switch_outputs,
};
use super::super::sim_io::flush_writer;
use super::super::types::EventQueue;
use super::{SwitchStepRecoverResult, SWITCH_DISCONTINUITY_NUDGE_MAX};

#[allow(clippy::too_many_arguments)]
fn try_switch_step(
    dt_try: f64,
    use_be: bool,
    time: f64,
    t_end: f64,
    active_calc_derivs: CalcDerivsFunc,
    params: &[f64],
    pre_states: &[f64],
    pre_discrete_vals: &[f64],
    output_vars: &[String],
    save_st: &[f64],
    save_disc: &[f64],
    save_out: &[f64],
    states: &mut [f64],
    discrete_vals: &mut [f64],
    outputs: &mut [f64],
    when_states: &mut [f64],
    crossings: &mut [f64],
    scratch: &mut [f64],
    be: &mut BackwardEulerSolver,
    rk4: &mut RungeKutta4Solver,
    step_buf_discrete: &mut Vec<f64>,
    step_buf_when: &mut Vec<f64>,
    step_buf_crossings: &mut Vec<f64>,
    step_buf_outputs: &mut Vec<f64>,
    step_buf_guess: &mut Vec<f64>,
    use_scratch_outputs_for_solver: bool,
    diag_res_ptr: *mut f64,
    diag_x_ptr: *mut f64,
    eval_call_index_ptr: *mut u32,
    last_eval_time_ptr: *mut f64,
    last_eval_state_ptr: *mut f64,
    last_eval_state_len: usize,
    homotopy_lambda_ptr: *const f64,
    hotspot_threshold: u64,
    stack_scratch_enabled: bool,
) -> bool {
    if dt_try < 1e-15 {
        return false;
    }
    states.copy_from_slice(save_st);
    discrete_vals.copy_from_slice(save_disc);
    outputs.copy_from_slice(save_out);
    let _ = warm_start_ideal_switch_outputs(output_vars, outputs);
    for (i, name) in output_vars.iter().enumerate() {
        if i < outputs.len() && is_ideal_switch_complementarity_name(name) {
            outputs[i] = 0.0;
        }
    }
    scratch.copy_from_slice(outputs);
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
            Some(scratch)
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
    let r = if use_be {
        be.step(&mut system, time, dt_try, states)
    } else {
        rk4.step(&mut system, time, dt_try, states)
    };
    r.is_ok()
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn recover_failed_solver_step(
    status: i32,
    time: f64,
    dt: f64,
    mut dt_step: f64,
    mut dt_active: f64,
    mut switch_recover_streak: u32,
    mut prefer_implicit_near_switch: bool,
    mut switch_discontinuity_nudges: u32,
    model_has_ideal_switch: bool,
    active_calc_derivs: CalcDerivsFunc,
    when_count: usize,
    t_end: f64,
    params: &[f64],
    pre_states: &[f64],
    pre_discrete_vals: &[f64],
    output_vars: &[String],
    output_start_vals: &[f64],
    newton_tearing_var_names: &[String],
    states: &mut [f64],
    discrete_vals: &mut [f64],
    derivs: &mut [f64],
    outputs: &mut [f64],
    when_states: &mut [f64],
    crossings: &mut [f64],
    save_states: &mut [f64],
    save_discrete: &mut [f64],
    save_outputs: &mut [f64],
    scratch_outputs_for_step: &mut [f64],
    diag_residual: &mut f64,
    diag_x: &mut f64,
    diag_res_ptr: *mut f64,
    diag_x_ptr: *mut f64,
    diag_call_index: &mut u32,
    diag_time: &mut f64,
    diag_state: &[f64],
    homotopy_lambda: &mut f64,
    homotopy_lambda_ptr: *const f64,
    backward_euler_solver: &mut BackwardEulerSolver,
    rk4_solver: &mut RungeKutta4Solver,
    step_buf_discrete: &mut Vec<f64>,
    step_buf_when: &mut Vec<f64>,
    step_buf_crossings: &mut Vec<f64>,
    step_buf_outputs: &mut Vec<f64>,
    step_buf_guess: &mut Vec<f64>,
    use_scratch_outputs_for_solver: bool,
    hotspot_threshold: u64,
    stack_scratch_enabled: bool,
    eval_call_index_ptr: *mut u32,
    last_eval_time_ptr: *mut f64,
    last_eval_state_ptr: *mut f64,
    last_eval_state_len: usize,
    clock_partition_schedule: &[ClockPartitionScheduleEntry],
    w: &mut dyn Write,
    event_queue: &mut EventQueue,
    prev_outputs: &mut [f64],
) -> SwitchStepRecoverResult {
    let has_switch = output_vars
        .iter()
        .any(|n| is_ideal_switch_complementarity_name(n));
    let mut step_ok_after_recover = false;
    if status == 2 && has_switch {
        let _ = recover_newton_ideal_switch(
            active_calc_derivs,
            time,
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

        // Prefer Backward-Euler micro-steps near ideal switches (RK4 stages
        // straddle PWM edges). Also bisect dt to localize the event.

        // Discrete scales: Backward-Euler first, then RK4.
        let scales: [f64; 8] = [1.0, 0.5, 0.25, 0.1, 0.01, 0.001, 1e-4, 1e-5];
        'scale_loop: for &use_be in &[true, false] {
            for &scale in &scales {
                let dt_try = dt * scale;
                if try_switch_step(
                    dt_try,
                    use_be,
                    time,
                    t_end,
                    active_calc_derivs,
                    params,
                    pre_states,
                    pre_discrete_vals,
                    output_vars,
                    save_states,
                    save_discrete,
                    save_outputs,
                    states,
                    discrete_vals,
                    outputs,
                    when_states,
                    crossings,
                    scratch_outputs_for_step,
                    backward_euler_solver,
                    rk4_solver,
                    step_buf_discrete,
                    step_buf_when,
                    step_buf_crossings,
                    step_buf_outputs,
                    step_buf_guess,
                    use_scratch_outputs_for_solver,
                    diag_res_ptr,
                    diag_x_ptr,
                    eval_call_index_ptr,
                    last_eval_time_ptr,
                    last_eval_state_ptr,
                    last_eval_state_len,
                    homotopy_lambda_ptr,
                    hotspot_threshold,
                    stack_scratch_enabled,
                ) {
                    step_ok_after_recover = true;
                    dt_step = dt_try;
                    dt_active = dt_try.max(dt * 1e-4);
                    switch_recover_streak = if model_has_ideal_switch { 32 } else { 8 };
                    prefer_implicit_near_switch = true;
                    eprintln!(
                        "[newton-switch-recover] {} micro-step ok at t={:.6} dt_scale={:.4} dt={:.6e}",
                        if use_be { "implicit" } else { "rk4" },
                        time,
                        scale,
                        dt_try
                    );
                    break 'scale_loop;
                }
            }
        }

        // Binary-search event localization when discrete scales miss.
        if !step_ok_after_recover {
            let mut lo = 0.0_f64;
            let mut hi = dt;
            let mut best = 0.0_f64;
            for _ in 0..24 {
                let mid = 0.5 * (lo + hi);
                if mid < 1e-15 {
                    break;
                }
                if try_switch_step(
                    mid,
                    true,
                    time,
                    t_end,
                    active_calc_derivs,
                    params,
                    pre_states,
                    pre_discrete_vals,
                    output_vars,
                    save_states,
                    save_discrete,
                    save_outputs,
                    states,
                    discrete_vals,
                    outputs,
                    when_states,
                    crossings,
                    scratch_outputs_for_step,
                    backward_euler_solver,
                    rk4_solver,
                    step_buf_discrete,
                    step_buf_when,
                    step_buf_crossings,
                    step_buf_outputs,
                    step_buf_guess,
                    use_scratch_outputs_for_solver,
                    diag_res_ptr,
                    diag_x_ptr,
                    eval_call_index_ptr,
                    last_eval_time_ptr,
                    last_eval_state_ptr,
                    last_eval_state_len,
                    homotopy_lambda_ptr,
                    hotspot_threshold,
                    stack_scratch_enabled,
                ) {
                    best = mid;
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            if best > 1e-15
                && try_switch_step(
                    best,
                    true,
                    time,
                    t_end,
                    active_calc_derivs,
                    params,
                    pre_states,
                    pre_discrete_vals,
                    output_vars,
                    save_states,
                    save_discrete,
                    save_outputs,
                    states,
                    discrete_vals,
                    outputs,
                    when_states,
                    crossings,
                    scratch_outputs_for_step,
                    backward_euler_solver,
                    rk4_solver,
                    step_buf_discrete,
                    step_buf_when,
                    step_buf_crossings,
                    step_buf_outputs,
                    step_buf_guess,
                    use_scratch_outputs_for_solver,
                    diag_res_ptr,
                    diag_x_ptr,
                    eval_call_index_ptr,
                    last_eval_time_ptr,
                    last_eval_state_ptr,
                    last_eval_state_len,
                    homotopy_lambda_ptr,
                    hotspot_threshold,
                    stack_scratch_enabled,
                )
            {
                step_ok_after_recover = true;
                dt_step = best;
                dt_active = best.max(dt * 1e-4);
                switch_recover_streak = if model_has_ideal_switch { 32 } else { 8 };
                prefer_implicit_near_switch = true;
                eprintln!(
                    "[newton-switch-recover] bisect localize ok at t={:.6} dt={:.6e}",
                    time, best
                );
            }
        }

        // Stuck on PWM edge: re-run event iteration, then tiny implicit step.
        if !step_ok_after_recover {
            states.copy_from_slice(save_states);
            discrete_vals.copy_from_slice(save_discrete);
            outputs.copy_from_slice(save_outputs);
            for (i, name) in output_vars.iter().enumerate() {
                if i < outputs.len() && is_ideal_switch_complementarity_name(name) {
                    outputs[i] = 0.0;
                }
            }
            let _ = recover_newton_ideal_switch(
                active_calc_derivs,
                time,
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
            // PWM edges can trip asserts during Newton probes; reset so event
            // re-iteration is not aborted by a prior storm from this step.
            native::reset_assert_counter();
            match run_event_iteration_at_time(
                time,
                t_end,
                active_calc_derivs,
                when_count,
                states,
                discrete_vals,
                derivs,
                params,
                outputs,
                when_states,
                crossings,
                pre_states,
                pre_discrete_vals,
                homotopy_lambda,
                homotopy_lambda_ptr,
                newton_tearing_var_names,
                output_start_vals,
                output_vars,
                diag_residual,
                diag_x,
                diag_res_ptr,
                diag_x_ptr,
                diag_call_index,
                diag_time,
                prev_outputs,
                clock_partition_schedule,
                w,
                Some(event_queue),
            ) {
                Ok(EventIterationOutcome::TerminatedOk) => {
                    native::suppress_assert_end();
                    return SwitchStepRecoverResult::TerminatedOk;
                }
                Ok(EventIterationOutcome::Completed) => {
                    save_states.copy_from_slice(states);
                    save_discrete.copy_from_slice(discrete_vals);
                    save_outputs.copy_from_slice(outputs);
                    let micro = (dt * 1e-3).clamp(1e-12, dt * 0.1);
                    if try_switch_step(
                        micro,
                        true,
                        time,
                        t_end,
                        active_calc_derivs,
                        params,
                        pre_states,
                        pre_discrete_vals,
                        output_vars,
                        save_states,
                        save_discrete,
                        save_outputs,
                        states,
                        discrete_vals,
                        outputs,
                        when_states,
                        crossings,
                        scratch_outputs_for_step,
                        backward_euler_solver,
                        rk4_solver,
                        step_buf_discrete,
                        step_buf_when,
                        step_buf_crossings,
                        step_buf_outputs,
                        step_buf_guess,
                        use_scratch_outputs_for_solver,
                        diag_res_ptr,
                        diag_x_ptr,
                        eval_call_index_ptr,
                        last_eval_time_ptr,
                        last_eval_state_ptr,
                        last_eval_state_len,
                        homotopy_lambda_ptr,
                        hotspot_threshold,
                        stack_scratch_enabled,
                    ) {
                        step_ok_after_recover = true;
                        dt_step = micro;
                        dt_active = micro.max(dt * 1e-4);
                        switch_recover_streak = if model_has_ideal_switch { 32 } else { 8 };
                        prefer_implicit_near_switch = true;
                        eprintln!(
                            "[newton-switch-recover] post-event implicit micro-step ok at t={:.6} dt={:.6e}",
                            time, micro
                        );
                    }
                }
                Err(e) => {
                    eprintln!(
                        "[newton-switch-recover] post-fail event-iteration: {}",
                        e
                    );
                }
            }
        }
    }

    if step_ok_after_recover {
        return SwitchStepRecoverResult::Recovered {
            dt_step,
            dt_active,
            switch_recover_streak,
            prefer_implicit_near_switch,
        };
    }

    let attempted_switch_recover = status == 2 && has_switch;
    if attempted_switch_recover && switch_discontinuity_nudges < SWITCH_DISCONTINUITY_NUDGE_MAX {
        switch_discontinuity_nudges = switch_discontinuity_nudges.saturating_add(1);
        let eps = (dt * 0.01).clamp(1e-12, dt * 0.1);
        states.copy_from_slice(save_states);
        discrete_vals.copy_from_slice(save_discrete);
        outputs.copy_from_slice(save_outputs);
        let _ = warm_start_ideal_switch_outputs(output_vars, outputs);
        eprintln!(
            "[newton-switch-recover] discontinuity nudge t={:.6} -> {:.6} (eps={:.3e}) #{}/{}",
            time,
            time + eps,
            eps,
            switch_discontinuity_nudges,
            SWITCH_DISCONTINUITY_NUDGE_MAX
        );
        return SwitchStepRecoverResult::Nudge {
            time: time + eps,
            switch_discontinuity_nudges,
        };
    }
    if attempted_switch_recover {
        eprintln!(
            "[newton-switch-recover] discontinuity nudge budget exceeded ({})",
            SWITCH_DISCONTINUITY_NUDGE_MAX
        );
    }
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
            "solver-step",
            *diag_call_index,
            *diag_time,
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
                *diag_call_index, *diag_time, state_display, diag_residual, diag_x
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
    SwitchStepRecoverResult::Fatal { status }
}
