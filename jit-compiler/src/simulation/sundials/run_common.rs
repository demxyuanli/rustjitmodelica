use super::*;

mod drive_loop;

use drive_loop::drive_print_loop;

enum SundialsKind {
    Cvode,
    Ida,
}

#[allow(clippy::too_many_arguments)]
fn run_sundials_common(
    kind: SundialsKind,
    calc_derivs: CalcDerivsFunc,
    when_count: usize,
    crossings_count: usize,
    states: &mut Vec<f64>,
    discrete_vals: &mut Vec<f64>,
    params: &[f64],
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
    _differential_index: u32,
    ida_component_id: &[f64],
    output_interval: f64,
    result_file: Option<&str>,
    result_collector: &mut Option<&mut ResultCollector>,
) -> Result<(), String> {
    let rt_cfg = SundialsRuntimeConfig::from_env();
    if std::env::var("RUSTMODLICA_SUNDIALS_TRACE_CONFIG")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE"))
        .unwrap_or(false)
    {
        eprintln!(
            "[sundials-config] max_order={:?} max_nonlin_iters={:?} max_step={:?}",
            rt_cfg.max_order, rt_cfg.max_nonlin_iters, rt_cfg.max_step
        );
    }
    warn_if_unsupported_backend_requested();
    let n = states.len();
    if n == 0 {
        return Err("sundials: empty state vector".to_string());
    }
    let mut derivs = vec![0.0_f64; n];
    let mut outputs = if output_start_vals.len() == output_vars.len() {
        output_start_vals.to_vec()
    } else {
        vec![0.0; output_vars.len()]
    };
    let mut when_states = vec![0.0_f64; when_count * 2];
    let mut crossings = vec![0.0_f64; crossings_count];
    let mut pre_states = vec![0.0_f64; n];
    let mut pre_discrete_vals = vec![0.0_f64; discrete_vals.len()];
    let mut homotopy_lambda: f64 = 1.0;
    let homotopy_lambda_ptr: *const f64 = &homotopy_lambda;

    let mut out: Box<dyn Write> = if result_collector.is_some() {
        Box::new(io::sink())
    } else if let Some(path) = result_file {
        let f = File::create(path)
            .map_err(|e| format!("Failed to create result file {}: {}", path, e))?;
        Box::new(std::io::BufWriter::new(f))
    } else {
        Box::new(std::io::BufWriter::new(io::stdout()))
    };
    let w = &mut out;

    let est_cols = 1 + state_vars.len() + discrete_vars.len() + output_vars.len();
    let mut header = String::with_capacity(est_cols * 16);
    header.push_str(i18n::msg0("time"));
    for var in state_vars {
        write!(&mut header, ", {}", var).map_err(|e| e.to_string())?;
    }
    for var in discrete_vars {
        write!(&mut header, ", {}", var).map_err(|e| e.to_string())?;
    }
    for var in output_vars {
        write!(&mut header, ", {}", var).map_err(|e| e.to_string())?;
    }
    write_csv_line(w, &header)?;
    flush_writer(w)?;

    let mut csv_row = String::with_capacity(est_cols * 24);
    let mut rows_since_flush: u32 = 0;
    let print_interval = output_interval;
    let mut next_print = 0.0_f64;
    let epsilon = 1e-5_f64;

    let mut diag_residual = 0.0_f64;
    let mut diag_x = 0.0_f64;
    let (diag_res_ptr, diag_x_ptr) = if newton_tearing_var_names.is_empty() {
        (ptr::null_mut(), ptr::null_mut())
    } else {
        (&mut diag_residual as *mut f64, &mut diag_x as *mut f64)
    };
    let mut diag_call_index = 0u32;
    let mut diag_time = 0.0_f64;
    let mut diag_state = vec![0.0_f64; n];
    let (eval_call_index_ptr, last_eval_time_ptr, last_eval_state_ptr, last_eval_state_len) =
        if newton_tearing_var_names.is_empty() {
            (ptr::null_mut(), ptr::null_mut(), ptr::null_mut(), 0)
        } else {
            (
                &mut diag_call_index as *mut u32,
                &mut diag_time as *mut f64,
                diag_state.as_mut_ptr(),
                diag_state.len(),
            )
        };

    let mut prev_outputs = vec![0.0_f64; output_vars.len()];

    let mut work_state = vec![0.0_f64; n];
    let mut work_deriv = vec![0.0_f64; n];
    let mut scratch_when_states = vec![0.0_f64; when_count * 2];
    let mut scratch_crossings = vec![0.0_f64; crossings_count];
    let ud = RawSundialsUserData {
        calc_derivs,
        params: params.as_ptr(),
        discrete: discrete_vals.as_mut_ptr(),
        outputs: outputs.as_mut_ptr(),
        when_states: when_states.as_mut_ptr(),
        crossings: crossings.as_mut_ptr(),
        pre_states: pre_states.as_ptr(),
        pre_discrete: pre_discrete_vals.as_ptr(),
        t_end,
        diag_residual: diag_res_ptr,
        diag_x: diag_x_ptr,
        homotopy_lambda_ptr,
        n,
        work_state: work_state.as_mut_ptr(),
        work_deriv: work_deriv.as_mut_ptr(),
        crossings_len: crossings_count,
        scratch_when_states: scratch_when_states.as_mut_ptr(),
        scratch_crossings: scratch_crossings.as_mut_ptr(),
    };
    let ud_ptr = Box::into_raw(Box::new(ud));

    native::reset_terminate_flag();
    native::reset_assert_counter();

    unsafe {
        let mut ctx = ptr::null_mut();
        if SUNContext_Create(comm_no_mpi(), &mut ctx) < 0 {
            drop(Box::from_raw(ud_ptr));
            return Err("SUNContext_Create failed".to_string());
        }

        let y = N_VMake_Serial(n as sunindextype, states.as_mut_ptr(), ctx);
        if y.is_null() {
            SUNContext_Free(&mut ctx);
            drop(Box::from_raw(ud_ptr));
            return Err("N_VMake_Serial(y) failed".to_string());
        }

        let linsol_kind = parse_linsol_env(n);
        let ls_pack = match attach_for_cvode_ida(y, ctx, n as sunindextype, linsol_kind) {
            Ok(p) => p,
            Err(e) => {
                N_VDestroy(y);
                SUNContext_Free(&mut ctx);
                drop(Box::from_raw(ud_ptr));
                return Err(e);
            }
        };

        let mut tret = 0.0_f64;
        let t0 = 0.0 as sunrealtype;
        match kind {
            SundialsKind::Cvode => {
                let mut mem = CVodeCreate(CV_BDF, ctx);
                if mem.is_null() {
                    drop(ls_pack);
                    N_VDestroy(y);
                    SUNContext_Free(&mut ctx);
                    drop(Box::from_raw(ud_ptr));
                    return Err("CVodeCreate failed".to_string());
                }
                let ir = CVodeInit(mem, Some(cv_rhs), t0, y);
                if ir != CV_SUCCESS as i32 {
                    drop(ls_pack);
                    CVodeFree(&mut mem);
                    N_VDestroy(y);
                    SUNContext_Free(&mut ctx);
                    drop(Box::from_raw(ud_ptr));
                    return Err(format!("CVodeInit failed: {}", ir));
                }
                let tr = CVodeSStolerances(mem, rtol as sunrealtype, atol as sunrealtype);
                if tr != CV_SUCCESS as i32 {
                    drop(ls_pack);
                    CVodeFree(&mut mem);
                    N_VDestroy(y);
                    SUNContext_Free(&mut ctx);
                    drop(Box::from_raw(ud_ptr));
                    return Err(format!("CVodeSStolerances failed: {}", tr));
                }
                CVodeSetUserData(mem, ud_ptr as *mut c_void);
                if let Some(max_ord) = rt_cfg.max_order {
                    let _ = sundials_sys::CVodeSetMaxOrd(mem, max_ord);
                }
                if let Some(max_step) = rt_cfg.max_step {
                    let _ = sundials_sys::CVodeSetMaxStep(mem, max_step as sunrealtype);
                }
                let lr = CVodeSetLinearSolver(mem, ls_pack.linsol, ls_pack.jacobian);
                if lr != sundials_sys::CVLS_SUCCESS as i32 {
                    drop(ls_pack);
                    CVodeFree(&mut mem);
                    N_VDestroy(y);
                    SUNContext_Free(&mut ctx);
                    drop(Box::from_raw(ud_ptr));
                    return Err(format!("CVodeSetLinearSolver failed: {}", lr));
                }
                if crossings_count > 0 {
                    let rr = CVodeRootInit(mem, crossings_count as i32, Some(cv_root));
                    if rr != CV_SUCCESS as i32 {
                        drop(ls_pack);
                        CVodeFree(&mut mem);
                        N_VDestroy(y);
                        SUNContext_Free(&mut ctx);
                        drop(Box::from_raw(ud_ptr));
                        return Err(format!("CVodeRootInit failed: {}", rr));
                    }
                }

                let r = drive_print_loop(
                    SundialsKind::Cvode,
                    mem,
                    y,
                    ptr::null_mut(),
                    &mut tret,
                    calc_derivs,
                    when_count,
                    states,
                    discrete_vals,
                    &mut derivs,
                    params,
                    &mut outputs,
                    &mut when_states,
                    &mut crossings,
                    &mut pre_states,
                    &mut pre_discrete_vals,
                    &mut homotopy_lambda,
                    homotopy_lambda_ptr,
                    newton_tearing_var_names,
                    output_start_vals,
                    output_vars,
                    discrete_vars,
                    state_vars,
                    state_var_index,
                    t_end,
                    dt,
                    numeric_ode_jacobian,
                    symbolic_ode_jacobian,
                    &mut diag_residual,
                    &mut diag_x,
                    diag_res_ptr,
                    diag_x_ptr,
                    &mut diag_call_index,
                    &mut diag_time,
                    &mut diag_state,
                    eval_call_index_ptr,
                    last_eval_time_ptr,
                    last_eval_state_ptr,
                    last_eval_state_len,
                    &mut prev_outputs,
                    w,
                    &mut csv_row,
                    &mut rows_since_flush,
                    print_interval,
                    &mut next_print,
                    epsilon,
                    result_collector,
                );
                drop(ls_pack);
                CVodeFree(&mut mem);
                if let Err(e) = r {
                    N_VDestroy(y);
                    SUNContext_Free(&mut ctx);
                    drop(Box::from_raw(ud_ptr));
                    return Err(e);
                }
            }
            SundialsKind::Ida => {
                let yp = N_VNew_Serial(n as sunindextype, ctx);
                if yp.is_null() {
                    drop(ls_pack);
                    N_VDestroy(y);
                    SUNContext_Free(&mut ctx);
                    drop(Box::from_raw(ud_ptr));
                    return Err("N_VNew_Serial(yp) failed".to_string());
                }
                native::suppress_assert_begin();
                let st0 = (calc_derivs)(
                    0.0,
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
                if st0 != 0 {
                    drop(ls_pack);
                    N_VDestroy(yp);
                    N_VDestroy(y);
                    SUNContext_Free(&mut ctx);
                    drop(Box::from_raw(ud_ptr));
                    return Err(format!("IDA: initial calc_derivs failed: {}", st0));
                }
                ptr::copy_nonoverlapping(derivs.as_ptr(), N_VGetArrayPointer(yp), n);

                let id_nv = N_VNew_Serial(n as sunindextype, ctx);
                if id_nv.is_null() {
                    drop(ls_pack);
                    N_VDestroy(yp);
                    N_VDestroy(y);
                    SUNContext_Free(&mut ctx);
                    drop(Box::from_raw(ud_ptr));
                    return Err("N_VNew_Serial(id) failed".to_string());
                }
                ptr::copy_nonoverlapping(
                    ida_component_id.as_ptr(),
                    N_VGetArrayPointer(id_nv),
                    n,
                );

                let mut mem = IDACreate(ctx);
                if mem.is_null() {
                    drop(ls_pack);
                    N_VDestroy(id_nv);
                    N_VDestroy(yp);
                    N_VDestroy(y);
                    SUNContext_Free(&mut ctx);
                    drop(Box::from_raw(ud_ptr));
                    return Err("IDACreate failed".to_string());
                }
                let ir = IDAInit(mem, Some(ida_res), t0, y, yp);
                if ir != IDA_SUCCESS as i32 {
                    drop(ls_pack);
                    IDAFree(&mut mem);
                    N_VDestroy(id_nv);
                    N_VDestroy(yp);
                    N_VDestroy(y);
                    SUNContext_Free(&mut ctx);
                    drop(Box::from_raw(ud_ptr));
                    return Err(format!("IDAInit failed: {}", ir));
                }
                let idr = IDASetId(mem, id_nv);
                if idr != IDA_SUCCESS as i32 {
                    drop(ls_pack);
                    IDAFree(&mut mem);
                    N_VDestroy(id_nv);
                    N_VDestroy(yp);
                    N_VDestroy(y);
                    SUNContext_Free(&mut ctx);
                    drop(Box::from_raw(ud_ptr));
                    return Err(format!("IDASetId failed: {}", idr));
                }
                let tr = IDASStolerances(mem, rtol as sunrealtype, atol as sunrealtype);
                if tr != IDA_SUCCESS as i32 {
                    drop(ls_pack);
                    IDAFree(&mut mem);
                    N_VDestroy(id_nv);
                    N_VDestroy(yp);
                    N_VDestroy(y);
                    SUNContext_Free(&mut ctx);
                    drop(Box::from_raw(ud_ptr));
                    return Err(format!("IDASStolerances failed: {}", tr));
                }
                IDASetUserData(mem, ud_ptr as *mut c_void);
                if let Some(max_ord) = rt_cfg.max_order {
                    let _ = sundials_sys::IDASetMaxOrd(mem, max_ord);
                }
                if let Some(max_step) = rt_cfg.max_step {
                    let _ = sundials_sys::IDASetMaxStep(mem, max_step as sunrealtype);
                }
                let lr = IDASetLinearSolver(mem, ls_pack.linsol, ls_pack.jacobian);
                if lr != sundials_sys::IDALS_SUCCESS as i32 {
                    drop(ls_pack);
                    IDAFree(&mut mem);
                    N_VDestroy(id_nv);
                    N_VDestroy(yp);
                    N_VDestroy(y);
                    SUNContext_Free(&mut ctx);
                    drop(Box::from_raw(ud_ptr));
                    return Err(format!("IDASetLinearSolver failed: {}", lr));
                }
                if crossings_count > 0 {
                    let rr = IDARootInit(mem, crossings_count as i32, Some(ida_root));
                    if rr != IDA_SUCCESS as i32 {
                        drop(ls_pack);
                        IDAFree(&mut mem);
                        N_VDestroy(id_nv);
                        N_VDestroy(yp);
                        N_VDestroy(y);
                        SUNContext_Free(&mut ctx);
                        drop(Box::from_raw(ud_ptr));
                        return Err(format!("IDARootInit failed: {}", rr));
                    }
                }
                let ic_tout = (t0 as f64 + dt.min(1e-6).max(1e-9)) as sunrealtype;
                let icr = IDACalcIC(mem, IDA_YA_YDP_INIT as i32, ic_tout);
                if icr != IDA_SUCCESS as i32 {
                    let _ = icr;
                }

                let r = drive_print_loop(
                    SundialsKind::Ida,
                    mem,
                    y,
                    yp,
                    &mut tret,
                    calc_derivs,
                    when_count,
                    states,
                    discrete_vals,
                    &mut derivs,
                    params,
                    &mut outputs,
                    &mut when_states,
                    &mut crossings,
                    &mut pre_states,
                    &mut pre_discrete_vals,
                    &mut homotopy_lambda,
                    homotopy_lambda_ptr,
                    newton_tearing_var_names,
                    output_start_vals,
                    output_vars,
                    discrete_vars,
                    state_vars,
                    state_var_index,
                    t_end,
                    dt,
                    numeric_ode_jacobian,
                    symbolic_ode_jacobian,
                    &mut diag_residual,
                    &mut diag_x,
                    diag_res_ptr,
                    diag_x_ptr,
                    &mut diag_call_index,
                    &mut diag_time,
                    &mut diag_state,
                    eval_call_index_ptr,
                    last_eval_time_ptr,
                    last_eval_state_ptr,
                    last_eval_state_len,
                    &mut prev_outputs,
                    w,
                    &mut csv_row,
                    &mut rows_since_flush,
                    print_interval,
                    &mut next_print,
                    epsilon,
                    result_collector,
                );
                drop(ls_pack);
                IDAFree(&mut mem);
                N_VDestroy(id_nv);
                N_VDestroy(yp);
                if let Err(e) = r {
                    N_VDestroy(y);
                    SUNContext_Free(&mut ctx);
                    drop(Box::from_raw(ud_ptr));
                    return Err(e);
                }
            }
        }

        N_VDestroy(y);
        SUNContext_Free(&mut ctx);
        drop(Box::from_raw(ud_ptr));
    }

    flush_writer(w)?;
    Ok(())
}
