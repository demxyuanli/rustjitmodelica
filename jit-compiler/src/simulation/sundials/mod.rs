mod kinsol;
mod linsol;

pub use kinsol::{kinsol_solve_square_spgmr, KinResidualFn, KinsolCallbackPack};
pub use linsol::{parse_linsol_env, warn_if_unsupported_backend_requested, SundialsLinSolKind};

use std::ffi::c_void;
use std::fmt::Write as FmtWrite;
use std::fs::File;
use std::io::{self, Write};
use std::ptr;

use sundials_sys::{
    comm_no_mpi, sunindextype, sunrealtype, CVode, CVodeCreate, CVodeFree, CVodeInit,
    CVodeGetRootInfo, CVodeReInit, CVodeRootInit, CVodeSetLinearSolver, CVodeSetUserData,
    CVodeSStolerances, IDAGetRootInfo, IDAInit, IDAReInit, IDARootInit, IDASetId,
    IDASetLinearSolver, IDASetUserData, IDACreate, IDAFree, IDASStolerances, IDASolve, N_VDestroy,
    N_VGetArrayPointer, N_VMake_Serial, N_VNew_Serial, SUNContext_Create, SUNContext_Free, CV_BDF,
    CV_NORMAL, CV_ROOT_RETURN, CV_SUCCESS, IDA_ROOT_RETURN, IDA_SUCCESS, IDA_NORMAL,
    IDA_YA_YDP_INIT, IDACalcIC, N_Vector,
};

use crate::ast::Expression;
use crate::i18n;
use crate::jit::native;
use crate::jit::CalcDerivsFunc;
use super::events::{run_event_iteration_at_time, EventIterationOutcome};
use super::newton_recovery::{fail_if_assert_storm, print_newton_diag};
use super::sim_io::{flush_writer, write_csv_line};
use super::step::maybe_print_numeric_jacobian;
use super::types::{
    EventDebounceConfig, EventQueue, QueuedEvent, QueuedEventKind, ResultCollector,
    SundialsRuntimeConfig,
};

use linsol::attach_for_cvode_ida;

const CSV_ROWS_PER_FLUSH: u32 = 64;

fn env_f64(name: &str) -> Option<f64> {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite())
}

fn env_u32(name: &str) -> Option<u32> {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
}

fn sundials_event_log_enabled() -> bool {
    std::env::var("RUSTMODLICA_SUNDIALS_EVENT_LOG")
        .ok()
        .map(|v| {
            let t = v.trim().to_ascii_lowercase();
            !(t == "0" || t == "false" || t == "off" || t == "no")
        })
        .unwrap_or(true)
}

#[repr(C)]
struct RawSundialsUserData {
    calc_derivs: CalcDerivsFunc,
    params: *const f64,
    discrete: *mut f64,
    outputs: *mut f64,
    when_states: *mut f64,
    crossings: *mut f64,
    pre_states: *const f64,
    pre_discrete: *const f64,
    t_end: f64,
    diag_residual: *mut f64,
    diag_x: *mut f64,
    homotopy_lambda_ptr: *const f64,
    n: usize,
    work_state: *mut f64,
    work_deriv: *mut f64,
    crossings_len: usize,
    scratch_when_states: *mut f64,
    scratch_crossings: *mut f64,
}

unsafe extern "C" fn cv_rhs(
    t: sunrealtype,
    y: N_Vector,
    ydot: N_Vector,
    user_data: *mut c_void,
) -> i32 {
    let ud = &*(user_data as *const RawSundialsUserData);
    let n = ud.n;
    let yp = N_VGetArrayPointer(y);
    let dyp = N_VGetArrayPointer(ydot);
    std::ptr::copy_nonoverlapping(yp, ud.work_state, n);
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
    std::ptr::copy_nonoverlapping(ud.work_deriv, dyp, n);
    sundials_sys::CV_SUCCESS as i32
}

