//! SolvableBlock (Newton tearing) scale limits and JIT workspace policy.
//!
//! Scale targets (memory / asymptotic cost, not OMC coupling):
//! - **n ~ 500**: dense Jacobian is about 500*500*8 B ≈ 2 MiB per factorization buffer; Gaussian elimination is O(n^3) (~1.25e8 mul-adds per step). Feasible for offline solves if storage is **heap**, not the default thread stack (~1 MiB).
//! - **n ~ 2000**: dense Jacobian is about 32 MiB per buffer and O(n^3) dominates; **sparse** Jacobian + sparse/iterative linear algebra is effectively required for practical turnaround.
//!
//! This crate keeps **dense** Newton for moderate `n` with a **heap workspace** once `n` exceeds a small stack-safe threshold.
//!
//! ## Sparse Jacobian / linear solve (in-tree, policy-gated)
//! - CSR Jacobian assembly and faer sparse LU are integrated for Newton blocks when
//!   `should_use_newton_sparse_path` selects sparse mode (`RUSTMODLICA_NEWTON_SPARSE_POLICY`,
//!   density heuristics using `NEWTON_SPARSE_AUTO_MAX_DENSITY`, and related constants below).
//! - Dense Newton (stack or heap workspace) remains for small systems and when sparse is not selected.
//! - **Future**: external direct solvers (KLU/UMFPACK-style) or iterative Krylov + ILU are not wired.

/// Maximum number of residuals (and matched unknowns) accepted for a single `SolvableBlock` in JIT and C emission.
pub const MAX_SOLVABLE_RESIDUALS: usize = 2048;

/// Emit a compile-time warning when dense Newton `n` exceeds this (sparse path skipped).
pub const DENSE_NEWTON_WARN_MIN_N: usize = 512;

/// Emit a compile-time warning when a dense Jacobian buffer would exceed this size (bytes).
pub const DENSE_JACOBIAN_WARN_BYTES: usize = 2 * 1024 * 1024;

/// Use faer sparse LU for CSR solves when `n` is at least this (in-tree solver below for small systems).
pub const CSR_FAER_SPARSE_LU_MIN_N: usize = 48;

/// Prefer faer when average row nnz is not huge (avoids pathological wide rows in one shot).
pub const CSR_FAER_MAX_AVG_NNZ_PER_ROW: usize = 256;

/// Auto sparse Jacobian path requires `nnz / (n*n)` <= this ratio.
pub const NEWTON_SPARSE_AUTO_MAX_DENSITY: f64 = 0.75;

/// Dense Newton: use Cranelift stack slot for J,r,dx only while `n` is small enough to stay well under typical thread stack limits.
pub const JIT_DENSE_STACK_MAX_N: usize = 64;

/// Sparse (and other) Newton: if the packed workspace would exceed this size, use a thread-local heap buffer instead of a stack slot.
pub const JIT_STACK_BUFFER_BYTES_MAX: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewtonSparsePolicy {
    Auto,
    Dense,
    Sparse,
}

#[inline]
pub fn newton_sparse_policy_from_env() -> NewtonSparsePolicy {
    match std::env::var("RUSTMODLICA_NEWTON_SPARSE_POLICY")
        .ok()
        .unwrap_or_else(|| "auto".to_string())
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "dense" => NewtonSparsePolicy::Dense,
        "sparse" => NewtonSparsePolicy::Sparse,
        _ => NewtonSparsePolicy::Auto,
    }
}

#[inline]
pub fn should_use_newton_sparse_path(
    policy: NewtonSparsePolicy,
    n: usize,
    nnz: usize,
    unknown_count: usize,
) -> bool {
    if policy == NewtonSparsePolicy::Dense {
        return false;
    }
    if n < 3 || unknown_count < n || nnz == 0 || nnz >= n.saturating_mul(n) {
        return false;
    }
    if policy == NewtonSparsePolicy::Sparse {
        return true;
    }
    let dense_size = n.saturating_mul(n);
    let density = (nnz as f64) / (dense_size as f64);
    density <= NEWTON_SPARSE_AUTO_MAX_DENSITY
}

/// `rustmodlica_solve_linear_csr` / [`crate::sparse_solve::CsrMatrix`] routing.
#[inline]
pub fn csr_use_faer_sparse_lu(n: usize, nnz: usize) -> bool {
    if n < CSR_FAER_SPARSE_LU_MIN_N || nnz == 0 {
        return false;
    }
    if nnz >= n.saturating_mul(n) / 2 {
        return false;
    }
    let max_nnz = n.saturating_mul(CSR_FAER_MAX_AVG_NNZ_PER_ROW);
    nnz <= max_nnz
}

#[inline]
pub fn csr_should_fallback_to_dense(n: usize, nnz: usize) -> bool {
    nnz >= n.saturating_mul(n) / 2 || n <= 4
}

pub fn validate_solvable_residual_count(n: usize) -> Result<(), String> {
    if n == 0 || n > MAX_SOLVABLE_RESIDUALS {
        Err(format!(
            "SolvableBlock residual count {} not in 1..={}",
            n, MAX_SOLVABLE_RESIDUALS
        ))
    } else {
        Ok(())
    }
}
