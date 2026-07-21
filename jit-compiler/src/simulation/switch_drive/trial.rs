use crate::diag::fallback_counter;
use crate::jit::CalcDerivsFunc;

use super::super::newton_recovery::{
    commit_solver_algebraic_guess, decide_switch_force_s0, is_ideal_switch_complementarity_name,
    recover_newton_ideal_switch, report_switch_fail_open_degraded,
    report_switch_force_s0_rejected, sanitize_extreme_algebraic_outputs,
    try_switch_mode_seed_newton, warm_start_ideal_switch_outputs, SwitchForceS0Decision,
};
use super::{SwitchTrialRecoverResult, SWITCH_DISCONTINUITY_NUDGE_MAX};

#[allow(clippy::too_many_arguments)]
pub(crate) fn recover_failed_trial_eval(
    status: i32,
    time: f64,
    dt: f64,
    dt_step: f64,
    mut dt_active: f64,
    mut switch_recover_streak: u32,
    mut prefer_implicit_near_switch: bool,
    mut switch_discontinuity_nudges: u32,
    model_has_ideal_switch: bool,
    t_trial: f64,
    active_calc_derivs: CalcDerivsFunc,
    t_end: f64,
    params: &[f64],
    pre_states: &[f64],
    pre_discrete_vals: &[f64],
    output_vars: &[String],
    states: &mut [f64],
    discrete_vals: &mut [f64],
    derivs: &mut [f64],
    outputs: &mut [f64],
    when_states: &mut [f64],
    crossings: &mut [f64],
    save_states: &[f64],
    save_discrete: &[f64],
    save_outputs: &[f64],
    scratch_outputs_for_step: &mut [f64],
    diag_residual: &mut f64,
    homotopy_lambda: &mut f64,
    homotopy_lambda_ptr: *const f64,
    diag_res_ptr: *mut f64,
    diag_x_ptr: *mut f64,
    use_scratch_outputs_for_solver: bool,
) -> SwitchTrialRecoverResult {
    let has_switch = model_has_ideal_switch;
    let mut trial_recovered = false;
    if status == 2 && has_switch {
        let _ = commit_solver_algebraic_guess(
            output_vars,
            outputs,
            scratch_outputs_for_step,
            use_scratch_outputs_for_solver,
        );
        let _ = sanitize_extreme_algebraic_outputs(outputs);
        for (i, name) in output_vars.iter().enumerate() {
            if i < outputs.len() && is_ideal_switch_complementarity_name(name) {
                outputs[i] = 0.0;
            }
        }
        scratch_outputs_for_step.copy_from_slice(outputs);
        let recovered = recover_newton_ideal_switch(
            active_calc_derivs,
            t_trial,
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
        if recovered || crate::newton_policy::allow_zero_residual_newton(2, *diag_residual) {
            trial_recovered = true;
            eprintln!(
                "[newton-switch-recover] trial-eval recovered at t={:.6} residual={:.6e}",
                t_trial, diag_residual
            );
        } else {
            match decide_switch_force_s0(status, *diag_residual) {
                SwitchForceS0Decision::AcceptConverged => {
                    trial_recovered = true;
                    eprintln!(
                        "[newton-switch-recover] trial-eval force-s0 accepted at t={:.6} residual={:.6e}",
                        t_trial, diag_residual
                    );
                }
                SwitchForceS0Decision::AcceptDegraded => {
                    let _ = sanitize_extreme_algebraic_outputs(outputs);
                    for (i, name) in output_vars.iter().enumerate() {
                        if i < outputs.len() && is_ideal_switch_complementarity_name(name) {
                            outputs[i] = 0.0;
                        }
                    }
                    trial_recovered = true;
                    prefer_implicit_near_switch = true;
                    switch_recover_streak = switch_recover_streak.max(32);
                    dt_active = (dt_step * 0.5).max(dt * 1e-4);
                    fallback_counter::inc_switch_fail_open();
                    report_switch_fail_open_degraded(
                        "trial-eval",
                        t_trial,
                        *diag_residual,
                        "force-s0",
                    );
                }
                SwitchForceS0Decision::Reject => {
                    if try_switch_mode_seed_newton(
                        active_calc_derivs,
                        t_trial,
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
                    ) {
                        trial_recovered = true;
                        prefer_implicit_near_switch = true;
                        switch_recover_streak = switch_recover_streak.max(32);
                        dt_active = (dt_step * 0.5).max(dt * 1e-4);
                    } else {
                    report_switch_force_s0_rejected(
                        "trial-eval",
                        t_trial,
                        *diag_residual,
                        status,
                    );
                    if switch_discontinuity_nudges < SWITCH_DISCONTINUITY_NUDGE_MAX {
                        switch_discontinuity_nudges =
                            switch_discontinuity_nudges.saturating_add(1);
                        let eps = (dt * 0.01).clamp(1e-12, dt * 0.1);
                        states.copy_from_slice(save_states);
                        discrete_vals.copy_from_slice(save_discrete);
                        outputs.copy_from_slice(save_outputs);
                        let _ = warm_start_ideal_switch_outputs(output_vars, outputs);
                        eprintln!(
                            "[newton-switch-recover] trial-eval discontinuity nudge t={:.6} -> {:.6} (eps={:.3e}) #{}/{}",
                            time,
                            time + eps,
                            eps,
                            switch_discontinuity_nudges,
                            SWITCH_DISCONTINUITY_NUDGE_MAX
                        );
                        return SwitchTrialRecoverResult::Nudge {
                            time: time + eps,
                            switch_discontinuity_nudges,
                        };
                    }
                    }
                }
            }
        }
    }
    if trial_recovered {
        SwitchTrialRecoverResult::Recovered {
            prefer_implicit_near_switch,
            switch_recover_streak,
            dt_active,
        }
    } else {
        SwitchTrialRecoverResult::NotRecovered
    }
}
