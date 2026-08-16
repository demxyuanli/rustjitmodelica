use crate::ast::{AlgorithmStatement, Equation, Expression};
use crate::string_intern::resolve_id;
use std::collections::HashSet;

pub fn extract_unknowns(eq: &Equation, knowns: &HashSet<String>) -> Vec<String> {
    let mut vars = HashSet::new();
    collect_vars_eq(eq, &mut vars);
    let mut unknowns = Vec::new();
    for v in vars {
        if !knowns.contains(&v) {
            unknowns.push(v);
        }
    }
    unknowns.sort();
    unknowns
}

/// Names assigned on the left of `Simple` equations (`q_1`, `frame_b_R_4`, ...).
pub(crate) fn collect_simple_lhs_names(equations: &[Equation]) -> HashSet<String> {
    let mut out = HashSet::new();
    for eq in equations {
        if let Equation::Simple(Expression::Variable(id), _) = eq {
            out.insert(resolve_id(*id));
        }
    }
    out
}

/// `resolve1(frame_b.R, r)` / `absoluteRotation(R1, R2)` mention the array base
/// while producers assign `R_1..R_4`. Replace the base with those scalars so
/// BLT can order orientation before `r_0`.
pub(crate) fn expand_array_base_unknowns(
    unknowns: &mut Vec<String>,
    scalar_lhs: &HashSet<String>,
    knowns: &HashSet<String>,
) {
    let mut extra = Vec::new();
    let mut remove = Vec::new();
    for base in unknowns.iter() {
        if scalar_lhs.contains(base) || knowns.contains(base) {
            continue;
        }
        let mut n = 0usize;
        for i in 1..=8 {
            let c = format!("{base}_{i}");
            if scalar_lhs.contains(&c) || knowns.contains(&c) {
                n = i;
            } else {
                break;
            }
        }
        if n >= 3 {
            remove.push(base.clone());
            for i in 1..=n {
                extra.push(format!("{base}_{i}"));
            }
        }
    }
    if remove.is_empty() {
        return;
    }
    unknowns.retain(|v| !remove.iter().any(|r| r == v));
    for e in extra {
        if !unknowns.contains(&e) && !knowns.contains(&e) {
            unknowns.push(e);
        }
    }
    unknowns.sort();
    unknowns.dedup();
}

