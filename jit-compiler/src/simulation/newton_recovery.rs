use crate::jit::{native, CalcDerivsFunc};

const ASSERT_STORM_LIMIT: u64 = 256;

pub fn allow_zero_residual_newton(status: i32, diag_residual: f64) -> bool {
    status == 2 && diag_residual.abs() <= 1e-5
}

pub fn allow_algebraic_newton_fallback(status: i32, state_len: usize) -> bool {
    status == 2 && state_len == 0
}

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

pub fn fail_if_assert_storm(stage: &str, time: f64) -> Result<(), String> {
    let hits = native::assert_hit_count();
    if hits > ASSERT_STORM_LIMIT {
        return Err(format!(
            "Aborting due to assertion storm at stage={} time={:.6} assert_hits={}",
            stage, time, hits
        ));
    }
    Ok(())
}

pub fn print_newton_diag(
    phase: &str,
    eval_calls: u32,
    last_eval_time: f64,
    diag_residual: f64,
    diag_x: f64,
) {
    let assert_hits = native::assert_hit_count();
    eprintln!(
        "[newton-diag] phase={} eval_calls={} last_eval_time={:.6} diag_residual={:.6e} diag_x={:.6e} assert_hits={}",
        phase,
        eval_calls,
        last_eval_time,
        diag_residual,
        diag_x,
        assert_hits
    );
}

