use crate::ast::{Equation, Expression, Operator};
use crate::string_intern::{resolve_id, var_starts_with};
use std::collections::{HashMap, HashSet};

use crate::analysis::expression_utils::{make_binary, time_derivative};
use crate::analysis::variable_collection::{
    collect_vars_expr, contains_var, equation_contains_var,
};
use crate::analysis::AnalysisOptions;

use super::blt_expr::{simplify_expr, solve_residual_linear, substitute_der_in_expr};

fn build_der_map(equations: &[Equation]) -> HashMap<String, Expression> {
    let mut der_map: HashMap<String, Expression> = HashMap::new();
    for eq in equations.iter() {
        if let Equation::Simple(lhs, rhs) = eq {
            let entry = match lhs {
                Expression::Variable(id) if var_starts_with(*id, "der_") => Some((resolve_id(*id), rhs.clone())),
                Expression::Der(inner) => {
                    if let Expression::Variable(id) = inner.as_ref() {
                        Some((format!("der_{}", resolve_id(*id)), rhs.clone()))
                    } else {
                        None
                    }
                }
                Expression::BinaryOp(coeff, Operator::Mul, r) => {
                    let (der_name, div_by) = if let Expression::Variable(id) = &**r {
                        if var_starts_with(*id, "der_") {
                            (Some(resolve_id(*id)), Some(coeff.clone()))
                        } else {
                            (None, None)
                        }
                    } else if let Expression::Variable(id) = &**coeff {
                        if var_starts_with(*id, "der_") {
                            (Some(resolve_id(*id)), Some(r.clone()))
                        } else {
                            (None, None)
                        }
                    } else if let Expression::BinaryOp(c2, Operator::Mul, r2) = r.as_ref() {
                        if let Expression::Variable(id) = &**r2 {
                            if var_starts_with(*id, "der_") {
                                (
                                    Some(resolve_id(*id)),
                                    Some(Box::new(Expression::BinaryOp(
                                        coeff.clone(),
                                        Operator::Mul,
                                        c2.clone(),
                                    ))),
                                )
                            } else {
                                (None, None)
                            }
                        } else if let Expression::Variable(id) = &**c2 {
                            if var_starts_with(*id, "der_") {
                                (
                                    Some(resolve_id(*id)),
                                    Some(Box::new(Expression::BinaryOp(
                                        coeff.clone(),
                                        Operator::Mul,
                                        r2.clone(),
                                    ))),
                                )
                            } else {
                                (None, None)
                            }
                        } else {
                            (None, None)
                        }
                    } else {
                        (None, None)
                    };
                    match (der_name, div_by) {
                        (Some(name), Some(div_by)) => Some((
                            name,
                            Expression::BinaryOp(Box::new(rhs.clone()), Operator::Div, div_by),
                        )),
                        _ => None,
                    }
                }
                _ => None,
            };
            if let Some((name, expr)) = entry {
                der_map.insert(name, expr);
            }
        }
    }
    der_map
}

