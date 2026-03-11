use crate::ast::{Equation, Expression, AlgorithmStatement};

/// CG1-4: Substitute all array indices in expr: every Variable(base_j) becomes Variable(base_{j+shift}).
/// Used for run detection when RHS involves multiple arrays (e.g. x_i = p_i or y_i = x_i).
pub fn expr_substitute_all_array_indices(expr: &Expression, shift: usize) -> Expression {
    use Expression::*;
    match expr {
        Variable(name) => {
            if let Some((b, idx)) = parse_array_index(name) {
                return Variable(format!("{}_{}", b, idx + shift));
            }
            expr.clone()
        }
        Number(_) => expr.clone(),
        BinaryOp(l, op, r) => BinaryOp(
            Box::new(expr_substitute_all_array_indices(l, shift)),
            *op,
            Box::new(expr_substitute_all_array_indices(r, shift)),
        ),
        Call(n, args) => Call(
            n.clone(),
            args.iter()
                .map(|a| expr_substitute_all_array_indices(a, shift))
                .collect(),
        ),
        Der(inner) => Der(Box::new(expr_substitute_all_array_indices(inner, shift))),
        Sample(inner) => Sample(Box::new(expr_substitute_all_array_indices(inner, shift))),
        Interval(inner) => Interval(Box::new(expr_substitute_all_array_indices(inner, shift))),
        Hold(inner) => Hold(Box::new(expr_substitute_all_array_indices(inner, shift))),
        Previous(inner) => Previous(Box::new(expr_substitute_all_array_indices(inner, shift))),
        SubSample(c, n) => SubSample(Box::new(expr_substitute_all_array_indices(c, shift)), Box::new(expr_substitute_all_array_indices(n, shift))),
        SuperSample(c, n) => SuperSample(Box::new(expr_substitute_all_array_indices(c, shift)), Box::new(expr_substitute_all_array_indices(n, shift))),
        ShiftSample(c, n) => ShiftSample(Box::new(expr_substitute_all_array_indices(c, shift)), Box::new(expr_substitute_all_array_indices(n, shift))),
        ArrayAccess(arr, idx) => ArrayAccess(
            Box::new(expr_substitute_all_array_indices(arr, shift)),
            Box::new(expr_substitute_all_array_indices(idx, shift)),
        ),
        If(c, t, e) => If(
            Box::new(expr_substitute_all_array_indices(c, shift)),
            Box::new(expr_substitute_all_array_indices(t, shift)),
            Box::new(expr_substitute_all_array_indices(e, shift)),
        ),
        Dot(e, s) => Dot(Box::new(expr_substitute_all_array_indices(e, shift)), s.clone()),
        Range(a, b, c) => Range(
            Box::new(expr_substitute_all_array_indices(a, shift)),
            Box::new(expr_substitute_all_array_indices(b, shift)),
            Box::new(expr_substitute_all_array_indices(c, shift)),
        ),
        ArrayLiteral(es) => ArrayLiteral(
            es.iter()
                .map(|e| expr_substitute_all_array_indices(e, shift))
                .collect(),
        ),
        StringLiteral(s) => StringLiteral(s.clone()),
    }
}

