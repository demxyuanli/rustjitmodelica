use std::collections::HashMap;
use crate::ast::{Equation, Expression, Operator};

use crate::analysis::variable_collection::contains_var;
use crate::analysis::expression_utils::{make_mul, make_num};

pub(crate) fn eliminate_aliases(equations: &[Equation]) -> (Vec<Equation>, HashMap<String, Expression>) {
    let mut alias_map: HashMap<String, Expression> = HashMap::new();
    let mut current_eqs = equations.to_vec();
    let mut changed = true;

    while changed {
        changed = false;
        let mut next_eqs = Vec::with_capacity(current_eqs.len());

        for eq in &current_eqs {
            let mut is_alias = false;

            if let Equation::Simple(lhs, rhs) = eq {
                if let Expression::Variable(v) = lhs {
                    if *lhs != *rhs && !contains_var(rhs, v) {
                        if !v.starts_with("der_") {
                            if !alias_map.contains_key(v) {
                                alias_map.insert(v.clone(), rhs.clone());
                                changed = true;
                                is_alias = true;
                            }
                        }
                    }
                }

                if !is_alias {
                    if let Expression::Variable(v) = rhs {
                        if *lhs != *rhs && !contains_var(lhs, v) {
                            if !v.starts_with("der_") {
                                let lhs_is_der = if let Expression::Variable(l) = lhs {
                                    l.starts_with("der_")
                                } else {
                                    false
                                };
                                if !lhs_is_der && !alias_map.contains_key(v) {
                                    alias_map.insert(v.clone(), lhs.clone());
                                    changed = true;
                                    is_alias = true;
                                }
                            }
                        }
                    }
                }

                if !is_alias {
                    if let Expression::BinaryOp(l, Operator::Sub, r) = lhs {
                        if let Expression::Number(n) = &**l {
                            if n.abs() < 1e-10 {
                                if let Expression::Variable(v) = &**r {
                                    if !alias_map.contains_key(v) && !contains_var(rhs, v) {
                                        if !v.starts_with("der_") {
                                            let neg_rhs = make_mul(make_num(-1.0), rhs.clone());
                                            alias_map.insert(v.clone(), neg_rhs);
                                            changed = true;
                                            is_alias = true;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if !is_alias {
                next_eqs.push(eq.clone());
            }
        }

        if changed {
            let mut new_alias_map = alias_map.clone();
            for _ in 0..10 {
                let mut map_changed = false;
                let keys: Vec<String> = new_alias_map.keys().cloned().collect();
                for k in keys {
                    let val = new_alias_map[&k].clone();
                    let new_val = substitute_aliases_in_expr(&val, &new_alias_map);
                    if val != new_val {
                        new_alias_map.insert(k, new_val);
                        map_changed = true;
                    }
                }
                if !map_changed {
                    break;
                }
            }
            alias_map = new_alias_map;

            let mut substituted_eqs = Vec::new();
            for eq in next_eqs {
                let new_eq = substitute_aliases_in_eq(&eq, &alias_map);
                substituted_eqs.push(new_eq);
            }
            current_eqs = substituted_eqs;
        } else {
            current_eqs = next_eqs;
        }
    }

    (current_eqs, alias_map)
}

fn substitute_aliases_in_eq(eq: &Equation, map: &HashMap<String, Expression>) -> Equation {
    match eq {
        Equation::Simple(lhs, rhs) => Equation::Simple(
            substitute_aliases_in_expr(lhs, map),
            substitute_aliases_in_expr(rhs, map),
        ),
        Equation::For(v, s, e, body) => Equation::For(
            v.clone(),
            Box::new(substitute_aliases_in_expr(s, map)),
            Box::new(substitute_aliases_in_expr(e, map)),
            body.iter()
                .map(|b_eq| substitute_aliases_in_eq(b_eq, map))
                .collect(),
        ),
        Equation::When(cond, body, else_whens) => Equation::When(
            substitute_aliases_in_expr(cond, map),
            body.iter()
                .map(|b_eq| substitute_aliases_in_eq(b_eq, map))
                .collect(),
            else_whens
                .iter()
                .map(|(ec, eb)| (
                    substitute_aliases_in_expr(ec, map),
                    eb.iter()
                        .map(|b_eq| substitute_aliases_in_eq(b_eq, map))
                        .collect(),
                ))
                .collect(),
        ),
        Equation::If(cond, then_eqs, elseif_list, else_eqs) => Equation::If(
            substitute_aliases_in_expr(cond, map),
            then_eqs
                .iter()
                .map(|e| substitute_aliases_in_eq(e, map))
                .collect(),
            elseif_list
                .iter()
                .map(|(c, eb)| (
                    substitute_aliases_in_expr(c, map),
                    eb.iter()
                        .map(|e| substitute_aliases_in_eq(e, map))
                        .collect(),
                ))
                .collect(),
            else_eqs.as_ref().map(|eqs| {
                eqs.iter()
                    .map(|e| substitute_aliases_in_eq(e, map))
                    .collect()
            }),
        ),
        Equation::Connect(_, _)
        | Equation::Reinit(_, _)
        | Equation::Assert(_, _)
        | Equation::Terminate(_)
        | Equation::SolvableBlock { .. }
        | Equation::MultiAssign(_, _) => eq.clone(),
    }
}

fn substitute_aliases_in_expr(
    expr: &Expression,
    map: &HashMap<String, Expression>,
) -> Expression {
    match expr {
        Expression::Variable(name) => map.get(name).cloned().unwrap_or_else(|| expr.clone()),
        Expression::BinaryOp(lhs, op, rhs) => Expression::BinaryOp(
            Box::new(substitute_aliases_in_expr(lhs, map)),
            *op,
            Box::new(substitute_aliases_in_expr(rhs, map)),
        ),
        Expression::Call(name, args) => Expression::Call(
            name.clone(),
            args.iter()
                .map(|a| substitute_aliases_in_expr(a, map))
                .collect(),
        ),
        Expression::Der(arg) => {
            Expression::Der(Box::new(substitute_aliases_in_expr(arg, map)))
        }
        Expression::ArrayAccess(arr, idx) => Expression::ArrayAccess(
            Box::new(substitute_aliases_in_expr(arr, map)),
            Box::new(substitute_aliases_in_expr(idx, map)),
        ),
        Expression::If(c, t, f) => Expression::If(
            Box::new(substitute_aliases_in_expr(c, map)),
            Box::new(substitute_aliases_in_expr(t, map)),
            Box::new(substitute_aliases_in_expr(f, map)),
        ),
        Expression::ArrayLiteral(es) => Expression::ArrayLiteral(
            es.iter()
                .map(|e| substitute_aliases_in_expr(e, map))
                .collect(),
        ),
        Expression::Dot(base, member) => Expression::Dot(
            Box::new(substitute_aliases_in_expr(base, map)),
            member.clone(),
        ),
        Expression::Range(start, step, end) => Expression::Range(
            Box::new(substitute_aliases_in_expr(start, map)),
            Box::new(substitute_aliases_in_expr(step, map)),
            Box::new(substitute_aliases_in_expr(end, map)),
        ),
        _ => expr.clone(),
    }
}
