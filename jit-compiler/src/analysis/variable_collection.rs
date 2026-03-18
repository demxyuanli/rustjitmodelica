use crate::ast::{Equation, Expression};
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
        Expression::SubSample(c, n)
        | Expression::SuperSample(c, n)
        | Expression::ShiftSample(c, n) => {
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
            Expression::Variable(name) => {
                if name == var_name {
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
            | Expression::ShiftSample(c, n) => {
                stack.push(n);
                stack.push(c);
            }
            _ => {}
        }
    }
    false
}