/// CG1-4: Substitute array index in expr: Variable(base_j) becomes Variable(base_{j+shift}) for the given base.
#[allow(dead_code)]
pub fn expr_substitute_array_shift(expr: &Expression, base: &str, shift: usize) -> Expression {
    use Expression::*;
    match expr {
        Variable(name) => {
            if let Some((b, idx)) = parse_array_index(name) {
                if b == base {
                    return Variable(format!("{}_{}", base, idx + shift));
                }
            }
            expr.clone()
        }
        Number(_) => expr.clone(),
        BinaryOp(l, op, r) => BinaryOp(
            Box::new(expr_substitute_array_shift(l, base, shift)),
            *op,
            Box::new(expr_substitute_array_shift(r, base, shift)),
        ),
        Call(n, args) => Call(
            n.clone(),
            args.iter()
                .map(|a| expr_substitute_array_shift(a, base, shift))
                .collect(),
        ),
        Der(inner) => Der(Box::new(expr_substitute_array_shift(inner, base, shift))),
        Sample(inner) => Sample(Box::new(expr_substitute_array_shift(inner, base, shift))),
        Interval(inner) => Interval(Box::new(expr_substitute_array_shift(inner, base, shift))),
        Hold(inner) => Hold(Box::new(expr_substitute_array_shift(inner, base, shift))),
        Previous(inner) => Previous(Box::new(expr_substitute_array_shift(inner, base, shift))),
        SubSample(c, n) => SubSample(Box::new(expr_substitute_array_shift(c, base, shift)), Box::new(expr_substitute_array_shift(n, base, shift))),
        SuperSample(c, n) => SuperSample(Box::new(expr_substitute_array_shift(c, base, shift)), Box::new(expr_substitute_array_shift(n, base, shift))),
        ShiftSample(c, n) => ShiftSample(Box::new(expr_substitute_array_shift(c, base, shift)), Box::new(expr_substitute_array_shift(n, base, shift))),
        ArrayAccess(arr, idx) => ArrayAccess(
            Box::new(expr_substitute_array_shift(arr, base, shift)),
            Box::new(expr_substitute_array_shift(idx, base, shift)),
        ),
        If(c, t, e) => If(
            Box::new(expr_substitute_array_shift(c, base, shift)),
            Box::new(expr_substitute_array_shift(t, base, shift)),
            Box::new(expr_substitute_array_shift(e, base, shift)),
        ),
        Dot(e, s) => Dot(Box::new(expr_substitute_array_shift(e, base, shift)), s.clone()),
        Range(a, b, c) => Range(
            Box::new(expr_substitute_array_shift(a, base, shift)),
            Box::new(expr_substitute_array_shift(b, base, shift)),
            Box::new(expr_substitute_array_shift(c, base, shift)),
        ),
        ArrayLiteral(es) => ArrayLiteral(
            es.iter()
                .map(|e| expr_substitute_array_shift(e, base, shift))
                .collect(),
        ),
        StringLiteral(s) => StringLiteral(s.clone()),
    }
}

pub fn convert_eq_to_alg_stmt(eq: Equation) -> AlgorithmStatement {
    match eq {
        Equation::Simple(lhs, rhs) => AlgorithmStatement::Assignment(lhs, rhs),
        Equation::Reinit(var, val) => AlgorithmStatement::Reinit(var, val),
        Equation::For(var, start, end, body) => {
            let alg_body = body.into_iter().map(convert_eq_to_alg_stmt).collect();
            let range = Expression::Range(start, Box::new(Expression::Number(1.0)), end);
            AlgorithmStatement::For(var, Box::new(range), alg_body)
        }
        Equation::When(cond, body, else_whens) => {
            let alg_body = body.into_iter().map(convert_eq_to_alg_stmt).collect();
            let alg_else = else_whens
                .into_iter()
                .map(|(c, b)| (c, b.into_iter().map(convert_eq_to_alg_stmt).collect()))
                .collect();
            AlgorithmStatement::When(cond, alg_body, alg_else)
        }
        Equation::If(cond, then_eqs, elseif_list, else_eqs) => {
            let then_alg = then_eqs.into_iter().map(convert_eq_to_alg_stmt).collect();
            let elseif_alg = elseif_list
                .into_iter()
                .map(|(c, eb)| (c, eb.into_iter().map(convert_eq_to_alg_stmt).collect()))
                .collect();
            let else_alg = else_eqs.map(|eqs| eqs.into_iter().map(convert_eq_to_alg_stmt).collect());
            AlgorithmStatement::If(cond, then_alg, elseif_alg, else_alg)
        }
        Equation::Assert(cond, msg) => AlgorithmStatement::Assert(cond, msg),
        Equation::Terminate(msg) => AlgorithmStatement::Terminate(msg),
        Equation::Connect(_, _) => panic!(
            "connect() inside when/algorithm is not supported; use equation section"
        ),
        Equation::SolvableBlock { .. } => panic!(
            "SolvableBlock (algebraic loop) inside when/algorithm is not supported; put equations in the equation section instead"
        ),
        Equation::MultiAssign(_, _) => panic!(
            "(a,b,...)=f(x) in when/algorithm is not supported; use equation section"
        ),
    }
}

pub fn parse_array_index(name: &str) -> Option<(String, usize)> {
    if let Some((base, idx_str)) = name.rsplit_once('_') {
        if let Ok(idx) = idx_str.parse::<usize>() {
            return Some((base.to_string(), idx));
        }
    }
    None
}
