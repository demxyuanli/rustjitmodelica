use crate::ast::{Equation, Expression, Operator};
use crate::string_intern::resolve_id;
use std::collections::HashMap;

use crate::analysis::expression_utils::{expression_is_zero, make_binary, make_num};
use crate::analysis::variable_collection::{contains_var, equation_contains_var};

fn expr_is_nonlinear_in_var(expr: &Expression, var: &str) -> bool {
    match expr {
        Expression::BinaryOp(lhs, Operator::Mul, rhs) => {
            contains_var(lhs, var) && contains_var(rhs, var)
        }
        Expression::BinaryOp(lhs, _, rhs) => {
            expr_is_nonlinear_in_var(lhs, var) || expr_is_nonlinear_in_var(rhs, var)
        }
        Expression::Call(_, args) => args.iter().any(|a| contains_var(a, var)),
        Expression::If(c, t, e) => {
            expr_is_nonlinear_in_var(c, var)
                || expr_is_nonlinear_in_var(t, var)
                || expr_is_nonlinear_in_var(e, var)
        }
        _ => false,
    }
}

fn equation_nonlinear_for_var(eq: &Equation, var: &str) -> bool {
    match eq {
        Equation::Simple(lhs, rhs) => expr_is_nonlinear_in_var(lhs, var) || expr_is_nonlinear_in_var(rhs, var),
        Equation::SolvableBlock { residuals, .. } => residuals.iter().any(|r| expr_is_nonlinear_in_var(r, var)),
        _ => false,
    }
}

pub(super) fn select_tearing_variable(
    block_unknowns: &[String],
    block_eqs: &[Equation],
    _unknown_map: &HashMap<String, usize>,
    method: &str,
) -> Option<String> {
    fn env_weight(name: &str, default_v: f64) -> f64 {
        std::env::var(name)
            .ok()
            .and_then(|v| v.trim().parse::<f64>().ok())
            .filter(|v| v.is_finite())
            .unwrap_or(default_v)
    }

    fn candidate_score(block_eqs: &[Equation], u: &str) -> f64 {
        let occ = block_eqs
            .iter()
            .filter(|eq| equation_contains_var(eq, u))
            .count() as f64;
        let nonlinear_hits = block_eqs
            .iter()
            .filter(|eq| equation_nonlinear_for_var(eq, u))
            .count() as f64;
        let solve_bias = block_eqs
            .iter()
            .filter(|eq| solve_for_variable(eq, u).is_some())
            .count() as f64;
        let residual_size_bias = if occ > 0.0 { 1.0 / occ } else { 1.0 };

        let w_occ = env_weight("RUSTMODLICA_TEAR_W_OCC", 1.0);
        let w_nonlin = env_weight("RUSTMODLICA_TEAR_W_NONLINEAR", 3.0);
        let w_solve = env_weight("RUSTMODLICA_TEAR_W_SOLVABLE", 0.75);
        let w_residual = env_weight("RUSTMODLICA_TEAR_W_RESIDUAL", 0.5);
        occ * w_occ + nonlinear_hits * w_nonlin - solve_bias * w_solve + residual_size_bias * w_residual
    }

    if block_unknowns.is_empty() {
        return None;
    }
    match method {
        "maxEquation" => {
            let mut best = block_unknowns[0].clone();
            let mut best_count = 0usize;
            for u in block_unknowns {
                let count = block_eqs
                    .iter()
                    .filter(|eq| equation_contains_var(eq, u))
                    .count();
                if count > best_count {
                    best_count = count;
                    best = u.clone();
                }
            }
            Some(best)
        }
        "minCellier" | "leastOccurrence" => {
            let mut best = block_unknowns[0].clone();
            let mut best_score = usize::MAX;
            for u in block_unknowns {
                let count = block_eqs
                    .iter()
                    .filter(|eq| equation_contains_var(eq, u))
                    .count();
                if count < best_score {
                    best_score = count;
                    best = u.clone();
                }
            }
            Some(best)
        }
        "smart" | "hybrid" | "smartBlock" => {
            let mut best = block_unknowns[0].clone();
            let mut best_score = f64::INFINITY;
            for u in block_unknowns {
                let score = candidate_score(block_eqs, u);
                if score < best_score {
                    best_score = score;
                    best = u.clone();
                }
            }
            Some(best)
        }
        _ => block_unknowns.first().cloned(),
    }
}

