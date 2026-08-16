fn sundials_jac_enabled() -> bool {
    std::env::var("RUSTMODLICA_SUNDIALS_JACOBIAN")
        .ok()
        .map(|v| {
            let t = v.trim().to_ascii_lowercase();
            !(t == "0" || t == "false" || t == "off" || t == "no")
        })
        .unwrap_or(false)
}

/// Numerical Jacobian callback for CVODE (dense solver).
/// Fills the dense SUNMatrix with finite-difference Jacobian.
/// Uses structural coloring when available to reduce function evaluations.
unsafe extern "C" fn cv_jac(
    t: sunrealtype,
    y: N_Vector,
    fy: N_Vector,
    Jac: SUNMatrix,
    user_data: *mut c_void,
    tmp1: N_Vector,
    tmp2: N_Vector,
    _tmp3: N_Vector,
) -> c_int {
    let ud = &*(user_data as *const RawSundialsUserData);
    let n = ud.n;
    let jac_data = SUNDenseMatrix_Data(Jac);
    if jac_data.is_null() {
        return -1;
    }
    let y_ptr = N_VGetArrayPointer(y);
    let fy_ptr = N_VGetArrayPointer(fy);
    let t1_ptr = N_VGetArrayPointer(tmp1);
    let t2_ptr = N_VGetArrayPointer(tmp2);
    if y_ptr.is_null() || fy_ptr.is_null() || t1_ptr.is_null() || t2_ptr.is_null() {
        return -1;
    }

    let eps = 1e-6_f64;
    let color_groups_ptr: *const Vec<Vec<usize>> = ud.jacobian_color_groups;

    if !color_groups_ptr.is_null() {
        let groups = &*color_groups_ptr;
        for group in groups {
            // Copy y, then perturb all columns in this color group simultaneously
            std::ptr::copy_nonoverlapping(y_ptr, t1_ptr, n);
            let mut deltas: Vec<f64> = Vec::with_capacity(group.len());
            for &j in group {
                let yj = *y_ptr.add(j);
                let delta = eps.max(eps * yj.abs());
                *t1_ptr.add(j) = yj + delta;
                deltas.push(delta);
            }

            let status = (ud.calc_derivs)(
                t as f64,
                t1_ptr,
                ud.discrete,
                t2_ptr,
                ud.params,
                ud.outputs,
                ud.scratch_when_states,
                ud.scratch_crossings,
                ud.pre_states,
                ud.pre_discrete,
                ud.t_end,
                ud.diag_residual,
                ud.diag_x,
                ud.homotopy_lambda_ptr,
            );
            if status != 0 {
                return status;
            }

            // Fill each column in the group
            for (&j, &delta) in group.iter().zip(deltas.iter()) {
                for i in 0..n {
                    *jac_data.add(j * n + i) = (*t2_ptr.add(i) - *fy_ptr.add(i)) / delta;
                }
            }
        }
    } else {
        // Fallback: per-column evaluation (no coloring available)
        for j in 0..n {
            std::ptr::copy_nonoverlapping(y_ptr, t1_ptr, n);
            let yj = *y_ptr.add(j);
            let delta = eps.max(eps * yj.abs());
            *t1_ptr.add(j) = yj + delta;

            let status = (ud.calc_derivs)(
                t as f64,
                t1_ptr,
                ud.discrete,
                t2_ptr,
                ud.params,
                ud.outputs,
                ud.scratch_when_states,
                ud.scratch_crossings,
                ud.pre_states,
                ud.pre_discrete,
                ud.t_end,
                ud.diag_residual,
                ud.diag_x,
                ud.homotopy_lambda_ptr,
            );
            if status != 0 {
                return status;
            }

            for i in 0..n {
                *jac_data.add(j * n + i) = (*t2_ptr.add(i) - *fy_ptr.add(i)) / delta;
            }
        }
    }

    sundials_sys::CV_SUCCESS as i32
}

