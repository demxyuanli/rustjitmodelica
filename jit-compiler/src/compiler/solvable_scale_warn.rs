use crate::ast::Equation;
use crate::diag::WarningInfo;
use crate::jit::translator::equation::solvable_block_uses_sparse_jacobian_path;
use crate::solvable_limits::{
    DENSE_JACOBIAN_WARN_BYTES, DENSE_NEWTON_WARN_MIN_N, MAX_SOLVABLE_RESIDUALS,
};

pub(super) fn push_dense_newton_scale_warnings(
    equations: &[Equation],
    warnings: &mut Vec<WarningInfo>,
    path: String,
    warnings_level: &str,
) {
    if warnings_level == "none" {
        return;
    }
    for eq in equations {
        let Equation::SolvableBlock {
            unknowns,
            residuals,
            ..
        } = eq
        else {
            continue;
        };
        let n = residuals.len();
        if n < 2 || n > MAX_SOLVABLE_RESIDUALS || unknowns.len() < n {
            continue;
        }
        let u_prefix = &unknowns[..n];
        if solvable_block_uses_sparse_jacobian_path(u_prefix, residuals.as_slice()) {
            continue;
        }
        let jac_bytes = match n.checked_mul(n).and_then(|x| x.checked_mul(8)) {
            Some(b) => b,
            None => continue,
        };
        if n <= DENSE_NEWTON_WARN_MIN_N && jac_bytes <= DENSE_JACOBIAN_WARN_BYTES {
            continue;
        }
        warnings.push(WarningInfo {
            path: path.clone(),
            line: 0,
            column: 0,
            message: format!(
                "SolvableBlock uses dense Newton with n={} (Jacobian buffer ~{} bytes); expect high CPU/memory cost. Prefer additional tearing for sparsity; future work: sparse direct (e.g. KLU/UMFPACK-class) or iterative linear solve (see solvable_limits module notes).",
                n, jac_bytes
            ),
            source: None,
        });
    }
}
