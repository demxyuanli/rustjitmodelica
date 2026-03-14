use std::collections::HashSet;
use crate::ast::{Equation, Expression};

pub fn extract_unknowns(eq: &Equation, knowns: &HashSet<String>) -> Vec<String> {
    let mut vars = HashSet::new();
    collect_vars_eq(eq, &mut vars);
    let mut unknowns = Vec::new();
    for v in vars {
        if !knowns.contains(&v) {
            unknowns.push(v);
        }
    }
    unknowns
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
        _ => {}
    }
}

pub(crate) fn collect_vars_expr(expr: &Expression, vars: &mut HashSet<String>) {
    match expr {
        Expression::Variable(n) => {
            vars.insert(n.clone());
        }
        Expression::Der(e) => {
            if let Expression::Variable(n) = &**e {
                vars.insert(format!("der_{}", n));
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
        Expression::SubSample(c, n) | Expression::SuperSample(c, n) | Expression::ShiftSample(c, n) => {
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
    match expr {
        Expression::Variable(name) => name == var_name,
        Expression::BinaryOp(lhs, _, rhs) => contains_var(lhs, var_name) || contains_var(rhs, var_name),
        Expression::Call(_, args) => args.iter().any(|arg| contains_var(arg, var_name)),
        Expression::Der(arg) => contains_var(arg, var_name),
        Expression::ArrayAccess(arr, idx) => contains_var(arr, var_name) || contains_var(idx, var_name),
        Expression::Dot(base, _) => contains_var(base, var_name),
        Expression::If(c, t, f) => {
            contains_var(c, var_name) || contains_var(t, var_name) || contains_var(f, var_name)
        }
        Expression::ArrayLiteral(es) => es.iter().any(|e| contains_var(e, var_name)),
        Expression::Range(start, step, end) => {
            contains_var(start, var_name) || contains_var(step, var_name) || contains_var(end, var_name)
        }
        Expression::Sample(inner) => contains_var(inner, var_name),
        Expression::Interval(inner) => contains_var(inner, var_name),
        Expression::Hold(inner) => contains_var(inner, var_name),
        Expression::Previous(inner) => contains_var(inner, var_name),
        Expression::SubSample(c, n) | Expression::SuperSample(c, n) | Expression::ShiftSample(c, n) => {
            contains_var(c, var_name) || contains_var(n, var_name)
        }
        _ => false,
    }
}
