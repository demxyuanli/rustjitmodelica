use crate::ast::{AlgorithmStatement, Equation, Expression};
use std::collections::HashSet;

fn collect_vars_expr(expr: &Expression, vars: &mut HashSet<String>) {
    match expr {
        Expression::Variable(name) => {
            vars.insert(name.clone());
        }
        Expression::Number(_) | Expression::StringLiteral(_) => {}
        Expression::BinaryOp(l, _, r) => {
            collect_vars_expr(l, vars);
            collect_vars_expr(r, vars);
        }
        Expression::Call(_, args) | Expression::ArrayLiteral(args) => {
            for a in args {
                collect_vars_expr(a, vars);
            }
        }
        Expression::Der(inner)
        | Expression::Sample(inner)
        | Expression::Interval(inner)
        | Expression::Hold(inner)
        | Expression::Previous(inner) => {
            collect_vars_expr(inner, vars);
        }
        Expression::SubSample(c, n)
        | Expression::SuperSample(c, n)
        | Expression::ShiftSample(c, n) => {
            collect_vars_expr(c, vars);
            collect_vars_expr(n, vars);
        }
        Expression::ArrayAccess(a, i) => {
            collect_vars_expr(a, vars);
            collect_vars_expr(i, vars);
        }
        Expression::Dot(b, _) => {
            collect_vars_expr(b, vars);
        }
        Expression::If(c, t, f) => {
            collect_vars_expr(c, vars);
            collect_vars_expr(t, vars);
            collect_vars_expr(f, vars);
        }
        Expression::Range(s, st, e) => {
            collect_vars_expr(s, vars);
            collect_vars_expr(st, vars);
            collect_vars_expr(e, vars);
        }
        Expression::ArrayComprehension { expr, iter_range, .. } => {
            collect_vars_expr(expr, vars);
            collect_vars_expr(iter_range, vars);
        }
    }
}

pub fn collect_modified(stmt: &AlgorithmStatement, vars: &mut HashSet<String>) {
    match stmt {
        AlgorithmStatement::Assignment(lhs, _) => {
            if let Expression::Variable(name) = lhs {
                vars.insert(name.clone());
            }
        }
        AlgorithmStatement::MultiAssign(lhss, _) => {
            for lhs in lhss {
                if let Expression::Variable(name) = lhs {
                    vars.insert(name.clone());
                }
            }
        }
        AlgorithmStatement::If(_, true_stmts, else_ifs, else_stmts) => {
            for s in true_stmts {
                collect_modified(s, vars);
            }
            for (_, s) in else_ifs {
                for stmt in s {
                    collect_modified(stmt, vars);
                }
            }
            if let Some(s) = else_stmts {
                for stmt in s {
                    collect_modified(stmt, vars);
                }
            }
        }
        AlgorithmStatement::For(var_name, _, body) => {
            vars.insert(var_name.clone()); // Loop variable is implicitly modified
            for s in body {
                collect_modified(s, vars);
            }
        }
        AlgorithmStatement::While(_, body) => {
            for s in body {
                collect_modified(s, vars);
            }
        }
        AlgorithmStatement::When(_, body, else_whens) => {
            for s in body {
                collect_modified(s, vars);
            }
            for (_, s) in else_whens {
                for stmt in s {
                    collect_modified(stmt, vars);
                }
            }
        }
        AlgorithmStatement::Reinit(_, _) => {}
        AlgorithmStatement::Assert(_, _)
        | AlgorithmStatement::Terminate(_)
        | AlgorithmStatement::CallStmt(_)
        | AlgorithmStatement::NoOp => {}
    }
}

pub fn collect_modified_equations(equations: &[Equation], vars: &mut HashSet<String>) {
    for eq in equations {
        match eq {
            Equation::Simple(lhs, _) => {
                if let Expression::Variable(name) = lhs {
                    vars.insert(name.clone());
                }
            }
            Equation::MultiAssign(lhss, _) => {
                for lhs in lhss {
                    if let Expression::Variable(name) = lhs {
                        vars.insert(name.clone());
                    }
                }
            }
            Equation::SolvableBlock { unknowns, tearing_var, .. } => {
                for u in unknowns {
                    vars.insert(u.clone());
                }
                if let Some(tv) = tearing_var {
                    vars.insert(tv.clone());
                }
            }
            Equation::For(loop_var, _, _, body) => {
                // Loop var must get a stack slot so JIT can iterate; collect it as modified.
                vars.insert(loop_var.clone());
                collect_modified_equations(body, vars);
            }
            Equation::When(_, body, else_whens) => {
                collect_modified_equations(body, vars);
                for (_, b) in else_whens {
                    collect_modified_equations(b, vars);
                }
            }
            Equation::If(_, then_eqs, elseif_list, else_eqs) => {
                collect_modified_equations(then_eqs, vars);
                for (_, eb) in elseif_list {
                    collect_modified_equations(eb, vars);
                }
                if let Some(eqs) = else_eqs {
                    collect_modified_equations(eqs, vars);
                }
            }
            Equation::Reinit(_, _)
            | Equation::Connect(_, _)
            | Equation::Assert(_, _)
            | Equation::Terminate(_)
            | Equation::CallStmt(_) => {}
        }

        // Also collect referenced variables to allow forward references / BLT temporaries.
        match eq {
            Equation::Simple(l, r) => {
                collect_vars_expr(l, vars);
                collect_vars_expr(r, vars);
            }
            Equation::MultiAssign(lhss, r) => {
                for l in lhss {
                    collect_vars_expr(l, vars);
                }
                collect_vars_expr(r, vars);
            }
            Equation::For(_, s, e, body) => {
                collect_vars_expr(s, vars);
                collect_vars_expr(e, vars);
                collect_modified_equations(body, vars);
            }
            Equation::When(c, body, else_whens) => {
                collect_vars_expr(c, vars);
                collect_modified_equations(body, vars);
                for (ec, eb) in else_whens {
                    collect_vars_expr(ec, vars);
                    collect_modified_equations(eb, vars);
                }
            }
            Equation::If(c, then_eqs, elseif_list, else_eqs) => {
                collect_vars_expr(c, vars);
                collect_modified_equations(then_eqs, vars);
                for (ec, eb) in elseif_list {
                    collect_vars_expr(ec, vars);
                    collect_modified_equations(eb, vars);
                }
                if let Some(eqs) = else_eqs {
                    collect_modified_equations(eqs, vars);
                }
            }
            Equation::Connect(a, b) => {
                collect_vars_expr(a, vars);
                collect_vars_expr(b, vars);
            }
            Equation::Reinit(_, v) => {
                collect_vars_expr(v, vars);
            }
            Equation::Assert(cond, msg) => {
                collect_vars_expr(cond, vars);
                collect_vars_expr(msg, vars);
            }
            Equation::Terminate(msg) => {
                collect_vars_expr(msg, vars);
            }
            Equation::CallStmt(expr) => {
                collect_vars_expr(expr, vars);
            }
            Equation::SolvableBlock { residuals, .. } => {
                for r in residuals {
                    collect_vars_expr(r, vars);
                }
            }
        }
    }
}