/// Numerical Jacobian callback for IDA (dense solver).
/// Fills J = -df/dy + c_j * I with finite differences.
/// Uses structural coloring when available.
unsafe extern "C" fn ida_jac(
    t: sunrealtype,
    c_j: sunrealtype,
    y: N_Vector,
    _yp: N_Vector,
    _r: N_Vector,
    Jac: SUNMatrix,
    user_data: *mut c_void,
    tmp1: N_Vector,
    tmp2: N_Vector,
    _tmp3: N_Vector,
) -> c_int {
    let ud = &*(user_data as *const RawSundialsUserData);
    let n = ud.n;
    let jac_data = SUNDenseMatrix_Data(Jac);
    if jac_data.is_null() {
        return -1;
    }
    let y_ptr = N_VGetArrayPointer(y);
    let t1_ptr = N_VGetArrayPointer(tmp1);
    let t2_ptr = N_VGetArrayPointer(tmp2);
    if y_ptr.is_null() || t1_ptr.is_null() || t2_ptr.is_null() {
        return -1;
    }

    // Compute baseline derivs at y
    std::ptr::copy_nonoverlapping(y_ptr, ud.work_state, n);
    let st = (ud.calc_derivs)(
        t as f64,
        ud.work_state,
        ud.discrete,
        ud.work_deriv,
        ud.params,
        ud.outputs,
        ud.scratch_when_states,
        ud.scratch_crossings,
        ud.pre_states,
        ud.pre_discrete,
        ud.t_end,
        ud.diag_residual,
        ud.diag_x,
        ud.homotopy_lambda_ptr,
    );
    if st != 0 {
        return st;
    }

    let eps = 1e-6_f64;
    let cj = c_j as f64;

    let color_groups_ptr: *const Vec<Vec<usize>> = ud.jacobian_color_groups;
    let evaluate_per_group = |t1_ptr: *mut f64, t2_ptr: *mut f64| -> i32 {
        (ud.calc_derivs)(
            t as f64,
            t1_ptr,
            ud.discrete,
            t2_ptr,
            ud.params,
            ud.outputs,
            ud.scratch_when_states,
            ud.scratch_crossings,
            ud.pre_states,
            ud.pre_discrete,
            ud.t_end,
            ud.diag_residual,
            ud.diag_x,
            ud.homotopy_lambda_ptr,
        )
    };

    if !color_groups_ptr.is_null() {
        let groups = &*color_groups_ptr;
        for group in groups {
            std::ptr::copy_nonoverlapping(y_ptr, t1_ptr, n);
            let mut deltas: Vec<f64> = Vec::with_capacity(group.len());
            for &j in group {
                let yj = *y_ptr.add(j);
                let delta = eps.max(eps * yj.abs());
                *t1_ptr.add(j) = yj + delta;
                deltas.push(delta);
            }

            let status = evaluate_per_group(t1_ptr, t2_ptr);
            if status != 0 {
                return status;
            }

            for (&j, &delta) in group.iter().zip(deltas.iter()) {
                for i in 0..n {
                    let df = (*t2_ptr.add(i) - *ud.work_deriv.add(i)) / delta;
                    let mut val = -df;
                    if i == j {
                        val += cj;
                    }
                    *jac_data.add(j * n + i) = val;
                }
            }
        }
    } else {
        // Per-column fallback
        for j in 0..n {
            std::ptr::copy_nonoverlapping(y_ptr, t1_ptr, n);
            let yj = *y_ptr.add(j);
            let delta = eps.max(eps * yj.abs());
            *t1_ptr.add(j) = yj + delta;

            let status = evaluate_per_group(t1_ptr, t2_ptr);
            if status != 0 {
                return status;
            }

            for i in 0..n {
                let df = (*t2_ptr.add(i) - *ud.work_deriv.add(i)) / delta;
                let mut val = -df;
                if i == j {
                    val += cj;
                }
                *jac_data.add(j * n + i) = val;
            }
        }
    }

    sundials_sys::IDA_SUCCESS as i32
}

// ── SPGMR Jacobian-vector product callbacks ────────────────────────────────
// SPGMR is a Krylov solver that needs J*v products, not the full matrix.
// J*v ≈ (f(y + σ*v) − f(y)) / σ  via finite differences.
// σ = 1 / max(1/√ε, ||v||₂) — the standard SUNDIALS formula.

/// SPGMR setup callback — no-op (nothing to precompute).
unsafe extern "C" fn cv_jtimes_setup(
    _t: sunrealtype,
    _y: N_Vector,
    _fy: N_Vector,
    _user_data: *mut c_void,
) -> c_int {
    0
}