unsafe extern "C" fn ida_res(
    t: sunrealtype,
    y: N_Vector,
    yp: N_Vector,
    res: N_Vector,
    user_data: *mut c_void,
) -> i32 {
    let ud = &*(user_data as *const RawSundialsUserData);
    let n = ud.n;
    let yy = N_VGetArrayPointer(y);
    let ypp = N_VGetArrayPointer(yp);
    let rr = N_VGetArrayPointer(res);
    std::ptr::copy_nonoverlapping(yy, ud.work_state, n);
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
    for i in 0..n {
        *rr.add(i) = *ypp.add(i) - *ud.work_deriv.add(i);
    }
    sundials_sys::IDA_SUCCESS as i32
}

unsafe extern "C" fn cv_root(
    t: sunrealtype,
    y: N_Vector,
    gout: *mut sunrealtype,
    user_data: *mut c_void,
) -> i32 {
    let ud = &*(user_data as *const RawSundialsUserData);
    if ud.crossings_len == 0 {
        return sundials_sys::CV_SUCCESS as i32;
    }
    let n = ud.n;
    let y_ptr = N_VGetArrayPointer(y);
    if y_ptr.is_null() {
        return -1;
    }
    std::ptr::copy_nonoverlapping(y_ptr, ud.work_state, n);
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
    for i in 0..ud.crossings_len {
        *gout.add(i) = *ud.scratch_crossings.add(i);
    }
    sundials_sys::CV_SUCCESS as i32
}