pub(crate) fn collect_vars_eq(eq: &Equation, vars: &mut HashSet<String>) {
    match eq {
        Equation::Simple(l, r) => {
            collect_vars_expr(l, vars);
            collect_vars_expr(r, vars);
        }
        Equation::MultiAssign(lhss, r) => {
            for e in lhss {
                collect_vars_expr(e, vars);
            }
            collect_vars_expr(r, vars);
        }
        Equation::For(_, s, e, b) => {
            collect_vars_expr(s, vars);
            collect_vars_expr(e, vars);
            for sub in b {
                collect_vars_eq(sub, vars);
            }
        }
        Equation::When(c, b, e) => {
            collect_vars_expr(c, vars);
            for sub in b {
                collect_vars_eq(sub, vars);
            }
            for (ec, eb) in e {
                collect_vars_expr(ec, vars);
                for sub in eb {
                    collect_vars_eq(sub, vars);
                }
            }
        }
        Equation::If(c, then_eqs, elseif_list, else_eqs) => {
            collect_vars_expr(c, vars);
            for sub in then_eqs {
                collect_vars_eq(sub, vars);
            }
            for (ec, eb) in elseif_list {
                collect_vars_expr(ec, vars);
                for sub in eb {
                    collect_vars_eq(sub, vars);
                }
            }
            if let Some(eqs) = else_eqs {
                for sub in eqs {
                    collect_vars_eq(sub, vars);
                }
            }
        }
        Equation::Assert(cond, msg) => {
            collect_vars_expr(cond, vars);
            collect_vars_expr(msg, vars);
        }
        Equation::Terminate(msg) => collect_vars_expr(msg, vars),
        Equation::SolvableBlock {
            unknowns,
            tearing_var,
            equations,
            residuals,
        } => {
            for u in unknowns {
                vars.insert(u.clone());
            }
            if let Some(t) = tearing_var {
                vars.insert(t.clone());
            }
            for sub in equations {
                collect_vars_eq(sub, vars);
            }
            for r in residuals {
                collect_vars_expr(r, vars);
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_vars_alg(stmt: &AlgorithmStatement, vars: &mut HashSet<String>) {
    match stmt {
        AlgorithmStatement::Assignment(lhs, rhs) => {
            collect_vars_expr(lhs, vars);
            collect_vars_expr(rhs, vars);
        }
        AlgorithmStatement::MultiAssign(lhss, rhs) => {
            for e in lhss {
                collect_vars_expr(e, vars);
            }
            collect_vars_expr(rhs, vars);
        }
        AlgorithmStatement::CallStmt(expr) => {
            collect_vars_expr(expr, vars);
        }
        AlgorithmStatement::If(cond, then_stmts, elseif_list, else_stmts) => {
            collect_vars_expr(cond, vars);
            for s in then_stmts {
                collect_vars_alg(s, vars);
            }
            for (ec, es) in elseif_list {
                collect_vars_expr(ec, vars);
                for s in es {
                    collect_vars_alg(s, vars);
                }
            }
            if let Some(stmts) = else_stmts {
                for s in stmts {
                    collect_vars_alg(s, vars);
                }
            }
        }
        AlgorithmStatement::For(_, range, body) => {
            collect_vars_expr(range, vars);
            for s in body {
                collect_vars_alg(s, vars);
            }
        }
        AlgorithmStatement::While(cond, body) => {
            collect_vars_expr(cond, vars);
            for s in body {
                collect_vars_alg(s, vars);
            }
        }
        AlgorithmStatement::When(cond, body, elsewhen_list) => {
            collect_vars_expr(cond, vars);
            for s in body {
                collect_vars_alg(s, vars);
            }
            for (ec, es) in elsewhen_list {
                collect_vars_expr(ec, vars);
                for s in es {
                    collect_vars_alg(s, vars);
                }
            }
        }
        AlgorithmStatement::Reinit(var_name, expr) => {
            vars.insert(var_name.clone());
            collect_vars_expr(expr, vars);
        }
        AlgorithmStatement::Assert(cond, msg) => {
            collect_vars_expr(cond, vars);
            collect_vars_expr(msg, vars);
        }
        AlgorithmStatement::Terminate(msg) => {
            collect_vars_expr(msg, vars);
        }
        AlgorithmStatement::Return(Some(expr)) => {
            collect_vars_expr(expr, vars);
        }
        AlgorithmStatement::NoOp
        | AlgorithmStatement::Break
        | AlgorithmStatement::Return(None) => {}
    }
}

pub fn extract_unknowns_from_algorithm(
    stmt: &AlgorithmStatement,
    knowns: &HashSet<String>,
) -> Vec<String> {
    let mut vars = HashSet::new();
    collect_vars_alg(stmt, &mut vars);
    vars.into_iter()
        .filter(|v| !knowns.contains(v))
        .collect()
}

pub(crate) fn collect_vars_expr(expr: &Expression, vars: &mut HashSet<String>) {
    match expr {
        Expression::Variable(id) => {
            vars.insert(resolve_id(*id));
        }
        Expression::Der(e) => {
            if let Expression::Variable(id) = &**e {
                vars.insert(format!("der_{}", resolve_id(*id)));
            } else {
                collect_vars_expr(e, vars);
            }
        }
        Expression::BinaryOp(l, _, r) => {
            collect_vars_expr(l, vars);
            collect_vars_expr(r, vars);
        }
        Expression::Call(_, args) => {
            for a in args {
                collect_vars_expr(a, vars);
            }
        }
        Expression::ArrayAccess(a, i) => {
            collect_vars_expr(a, vars);
            collect_vars_expr(i, vars);
        }
        Expression::If(c, t, f) => {
            collect_vars_expr(c, vars);
            collect_vars_expr(t, vars);
            collect_vars_expr(f, vars);
        }
        Expression::ArrayLiteral(es) => {
            for e in es {
                collect_vars_expr(e, vars);
            }
        }
        Expression::Dot(b, _) => {
            collect_vars_expr(b, vars);
        }
        Expression::Sample(inner) => collect_vars_expr(inner, vars),
        Expression::Interval(inner) => collect_vars_expr(inner, vars),
        Expression::Hold(inner) => collect_vars_expr(inner, vars),
        Expression::Previous(inner) => collect_vars_expr(inner, vars),
        Expression::SubSample(c, n)
        | Expression::SuperSample(c, n)
        | Expression::ShiftSample(c, n)
        | Expression::BackSample(c, n) => {
            collect_vars_expr(c, vars);
            collect_vars_expr(n, vars);
        }
        _ => {}
    }
}

pub(crate) fn equation_contains_var(eq: &Equation, var: &str) -> bool {
    let mut vars = HashSet::new();
    collect_vars_eq(eq, &mut vars);
    vars.contains(var)
}

pub fn contains_var(expr: &Expression, var_name: &str) -> bool {
    let mut stack: Vec<&Expression> = vec![expr];
    while let Some(e) = stack.pop() {
        match e {
            Expression::Variable(id) => {
                if resolve_id(*id) == var_name {
                    return true;
                }
            }
            Expression::BinaryOp(lhs, _, rhs) => {
                stack.push(rhs);
                stack.push(lhs);
            }
            Expression::Call(_, args) => {
                for a in args {
                    stack.push(a);
                }
            }
            Expression::Der(arg) => stack.push(arg),
            Expression::ArrayAccess(arr, idx) => {
                stack.push(idx);
                stack.push(arr);
            }
            Expression::Dot(base, _) => stack.push(base),
            Expression::If(c, t, f) => {
                stack.push(f);
                stack.push(t);
                stack.push(c);
            }
            Expression::ArrayLiteral(es) => {
                for a in es {
                    stack.push(a);
                }
            }
            Expression::Range(start, step, end) => {
                stack.push(end);
                stack.push(step);
                stack.push(start);
            }
            Expression::Sample(inner)
            | Expression::Interval(inner)
            | Expression::Hold(inner)
            | Expression::Previous(inner) => stack.push(inner),
            Expression::SubSample(c, n)
            | Expression::SuperSample(c, n)
            | Expression::ShiftSample(c, n)
            | Expression::BackSample(c, n) => {
                stack.push(n);
                stack.push(c);
            }
            _ => {}
        }
    }
    false
}
