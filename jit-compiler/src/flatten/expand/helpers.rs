use crate::ast::{AlgorithmStatement, Equation, Expression};
use crate::compiler::inline::is_builtin_function;
use std::collections::HashMap;
use std::sync::OnceLock;

use super::super::expressions::{eval_const_expr, expr_to_path, index_expression, prefix_expression};
use super::super::utils::{convert_eq_to_alg, get_function_outputs};
use super::super::ExpandTarget;

static CONNECT_PATH_WARN_ENABLED: OnceLock<bool> = OnceLock::new();

pub(super) fn connect_path_warn_enabled() -> bool {
    *CONNECT_PATH_WARN_ENABLED.get_or_init(|| {
        std::env::var("RUSTMODLICA_CONNECT_WARN")
            .ok()
            .map(|v| {
                let t = v.trim().to_ascii_lowercase();
                t == "1" || t == "true" || t == "on" || t == "yes"
            })
            .unwrap_or(false)
    })
}

pub(super) fn extract_der_array_base(expr: &Expression) -> Option<String> {
    match expr {
        Expression::Variable(id) => Some(crate::string_intern::resolve_id(*id)),
        Expression::ArrayAccess(base, _) => extract_der_array_base(base),
        Expression::Dot(base, member) => {
            let base_name = extract_der_array_base(base)?;
            Some(format!("{}_{}", base_name, member))
        }
        _ => None,
    }
}

pub(super) fn is_scalar_lhs_target(expr: &Expression) -> bool {
    matches!(expr, Expression::Variable(_) | Expression::ArrayAccess(_, _))
}

pub(super) fn is_scalar_like_output(expr: &Expression) -> bool {
    !matches!(expr, Expression::ArrayLiteral(_))
}

pub(super) fn is_array_like_output(expr: &Expression) -> bool {
    matches!(expr, Expression::ArrayLiteral(_))
}

pub(super) fn array_literal_depth(expr: &Expression) -> usize {
    match expr {
        Expression::ArrayLiteral(items) => {
            1 + items
                .iter()
                .map(array_literal_depth)
                .max()
                .unwrap_or(0)
        }
        Expression::BinaryOp(l, _, r) => array_literal_depth(l).max(array_literal_depth(r)),
        Expression::Call(_, args) => args.iter().map(array_literal_depth).max().unwrap_or(0),
        Expression::Der(inner)
        | Expression::Sample(inner)
        | Expression::Interval(inner)
        | Expression::Hold(inner)
        | Expression::Previous(inner) => array_literal_depth(inner),
        Expression::SubSample(a, b)
        | Expression::SuperSample(a, b)
        | Expression::ShiftSample(a, b)
        | Expression::BackSample(a, b)
        | Expression::ArrayAccess(a, b) => array_literal_depth(a).max(array_literal_depth(b)),
        Expression::Dot(base, _) => array_literal_depth(base),
        Expression::If(c, t, f) => array_literal_depth(c)
            .max(array_literal_depth(t))
            .max(array_literal_depth(f)),
        Expression::Range(s, st, e) => array_literal_depth(s)
            .max(array_literal_depth(st))
            .max(array_literal_depth(e)),
        Expression::ArrayComprehension { expr, iter_range, .. } => {
            array_literal_depth(expr).max(array_literal_depth(iter_range))
        }
        Expression::Variable(_) | Expression::Number(_) | Expression::StringLiteral(_) => 0,
    }
}

pub(super) fn expr_contains_array_comprehension(expr: &Expression) -> bool {
    match expr {
        Expression::ArrayComprehension { .. } => true,
        Expression::BinaryOp(l, _, r) => {
            expr_contains_array_comprehension(l) || expr_contains_array_comprehension(r)
        }
        Expression::Call(_, args) => args.iter().any(expr_contains_array_comprehension),
        Expression::Der(inner)
        | Expression::Sample(inner)
        | Expression::Interval(inner)
        | Expression::Hold(inner)
        | Expression::Previous(inner) => expr_contains_array_comprehension(inner),
        Expression::SubSample(a, b)
        | Expression::SuperSample(a, b)
        | Expression::ShiftSample(a, b)
        | Expression::BackSample(a, b)
        | Expression::ArrayAccess(a, b) => {
            expr_contains_array_comprehension(a) || expr_contains_array_comprehension(b)
        }
        Expression::Dot(base, _) => expr_contains_array_comprehension(base),
        Expression::If(c, t, f) => {
            expr_contains_array_comprehension(c)
                || expr_contains_array_comprehension(t)
                || expr_contains_array_comprehension(f)
        }
        Expression::Range(s, st, e) => {
            expr_contains_array_comprehension(s)
                || expr_contains_array_comprehension(st)
                || expr_contains_array_comprehension(e)
        }
        Expression::ArrayLiteral(items) => items.iter().any(expr_contains_array_comprehension),
        Expression::Variable(_) | Expression::Number(_) | Expression::StringLiteral(_) => false,
    }
}

pub(super) fn is_record_like_output_type(type_name: &str) -> bool {
    !crate::flatten::utils::is_primitive(type_name)
}

pub(super) fn is_complex_lhs_target(expr: &Expression) -> bool {
    match expr {
        Expression::Dot(_, _) => true,
        Expression::ArrayAccess(base, _) => is_complex_lhs_target(base),
        _ => false,
    }
}

pub(super) fn collect_complex_lhs_targets(lhss: &[Expression]) -> Vec<String> {
    lhss.iter()
        .enumerate()
        .filter(|(_, lhs)| is_complex_lhs_target(lhs))
        .map(|(i, lhs)| format!("#{}={:?}", i + 1, lhs))
        .collect()
}
