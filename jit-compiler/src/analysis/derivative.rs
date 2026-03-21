use crate::ast::{Equation, Expression, Operator};
use std::collections::HashSet;

fn expand_der_linear(inner: &Expression) -> Option<Expression> {
    use Expression::*;
    match inner {
        Variable(name) => Some(Variable(format!("der_{}", name))),
        BinaryOp(l, Operator::Add, r) => {
            let dl = expand_der_linear(l)?;
            let dr = expand_der_linear(r)?;
            Some(BinaryOp(Box::new(dl), Operator::Add, Box::new(dr)))
        }
        BinaryOp(l, Operator::Sub, r) => {
            let dl = expand_der_linear(l)?;
            let dr = expand_der_linear(r)?;
            Some(BinaryOp(Box::new(dl), Operator::Sub, Box::new(dr)))
        }
        BinaryOp(l, Operator::Mul, r) => match (&**l, &**r) {
            (Number(c), Variable(name)) => Some(BinaryOp(
                Box::new(Number(*c)),
                Operator::Mul,
                Box::new(Variable(format!("der_{}", name))),
            )),
            (Variable(name), Number(c)) => Some(BinaryOp(
                Box::new(Number(*c)),
                Operator::Mul,
                Box::new(Variable(format!("der_{}", name))),
            )),
            _ => None,
        },
        _ => None,
    }
}

pub fn flatten_dot_to_name(expr: &Expression) -> Option<String> {
    match expr {
        Expression::Variable(name) => Some(name.clone()),
        Expression::Dot(base, member) => {
            flatten_dot_to_name(base).map(|b| format!("{}_{}", b, member))
        }
        _ => None,
    }
}

pub fn normalize_der(expr: &Expression) -> Expression {
    match expr {
        Expression::Der(inner) => {
            if let Some(expanded) = expand_der_linear(inner) {
                normalize_der(&expanded)
            } else if let Expression::Variable(name) = &**inner {
                Expression::Variable(format!("der_{}", name))
            } else if let Some(flat) = flatten_dot_to_name(inner) {
                Expression::Variable(format!("der_{}", flat))
            } else {
                Expression::Der(Box::new(normalize_der(inner)))
            }
        }
        Expression::BinaryOp(lhs, op, rhs) => Expression::BinaryOp(
            Box::new(normalize_der(lhs)),
            *op,
            Box::new(normalize_der(rhs)),
        ),
        Expression::Call(func, args) => Expression::Call(
            func.clone(),
            args.iter().map(|a| normalize_der(a)).collect(),
        ),
        Expression::ArrayAccess(arr, idx) => {
            Expression::ArrayAccess(Box::new(normalize_der(arr)), Box::new(normalize_der(idx)))
        }
        Expression::If(c, t, f) => Expression::If(
            Box::new(normalize_der(c)),
            Box::new(normalize_der(t)),
            Box::new(normalize_der(f)),
        ),
        Expression::Sample(inner) => Expression::Sample(Box::new(normalize_der(inner))),
        Expression::Interval(inner) => Expression::Interval(Box::new(normalize_der(inner))),
        Expression::Hold(inner) => Expression::Hold(Box::new(normalize_der(inner))),
        Expression::Previous(inner) => Expression::Previous(Box::new(normalize_der(inner))),
        Expression::SubSample(c, n) => {
            Expression::SubSample(Box::new(normalize_der(c)), Box::new(normalize_der(n)))
        }
        Expression::SuperSample(c, n) => {
            Expression::SuperSample(Box::new(normalize_der(c)), Box::new(normalize_der(n)))
        }
        Expression::ShiftSample(c, n) => {
            Expression::ShiftSample(Box::new(normalize_der(c)), Box::new(normalize_der(n)))
        }
        _ => expr.clone(),
    }
}

