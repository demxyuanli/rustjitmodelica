use crate::jit::{native, CalcDerivsFunc};

use super::algebraic::{sanitize_extreme_algebraic_outputs, warm_start_ideal_switch_outputs};
use super::eval_helpers::{newton_eval_accepted, sanitize_newton_eval_buffers};
use super::is_ideal_switch_complementarity_name;

/// Mid-run recovery when Newton fails near ideal-switch events:
/// clamp complementarity `s` and retry with a short homotopy on `homotopy_lambda`.
#[allow(clippy::too_many_arguments)]
pub fn recover_newton_ideal_switch(
    calc_derivs: CalcDerivsFunc,
    time: f64,
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
    diag_res_ptr: *mut f64,
    diag_x_ptr: *mut f64,
    diag_residual: &mut f64,
    homotopy_lambda: &mut f64,
    homotopy_lambda_ptr: *const f64,
) -> bool {
    let clamped = warm_start_ideal_switch_outputs(output_vars, outputs);
    // Also clamp any remaining non-finite / extreme algebraic values that poison Newton.
    let extreme = sanitize_extreme_algebraic_outputs(outputs);
    let has_switch = output_vars
        .iter()
        .any(|n| is_ideal_switch_complementarity_name(n));
    if !has_switch && clamped == 0 && extreme == 0 {
        return false;
    }
    eprintln!(
        "[newton-switch-recover] t={:.6}: clamped_switch_s={} extreme_outputs={} has_switch={}",
        time, clamped, extreme, has_switch
    );

    sanitize_newton_eval_buffers(states, discrete_vals, derivs, outputs, when_states, crossings);

    let lambdas: [f64; 5] = [0.0, 0.25, 0.5, 0.75, 1.0];
    for &lam in &lambdas {
        *homotopy_lambda = lam;
        sanitize_newton_eval_buffers(states, discrete_vals, derivs, outputs, when_states, crossings);
        native::suppress_assert_begin();
        let rs = unsafe {
            (calc_derivs)(
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
            )
        };
        native::suppress_assert_end();
        if newton_eval_accepted(rs, *diag_residual) {
            *homotopy_lambda = 1.0;
            eprintln!(
                "[newton-switch-recover] succeeded at lambda={:.2} residual={:.6e}",
                lam, *diag_residual
            );
            return true;
        }
        // Keep latest clamped outputs as warm-start for the next lambda.
        let _ = warm_start_ideal_switch_outputs(output_vars, outputs);
    }
    *homotopy_lambda = 1.0;

    // Mode seeds: IdealDiode/transistor regions need distinct s starts. With JIT now
    // warm-starting from outputs (|s|<=10), these seeds can actually reach Newton.
    if try_switch_mode_seed_newton(
        calc_derivs,
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
    ) {
        return true;
    }
    false
}

/// Try discrete on/off/boundary seeds for each ideal-switch complementarity `s`.
/// Returns true when calc_derivs accepts under production residual policy.
#[allow(clippy::too_many_arguments)]
pub fn try_switch_mode_seed_newton(
    calc_derivs: CalcDerivsFunc,
    time: f64,
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
    diag_res_ptr: *mut f64,
    diag_x_ptr: *mut f64,
    diag_residual: &mut f64,
    homotopy_lambda: &mut f64,
    homotopy_lambda_ptr: *const f64,
) -> bool {
    let switch_idxs: Vec<usize> = output_vars
        .iter()
        .enumerate()
        .filter(|(i, n)| *i < outputs.len() && is_ideal_switch_complementarity_name(n))
        .map(|(i, _)| i)
        .collect();
    if switch_idxs.is_empty() {
        return false;
    }

    const SEEDS: [f64; 5] = [-1.0, -1e-3, 0.0, 1e-3, 1.0];
    let mut baseline = outputs.to_vec();
    let _ = sanitize_extreme_algebraic_outputs(&mut baseline);
    for &i in &switch_idxs {
        if i < baseline.len() && baseline[i].abs() > 10.0 {
            baseline[i] = 0.0;
        }
    }
    *homotopy_lambda = 1.0;

    let eval_once = |outputs: &mut [f64],
                     states: &mut [f64],
                     discrete_vals: &mut [f64],
                     derivs: &mut [f64],
                     when_states: &mut [f64],
                     crossings: &mut [f64],
                     diag_residual: &mut f64|
     -> bool {
        sanitize_newton_eval_buffers(states, discrete_vals, derivs, outputs, when_states, crossings);
        native::suppress_assert_begin();
        let rs = unsafe {
            (calc_derivs)(
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
            )
        };
        native::suppress_assert_end();
        newton_eval_accepted(rs, *diag_residual)
            || (rs == 2
                && diag_residual.is_finite()
                && diag_residual.abs() <= crate::newton_policy::switch_accept_residual_tol())
    };

    // Phase A: all switches share the same seed (cheap, often enough for PWM edges).
    for &seed in &SEEDS {
        outputs.copy_from_slice(&baseline);
        for &i in &switch_idxs {
            outputs[i] = seed;
        }
        if eval_once(
            outputs,
            states,
            discrete_vals,
            derivs,
            when_states,
            crossings,
            diag_residual,
        ) {
            eprintln!(
                "[newton-switch-recover] mode-seed all-s={:.3e} ok at t={:.6} residual={:.6e}",
                seed, time, *diag_residual
            );
            return true;
        }
    }

    // Phase B: full cartesian product when few switches (DutyCycle has ~3).
    if switch_idxs.len() <= 3 {
        let n = switch_idxs.len();
        let mut combo = vec![0usize; n];
        loop {
            outputs.copy_from_slice(&baseline);
            for (k, &si) in switch_idxs.iter().enumerate() {
                outputs[si] = SEEDS[combo[k]];
            }
            if eval_once(
                outputs,
                states,
                discrete_vals,
                derivs,
                when_states,
                crossings,
                diag_residual,
            ) {
                eprintln!(
                    "[newton-switch-recover] mode-seed combo ok at t={:.6} residual={:.6e} seeds={:?}",
                    time,
                    *diag_residual,
                    combo.iter().map(|&c| SEEDS[c]).collect::<Vec<_>>()
                );
                return true;
            }
            // odometer increment
            let mut carry = true;
            for c in combo.iter_mut() {
                if !carry {
                    break;
                }
                *c += 1;
                if *c < SEEDS.len() {
                    carry = false;
                } else {
                    *c = 0;
                }
            }
            if carry {
                break;
            }
        }
    } else {
        // Phase B': vary one switch at a time; others at 0.
        for (focus, &si) in switch_idxs.iter().enumerate() {
            for &seed in &SEEDS {
                outputs.copy_from_slice(&baseline);
                for &j in &switch_idxs {
                    outputs[j] = 0.0;
                }
                outputs[si] = seed;
                if eval_once(
                    outputs,
                    states,
                    discrete_vals,
                    derivs,
                    when_states,
                    crossings,
                    diag_residual,
                ) {
                    eprintln!(
                        "[newton-switch-recover] mode-seed focus={} s={:.3e} ok at t={:.6} residual={:.6e}",
                        focus, seed, time, *diag_residual
                    );
                    return true;
                }
            }
        }
    }

    outputs.copy_from_slice(&baseline);
    false
}
