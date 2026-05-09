use crate::jit::{native, CalcDerivsFunc};

pub use crate::newton_policy::{allow_algebraic_newton_fallback, allow_zero_residual_newton};

fn assert_storm_limit() -> u64 {
    std::env::var("RUSTMODLICA_ASSERT_STORM_LIMIT")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(256)
}

// With feature `sundials`, `crate::simulation::kinsol_solve_square_spgmr` can solve isolated F(u)=0 systems.

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
    if hits > assert_storm_limit() {
        return Err(format!(
            "Aborting due to assertion storm at stage={} time={:.6} assert_hits={}",
            stage, time, hits
        ));
    }
    Ok(())
}

fn sanitize_newton_eval_buffers(
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

fn copy_output_starts_finite(output_start_vals: &[f64], outputs: &mut [f64]) {
    for (i, v) in output_start_vals.iter().enumerate() {
        if i < outputs.len() {
            outputs[i] = if v.is_finite() { *v } else { 0.0 };
        }
    }
}

fn newton_eval_accepted(rs: i32, diag_residual: f64) -> bool {
    (rs == 0 && diag_residual.is_finite()) || allow_zero_residual_newton(rs, diag_residual)
}

fn relaxed_newton_diag_ok(diag_residual: f64, tol: f64) -> bool {
    diag_residual.is_finite() && diag_residual.abs() <= tol
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

/// KINSOL-based algebraic initialization: solve F(states)=0 at t=0 using KINSOL Newton+linesearch.
/// Returns true if KINSOL converged successfully.
#[cfg(feature = "sundials")]
fn try_kinsol_init(
    calc_derivs: CalcDerivsFunc,
    time: f64,
    t_end: f64,
    params: &[f64],
    pre_states: &[f64],
    pre_discrete_vals: &[f64],
    states: &mut [f64],
    discrete_vals: &mut [f64],
    derivs: &mut [f64],
    outputs: &mut [f64],
    when_states: &mut [f64],
    crossings: &mut [f64],
    diag_res_ptr: *mut f64,
    diag_x_ptr: *mut f64,
    homotopy_lambda_ptr: *const f64,
    n: usize,
) -> bool {
    use crate::simulation::sundials::kinsol_solve_square_spgmr;

    #[repr(C)]
    struct KinInitCtx {
        calc_derivs: CalcDerivsFunc,
        time: f64,
        t_end: f64,
        params: *const f64,
        pre_states: *const f64,
        pre_discrete: *const f64,
        outputs: *mut f64,
        when_states: *mut f64,
        crossings: *mut f64,
        diag_res_ptr: *mut f64,
        diag_x_ptr: *mut f64,
        homotopy_lambda_ptr: *const f64,
        discrete: *mut f64,
        work_deriv: *mut f64,
    }

    unsafe extern "C" fn kin_residual(
        u: *const sundials_sys::sunrealtype,
        fu: *mut sundials_sys::sunrealtype,
        nn: usize,
        user_data: *mut libc::c_void,
    ) -> i32 {
        let ctx = &*(user_data as *const KinInitCtx);
        // Copy trial states to work buffer
        std::ptr::copy_nonoverlapping(u, ctx.discrete.offset(0) as *mut f64, nn);
        // Evaluate calc_derivs — residual is derivs (we want them ≈ 0)
        let status = (ctx.calc_derivs)(
            ctx.time,
            ctx.discrete.offset(0) as *mut f64, // states = u
            ctx.discrete.add(nn), // discrete after states
            fu, // derivs = residual output
            ctx.params,
            ctx.outputs,
            ctx.when_states,
            ctx.crossings,
            ctx.pre_states,
            ctx.pre_discrete,
            ctx.t_end,
            ctx.diag_res_ptr,
            ctx.diag_x_ptr,
            ctx.homotopy_lambda_ptr,
        );
        status
    }

    // Build context: store discrete_vals and derivs adjacent for the callback
    let mut work_buf: Vec<f64> = Vec::with_capacity(n + discrete_vals.len());
    work_buf.extend_from_slice(states);
    work_buf.extend_from_slice(discrete_vals);

    let ctx = KinInitCtx {
        calc_derivs,
        time,
        t_end,
        params: params.as_ptr(),
        pre_states: pre_states.as_ptr(),
        pre_discrete: pre_discrete_vals.as_ptr(),
        outputs: outputs.as_mut_ptr(),
        when_states: when_states.as_mut_ptr(),
        crossings: crossings.as_mut_ptr(),
        diag_res_ptr,
        diag_x_ptr,
        homotopy_lambda_ptr,
        discrete: work_buf.as_mut_ptr(),
        work_deriv: derivs.as_mut_ptr(),
    };

    match kinsol_solve_square_spgmr(
        n,
        states,
        kin_residual,
        &ctx as *const KinInitCtx as *mut libc::c_void,
    ) {
        Ok(()) => {
            // Copy converged discrete values back
            discrete_vals.copy_from_slice(&work_buf[n..n + discrete_vals.len()]);
            true
        }
        Err(_) => false,
    }
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

    // Phase 0: KINSOL algebraic initialization (SUNDIALS Newton+linesearch, converges faster for stiff systems).
    #[cfg(feature = "sundials")]
    {
        let n = states.len();
        if try_kinsol_init(
            calc_derivs, time, t_end, params,
            pre_states, pre_discrete_vals,
            states, discrete_vals, derivs, outputs,
            when_states, crossings,
            diag_res_ptr, diag_x_ptr, homotopy_lambda_ptr, n,
        ) {
            return true;
        }
    }

    // Phase 1: True homotopy continuation (finer steps help large MultiBody DAEs at t=0).
    let mut lambda_step = 0.05_f64;
    let mut lam = 0.0_f64;
    *homotopy_lambda = 0.0;
    states.copy_from_slice(pre_states);
    discrete_vals.copy_from_slice(pre_discrete_vals);
    derivs.fill(0.0);
    when_states.fill(0.0);
    crossings.fill(0.0);
    copy_output_starts_finite(output_start_vals, outputs);
    sanitize_newton_eval_buffers(states, discrete_vals, derivs, outputs, when_states, crossings);
    let mut homotopy_ok = true;
    let mut halve_count = 0_u32;
    while lam < 1.0 {
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
            lam += lambda_step;
            halve_count = 0;
        } else {
            lambda_step *= 0.5;
            halve_count += 1;
            if halve_count > 16 || lambda_step < 1e-7 {
                homotopy_ok = false;
                break;
            }
            states.copy_from_slice(pre_states);
            discrete_vals.copy_from_slice(pre_discrete_vals);
            derivs.fill(0.0);
            when_states.fill(0.0);
            crossings.fill(0.0);
            copy_output_starts_finite(output_start_vals, outputs);
            sanitize_newton_eval_buffers(states, discrete_vals, derivs, outputs, when_states, crossings);
        }
    }
    if homotopy_ok {
        *homotopy_lambda = 1.0;
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
                    let sv = if v.is_finite() { *v } else { 0.0 };
                    outputs[i] = if sv == 0.0 { perturb } else { sv };
                }
            }
            for out in outputs.iter_mut().skip(output_start_vals.len()) {
                *out = perturb;
            }
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
                eprintln!(
                    "[newton-homotopy] retry {} (perturb={}) succeeded at t=0",
                    retry_round, perturb
                );
                recovered = true;
                break;
            }
            if rs == 2 && relaxed_newton_diag_ok(*diag_residual, 1e-6) {
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
                    let sv = if v.is_finite() { *v } else { 0.0 };
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
                eprintln!("[newton-multistart] seed {} succeeded at t=0", seed);
                recovered = true;
                break;
            }
            if rs == 2 && relaxed_newton_diag_ok(*diag_residual, 1e-4) {
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
            copy_output_starts_finite(output_start_vals, outputs);
            for (_, indices) in &geo_candidates {
                outputs[indices[0]] = bv[0];
                outputs[indices[1]] = bv[1];
                outputs[indices[2]] = bv[2];
            }
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
                eprintln!("[newton-geo-seed] basis {:?} succeeded at t=0", bv);
                return true;
            }
            if rs == 2 && relaxed_newton_diag_ok(*diag_residual, 1e-4) {
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
                outputs[oi] = if sv.is_finite() { sv } else { 0.0 };
            }
        }
    }
    if applied_geo {
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
            eprintln!("[newton-geo-fix] geometric defaults succeeded at t=0");
            return true;
        }
        if rs == 2 && relaxed_newton_diag_ok(*diag_residual, 1e-3) {
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
    copy_output_starts_finite(output_start_vals, outputs);
    let projected = project_geometric_vectors_in_place(output_vars, outputs);
    if projected {
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
            eprintln!("[newton-geo-project] unit-vector projection succeeded at t=0");
            return true;
        }
        if rs == 2 && relaxed_newton_diag_ok(*diag_residual, 1e-3) {
            eprintln!(
                "[newton-geo-project] accepted (relaxed, residual={:.6e})",
                diag_residual
            );
            return true;
        }
    }

    false
}
