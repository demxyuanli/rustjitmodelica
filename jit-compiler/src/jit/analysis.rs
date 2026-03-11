use std::collections::HashSet;
use crate::ast::{AlgorithmStatement, Equation, Expression};

pub fn collect_modified(stmt: &AlgorithmStatement, vars: &mut HashSet<String>) {
    match stmt {
        AlgorithmStatement::Assignment(lhs, _) => {
            if let Expression::Variable(name) = lhs {
                vars.insert(name.clone());
            }
        }
        AlgorithmStatement::If(_, true_stmts, else_ifs, else_stmts) => {
            for s in true_stmts { collect_modified(s, vars); }
            for (_, s) in else_ifs { for stmt in s { collect_modified(stmt, vars); } }
            if let Some(s) = else_stmts { for stmt in s { collect_modified(stmt, vars); } }
        }
        AlgorithmStatement::For(var_name, _, body) => {
            vars.insert(var_name.clone()); // Loop variable is implicitly modified
            for s in body { collect_modified(s, vars); }
        }
        AlgorithmStatement::While(_, body) => {
            for s in body { collect_modified(s, vars); }
        }
        AlgorithmStatement::When(_, body, else_whens) => {
            for s in body { collect_modified(s, vars); }
            for (_, s) in else_whens { for stmt in s { collect_modified(stmt, vars); } }
        }
        AlgorithmStatement::Reinit(_, _) => {}
        AlgorithmStatement::Assert(_, _) | AlgorithmStatement::Terminate(_) => {}
    }
}

pub fn collect_modified_equations(equations: &[Equation], vars: &mut HashSet<String>) {
    for eq in equations {
        if let Equation::SolvableBlock { unknowns, .. } = eq {
            for u in unknowns {
                vars.insert(u.clone());
            }
        } else if let Equation::For(loop_var, _, _, body) = eq {
            vars.insert(loop_var.clone());
            collect_modified_equations(body, vars);
        } else if let Equation::Assert(_, _) | Equation::Terminate(_) = eq {
        } else if let Equation::If(_, then_eqs, elseif_list, else_eqs) = eq {
            collect_modified_equations(then_eqs, vars);
            for (_, eb) in elseif_list {
                collect_modified_equations(eb, vars);
            }
            if let Some(eqs) = else_eqs {
                collect_modified_equations(eqs, vars);
            }
        }
    }
}
