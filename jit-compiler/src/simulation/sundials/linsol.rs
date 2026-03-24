//! Attach SUNLinSol to CVODE/IDA Newton: dense, SPGMR, or (optional `sundials-klu`) KLU on a dense-derived sparse pattern.
//! In-tree algebraic Newton still uses faer (`sparse_solve.rs`); this module only selects SUNDIALS steppers' linear solver.

use std::ptr;

use sundials_sys::{
    sunindextype, SUNContext, SUNLinSol_Dense, SUNLinSol_SPGMR, SUNLinSolFree, SUNMatDestroy,
    SUNMatrix, SUNDenseMatrix, SUNLinearSolver, SUN_PREC_NONE, N_Vector,
};
#[cfg(feature = "sundials-klu")]
use sundials_sys::{SUNLinSol_KLU, SUNSparseFromDenseMatrix};

#[cfg(feature = "sundials-klu")]
const CSR_MAT: i32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SundialsLinSolKind {
    Dense,
    Spgmr,
    #[cfg(feature = "sundials-klu")]
    Klu,
}

fn default_linsol_auto(n: usize) -> SundialsLinSolKind {
    let dense_max_n = std::env::var("RUSTMODLICA_SUNDIALS_DENSE_MAX_N")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(64);
    let klu_min_n = std::env::var("RUSTMODLICA_SUNDIALS_KLU_MIN_N")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(512);
    let spgmr_max_n = std::env::var("RUSTMODLICA_SUNDIALS_SPGMR_MAX_N")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(klu_min_n.saturating_sub(1));
    if n <= dense_max_n {
        SundialsLinSolKind::Dense
    } else if n <= spgmr_max_n {
        SundialsLinSolKind::Spgmr
    } else if cfg!(feature = "sundials-klu") && n >= klu_min_n {
        #[cfg(feature = "sundials-klu")]
        {
            SundialsLinSolKind::Klu
        }
        #[cfg(not(feature = "sundials-klu"))]
        {
            SundialsLinSolKind::Spgmr
        }
    } else {
        SundialsLinSolKind::Spgmr
    }
}

pub fn parse_linsol_env(n: usize) -> SundialsLinSolKind {
    warn_if_unsupported_backend_requested();
    let Ok(s) = std::env::var("RUSTMODLICA_SUNDIALS_LINSOL") else {
        return default_linsol_auto(n);
    };
    let s = s.trim();
    if s.eq_ignore_ascii_case("dense") {
        SundialsLinSolKind::Dense
    } else if s.eq_ignore_ascii_case("spgmr") {
        SundialsLinSolKind::Spgmr
    } else if s.eq_ignore_ascii_case("auto") {
        default_linsol_auto(n)
    } else {
        #[cfg(feature = "sundials-klu")]
        {
            if s.eq_ignore_ascii_case("klu") {
                return SundialsLinSolKind::Klu;
            }
        }
        eprintln!(
            "RUSTMODLICA_SUNDIALS_LINSOL='{}' is not recognized, falling back to auto policy.",
            s
        );
        default_linsol_auto(n)
    }
}

/// Linear solver + Jacobian matrix handles for CVODE/IDA; frees in `Drop`.
pub struct AttachedSunLinSol {
    pub linsol: SUNLinearSolver,
    pub jacobian: SUNMatrix,
    /// Dense template when `jacobian` is sparse-from-dense (KLU); otherwise null.
    pub jacobian_dense: SUNMatrix,
}

impl Drop for AttachedSunLinSol {
    fn drop(&mut self) {
        unsafe {
            if !self.linsol.is_null() {
                SUNLinSolFree(self.linsol);
                self.linsol = ptr::null_mut();
            }
            if !self.jacobian.is_null() {
                SUNMatDestroy(self.jacobian);
                self.jacobian = ptr::null_mut();
            }
            if !self.jacobian_dense.is_null() {
                SUNMatDestroy(self.jacobian_dense);
                self.jacobian_dense = ptr::null_mut();
            }
        }
    }
}