#[allow(clippy::too_many_arguments)]
pub fn recover_newton_at_t0(
    calc_derivs: CalcDerivsFunc,
    time: f64,
    t_end: f64,
    params: &[f64],
    pre_states: &[f64],
    pre_discrete_vals: &[f64],
    output_start_vals: &[f64],
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
    let mut recovered = false;

    // Phase 1: True homotopy continuation.
    let mut lambda_step = 0.1_f64;
    let mut lam = 0.0_f64;
    *homotopy_lambda = 0.0;
    states.copy_from_slice(pre_states);
    discrete_vals.copy_from_slice(pre_discrete_vals);
    derivs.fill(0.0);
    when_states.fill(0.0);
    crossings.fill(0.0);
    for (i, v) in output_start_vals.iter().enumerate() {
        if i < outputs.len() {
            outputs[i] = *v;
        }
    }
    let mut homotopy_ok = true;
    let mut halve_count = 0_u32;
    while lam < 1.0 {
        *homotopy_lambda = lam;
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
        if rs == 0 || allow_zero_residual_newton(rs, *diag_residual) {
            lam += lambda_step;
            halve_count = 0;
        } else {
            lambda_step *= 0.5;
            halve_count += 1;
            if halve_count > 10 || lambda_step < 1e-6 {
                homotopy_ok = false;
                break;
            }
            states.copy_from_slice(pre_states);
            discrete_vals.copy_from_slice(pre_discrete_vals);
            derivs.fill(0.0);
            when_states.fill(0.0);
            crossings.fill(0.0);
            for (i, v) in output_start_vals.iter().enumerate() {
                if i < outputs.len() {
                    outputs[i] = *v;
                }
            }
        }
    }
    if homotopy_ok {
        *homotopy_lambda = 1.0;
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
        if rs == 0 || allow_zero_residual_newton(rs, *diag_residual) {
            eprintln!("[homotopy] continuation succeeded at t=0");
            recovered = true;
        }
    }

    // Phase 2: Perturbation fallback.
    if !recovered {
        *homotopy_lambda = 1.0;
        let perturbations: &[f64] = &[0.0, 1.0, 0.1, -0.1, 0.01, 1e-3, -1.0, 10.0];
        for (retry_round, &perturb) in perturbations.iter().enumerate() {
            states.copy_from_slice(pre_states);
            discrete_vals.copy_from_slice(pre_discrete_vals);
            derivs.fill(0.0);
            when_states.fill(0.0);
            crossings.fill(0.0);
            for (i, v) in output_start_vals.iter().enumerate() {
                if i < outputs.len() {
                    let sv = *v;
                    outputs[i] = if sv == 0.0 { perturb } else { sv };
                }
            }
            for out in outputs.iter_mut().skip(output_start_vals.len()) {
                *out = perturb;
            }
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
            if rs == 0 || allow_zero_residual_newton(rs, *diag_residual) {
                eprintln!(
                    "[newton-homotopy] retry {} (perturb={}) succeeded at t=0",
                    retry_round, perturb
                );
                recovered = true;
                break;
            }
            if rs == 2 && diag_residual.abs() <= 1e-6 {
                eprintln!(
                    "[newton-homotopy] retry {} (perturb={}) accepted at t=0 (relaxed tol, residual={:.6e})",
                    retry_round, perturb, diag_residual
                );
                recovered = true;
                break;
            }
        }
    }

    // Phase 3: Randomized multi-start.
    if !recovered {
        let seeds: &[u64] = &[42, 137, 271, 1009, 31337, 99991, 54321, 77777];
        for &seed in seeds {
            states.copy_from_slice(pre_states);
            discrete_vals.copy_from_slice(pre_discrete_vals);
            derivs.fill(0.0);
            when_states.fill(0.0);
            crossings.fill(0.0);
            let mut rng_state = seed;
            for (i, v) in output_start_vals.iter().enumerate() {
                if i < outputs.len() {
                    let sv = *v;
                    if sv == 0.0 {
                        rng_state = rng_state
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add(1442695040888963407);
                        let bits = (rng_state >> 33) as u32;
                        let frac = (bits as f64) / (u32::MAX as f64);
                        outputs[i] = frac * 2.0 - 1.0;
                    } else {
                        outputs[i] = sv;
                    }
                }
            }
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
            if rs == 0 || allow_zero_residual_newton(rs, *diag_residual) {
                eprintln!("[newton-multistart] seed {} succeeded at t=0", seed);
                recovered = true;
                break;
            }
            if rs == 2 && diag_residual.abs() <= 1e-4 {
                eprintln!(
                    "[newton-multistart] seed {} accepted (relaxed, residual={:.6e})",
                    seed, diag_residual
                );
                recovered = true;
                break;
            }
        }
    }
    if recovered {
        return true;
    }

    // Phase 3.5: Constraint-aware geometric seed initialization.
    let basis_vectors: &[[f64; 3]] = &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    let mut geo_candidates: Vec<(usize, [usize; 3])> = Vec::new();
    let mut i = 0;
    while i + 2 < output_start_vals.len() && i + 2 < outputs.len() {
        let all_zero = output_start_vals[i] == 0.0
            && output_start_vals[i + 1] == 0.0
            && output_start_vals[i + 2] == 0.0;
        if all_zero && i + 2 < output_vars.len() {
            let ends_with_1 = output_vars[i].to_lowercase().ends_with("_1")
                && output_vars
                    .get(i + 1)
                    .map(|v| v.to_lowercase().ends_with("_2"))
                    .unwrap_or(false)
                && output_vars
                    .get(i + 2)
                    .map(|v| v.to_lowercase().ends_with("_3"))
                    .unwrap_or(false);
            if is_geometric_vector_component(&output_vars[i]) && ends_with_1 {
                geo_candidates.push((i, [i, i + 1, i + 2]));
                i += 3;
                continue;
            }
        }
        i += 1;
    }

    if !geo_candidates.is_empty() {
        for bv in basis_vectors {
            states.copy_from_slice(pre_states);
            discrete_vals.copy_from_slice(pre_discrete_vals);
            derivs.fill(0.0);
            when_states.fill(0.0);
            crossings.fill(0.0);
            for (oi, v) in output_start_vals.iter().enumerate() {
                if oi < outputs.len() {
                    outputs[oi] = *v;
                }
            }
            for (_, indices) in &geo_candidates {
                outputs[indices[0]] = bv[0];
                outputs[indices[1]] = bv[1];
                outputs[indices[2]] = bv[2];
            }
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
            if rs == 0 || allow_zero_residual_newton(rs, *diag_residual) {
                eprintln!("[newton-geo-seed] basis {:?} succeeded at t=0", bv);
                return true;
            }
            if rs == 2 && diag_residual.abs() <= 1e-4 {
                eprintln!(
                    "[newton-geo-seed] basis {:?} accepted (relaxed, residual={:.6e})",
                    bv, diag_residual
                );
                return true;
            }
        }
    }

    // Phase 4: Apply geometric defaults to all output variables and retry.
    states.copy_from_slice(pre_states);
    discrete_vals.copy_from_slice(pre_discrete_vals);
    derivs.fill(0.0);
    when_states.fill(0.0);
    crossings.fill(0.0);
    let mut applied_geo = false;
    for (oi, name) in output_vars.iter().enumerate() {
        if oi < outputs.len() {
            let geo = crate::compiler::geometric_default_for_name(name);
            if geo != 0.0 {
                outputs[oi] = geo;
                applied_geo = true;
            } else if let Some(&sv) = output_start_vals.get(oi) {
                outputs[oi] = sv;
            }
        }
    }
    if applied_geo {
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
        if rs == 0 || allow_zero_residual_newton(rs, *diag_residual) {
            eprintln!("[newton-geo-fix] geometric defaults succeeded at t=0");
            return true;
        }
        if rs == 2 && diag_residual.abs() <= 1e-3 {
            eprintln!(
                "[newton-geo-fix] accepted (relaxed, residual={:.6e})",
                diag_residual
            );
            return true;
        }
    }

    // Phase 4.5: Project geometric vectors to unit norm and retry.
    states.copy_from_slice(pre_states);
    discrete_vals.copy_from_slice(pre_discrete_vals);
    derivs.fill(0.0);
    when_states.fill(0.0);
    crossings.fill(0.0);
    for (oi, v) in output_start_vals.iter().enumerate() {
        if oi < outputs.len() {
            outputs[oi] = *v;
        }
    }
    let projected = project_geometric_vectors_in_place(output_vars, outputs);
    if projected {
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
        if rs == 0 || allow_zero_residual_newton(rs, *diag_residual) {
            eprintln!("[newton-geo-project] unit-vector projection succeeded at t=0");
            return true;
        }
        if rs == 2 && diag_residual.abs() <= 1e-3 {
            eprintln!(
                "[newton-geo-project] accepted (relaxed, residual={:.6e})",
                diag_residual
            );
            return true;
        }
    }

    false
}