pub(super) fn try_index_reduction(
    equations: &[Equation],
    assigned_var: &[Option<usize>],
    _assigned_eq: &[Option<usize>],
    unknown_list: &[String],
    state_vars: &[String],
    options: &AnalysisOptions,
) -> Option<Vec<Equation>> {
    fn normalize_index_method(method: &str) -> &str {
        let m = method.trim();
        if m.eq_ignore_ascii_case("pantelides") {
            "pantelides"
        } else if m.eq_ignore_ascii_case("pantelidesdummy")
            || m.eq_ignore_ascii_case("dummyderivative")
        {
            "dummyDerivative"
        } else if m.eq_ignore_ascii_case("debugprint") {
            "debugPrint"
        } else if m.eq_ignore_ascii_case("none") {
            "none"
        } else {
            method
        }
    }
    fn max_pantelides_order() -> usize {
        std::env::var("RUSTMODLICA_PANTELIDES_MAX_ORDER")
            .ok()
            .and_then(|v| v.trim().parse::<usize>().ok())
            .map(|v| v.clamp(2, 6))
            .unwrap_or(3)
    }

    let der_map = build_der_map(equations);
    let unassigned: Vec<usize> = assigned_var
        .iter()
        .enumerate()
        .filter_map(|(i, o)| if o.is_none() { Some(i) } else { None })
        .collect();
    let method = normalize_index_method(&options.index_reduction_method);
    let use_dummy = matches!(method, "dummyDerivative" | "pantelides");

    for eq_idx in unassigned {
        let eq = &equations[eq_idx];
        let (is_constraint, residual) = match eq {
            Equation::Simple(lhs, rhs) => {
                let mut lhs_vars = HashSet::new();
                collect_vars_expr(lhs, &mut lhs_vars);
                let lhs_has_der = lhs_vars.iter().any(|v| v.starts_with("der_"))
                    || matches!(lhs, Expression::Der(_))
                    || matches!(lhs, Expression::Variable(id) if var_starts_with(*id, "der_"));
                (
                    !lhs_has_der,
                    make_binary(lhs.clone(), Operator::Sub, rhs.clone()),
                )
            }
            _ => continue,
        };
        if !is_constraint {
            continue;
        }
        let mut diff_expr = time_derivative(&residual, state_vars);
        diff_expr = substitute_der_in_expr(&diff_expr, &der_map);

        // Phase 1: Try linear symbolic solving (original approach)
        let mut alg_vars: Vec<&String> = unknown_list
            .iter()
            .filter(|u| !u.starts_with("der_") && !state_vars.iter().any(|s| s == *u))
            .collect();
        alg_vars.sort_by_key(|v| {
            equations
                .iter()
                .filter(|eq| equation_contains_var(eq, v))
                .count()
        });
        for alg_var in &alg_vars {
            if contains_var(&diff_expr, alg_var) {
                if let Some(sol) = solve_residual_linear(&diff_expr, alg_var) {
                    let mut new_eqs = equations.to_vec();
                    new_eqs[eq_idx] =
                        Equation::Simple(Expression::var(alg_var), sol);
                    return Some(new_eqs);
                }
            }
        }

        // Phase 1b: generalized Pantelides-like repeated differentiation.
        let max_order = max_pantelides_order();
        let mut lifted = diff_expr.clone();
        for _ord in 2..=max_order {
            lifted = time_derivative(&lifted, state_vars);
            let lifted_sub = simplify_expr(&substitute_der_in_expr(&lifted, &der_map));
            for alg_var in &alg_vars {
                if let Some(sol) = solve_residual_linear(&lifted_sub, alg_var) {
                    let mut new_eqs = equations.to_vec();
                    new_eqs[eq_idx] = Equation::Simple(Expression::var(alg_var), sol);
                    return Some(new_eqs);
                }
            }
        }

        // Phase 2: Dummy derivative method (Pantelides-style)
        if use_dummy {
            let diff_simplified = simplify_expr(&diff_expr);
            let mut diff_vars = HashSet::new();
            collect_vars_expr(&diff_simplified, &mut diff_vars);

            // Find a state variable whose der_x appears in the differentiated constraint
            let mut best_state: Option<String> = None;
            for sv in state_vars {
                let der_name = format!("der_{}", sv);
                if diff_vars.contains(&der_name) {
                    best_state = Some(sv.clone());
                    break;
                }
            }

            // Fallback: if differentiated constraint has no der_ vars but has state vars,
            // pick the first state variable that appears in the constraint itself.
            if best_state.is_none() {
                let mut residual_vars = HashSet::new();
                collect_vars_expr(&residual, &mut residual_vars);
                for sv in state_vars {
                    if residual_vars.contains(sv) {
                        best_state = Some(sv.clone());
                        break;
                    }
                }
            }
            if let Some(state_var) = best_state {
                let der_name = format!("der_{}", state_var);
                let dummy_name = format!("$dummy_{}", der_name);
                let mut new_eqs = equations.to_vec();

                // Replace the constraint with: $dummy_der_x = 0 (placeholder; Newton will solve)
                // The differentiated constraint becomes a new equation that replaces it
                new_eqs[eq_idx] = Equation::Simple(
                    Expression::var(&dummy_name),
                    Expression::var(&der_name),
                );

                // Add the differentiated constraint as a new residual equation
                new_eqs.push(Equation::Simple(diff_simplified, Expression::Number(0.0)));

                eprintln!(
                    "[index-reduction] dummy derivative: {} replaces {} in constraint {}",
                    dummy_name, der_name, eq_idx
                );
                return Some(new_eqs);
            }
        }
        eprintln!(
            "[index-reduction] constraint equation {} could not be reduced (nonlinear or unsupported form)",
            eq_idx
        );
    }
    None
}

pub(super) fn eval_const_expr(expr: &Expression) -> Option<f64> {
    match expr {
        Expression::Number(n) => Some(*n),
        Expression::BinaryOp(lhs, Operator::Add, rhs) => {
            Some(eval_const_expr(lhs)? + eval_const_expr(rhs)?)
        }
        Expression::BinaryOp(lhs, Operator::Sub, rhs) => {
            Some(eval_const_expr(lhs)? - eval_const_expr(rhs)?)
        }
        Expression::BinaryOp(lhs, Operator::Mul, rhs) => {
            Some(eval_const_expr(lhs)? * eval_const_expr(rhs)?)
        }
        Expression::BinaryOp(lhs, Operator::Div, rhs) => {
            let denom = eval_const_expr(rhs)?;
            if denom.abs() < 1e-15 {
                None
            } else {
                Some(eval_const_expr(lhs)? / denom)
            }
        }
        _ => None,
    }
}