pub(super) fn solve_for_variable(eq: &Equation, var: &str) -> Option<Expression> {
    if let Equation::Simple(lhs, rhs) = eq {
        if let Expression::Variable(id) = lhs {
            if resolve_id(*id) == var && !contains_var(rhs, var) {
                return Some(rhs.clone());
            }
        }
        if let Expression::Variable(id) = rhs {
            if resolve_id(*id) == var && !contains_var(lhs, var) {
                return Some(lhs.clone());
            }
        }
        let residual = make_binary(lhs.clone(), Operator::Sub, rhs.clone());
        if let Some(sol) = solve_residual_linear(&residual, var) {
            return Some(sol);
        }
    }
    None
}

pub(super) fn make_residual(eq: &Equation) -> Expression {
    match eq {
        Equation::Simple(lhs, rhs) => make_binary(lhs.clone(), Operator::Sub, rhs.clone()),
        Equation::MultiAssign(_, _) => Expression::Number(0.0),
        _ => make_num(0.0),
    }
}

pub(super) fn substitute_der_in_expr(
    expr: &Expression,
    der_map: &HashMap<String, Expression>,
) -> Expression {
    match expr {
        Expression::Variable(id) => {
            let name = resolve_id(*id);
            if name.starts_with("der_") {
                der_map.get(&name).cloned().unwrap_or_else(|| expr.clone())
            } else {
                expr.clone()
            }
        }
        Expression::BinaryOp(lhs, op, rhs) => Expression::BinaryOp(
            Box::new(substitute_der_in_expr(lhs, der_map)),
            *op,
            Box::new(substitute_der_in_expr(rhs, der_map)),
        ),
        Expression::Call(n, args) => Expression::Call(
            n.clone(),
            args.iter()
                .map(|a| substitute_der_in_expr(a, der_map))
                .collect(),
        ),
        Expression::Der(inner) => Expression::Der(Box::new(substitute_der_in_expr(inner, der_map))),
        Expression::If(c, t, f) => Expression::If(
            Box::new(substitute_der_in_expr(c, der_map)),
            Box::new(substitute_der_in_expr(t, der_map)),
            Box::new(substitute_der_in_expr(f, der_map)),
        ),
        _ => expr.clone(),
    }
}

/// Simplify 0*e, 1*e, e+0, 0+e, e-0, 0-e, e/1 for easier linear splitting.
pub(super) fn simplify_expr(expr: &Expression) -> Expression {
    use crate::ast::Expression::*;
    match expr {
        BinaryOp(l, Operator::Mul, r) => {
            let (sl, sr) = (simplify_expr(l.as_ref()), simplify_expr(r.as_ref()));
            if let Number(n) = &sl {
                if n.abs() < 1e-15 {
                    return make_num(0.0);
                }
                if (n - 1.0).abs() < 1e-15 {
                    return sr;
                }
            }
            if let Number(n) = &sr {
                if n.abs() < 1e-15 {
                    return make_num(0.0);
                }
                if (n - 1.0).abs() < 1e-15 {
                    return sl;
                }
            }
            Expression::BinaryOp(Box::new(sl), Operator::Mul, Box::new(sr))
        }
        BinaryOp(l, Operator::Add, r) => {
            let (sl, sr) = (simplify_expr(l.as_ref()), simplify_expr(r.as_ref()));
            if let Number(n) = &sl {
                if n.abs() < 1e-15 {
                    return sr;
                }
            }
            if let Number(n) = &sr {
                if n.abs() < 1e-15 {
                    return sl;
                }
            }
            Expression::BinaryOp(Box::new(sl), Operator::Add, Box::new(sr))
        }
        BinaryOp(l, Operator::Sub, r) => {
            let (sl, sr) = (simplify_expr(l.as_ref()), simplify_expr(r.as_ref()));
            if let Number(n) = &sr {
                if n.abs() < 1e-15 {
                    return sl;
                }
            }
            if let Number(n) = &sl {
                if n.abs() < 1e-15 {
                    return Expression::BinaryOp(
                        Box::new(make_num(0.0)),
                        Operator::Sub,
                        Box::new(sr),
                    );
                }
            }
            Expression::BinaryOp(Box::new(sl), Operator::Sub, Box::new(sr))
        }
        BinaryOp(l, Operator::Div, r) => {
            let (sl, sr) = (simplify_expr(l.as_ref()), simplify_expr(r.as_ref()));
            if let Number(n) = &sr {
                if (n - 1.0).abs() < 1e-15 {
                    return sl;
                }
            }
            Expression::BinaryOp(Box::new(sl), Operator::Div, Box::new(sr))
        }
        BinaryOp(l, op, r) => Expression::BinaryOp(
            Box::new(simplify_expr(l.as_ref())),
            *op,
            Box::new(simplify_expr(r.as_ref())),
        ),
        _ => expr.clone(),
    }
}

