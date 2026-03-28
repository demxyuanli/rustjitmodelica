use crate::ast::Expression;
use crate::analysis::partial_derivative;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;

use crate::jit::context::TranslationContext;
use crate::solvable_limits::validate_solvable_residual_count;

pub(super) use super::solvable_assert::{emit_assert_suppress_begin, emit_assert_suppress_end};
use super::solvable_general_dense::compile_solvable_block_general_dense_n;
use super::solvable_general_sparse::{
    build_sparse_jacobian_pattern, compile_solvable_block_general_sparse_n, SparseJacobianPattern,
};
use super::linearized::{
    parse_newton_path_preference, NewtonLinearizationStats, NewtonLinearizedSystem,
    NewtonPathPreference,
};

/// SPARSE-2: Sparsity detection heuristic - returns true if Jacobian is sparse enough.
/// Uses density threshold (default 0.3) and minimum size (default 4).
fn is_sparse_heuristic(nnz: usize, n: usize) -> bool {
    if n < sparse_min_size() {
        return false;
    }
    let total = n.saturating_mul(n);
    if total == 0 {
        return false;
    }
    let density = nnz as f64 / total as f64;
    density < sparse_density_threshold()
}

/// SPARSE-2: Minimum size for sparse heuristic to apply.
fn sparse_min_size() -> usize {
    std::env::var("RUSTMODLICA_SPARSE_MIN_SIZE")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .unwrap_or(4)
}

/// SPARSE-2: Density threshold for sparse heuristic (default 0.3 = 30% non-zeros).
fn sparse_density_threshold() -> f64 {
    std::env::var("RUSTMODLICA_SPARSE_DENSITY_THRESHOLD")
        .ok()
        .and_then(|v| v.trim().parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0 && *v < 1.0)
        .unwrap_or(0.3)
}

#[derive(Debug, Clone)]
pub(super) struct SymbolicJacobianPlan {
    n: usize,
    entries: Vec<Option<Expression>>,
}

impl SymbolicJacobianPlan {
    pub(super) fn get(&self, row: usize, col: usize) -> Option<&Expression> {
        self.entries
            .get(row.saturating_mul(self.n).saturating_add(col))
            .and_then(|e| e.as_ref())
    }
}

fn symbolic_jacobian_enabled() -> bool {
    std::env::var("RUSTMODLICA_NEWTON_SYMBOLIC_JACOBIAN")
        .ok()
        .map(|v| !matches!(v.trim(), "0" | "false" | "FALSE" | "off" | "OFF"))
        .unwrap_or(true)
}

fn symbolic_safe_expr(expr: &Expression) -> bool {
    match expr {
        Expression::Number(_) | Expression::Variable(_) => true,
        Expression::BinaryOp(lhs, _, rhs) => symbolic_safe_expr(lhs) && symbolic_safe_expr(rhs),
        Expression::Call(name, args) => {
            if args.len() == 1 {
                matches!(
                    name.as_str(),
                    "sin" | "cos" | "exp" | "log" | "ln" | "sqrt" | "tan" | "asin" | "acos" | "atan" | "abs"
                ) && symbolic_safe_expr(&args[0])
            } else if args.len() == 2 && name == "atan2" {
                symbolic_safe_expr(&args[0]) && symbolic_safe_expr(&args[1])
            } else {
                false
            }
        }
        Expression::If(cond, then_expr, else_expr) => {
            symbolic_safe_expr(cond) && symbolic_safe_expr(then_expr) && symbolic_safe_expr(else_expr)
        }
        // Keep MVP conservative: avoid calls/array/dot/etc in symbolic plan.
        _ => false,
    }
}

pub(super) fn build_symbolic_jacobian_plan(
    unknowns: &[String],
    residuals: &[Expression],
) -> SymbolicJacobianPlan {
    let n = residuals.len();
    let mut entries = Vec::with_capacity(n.saturating_mul(n));
    for residual in residuals {
        for unknown in unknowns.iter().take(n) {
            if symbolic_jacobian_enabled() && symbolic_safe_expr(residual) {
                let d = partial_derivative(residual, unknown);
                if symbolic_safe_expr(&d) {
                    entries.push(Some(d));
                } else {
                    entries.push(None);
                }
            } else {
                entries.push(None);
            }
        }
    }
    SymbolicJacobianPlan { n, entries }
}