fn collect_vars_in_expr(expr: &Expression, out: &mut HashSet<String>) {
    match expr {
        Expression::Variable(name) => {
            out.insert(name.clone());
        }
        Expression::Dot(_, _) => {
            if let Some(flat) = flatten_dot_to_name(expr) {
                out.insert(flat);
            }
        }
        Expression::BinaryOp(lhs, _, rhs) => {
            collect_vars_in_expr(lhs, out);
            collect_vars_in_expr(rhs, out);
        }
        Expression::Call(_, args) => {
            for a in args {
                collect_vars_in_expr(a, out);
            }
        }
        Expression::If(c, t, f) => {
            collect_vars_in_expr(c, out);
            collect_vars_in_expr(t, out);
            collect_vars_in_expr(f, out);
        }
        Expression::Der(inner) => collect_vars_in_expr(inner, out),
        Expression::Sample(inner) => collect_vars_in_expr(inner, out),
        Expression::Interval(inner) => collect_vars_in_expr(inner, out),
        Expression::Hold(inner) => collect_vars_in_expr(inner, out),
        Expression::Previous(inner) => collect_vars_in_expr(inner, out),
        Expression::SubSample(c, n)
        | Expression::SuperSample(c, n)
        | Expression::ShiftSample(c, n) => {
            collect_vars_in_expr(c, out);
            collect_vars_in_expr(n, out);
        }
        _ => {}
    }
}

fn collect_states_from_expr(expr: &Expression, states: &mut HashSet<String>) {
    match expr {
        Expression::Der(inner) => {
            if let Some(flat) = flatten_dot_to_name(inner) {
                states.insert(flat);
            } else {
                collect_vars_in_expr(inner, states);
            }
        }
        Expression::Variable(name) if name.starts_with("der_") => {
            if let Some(base) = name.strip_prefix("der_") {
                states.insert(base.to_string());
            }
        }
        Expression::BinaryOp(lhs, _, rhs) => {
            collect_states_from_expr(lhs, states);
            collect_states_from_expr(rhs, states);
        }
        Expression::Call(_, args) => {
            for arg in args {
                collect_states_from_expr(arg, states);
            }
        }
        Expression::If(c, t, f) => {
            collect_states_from_expr(c, states);
            collect_states_from_expr(t, states);
            collect_states_from_expr(f, states);
        }
        Expression::Sample(inner) => collect_states_from_expr(inner, states),
        Expression::Interval(inner) => collect_states_from_expr(inner, states),
        Expression::Hold(inner) => collect_states_from_expr(inner, states),
        Expression::Previous(inner) => collect_states_from_expr(inner, states),
        Expression::SubSample(c, n)
        | Expression::SuperSample(c, n)
        | Expression::ShiftSample(c, n) => {
            collect_states_from_expr(c, states);
            collect_states_from_expr(n, states);
        }
        _ => {}
    }
}

pub fn collect_states_from_eq(eq: &Equation, states: &mut HashSet<String>) {
    match eq {
        Equation::Simple(lhs, rhs) => {
            collect_states_from_expr(lhs, states);
            collect_states_from_expr(rhs, states);
        }
        Equation::MultiAssign(lhss, rhs) => {
            for e in lhss {
                collect_states_from_expr(e, states);
            }
            collect_states_from_expr(rhs, states);
        }
        Equation::For(_, s, e, body) => {
            collect_states_from_expr(s, states);
            collect_states_from_expr(e, states);
            for sub_eq in body {
                collect_states_from_eq(sub_eq, states);
            }
        }
        Equation::When(c, b, e) => {
            collect_states_from_expr(c, states);
            for sub_eq in b {
                collect_states_from_eq(sub_eq, states);
            }
            for (ec, eb) in e {
                collect_states_from_expr(ec, states);
                for sub_eq in eb {
                    collect_states_from_eq(sub_eq, states);
                }
            }
        }
        Equation::If(c, then_eqs, elseif_list, else_eqs) => {
            collect_states_from_expr(c, states);
            for eq in then_eqs {
                collect_states_from_eq(eq, states);
            }
            for (ec, eb) in elseif_list {
                collect_states_from_expr(ec, states);
                for eq in eb {
                    collect_states_from_eq(eq, states);
                }
            }
            if let Some(eqs) = else_eqs {
                for eq in eqs {
                    collect_states_from_eq(eq, states);
                }
            }
        }
        Equation::Assert(cond, msg) => {
            collect_states_from_expr(cond, states);
            collect_states_from_expr(msg, states);
        }
        Equation::Terminate(msg) => {
            collect_states_from_expr(msg, states);
        }
        _ => {}
    }
}