/// Collect linear term in var: expr = coeff*var + rest, return (coeff, rest). Returns None if not linear in var.
pub(super) fn split_linear(expr: &Expression, var: &str) -> Option<(Expression, Expression)> {
    use crate::ast::Expression::*;
    if !contains_var(expr, var) {
        return Some((make_num(0.0), expr.clone()));
    }
    match expr {
        Variable(id) if resolve_id(*id) == var => Some((make_num(1.0), make_num(0.0))),
        BinaryOp(mul_l, Operator::Mul, mul_r) => {
            if let Number(n) = mul_l.as_ref() {
                if n.abs() < 1e-15 {
                    return Some((make_num(0.0), make_num(0.0)));
                }
                if (n - 1.0).abs() < 1e-15 {
                    return split_linear(mul_r.as_ref(), var);
                }
            }
            if let Number(n) = mul_r.as_ref() {
                if n.abs() < 1e-15 {
                    return Some((make_num(0.0), make_num(0.0)));
                }
                if (n - 1.0).abs() < 1e-15 {
                    return split_linear(mul_l.as_ref(), var);
                }
            }
            if let Variable(id) = mul_r.as_ref() {
                if resolve_id(*id) == var && !contains_var(mul_l, var) {
                    return Some(((**mul_l).clone(), make_num(0.0)));
                }
            }
            if let Variable(id) = mul_l.as_ref() {
                if resolve_id(*id) == var && !contains_var(mul_r, var) {
                    return Some(((**mul_r).clone(), make_num(0.0)));
                }
            }
            if let BinaryOp(a, Operator::Mul, b) = mul_r.as_ref() {
                if let Variable(id) = b.as_ref() {
                    if resolve_id(*id) == var && !contains_var(mul_l, var) && !contains_var(a, var) {
                        return Some((
                            make_binary((**mul_l).clone(), Operator::Mul, (**a).clone()),
                            make_num(0.0),
                        ));
                    }
                }
                if let Variable(id) = a.as_ref() {
                    if resolve_id(*id) == var && !contains_var(mul_l, var) && !contains_var(b, var) {
                        return Some((
                            make_binary((**mul_l).clone(), Operator::Mul, (**b).clone()),
                            make_num(0.0),
                        ));
                    }
                }
            }
            if let BinaryOp(a, Operator::Mul, b) = mul_l.as_ref() {
                if let Variable(id) = b.as_ref() {
                    if resolve_id(*id) == var && !contains_var(mul_r, var) && !contains_var(a, var) {
                        return Some((
                            make_binary((**a).clone(), Operator::Mul, (**mul_r).clone()),
                            make_num(0.0),
                        ));
                    }
                }
                if let Variable(id) = a.as_ref() {
                    if resolve_id(*id) == var && !contains_var(mul_r, var) && !contains_var(b, var) {
                        return Some((
                            make_binary((**b).clone(), Operator::Mul, (**mul_r).clone()),
                            make_num(0.0),
                        ));
                    }
                }
            }
            if !contains_var(mul_r, var) {
                if let Some((c_inner, r_inner)) = split_linear(mul_l, var) {
                    if expression_is_zero(&r_inner) {
                        return Some((
                            make_binary(c_inner, Operator::Mul, (**mul_r).clone()),
                            make_num(0.0),
                        ));
                    }
                }
            }
            if !contains_var(mul_l, var) {
                if let Some((c_inner, r_inner)) = split_linear(mul_r, var) {
                    if expression_is_zero(&r_inner) {
                        return Some((
                            make_binary((**mul_l).clone(), Operator::Mul, c_inner),
                            make_num(0.0),
                        ));
                    }
                    return Some((
                        make_binary((**mul_l).clone(), Operator::Mul, c_inner),
                        make_binary((**mul_l).clone(), Operator::Mul, r_inner),
                    ));
                }
            }
            if !contains_var(mul_r, var) {
                if let Some((c_inner, r_inner)) = split_linear(mul_l, var) {
                    if expression_is_zero(&r_inner) {
                        return Some((
                            make_binary(c_inner, Operator::Mul, (**mul_r).clone()),
                            make_num(0.0),
                        ));
                    }
                    return Some((
                        make_binary(c_inner, Operator::Mul, (**mul_r).clone()),
                        make_binary(r_inner, Operator::Mul, (**mul_r).clone()),
                    ));
                }
            }
            None
        }
        BinaryOp(l, Operator::Add, r) => {
            let (c_l, r_l) = split_linear(l, var)?;
            let (c_r, r_r) = split_linear(r, var)?;
            Some((
                make_binary(c_l.clone(), Operator::Add, c_r.clone()),
                make_binary(r_l, Operator::Add, r_r),
            ))
        }
        BinaryOp(l, Operator::Sub, r) => {
            if let Number(n) = l.as_ref() {
                if n.abs() < 1e-15 {
                    let (c, rest) = split_linear(r, var)?;
                    return Some((
                        make_binary(make_num(0.0), Operator::Sub, c),
                        make_binary(make_num(0.0), Operator::Sub, rest),
                    ));
                }
            }
            let (c_l, r_l) = split_linear(l, var)?;
            let (c_r, r_r) = split_linear(r, var)?;
            Some((
                make_binary(c_l, Operator::Sub, c_r.clone()),
                make_binary(r_l, Operator::Sub, r_r),
            ))
        }
        BinaryOp(l, Operator::Div, r) => {
            if contains_var(r.as_ref(), var) {
                return None;
            }
            let (c, rest) = split_linear(l.as_ref(), var)?;
            Some((
                make_binary(c, Operator::Div, (**r).clone()),
                make_binary(rest, Operator::Div, (**r).clone()),
            ))
        }
        _ => None,
    }
}