pub(super) fn compile_solvable_block_general_n(
    unknowns: &[String],
    residuals: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    fn dual_path_check_enabled() -> bool {
        std::env::var("RUSTMODLICA_NEWTON_DUAL_PATH_CHECK")
            .ok()
            .map(|v| matches!(v.trim(), "1" | "true" | "TRUE"))
            .unwrap_or(false)
    }

    let n = residuals.len();
    validate_solvable_residual_count(n)?;
    let slots: Vec<_> = unknowns
        .iter()
        .take(n)
        .map(|v| -> Result<_, String> {
            Ok(*ctx
                .stack_slots
                .get(v)
                .ok_or_else(|| format!("SolvableBlock unknown {} missing stack slot", v))?)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for v in unknowns.iter().take(n) {
        ctx.var_map.remove(v);
    }
    for (var, slot) in unknowns.iter().take(n).zip(slots.iter()) {
        if let Some(idx) = ctx.output_index(var) {
            let offset = (idx * 8) as i32;
            let init_val =
                builder
                    .ins()
                    .load(cl_types::F64, MemFlags::new(), ctx.outputs_ptr, offset);
            builder.ins().stack_store(init_val, *slot, 0);
        } else {
            let default_val = crate::compiler::geometric_default_for_name(var);
            let fallback_f = if default_val != 0.0 { default_val } else { 1e-3 };
            let fallback = builder.ins().f64const(fallback_f);
            builder.ins().stack_store(fallback, *slot, 0);
        }
    }
    
    let preference = parse_newton_path_preference();
    
    // SPARSE-2: Build sparse pattern if not dense-only preference
    let sparse_pattern: Option<SparseJacobianPattern> = if preference == NewtonPathPreference::DenseOnly {
        None
    } else {
        build_sparse_jacobian_pattern(&unknowns[..n], residuals)
    };
    
    // SPARSE-2: Use sparsity detection heuristic to select path
    let use_sparse = if let Some(ref pattern) = sparse_pattern {
        let nnz = pattern.nnz();
        is_sparse_heuristic(nnz, n)
    } else {
        false
    };

    let selected = if use_sparse {
        NewtonLinearizedSystem::Csr(NewtonLinearizationStats {
            residual_count: n,
            nnz: sparse_pattern.as_ref().map(|p| p.nnz()).unwrap_or(n * n),
        })
    } else {
        NewtonLinearizedSystem::Dense(NewtonLinearizationStats {
            residual_count: n,
            nnz: n.saturating_mul(n),
        })
    };

    // SPARSE-2: Path selection trace logging
    let path_trace = std::env::var("RUSTMODLICA_NEWTON_PATH_TRACE")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE"))
        .unwrap_or(false);
    if path_trace {
        match &selected {
            NewtonLinearizedSystem::Dense(stats) => {
                eprintln!(
                    "[newton-path] dense n={} nnz={} density={:.2}% heuristic={}",
                    stats.residual_count,
                    stats.nnz,
                    100.0,
                    if use_sparse { "sparse" } else { "dense" }
                );
            }
            NewtonLinearizedSystem::Csr(stats) => {
                let total = stats.residual_count.saturating_mul(stats.residual_count);
                let density = if total == 0 {
                    0.0
                } else {
                    stats.nnz as f64 * 100.0 / total as f64
                };
                eprintln!(
                    "[newton-path] csr n={} nnz={} density={:.2}% heuristic=sparse",
                    stats.residual_count,
                    stats.nnz,
                    density
                );
            }
        }
    }

    if dual_path_check_enabled() && path_trace {
        eprintln!(
            "[newton-path] dual-check=on n={} selected={:?}",
            n,
            selected.kind()
        );
    }

    let symbolic_plan = build_symbolic_jacobian_plan(unknowns, residuals);
    match (preference, selected, sparse_pattern.as_ref()) {
        (NewtonPathPreference::SparseOnly, NewtonLinearizedSystem::Dense(_), _) => {
            compile_solvable_block_general_dense_n(
                unknowns,
                residuals,
                &slots,
                &symbolic_plan,
                ctx,
                builder,
            )
        }
        (_, NewtonLinearizedSystem::Csr(_), Some(pattern)) => {
            compile_solvable_block_general_sparse_n(
                unknowns,
                residuals,
                &slots,
                &symbolic_plan,
                pattern,
                ctx,
                builder,
            )
        }
        (_, NewtonLinearizedSystem::Dense(_), _) => {
            compile_solvable_block_general_dense_n(
                unknowns,
                residuals,
                &slots,
                &symbolic_plan,
                ctx,
                builder,
            )
        }
        (_, NewtonLinearizedSystem::Csr(_), None) => Err(
            "sparse Newton path selected but no CSR Jacobian pattern available".to_string(),
        ),
    }
}