/// F2-1: Check for der(expr) where expr is not Variable or linear combination (unsupported).
/// Returns a short error hint if found.
fn find_unsupported_der_in_expr(expr: &Expression) -> Option<String> {
    match expr {
        Expression::Der(inner) => {
            if matches!(inner.as_ref(), Expression::Variable(_)) {
                None
            } else if expand_der_linear(inner).is_some() {
                None
            } else if flatten_dot_to_name(inner).is_some() {
                None
            } else {
                Some("der(expr) only supports der(x) for state variable x or linear combinations of states (e.g. der(a+b), der(c*x)). Unsupported expression in der().".to_string())
            }
        }
        Expression::BinaryOp(l, _, r) => {
            find_unsupported_der_in_expr(l).or_else(|| find_unsupported_der_in_expr(r))
        }
        Expression::Call(_, args) => {
            for a in args {
                if let Some(hint) = find_unsupported_der_in_expr(a) {
                    return Some(hint);
                }
            }
            None
        }
        Expression::ArrayAccess(arr, idx) => {
            find_unsupported_der_in_expr(arr).or_else(|| find_unsupported_der_in_expr(idx))
        }
        Expression::If(c, t, f) => find_unsupported_der_in_expr(c)
            .or_else(|| find_unsupported_der_in_expr(t))
            .or_else(|| find_unsupported_der_in_expr(f)),
        _ => None,
    }
}

/// F2-1: Check equation (and nested equations) for unsupported nested der(); return error hint if any.
pub fn find_unsupported_der_in_eq(eq: &Equation) -> Option<String> {
    match eq {
        Equation::Simple(lhs, rhs) => {
            find_unsupported_der_in_expr(lhs).or_else(|| find_unsupported_der_in_expr(rhs))
        }
        Equation::MultiAssign(lhss, rhs) => {
            for e in lhss {
                if let Some(h) = find_unsupported_der_in_expr(e) {
                    return Some(h);
                }
            }
            find_unsupported_der_in_expr(rhs)
        }
        Equation::For(_, s, e, body) => find_unsupported_der_in_expr(s)
            .or_else(|| find_unsupported_der_in_expr(e))
            .or_else(|| {
                for sub in body {
                    if let Some(h) = find_unsupported_der_in_eq(sub) {
                        return Some(h);
                    }
                }
                None
            }),
        Equation::When(c, b, e) => find_unsupported_der_in_expr(c)
            .or_else(|| {
                for sub in b {
                    if let Some(h) = find_unsupported_der_in_eq(sub) {
                        return Some(h);
                    }
                }
                None
            })
            .or_else(|| {
                for (ec, eb) in e {
                    if let Some(h) = find_unsupported_der_in_expr(ec) {
                        return Some(h);
                    }
                    for sub in eb {
                        if let Some(h) = find_unsupported_der_in_eq(sub) {
                            return Some(h);
                        }
                    }
                }
                None
            }),
        Equation::If(c, then_eqs, elseif_list, else_eqs) => find_unsupported_der_in_expr(c)
            .or_else(|| {
                for sub in then_eqs {
                    if let Some(h) = find_unsupported_der_in_eq(sub) {
                        return Some(h);
                    }
                }
                None
            })
            .or_else(|| {
                for (ec, eb) in elseif_list {
                    if let Some(h) = find_unsupported_der_in_expr(ec) {
                        return Some(h);
                    }
                    for sub in eb {
                        if let Some(h) = find_unsupported_der_in_eq(sub) {
                            return Some(h);
                        }
                    }
                }
                None
            })
            .or_else(|| {
                if let Some(eqs) = else_eqs {
                    for sub in eqs {
                        if let Some(h) = find_unsupported_der_in_eq(sub) {
                            return Some(h);
                        }
                    }
                }
                None
            }),
        Equation::Assert(cond, msg) => {
            find_unsupported_der_in_expr(cond).or_else(|| find_unsupported_der_in_expr(msg))
        }
        Equation::Terminate(msg) => find_unsupported_der_in_expr(msg),
        _ => None,
    }
}