/// PETSc and UMFPACK are not wired in this crate; env `petsc` / `umfpack` map to SPGMR with a stderr hint.
pub fn attach_for_cvode_ida(
    y: N_Vector,
    ctx: SUNContext,
    n: sunindextype,
    kind: SundialsLinSolKind,
) -> Result<AttachedSunLinSol, String> {
    fn spgmr_krylov_dim_from_env() -> i32 {
        std::env::var("RUSTMODLICA_SUNDIALS_SPGMR_MAXL")
            .ok()
            .and_then(|v| v.trim().parse::<i32>().ok())
            .map(|v| v.clamp(5, 256))
            .unwrap_or(30)
    }
    unsafe {
        match kind {
            SundialsLinSolKind::Dense => {
                let a = SUNDenseMatrix(n, n, ctx);
                if a.is_null() {
                    return Err("SUNDenseMatrix returned null".to_string());
                }
                let ls = SUNLinSol_Dense(y, a, ctx);
                if ls.is_null() {
                    SUNMatDestroy(a);
                    return Err("SUNLinSol_Dense returned null".to_string());
                }
                Ok(AttachedSunLinSol {
                    linsol: ls,
                    jacobian: a,
                    jacobian_dense: ptr::null_mut(),
                })
            }
            SundialsLinSolKind::Spgmr => {
                let maxl = spgmr_krylov_dim_from_env();
                let ls = SUNLinSol_SPGMR(y, SUN_PREC_NONE as i32, maxl, ctx);
                if ls.is_null() {
                    return Err("SUNLinSol_SPGMR returned null".to_string());
                }
                Ok(AttachedSunLinSol {
                    linsol: ls,
                    jacobian: ptr::null_mut(),
                    jacobian_dense: ptr::null_mut(),
                })
            }
            #[cfg(feature = "sundials-klu")]
            SundialsLinSolKind::Klu => {
                let a_dense = SUNDenseMatrix(n, n, ctx);
                if a_dense.is_null() {
                    return Err("SUNDenseMatrix for KLU returned null".to_string());
                }
                let a_sparse = SUNSparseFromDenseMatrix(a_dense, 0.0, CSR_MAT);
                if a_sparse.is_null() {
                    SUNMatDestroy(a_dense);
                    return Err("SUNSparseFromDenseMatrix returned null".to_string());
                }
                let ls = SUNLinSol_KLU(y, a_sparse, ctx);
                if ls.is_null() {
                    SUNMatDestroy(a_sparse);
                    SUNMatDestroy(a_dense);
                    return Err("SUNLinSol_KLU returned null".to_string());
                }
                Ok(AttachedSunLinSol {
                    linsol: ls,
                    jacobian: a_sparse,
                    jacobian_dense: a_dense,
                })
            }
        }
    }
}

pub fn warn_if_unsupported_backend_requested() {
    let Ok(s) = std::env::var("RUSTMODLICA_SUNDIALS_LINSOL") else {
        return;
    };
    let s = s.trim();
    if s.eq_ignore_ascii_case("petsc") || s.eq_ignore_ascii_case("umfpack") {
        eprintln!(
            "RUSTMODLICA_SUNDIALS_LINSOL={}: not linked in rustmodlica; use spgmr, dense, or build with sundials-klu for KLU.",
            s
        );
    }
    if s.eq_ignore_ascii_case("auto") {
        eprintln!(
            "RUSTMODLICA_SUNDIALS_LINSOL=auto uses dense for small n, SPGMR for medium n, and KLU (if enabled) for larger sparse-ready systems."
        );
    }
    #[cfg(not(feature = "sundials-klu"))]
    {
        if s.eq_ignore_ascii_case("klu") {
            eprintln!(
                "RUSTMODLICA_SUNDIALS_LINSOL=klu requires building rustmodlica with --features sundials-klu."
            );
        }
    }
}
