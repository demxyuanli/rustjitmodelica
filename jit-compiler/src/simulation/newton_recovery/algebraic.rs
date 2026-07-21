use super::is_ideal_switch_complementarity_name;

pub fn is_geometric_vector_component(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower.contains("_e_")
        || lower.contains("_n_")
        || lower.contains(".e[")
        || lower.contains(".n[")
        || lower.contains("_ex_")
        || lower.contains("_ey_")
        || lower.contains("_ez_")
        || lower.contains("_delta_")
        || lower.contains("_e_axis")
        || lower.contains("_e_lat")
        || lower.contains("_e_long")
        || lower.contains("_e_n_")
        || lower.contains("_e_s_")
}

/// Reset blown-up complementarity unknowns to the on/off boundary (s=0).
/// Returns how many entries were clamped.
pub fn warm_start_ideal_switch_outputs(output_vars: &[String], outputs: &mut [f64]) -> usize {
    const BLOW_UP: f64 = 1.0;
    let mut n = 0usize;
    for (i, name) in output_vars.iter().enumerate() {
        if i >= outputs.len() {
            break;
        }
        if !is_ideal_switch_complementarity_name(name) {
            continue;
        }
        let v = outputs[i];
        if !v.is_finite() || v.abs() > BLOW_UP {
            outputs[i] = 0.0;
            n += 1;
        }
    }
    n
}

/// After a zero-crossing rewind: restore pre-step algebraic guess, then clamp switch `s`.
pub fn prepare_event_step_algebraic_guess(
    output_vars: &[String],
    save_outputs: &[f64],
    outputs: &mut [f64],
    scratch: &mut [f64],
) -> usize {
    let n = outputs
        .len()
        .min(save_outputs.len())
        .min(scratch.len());
    outputs[..n].copy_from_slice(&save_outputs[..n]);
    let clamped = warm_start_ideal_switch_outputs(output_vars, outputs);
    scratch[..n].copy_from_slice(&outputs[..n]);
    if clamped > 0 {
        eprintln!(
            "[newton-switch-warmstart] event-step: restored outputs and clamped {} ideal-switch s vars",
            clamped
        );
    }
    clamped
}

/// After a solver step that used `scratch_outputs`, copy the accepted algebraics back
/// into `outputs` so trial-eval / CSV / event-iteration see the same warm-start.
/// Also clamps blown-up ideal-switch complementarity unknowns.
pub fn commit_solver_algebraic_guess(
    output_vars: &[String],
    outputs: &mut [f64],
    scratch: &[f64],
    use_scratch: bool,
) -> usize {
    if use_scratch {
        let n = outputs.len().min(scratch.len());
        outputs[..n].copy_from_slice(&scratch[..n]);
    }
    warm_start_ideal_switch_outputs(output_vars, outputs)
}

/// Clamp non-finite / extreme algebraic values that poison Newton warm-starts.
/// Returns how many entries were sanitized.
pub fn sanitize_extreme_algebraic_outputs(outputs: &mut [f64]) -> usize {
    const EXTREME: f64 = 1e6;
    let mut n = 0usize;
    for v in outputs.iter_mut() {
        if !v.is_finite() || v.abs() > EXTREME {
            *v = 0.0;
            n += 1;
        }
    }
    n
}

pub fn project_geometric_vectors_in_place(output_vars: &[String], outputs: &mut [f64]) -> bool {
    let mut projected_any = false;
    let mut i = 0usize;
    while i + 2 < output_vars.len() && i + 2 < outputs.len() {
        let n0 = output_vars[i].to_lowercase();
        let n1 = output_vars[i + 1].to_lowercase();
        let n2 = output_vars[i + 2].to_lowercase();
        let is_triplet = n0.ends_with("_1") && n1.ends_with("_2") && n2.ends_with("_3");
        if is_triplet && is_geometric_vector_component(&output_vars[i]) {
            let x = outputs[i];
            let y = outputs[i + 1];
            let z = outputs[i + 2];
            let norm = (x * x + y * y + z * z).sqrt();
            if norm > 1e-12 {
                outputs[i] = x / norm;
                outputs[i + 1] = y / norm;
                outputs[i + 2] = z / norm;
            } else {
                outputs[i] = 1.0;
                outputs[i + 1] = 0.0;
                outputs[i + 2] = 0.0;
            }
            projected_any = true;
            i += 3;
            continue;
        }
        i += 1;
    }
    projected_any
}
