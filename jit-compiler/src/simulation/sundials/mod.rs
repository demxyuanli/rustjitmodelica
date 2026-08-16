mod kinsol;
mod linsol;
mod run_common;

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
    SUNDenseMatrix_Data, CVodeSetJacFn, IDASetJacFn, SUNMatrix,
    CVodeSetJacTimes, IDASetJacTimes,
    SUNSparseMatrix_Data, SUNSparseMatrix_IndexValues, SUNSparseMatrix_IndexPointers,
    SUNSparseMatrix_Rows, SUNSparseMatrix_NNZ,
};
use std::os::raw::c_int;

use crate::ast::Expression;
use crate::i18n;
use crate::jit::native;
use crate::jit::CalcDerivsFunc;
use super::events::{
    commit_pre_after_event_and_reeval, run_event_iteration_at_time, EventIterationOutcome,
};
use super::newton_recovery::{fail_if_assert_storm, print_newton_diag};
use super::sim_io::{flush_writer, write_csv_line};
use super::step::maybe_print_numeric_jacobian;
use super::types::{
    EventDebounceConfig, EventQueue, QueuedEvent, QueuedEventKind, ResultCollector,
    SundialsRuntimeConfig,
};

use linsol::attach_for_cvode_ida;
use run_common::{run_sundials_common, SundialsKind};

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
    /// Dense Jacobian template for KLU (populate first, then copy to sparse CSR).
    jacobian_dense: *mut c_void,
    /// Jacobian coloring groups pointer (Box<Vec<Vec<usize>>> cast to raw).
    /// NULL means no coloring available (fall back to per-column evaluation).
    jacobian_color_groups: *const Vec<Vec<usize>>,
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

include!("callbacks.rs");

