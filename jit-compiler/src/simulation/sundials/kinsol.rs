//! Optional KINSOL entry point for square algebraic systems F(u)=0 (initialization / algebraic hooks).
//! Uses SPGMR without an explicit Jacobian matrix.

use std::ptr;

use sundials_sys::{
    comm_no_mpi, sunindextype, sunrealtype, KINCreate, KINFree, KINInit, KINSetLinearSolver,
    KINSetUserData, KINSol, N_VConst, N_VClone, N_VDestroy, N_VGetArrayPointer, N_VNew_Serial,
    SUNContext_Create, SUNContext_Free, SUNLinSol_SPGMR, SUNLinSolFree, SUN_PREC_NONE, N_Vector,
    SUNLinearSolver,
};

pub type KinResidualFn = unsafe extern "C" fn(
    u: *const sunrealtype,
    fu: *mut sunrealtype,
    n: usize,
    user_data: *mut libc::c_void,
) -> i32;

#[repr(C)]
pub struct KinsolCallbackPack {
    pub n: usize,
    pub residual: KinResidualFn,
    pub user_data: *mut libc::c_void,
}

unsafe extern "C" fn kin_sys_fn(uu: N_Vector, fval: N_Vector, user_data: *mut libc::c_void) -> i32 {
    let p = &*(user_data as *const KinsolCallbackPack);
    let up = N_VGetArrayPointer(uu);
    let fp = N_VGetArrayPointer(fval);
    (p.residual)(up, fp, p.n, p.user_data)
}

/// Solve F(u)=0 with KINSOL (linesearch + SPGMR). `u` is initial guess and output.
pub fn kinsol_solve_square_spgmr(
    n: usize,
    u: &mut [sunrealtype],
    residual: KinResidualFn,
    user_data: *mut libc::c_void,
) -> Result<(), String> {
    if u.len() != n {
        return Err("kinsol: u length mismatch".to_string());
    }
    let nn = n as sunindextype;
    unsafe {
        let mut ctx = ptr::null_mut();
        if SUNContext_Create(comm_no_mpi(), &mut ctx) != 0 {
            return Err("SUNContext_Create failed".to_string());
        }
        let mut kin = KINCreate(ctx);
        if kin.is_null() {
            SUNContext_Free(&mut ctx);
            return Err("KINCreate failed".to_string());
        }
        let tmpl = N_VNew_Serial(nn, ctx);
        if tmpl.is_null() {
            KINFree(&mut kin);
            SUNContext_Free(&mut ctx);
            return Err("N_VNew_Serial (tmpl) failed".to_string());
        }
        let nv_u = N_VNew_Serial(nn, ctx);
        if nv_u.is_null() {
            N_VDestroy(tmpl);
            KINFree(&mut kin);
            SUNContext_Free(&mut ctx);
            return Err("N_VNew_Serial (u) failed".to_string());
        }
        ptr::copy_nonoverlapping(u.as_ptr(), N_VGetArrayPointer(nv_u), n);

        let r = KINInit(kin, Some(kin_sys_fn), tmpl);
        if r != sundials_sys::KIN_SUCCESS as i32 {
            N_VDestroy(nv_u);
            N_VDestroy(tmpl);
            KINFree(&mut kin);
            SUNContext_Free(&mut ctx);
            return Err(format!("KINInit failed: {}", r));
        }

        let ls: SUNLinearSolver = SUNLinSol_SPGMR(nv_u, SUN_PREC_NONE as i32, 30, ctx);
        if ls.is_null() {
            N_VDestroy(nv_u);
            N_VDestroy(tmpl);
            KINFree(&mut kin);
            SUNContext_Free(&mut ctx);
            return Err("SUNLinSol_SPGMR (KIN) failed".to_string());
        }
        let lr = KINSetLinearSolver(kin, ls, ptr::null_mut());
        if lr != sundials_sys::KINLS_SUCCESS as i32 {
            SUNLinSolFree(ls);
            N_VDestroy(nv_u);
            N_VDestroy(tmpl);
            KINFree(&mut kin);
            SUNContext_Free(&mut ctx);
            return Err(format!("KINSetLinearSolver failed: {}", lr));
        }

        let pack = Box::new(KinsolCallbackPack {
            n,
            residual,
            user_data,
        });
        let pack_ptr = Box::into_raw(pack);
        let ur = KINSetUserData(kin, pack_ptr as *mut libc::c_void);
        if ur != sundials_sys::KIN_SUCCESS as i32 {
            let _ = Box::from_raw(pack_ptr);
            SUNLinSolFree(ls);
            N_VDestroy(nv_u);
            N_VDestroy(tmpl);
            KINFree(&mut kin);
            SUNContext_Free(&mut ctx);
            return Err(format!("KINSetUserData failed: {}", ur));
        }

        let scale_u = N_VClone(tmpl);
        let scale_f = N_VClone(tmpl);
        if scale_u.is_null() || scale_f.is_null() {
            let _ = Box::from_raw(pack_ptr);
            if !scale_u.is_null() {
                N_VDestroy(scale_u);
            }
            if !scale_f.is_null() {
                N_VDestroy(scale_f);
            }
            SUNLinSolFree(ls);
            N_VDestroy(nv_u);
            N_VDestroy(tmpl);
            KINFree(&mut kin);
            SUNContext_Free(&mut ctx);
            return Err("N_VClone (scaling) failed".to_string());
        }
        N_VConst(1.0, scale_u);
        N_VConst(1.0, scale_f);

        let sol = KINSol(
            kin,
            nv_u,
            sundials_sys::KIN_LINESEARCH as i32,
            scale_u,
            scale_f,
        );
        let code = sol;
        let ok = code == sundials_sys::KIN_SUCCESS as i32;

        ptr::copy_nonoverlapping(N_VGetArrayPointer(nv_u), u.as_mut_ptr(), n);

        N_VDestroy(scale_f);
        N_VDestroy(scale_u);
        SUNLinSolFree(ls);
        N_VDestroy(nv_u);
        N_VDestroy(tmpl);
        KINFree(&mut kin);
        SUNContext_Free(&mut ctx);
        let _ = Box::from_raw(pack_ptr);

        if ok {
            Ok(())
        } else {
            Err(format!("KINSol failed: {}", code))
        }
    }
}