pub(super) fn solve_residual_linear(expr: &Expression, var: &str) -> Option<Expression> {
    if !contains_var(expr, var) {
        return None;
    }
    if let Some((coeff, rest)) = split_linear(expr, var) {
        if expression_is_zero(&coeff) {
            return None;
        }
        return Some(make_binary(
            make_binary(make_num(0.0), Operator::Sub, rest),
            Operator::Div,
            coeff,
        ));
    }
    if let Expression::BinaryOp(lhs, op, rhs) = expr {
        let (rest, coeff) = match (op, lhs.as_ref(), rhs.as_ref()) {
            (Operator::Sub, rest, Expression::BinaryOp(mul_l, Operator::Mul, mul_r)) => {
                let coeff = if let Expression::Variable(id) = mul_r.as_ref() {
                    if resolve_id(*id) == var && !contains_var(rest, var) && !contains_var(mul_l, var) {
                        mul_l.clone()
                    } else if let Expression::Variable(id2) = mul_l.as_ref() {
                        if resolve_id(*id2) == var && !contains_var(rest, var) && !contains_var(mul_r, var) {
                            mul_r.clone()
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }
                } else {
                    return None;
                };
                (rest.clone(), coeff)
            }
            (Operator::Sub, Expression::BinaryOp(mul_l, Operator::Mul, mul_r), rest) => {
                let coeff = if let Expression::Variable(id) = mul_r.as_ref() {
                    if resolve_id(*id) == var && !contains_var(rest, var) && !contains_var(mul_l, var) {
                        mul_l.clone()
                    } else if let Expression::Variable(id2) = mul_l.as_ref() {
                        if resolve_id(*id2) == var && !contains_var(rest, var) && !contains_var(mul_r, var) {
                            mul_r.clone()
                        } else {
                            return None;
                        }
                    } else {
                        return None;
                    }
                } else {
                    return None;
                };
                (rest.clone(), coeff)
            }
            _ => return None,
        };
        Some(make_binary(rest, Operator::Div, *coeff))
    } else {
        None
    }
}