unsafe extern "C" fn ida_root(
    t: sunrealtype,
    y: N_Vector,
    yp: N_Vector,
    gout: *mut sunrealtype,
    user_data: *mut c_void,
) -> i32 {
    let _ = yp;
    let ud = &*(user_data as *const RawSundialsUserData);
    if ud.crossings_len == 0 {
        return sundials_sys::IDA_SUCCESS as i32;
    }
    let n = ud.n;
    let y_ptr = N_VGetArrayPointer(y);
    if y_ptr.is_null() {
        return -1;
    }
    std::ptr::copy_nonoverlapping(y_ptr, ud.work_state, n);
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
    for i in 0..ud.crossings_len {
        *gout.add(i) = *ud.scratch_crossings.add(i);
    }
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
    if ida_component_id.iter().any(|&v| v == 0.0) {
        return Err(
            "solver ida: algebraic state components (IDASetId 0) are not supported in this JIT layout"
                .to_string(),
        );
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

#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
unsafe fn drive_print_loop(
    kind: SundialsKind,
    mem: *mut c_void,
    y: N_Vector,
    yp: N_Vector,
    tret: &mut f64,
    calc_derivs: CalcDerivsFunc,
    when_count: usize,
    states: &mut Vec<f64>,
    discrete_vals: &mut Vec<f64>,
    derivs: &mut [f64],
    params: &[f64],
    outputs: &mut [f64],
    when_states: &mut [f64],
    crossings: &mut [f64],
    pre_states: &mut [f64],
    pre_discrete_vals: &mut [f64],
    homotopy_lambda: &mut f64,
    homotopy_lambda_ptr: *const f64,
    newton_tearing_var_names: &[String],
    output_start_vals: &[f64],
    output_vars: &[String],
    _discrete_vars: &[String],
    _state_vars: &[String],
    state_var_index: &std::collections::HashMap<String, usize>,
    t_end: f64,
    dt: f64,
    numeric_ode_jacobian: bool,
    symbolic_ode_jacobian: Option<&Vec<Vec<Expression>>>,
    diag_residual: &mut f64,
    diag_x: &mut f64,
    diag_res_ptr: *mut f64,
    diag_x_ptr: *mut f64,
    diag_call_index: &mut u32,
    diag_time: &mut f64,
    _diag_state: &mut [f64],
    eval_call_index_ptr: *mut u32,
    last_eval_time_ptr: *mut f64,
    last_eval_state_ptr: *mut f64,
    last_eval_state_len: usize,
    prev_outputs: &mut [f64],
    w: &mut dyn Write,
    csv_row: &mut String,
    rows_since_flush: &mut u32,
    print_interval: f64,
    next_print: &mut f64,
    epsilon: f64,
    result_collector: &mut Option<&mut ResultCollector>,
) -> Result<(), String> {
    *tret = 0.0_f64;
    let mut time = *tret;
    let min_step = 1e-10_f64;
    let debounce_cfg = EventDebounceConfig::adaptive_from_dt(dt);
    let event_deadband = debounce_cfg.base_deadband;
    let count_deadband = debounce_cfg.count_deadband;
    let max_same_event_hits = debounce_cfg.max_same_event_hits;
    let tail_crossing_deadband = env_f64("RUSTMODLICA_TAIL_CROSSING_DEADBAND").unwrap_or(5e-3);
    let tail_height_deadband = env_f64("RUSTMODLICA_TAIL_HEIGHT_DEADBAND").unwrap_or(2e-4);
    let tail_velocity_deadband = env_f64("RUSTMODLICA_TAIL_VELOCITY_DEADBAND").unwrap_or(3e-2);
    let mut stagnant_steps = 0_u32;
    let mut stagnant_event_refines = 0_u32;
    let mut last_event_time: Option<f64> = None;
    let mut same_event_hits = 0_u32;
    let mut last_counted_event_time: Option<f64> = None;
    let mut save_states = vec![0.0_f64; states.len()];
    let mut save_discrete = vec![0.0_f64; discrete_vals.len()];
    let mut save_crossings = vec![0.0_f64; crossings.len()];
    let mut eval_discrete = vec![0.0_f64; discrete_vals.len()];
    let mut eval_when_states = vec![0.0_f64; when_states.len()];
    let mut eval_crossings = vec![0.0_f64; crossings.len()];
    let mut roots_found = vec![0_i32; crossings.len()];
    let mut event_queue = EventQueue::default();
    while time <= t_end + epsilon {
        native::reset_assert_counter();
        pre_states.copy_from_slice(states);
        pre_discrete_vals.copy_from_slice(discrete_vals);

        match run_event_iteration_at_time(
            time,
            t_end,
            calc_derivs,
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
            &[],
            w,
            Some(&mut event_queue),
        )? {
            EventIterationOutcome::TerminatedOk => return Ok(()),
            EventIterationOutcome::Completed => {}
        }
        let _dispatched_events = event_queue.drain_sorted();

        maybe_print_numeric_jacobian(
            numeric_ode_jacobian,
            time,
            epsilon,
            states,
            calc_derivs,
            params,
            discrete_vals,
            outputs,
            when_states,
            crossings,
            pre_states,
            pre_discrete_vals,
            t_end,
            symbolic_ode_jacobian,
            state_var_index,
            homotopy_lambda_ptr,
        );

        if time >= *next_print - epsilon {
            csv_row.clear();
            write!(csv_row, "{:.4}", time).map_err(|e| e.to_string())?;
            let mut display_states: Option<Vec<f64>> = None;
            if states.len() >= 2
                && !crossings.is_empty()
                && crossings[0].abs() < tail_crossing_deadband
                && states[0].abs() < tail_height_deadband
                && states[1].abs() < tail_velocity_deadband
            {
                let mut ds = states.clone();
                ds[1] = 0.0;
                display_states = Some(ds);
            }
            let state_view: &[f64] = match display_states.as_ref() {
                Some(v) => v.as_slice(),
                None => states.as_slice(),
            };
            for val in state_view.iter() {
                write!(csv_row, ", {:.4}", val).map_err(|e| e.to_string())?;
            }
            for val in discrete_vals.iter() {
                write!(csv_row, ", {:.4}", val).map_err(|e| e.to_string())?;
            }
            for val in outputs.iter() {
                write!(csv_row, ", {:.4}", val).map_err(|e| e.to_string())?;
            }
            write_csv_line(w, csv_row)?;
            *rows_since_flush = rows_since_flush.saturating_add(1);
            if *rows_since_flush >= CSV_ROWS_PER_FLUSH {
                flush_writer(w)?;
                *rows_since_flush = 0;
            }
            if let Some(ref mut c) = result_collector {
                c.push((time, states.clone(), discrete_vals.to_vec(), outputs.to_vec()));
            }
            *next_print += print_interval;
        }

        if time >= t_end - epsilon {
            break;
        }

        let dt_step = dt.abs().max(min_step);
        let mut tout = (time + dt_step).min(t_end);
        tout = tout.min(*next_print);
        if tout <= time + 1e-14 && time < t_end - epsilon {
            tout = (time + min_step).min(t_end);
        }
        let tout = tout.max(time + 1e-14);
        let tout = tout as sunrealtype;
        let t_prev = time;
        save_states.copy_from_slice(states);
        save_discrete.copy_from_slice(discrete_vals);
        save_crossings.copy_from_slice(crossings);

        native::suppress_assert_begin();
        let step_code = match kind {
            SundialsKind::Cvode => CVode(mem, tout, y, tret, CV_NORMAL as i32),
            SundialsKind::Ida => IDASolve(mem, tout, tret, y, yp, IDA_NORMAL as i32),
        };
        native::suppress_assert_end();

        let ok = match kind {
            SundialsKind::Cvode => {
                step_code == CV_SUCCESS as i32 || step_code == CV_ROOT_RETURN as i32
            }
            SundialsKind::Ida => {
                step_code == IDA_SUCCESS as i32 || step_code == IDA_ROOT_RETURN as i32
            }
        };
        if !ok {
            eprintln!(
                "{}",
                i18n::msg(
                    "simulation_failed_at",
                    &[&format!("{:.4}", time) as &dyn std::fmt::Display, &step_code]
                )
            );
            if step_code == 2 {
                print_newton_diag("sundials-step", *diag_call_index, *diag_time, *diag_residual, *diag_x);
            }
            let _ = flush_writer(w);
            return Err(format!("SUNDIALS step failed: {}", step_code));
        }

        time = *tret;
        let y_ptr = N_VGetArrayPointer(y);
        if y_ptr.is_null() {
            return Err("SUNDIALS returned null state vector pointer".to_string());
        }
        ptr::copy(y_ptr, states.as_mut_ptr(), states.len());
        if time <= t_prev + min_step * 0.1 {
            stagnant_steps = stagnant_steps.saturating_add(1);
            if stagnant_steps > 8 {
                return Err("SUNDIALS stepping stalled near an event (time not advancing)".to_string());
            }
        } else {
            stagnant_steps = 0;
        }

        fail_if_assert_storm("sundials-step", time)?;

        let t_trial = time;
        eval_discrete.copy_from_slice(discrete_vals);
        eval_when_states.fill(0.0);
        eval_crossings.fill(0.0);
        native::suppress_assert_begin();
        let eval_status = (calc_derivs)(
            t_trial,
            states.as_mut_ptr(),
            eval_discrete.as_mut_ptr(),
            derivs.as_mut_ptr(),
            params.as_ptr(),
            outputs.as_mut_ptr(),
            eval_when_states.as_mut_ptr(),
            eval_crossings.as_mut_ptr(),
            pre_states.as_ptr(),
            pre_discrete_vals.as_ptr(),
            t_end,
            diag_res_ptr,
            diag_x_ptr,
            homotopy_lambda_ptr,
        );
        native::suppress_assert_end();
        if eval_status != 0 {
            eprintln!(
                "{}",
                i18n::msg(
                    "simulation_failed_trial",
                    &[&format!("{:.4}", t_trial) as &dyn std::fmt::Display, &eval_status]
                )
            );
            let _ = flush_writer(w);
            return Err(format!("post-step evaluate failed: {}", eval_status));
        }
        if !eval_call_index_ptr.is_null() {
            *eval_call_index_ptr = (*eval_call_index_ptr).saturating_add(1);
        }
        if !last_eval_time_ptr.is_null() {
            *last_eval_time_ptr = t_trial;
        }
        if !last_eval_state_ptr.is_null() && last_eval_state_len == states.len() {
            ptr::copy_nonoverlapping(states.as_ptr(), last_eval_state_ptr, states.len());
        }
        crossings.copy_from_slice(&eval_crossings);

        let mut root_triggered = false;
        if crossings.len() > 0 {
            match kind {
                SundialsKind::Cvode => {
                    if step_code == CV_ROOT_RETURN as i32 {
                        let _ = CVodeGetRootInfo(mem, roots_found.as_mut_ptr());
                        root_triggered = roots_found.iter().any(|&v| v != 0);
                        for (idx, root) in roots_found.iter().enumerate() {
                            if *root != 0 {
                                event_queue.push_unique(QueuedEvent {
                                    time,
                                    kind: QueuedEventKind::ZeroCrossing(idx),
                                });
                            }
                        }
                    }
                }
                SundialsKind::Ida => {
                    if step_code == IDA_ROOT_RETURN as i32 {
                        let _ = IDAGetRootInfo(mem, roots_found.as_mut_ptr());
                        root_triggered = roots_found.iter().any(|&v| v != 0);
                        for (idx, root) in roots_found.iter().enumerate() {
                            if *root != 0 {
                                event_queue.push_unique(QueuedEvent {
                                    time,
                                    kind: QueuedEventKind::ZeroCrossing(idx),
                                });
                            }
                        }
                    }
                }
            }
        }
        let mut event_found = root_triggered;
        let mut min_alpha = 1.0_f64;
        for i in 0..crossings.len() {
            let c_prev = save_crossings[i];
            let c_curr = crossings[i];
            if c_prev * c_curr < 0.0 {
                event_found = true;
                event_queue.push_unique(QueuedEvent {
                    time,
                    kind: QueuedEventKind::ZeroCrossing(i),
                });
                let d = c_curr - c_prev;
                if d.abs() > 1e-12 {
                    let alpha = (-c_prev / d).clamp(0.0, 1.0);
                    if alpha > 0.0 && alpha < min_alpha {
                        min_alpha = alpha;
                    }
                }
            }
        }

        if event_found {
            let same_event_time = last_event_time
                .map(|t_last| (time - t_last).abs() <= event_deadband)
                .unwrap_or(false);
            let count_duplicate = last_counted_event_time
                .map(|t_last| (time - t_last).abs() <= count_deadband)
                .unwrap_or(false);
            if root_triggered {
                if same_event_time {
                    same_event_hits = same_event_hits.saturating_add(1);
                } else {
                    same_event_hits = 0;
                }
                last_event_time = Some(time);
                stagnant_event_refines = 0;
                roots_found.fill(0);
                if !count_duplicate {
                    if sundials_event_log_enabled() {
                        eprintln!(
                            "[sundials-event] t={:.6} root=true alpha={:.6}",
                            time, min_alpha
                        );
                    }
                    last_counted_event_time = Some(time);
                }
                // Already landed on a root by SUNDIALS; run_event_iteration_at_time will consume
                // when/reinit at the beginning of the next loop iteration.
                if same_event_hits > max_same_event_hits {
                    let mut t_force = (time + dt.abs() * 0.1).min(t_end);
                    if t_force <= time + min_step {
                        t_force = (time + min_step).min(t_end);
                    }
                    let force_code = match kind {
                        SundialsKind::Cvode => CVodeReInit(mem, t_force as sunrealtype, y),
                        SundialsKind::Ida => {
                            native::suppress_assert_begin();
                            let st0 = (calc_derivs)(
                                t_force,
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
                                return Err(format!(
                                    "IDA force-advance derivative failed near Zeno: {}",
                                    st0
                                ));
                            }
                            let yp_ptr = N_VGetArrayPointer(yp);
                            if yp_ptr.is_null() {
                                return Err(
                                    "IDA returned null derivative vector pointer on force advance"
                                        .to_string(),
                                );
                            }
                            ptr::copy(derivs.as_ptr(), yp_ptr, derivs.len());
                            IDAReInit(mem, t_force as sunrealtype, y, yp)
                        }
                    };
                    if force_code != CV_SUCCESS as i32 && force_code != IDA_SUCCESS as i32 {
                        return Err(format!(
                            "SUNDIALS force advance reinit failed near Zeno: {}",
                            force_code
                        ));
                    }
                    *tret = t_force;
                    time = t_force;
                    same_event_hits = 0;
                    last_event_time = Some(time);
                    continue;
                }
                continue;
            }
            if same_event_time && min_alpha <= 1e-9 {
                same_event_hits = same_event_hits.saturating_add(1);
                if same_event_hits > max_same_event_hits {
                    // Treat repeated alpha~=0 hits at same event time as consumed.
                    roots_found.fill(0);
                    continue;
                }
                continue;
            } else if !same_event_time {
                same_event_hits = 0;
            }
            if min_alpha <= 1e-9 {
                stagnant_event_refines = stagnant_event_refines.saturating_add(1);
                let mut t_force = (time + dt.abs() * 0.1).min(t_end);
                if t_force <= time + min_step {
                    t_force = (time + min_step).min(t_end);
                }
                let force_code = match kind {
                    SundialsKind::Cvode => CVodeReInit(mem, t_force as sunrealtype, y),
                    SundialsKind::Ida => {
                        native::suppress_assert_begin();
                        let st0 = (calc_derivs)(
                            t_force,
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
                            return Err(format!(
                                "IDA force-advance derivative failed on alpha~=0: {}",
                                st0
                            ));
                        }
                        let yp_ptr = N_VGetArrayPointer(yp);
                        if yp_ptr.is_null() {
                            return Err(
                                "IDA returned null derivative vector pointer on alpha~=0 advance"
                                    .to_string(),
                            );
                        }
                        ptr::copy(derivs.as_ptr(), yp_ptr, derivs.len());
                        IDAReInit(mem, t_force as sunrealtype, y, yp)
                    }
                };
                if force_code != CV_SUCCESS as i32 && force_code != IDA_SUCCESS as i32 {
                    return Err(format!(
                        "SUNDIALS force advance failed on alpha~=0: {}",
                        force_code
                    ));
                }
                *tret = t_force;
                time = t_force;
                last_event_time = Some(time);
                same_event_hits = 0;
                continue;
            }
            stagnant_event_refines = 0;
            let mut event_time = t_prev + (time - t_prev) * min_alpha;
            if event_time <= t_prev + min_step * 0.1 {
                event_time = (t_prev + min_step).min(t_end);
            }
            states.copy_from_slice(&save_states);
            discrete_vals.copy_from_slice(&save_discrete);
            crossings.copy_from_slice(&save_crossings);
            let y_ptr = N_VGetArrayPointer(y);
            if y_ptr.is_null() {
                return Err("SUNDIALS returned null state vector pointer before refine".to_string());
            }
            ptr::copy(states.as_ptr(), y_ptr, states.len());
            let reinit_code = match kind {
                SundialsKind::Cvode => CVodeReInit(mem, t_prev as sunrealtype, y),
                SundialsKind::Ida => {
                    native::suppress_assert_begin();
                    let st0 = (calc_derivs)(
                        t_prev,
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
                        return Err(format!("IDA reinit pre-derivative failed: {}", st0));
                    }
                    let yp_ptr = N_VGetArrayPointer(yp);
                    if yp_ptr.is_null() {
                        return Err("IDA returned null derivative vector pointer before reinit".to_string());
                    }
                    ptr::copy(derivs.as_ptr(), yp_ptr, derivs.len());
                    IDAReInit(mem, t_prev as sunrealtype, y, yp)
                }
            };
            if reinit_code != CV_SUCCESS as i32 && reinit_code != IDA_SUCCESS as i32 {
                return Err(format!("SUNDIALS reinit failed: {}", reinit_code));
            }
            roots_found.fill(0);
            native::suppress_assert_begin();
            let refine_code = match kind {
                SundialsKind::Cvode => {
                    CVode(mem, event_time as sunrealtype, y, tret, CV_NORMAL as i32)
                }
                SundialsKind::Ida => IDASolve(
                    mem,
                    event_time as sunrealtype,
                    tret,
                    y,
                    yp,
                    IDA_NORMAL as i32,
                ),
            };
            native::suppress_assert_end();
            let refine_ok = match kind {
                SundialsKind::Cvode => {
                    refine_code == CV_SUCCESS as i32 || refine_code == CV_ROOT_RETURN as i32
                }
                SundialsKind::Ida => {
                    refine_code == IDA_SUCCESS as i32 || refine_code == IDA_ROOT_RETURN as i32
                }
            };
            if !refine_ok {
                return Err(format!("SUNDIALS event refine failed: {}", refine_code));
            }
            time = *tret;
            last_event_time = Some(time);
            let y_ptr = N_VGetArrayPointer(y);
            if y_ptr.is_null() {
                return Err("SUNDIALS returned null state vector pointer after refine".to_string());
            }
            ptr::copy(y_ptr, states.as_mut_ptr(), states.len());
            if !count_duplicate {
                if sundials_event_log_enabled() {
                    eprintln!(
                        "[sundials-event] t={:.6} root={} alpha={:.6}",
                        time, root_triggered, min_alpha
                    );
                }
                last_counted_event_time = Some(time);
            }
        }
    }
    Ok(())
}