/// SPGMR Jacobian-vector product for CVODE: Jv ≈ (f(y+σv) − f(y)) / σ.
unsafe extern "C" fn cv_jtimes(
    v: N_Vector,
    Jv: N_Vector,
    t: sunrealtype,
    y: N_Vector,
    fy: N_Vector,
    user_data: *mut c_void,
    _tmp: N_Vector,
) -> c_int {
    let ud = &*(user_data as *const RawSundialsUserData);
    let n = ud.n;
    let y_ptr = N_VGetArrayPointer(y);
    let fy_ptr = N_VGetArrayPointer(fy);
    let v_ptr = N_VGetArrayPointer(v);
    let jv_ptr = N_VGetArrayPointer(Jv);

    let eps_sqrt = (f64::EPSILON as f64).sqrt(); // ~1.49e-8

    let mut vnrm = 0.0_f64;
    for i in 0..n {
        vnrm += (*v_ptr.add(i)).powi(2);
    }
    vnrm = vnrm.sqrt();

    let sigma = if vnrm > eps_sqrt {
        1.0_f64 / vnrm.max(1.0 / eps_sqrt)
    } else {
        1.0_f64
    };

    // y + σ·v → work_state
    for i in 0..n {
        *ud.work_state.add(i) = *y_ptr.add(i) + sigma * (*v_ptr.add(i));
    }

    let status = (ud.calc_derivs)(
        t as f64,
        ud.work_state,
        ud.discrete,
        ud.work_deriv,
        ud.params,
        ud.outputs,
        ud.scratch_when_states,
        ud.scratch_crossings,
        ud.pre_states,
        ud.pre_discrete,
        ud.t_end,
        ud.diag_residual,
        ud.diag_x,
        ud.homotopy_lambda_ptr,
    );
    if status != 0 {
        return status;
    }

    // Jv = (f(y+σv) − f(y)) / σ
    for i in 0..n {
        *jv_ptr.add(i) = (*ud.work_deriv.add(i) - *fy_ptr.add(i)) / sigma;
    }

    sundials_sys::CV_SUCCESS as i32
}

/// SPGMR setup callback for IDA — no-op.
unsafe extern "C" fn ida_jtimes_setup(
    _tt: sunrealtype,
    _yy: N_Vector,
    _yp: N_Vector,
    _rr: N_Vector,
    _c_j: sunrealtype,
    _user_data: *mut c_void,
) -> c_int {
    0
}

/// SPGMR Jacobian-vector product for IDA: Jv = −(f(y+σv)−f(y))/σ + c_j·v.
unsafe extern "C" fn ida_jtimes(
    _tt: sunrealtype,
    _yy: N_Vector,
    _yp: N_Vector,
    _rr: N_Vector,
    v: N_Vector,
    Jv: N_Vector,
    c_j: sunrealtype,
    user_data: *mut c_void,
    tmp1: N_Vector,
    _tmp2: N_Vector,
) -> c_int {
    let ud = &*(user_data as *const RawSundialsUserData);
    let n = ud.n;
    let y_ptr = N_VGetArrayPointer(_yy);
    let v_ptr = N_VGetArrayPointer(v);
    let jv_ptr = N_VGetArrayPointer(Jv);
    let scratch_ptr = N_VGetArrayPointer(tmp1); // for saving baseline

    let eps_sqrt = (f64::EPSILON as f64).sqrt();
    let mut vnrm = 0.0_f64;
    for i in 0..n {
        vnrm += (*v_ptr.add(i)).powi(2);
    }
    vnrm = vnrm.sqrt();

    let sigma = if vnrm > eps_sqrt {
        1.0_f64 / vnrm.max(1.0 / eps_sqrt)
    } else {
        1.0_f64
    };

    // Compute baseline f(y) into work_deriv, then save to scratch
    std::ptr::copy_nonoverlapping(y_ptr, ud.work_state, n);
    let st = (ud.calc_derivs)(
        _tt as f64,
        ud.work_state,
        ud.discrete,
        ud.work_deriv,
        ud.params,
        ud.outputs,
        ud.scratch_when_states,
        ud.scratch_crossings,
        ud.pre_states,
        ud.pre_discrete,
        ud.t_end,
        ud.diag_residual,
        ud.diag_x,
        ud.homotopy_lambda_ptr,
    );
    if st != 0 {
        return st;
    }
    // Save baseline to scratch buffer before overwriting work_deriv
    std::ptr::copy_nonoverlapping(ud.work_deriv, scratch_ptr, n);

    // y + σ·v → work_state
    for i in 0..n {
        *ud.work_state.add(i) = *y_ptr.add(i) + sigma * (*v_ptr.add(i));
    }

    let status = (ud.calc_derivs)(
        _tt as f64,
        ud.work_state,
        ud.discrete,
        ud.work_deriv, // now holds f(y+σv)
        ud.params,
        ud.outputs,
        ud.scratch_when_states,
        ud.scratch_crossings,
        ud.pre_states,
        ud.pre_discrete,
        ud.t_end,
        ud.diag_residual,
        ud.diag_x,
        ud.homotopy_lambda_ptr,
    );
    if status != 0 {
        return status;
    }

    // Jv = −(f(y+σv) − f(y)) / σ + c_j·v
    let cj = c_j as f64;
    for i in 0..n {
        let df = (*ud.work_deriv.add(i) - *scratch_ptr.add(i)) / sigma;
        *jv_ptr.add(i) = -df + cj * (*v_ptr.add(i));
    }

    sundials_sys::IDA_SUCCESS as i32
}

