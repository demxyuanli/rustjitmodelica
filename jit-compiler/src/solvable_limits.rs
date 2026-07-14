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

/// Emit a compile-time warning when dense Newton `n` exceeds this AND sparse path was not selected.
/// Warning fires when BOTH conditions hold: `n > DENSE_NEWTON_WARN_MIN_N` AND
/// `n*n*8 > DENSE_JACOBIAN_WARN_BYTES`.
pub const DENSE_NEWTON_WARN_MIN_N: usize = 512;

/// Emit a compile-time warning when a dense Jacobian buffer would exceed this size (bytes).
/// See `DENSE_NEWTON_WARN_MIN_N` -- warning requires both thresholds to be exceeded.
pub const DENSE_JACOBIAN_WARN_BYTES: usize = 2 * 1024 * 1024;

/// Use faer sparse LU for CSR solves when `n` is at least this (in-tree solver below for small systems).
pub const CSR_FAER_SPARSE_LU_MIN_N: usize = 48;

/// Prefer faer when average row nnz is not huge (avoids pathological wide rows in one shot).
pub const CSR_FAER_MAX_AVG_NNZ_PER_ROW: usize = 256;

/// Auto sparse Jacobian path requires `nnz / (n*n)` <= this ratio.
/// Calibrated (P5): 0.35 balances MultiBody tearing blocks vs dense GE cost;
/// denser blocks stay on the dense Newton path.
pub const NEWTON_SPARSE_AUTO_MAX_DENSITY: f64 = 0.35;

/// Auto sparse path requires at least this many residuals (CSR overhead on tiny n).
pub const NEWTON_SPARSE_AUTO_MIN_N: usize = 8;

/// Dense Newton: use Cranelift **stack slot** for J,r,dx only while `n <= JIT_DENSE_STACK_MAX_N`
/// (keeps workspace under ~200 KiB on stack). Larger dense blocks use a **heap workspace**.
pub const JIT_DENSE_STACK_MAX_N: usize = 64;

/// Sparse (and large dense) Newton: if the packed workspace would exceed this size, use a
/// **thread-local heap buffer** instead of a Cranelift stack slot. Applies to both sparse CSR
/// workspace and dense workspace when `n > JIT_DENSE_STACK_MAX_N`.
pub const JIT_STACK_BUFFER_BYTES_MAX: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NewtonSparsePolicy {
    Auto,
    Dense,
    Sparse,
}

#[inline]
pub fn newton_sparse_policy_from_env() -> NewtonSparsePolicy {
    // Align with JIT path preference: NEWTON_PATH overrides, else NEWTON_SPARSE_POLICY, else auto.
    let raw = std::env::var("RUSTMODLICA_NEWTON_PATH")
        .ok()
        .or_else(|| std::env::var("RUSTMODLICA_NEWTON_SPARSE_POLICY").ok())
        .unwrap_or_else(|| "auto".to_string());
    match raw.trim().to_ascii_lowercase().as_str() {
        "dense" | "dense_only" => NewtonSparsePolicy::Dense,
        "sparse" | "csr" | "sparse_only" => NewtonSparsePolicy::Sparse,
        _ => NewtonSparsePolicy::Auto,
    }
}

#[inline]
fn newton_sparse_auto_min_n() -> usize {
    std::env::var("RUSTMODLICA_SPARSE_MIN_SIZE")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v >= 3)
        .unwrap_or(NEWTON_SPARSE_AUTO_MIN_N)
}

#[inline]
fn newton_sparse_auto_max_density() -> f64 {
    std::env::var("RUSTMODLICA_SPARSE_DENSITY_THRESHOLD")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0 && *v < 1.0)
        .unwrap_or(NEWTON_SPARSE_AUTO_MAX_DENSITY)
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
    if n < newton_sparse_auto_min_n() {
        return false;
    }
    let dense_size = n.saturating_mul(n);
    let density = (nnz as f64) / (dense_size as f64);
    density <= newton_sparse_auto_max_density()
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
