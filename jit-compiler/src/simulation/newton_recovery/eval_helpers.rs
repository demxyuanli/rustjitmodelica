use super::allow_zero_residual_newton;

pub(super) fn sanitize_newton_eval_buffers(
    states: &mut [f64],
    discrete_vals: &mut [f64],
    derivs: &mut [f64],
    outputs: &mut [f64],
    when_states: &mut [f64],
    crossings: &mut [f64],
) {
    for x in states.iter_mut().chain(discrete_vals.iter_mut()).chain(derivs.iter_mut()) {
        if !x.is_finite() {
            *x = 0.0;
        }
    }
    for x in outputs
        .iter_mut()
        .chain(when_states.iter_mut())
        .chain(crossings.iter_mut())
    {
        if !x.is_finite() {
            *x = 0.0;
        }
    }
}

pub(super) fn copy_output_starts_finite(output_start_vals: &[f64], outputs: &mut [f64]) {
    for (i, v) in output_start_vals.iter().enumerate() {
        if i < outputs.len() {
            outputs[i] = if v.is_finite() { *v } else { 0.0 };
        }
    }
}

pub(super) fn newton_eval_accepted(rs: i32, diag_residual: f64) -> bool {
    (rs == 0 && diag_residual.is_finite()) || allow_zero_residual_newton(rs, diag_residual)
}

pub(super) fn relaxed_newton_diag_ok(diag_residual: f64, tol: f64) -> bool {
    diag_residual.is_finite() && diag_residual.abs() <= tol
}