// ── KLU sparse Jacobian callbacks ──────────────────────────────────────────
// KLU receives a CSR sparse Jacobian. We fill the dense template first,
// then copy non-zero values into the CSR arrays.

/// Copy column-major dense matrix to CSR sparse matrix.
/// `dense` is column-major: element (row i, col j) at dense[j * n + i].
unsafe fn dense_col_major_to_sparse_csr(dense: *const f64, n: usize, sparse: SUNMatrix) {
    let data = SUNSparseMatrix_Data(sparse);
    let col_idx = SUNSparseMatrix_IndexValues(sparse);
    let row_ptr = SUNSparseMatrix_IndexPointers(sparse);
    if data.is_null() || col_idx.is_null() || row_ptr.is_null() {
        return;
    }

    for i in 0..n {
        let start = *row_ptr.add(i) as usize;
        let end = *row_ptr.add(i + 1) as usize;
        for k in start..end {
            let j = *col_idx.add(k) as usize;
            *data.add(k) = *dense.add(j * n + i);
        }
    }
}

/// KLU sparse Jacobian callback for CVODE.
/// Fills the dense template (stored in user_data), then copies to sparse CSR.
unsafe extern "C" fn cv_jac_klu(
    t: sunrealtype,
    y: N_Vector,
    fy: N_Vector,
    Jac: SUNMatrix,
    user_data: *mut c_void,
    tmp1: N_Vector,
    tmp2: N_Vector,
    _tmp3: N_Vector,
) -> c_int {
    let ud = &*(user_data as *const RawSundialsUserData);
    let n = ud.n;
    if ud.jacobian_dense.is_null() {
        return -1;
    }

    // Fill the dense template first (same algorithm as cv_jac)
    let dense_data = ud.jacobian_dense as *mut f64;
    let y_ptr = N_VGetArrayPointer(y);
    let fy_ptr = N_VGetArrayPointer(fy);
    let t1_ptr = N_VGetArrayPointer(tmp1);
    let t2_ptr = N_VGetArrayPointer(tmp2);

    let eps = 1e-6_f64;
    for j in 0..n {
        std::ptr::copy_nonoverlapping(y_ptr, t1_ptr, n);
        let yj = *y_ptr.add(j);
        let delta = eps.max(eps * yj.abs());
        *t1_ptr.add(j) = yj + delta;

        let status = (ud.calc_derivs)(
            t as f64,
            t1_ptr,
            ud.discrete,
            t2_ptr,
            ud.params,
            ud.outputs,
            ud.scratch_when_states,
            ud.scratch_crossings,
            ud.pre_states,
            ud.pre_discrete,
            ud.t_end,
            ud.diag_residual,
            ud.diag_x,
            ud.homotopy_lambda_ptr,
        );
        if status != 0 {
            return status;
        }

        for i in 0..n {
            *dense_data.add(j * n + i) = (*t2_ptr.add(i) - *fy_ptr.add(i)) / delta;
        }
    }

    // Copy dense → sparse CSR
    dense_col_major_to_sparse_csr(dense_data, n, Jac);

    sundials_sys::CV_SUCCESS as i32
}

/// KLU sparse Jacobian callback for IDA. J = −df/dy + c_j·I.
unsafe extern "C" fn ida_jac_klu(
    t: sunrealtype,
    c_j: sunrealtype,
    y: N_Vector,
    _yp: N_Vector,
    _r: N_Vector,
    Jac: SUNMatrix,
    user_data: *mut c_void,
    tmp1: N_Vector,
    tmp2: N_Vector,
    _tmp3: N_Vector,
) -> c_int {
    let ud = &*(user_data as *const RawSundialsUserData);
    let n = ud.n;
    if ud.jacobian_dense.is_null() {
        return -1;
    }

    let dense_data = ud.jacobian_dense as *mut f64;
    let y_ptr = N_VGetArrayPointer(y);
    let t1_ptr = N_VGetArrayPointer(tmp1);
    let t2_ptr = N_VGetArrayPointer(tmp2);

    // Compute baseline f(y)
    std::ptr::copy_nonoverlapping(y_ptr, ud.work_state, n);
    let st = (ud.calc_derivs)(
        t as f64,
        ud.work_state,
        ud.discrete,
        ud.work_deriv,
        ud.params,
        ud.outputs,
        ud.scratch_when_states,
        ud.scratch_crossings,
        ud.pre_states,
        ud.pre_discrete,
        ud.t_end,
        ud.diag_residual,
        ud.diag_x,
        ud.homotopy_lambda_ptr,
    );
    if st != 0 {
        return st;
    }

    let eps = 1e-6_f64;
    let cj = c_j as f64;

    for j in 0..n {
        std::ptr::copy_nonoverlapping(y_ptr, t1_ptr, n);
        let yj = *y_ptr.add(j);
        let delta = eps.max(eps * yj.abs());
        *t1_ptr.add(j) = yj + delta;

        let status = (ud.calc_derivs)(
            t as f64,
            t1_ptr,
            ud.discrete,
            t2_ptr,
            ud.params,
            ud.outputs,
            ud.scratch_when_states,
            ud.scratch_crossings,
            ud.pre_states,
            ud.pre_discrete,
            ud.t_end,
            ud.diag_residual,
            ud.diag_x,
            ud.homotopy_lambda_ptr,
        );
        if status != 0 {
            return status;
        }

        for i in 0..n {
            let df = (*t2_ptr.add(i) - *ud.work_deriv.add(i)) / delta;
            let mut val = -df;
            if i == j {
                val += cj;
            }
            *dense_data.add(j * n + i) = val;
        }
    }

    dense_col_major_to_sparse_csr(dense_data, n, Jac);

    sundials_sys::IDA_SUCCESS as i32
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_with_cvode(
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
    state_var_index: &std::collections::HashMap<String, usize>,
    t_end: f64,
    dt: f64,
    numeric_ode_jacobian: bool,
    symbolic_ode_jacobian: Option<&Vec<Vec<Expression>>>,
    newton_tearing_var_names: &[String],
    atol: f64,
    rtol: f64,
    output_interval: f64,
    result_file: Option<&str>,
    mut result_collector: Option<&mut ResultCollector>,
) -> Result<(), String> {
    run_sundials_common(
        SundialsKind::Cvode,
        calc_derivs,
        when_count,
        crossings_count,
        &mut states,
        &mut discrete_vals,
        &params,
        state_vars,
        discrete_vars,
        output_vars,
        output_start_vals,
        state_var_index,
        t_end,
        dt,
        numeric_ode_jacobian,
        symbolic_ode_jacobian,
        newton_tearing_var_names,
        atol,
        rtol,
        1,
        &[],
        output_interval,
        result_file,
        &mut result_collector,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_with_ida(
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
    state_var_index: &std::collections::HashMap<String, usize>,
    t_end: f64,
    dt: f64,
    numeric_ode_jacobian: bool,
    symbolic_ode_jacobian: Option<&Vec<Vec<Expression>>>,
    newton_tearing_var_names: &[String],
    atol: f64,
    rtol: f64,
    differential_index: u32,
    ida_component_id: &[f64],
    output_interval: f64,
    result_file: Option<&str>,
    mut result_collector: Option<&mut ResultCollector>,
) -> Result<(), String> {
    let allow_high_index = std::env::var("RUSTMODLICA_IDA_ALLOW_HIGH_INDEX")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE"))
        .unwrap_or(false);
    if differential_index > 1 && !allow_high_index {
        return Err(format!(
            "solver ida requires differential index <= 1 (got {}), set RUSTMODLICA_IDA_ALLOW_HIGH_INDEX=1 to force run",
            differential_index
        ));
    }
    if ida_component_id.len() != states.len() {
        return Err(format!(
            "solver ida: id vector length {} must match state count {}",
            ida_component_id.len(),
            states.len()
        ));
    }
    run_sundials_common(
        SundialsKind::Ida,
        calc_derivs,
        when_count,
        crossings_count,
        &mut states,
        &mut discrete_vals,
        &params,
        state_vars,
        discrete_vars,
        output_vars,
        output_start_vals,
        state_var_index,
        t_end,
        dt,
        numeric_ode_jacobian,
        symbolic_ode_jacobian,
        newton_tearing_var_names,
        atol,
        rtol,
        differential_index,
        ida_component_id,
        output_interval,
        result_file,
        &mut result_collector,
    )
}
