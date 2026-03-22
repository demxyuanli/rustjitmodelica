use crate::ast::{AlgorithmStatement, Equation};
use crate::loader::ModelLoader;
use std::collections::HashMap;
use std::sync::Arc;

use crate::ast::Model;
use crate::flatten::FlattenedModel;

use super::rewrite::inline_expr;

pub(super) fn inline_equation(
    eq: &Equation,
    loader: &mut ModelLoader,
    cache: &mut HashMap<String, Arc<Model>>,
    max_depth: u32,
) -> Equation {
    match eq {
        Equation::Simple(lhs, rhs) => Equation::Simple(
            inline_expr(lhs, loader, cache, 0, max_depth),
            inline_expr(rhs, loader, cache, 0, max_depth),
        ),
        Equation::For(var, start, end, body) => Equation::For(
            var.clone(),
            Box::new(inline_expr(start, loader, cache, 0, max_depth)),
            Box::new(inline_expr(end, loader, cache, 0, max_depth)),
            body.iter()
                .map(|e| inline_equation(e, loader, cache, max_depth))
                .collect(),
        ),
        Equation::When(cond, body, elses) => Equation::When(
            inline_expr(cond, loader, cache, 0, max_depth),
            body.iter()
                .map(|e| inline_equation(e, loader, cache, max_depth))
                .collect(),
            elses
                .iter()
                .map(|(c, b)| {
                    (
                        inline_expr(c, loader, cache, 0, max_depth),
                        b.iter()
                            .map(|e| inline_equation(e, loader, cache, max_depth))
                            .collect(),
                    )
                })
                .collect(),
        ),
        Equation::Reinit(var, e) => {
            Equation::Reinit(var.clone(), inline_expr(e, loader, cache, 0, max_depth))
        }
        Equation::Connect(a, b) => Equation::Connect(
            inline_expr(a, loader, cache, 0, max_depth),
            inline_expr(b, loader, cache, 0, max_depth),
        ),
        Equation::Assert(cond, msg) => Equation::Assert(
            inline_expr(cond, loader, cache, 0, max_depth),
            inline_expr(msg, loader, cache, 0, max_depth),
        ),
        Equation::Terminate(msg) => {
            Equation::Terminate(inline_expr(msg, loader, cache, 0, max_depth))
        }
        Equation::CallStmt(expr) => {
            Equation::CallStmt(inline_expr(expr, loader, cache, 0, max_depth))
        }
        Equation::SolvableBlock {
            unknowns,
            tearing_var,
            equations,
            residuals,
        } => Equation::SolvableBlock {
            unknowns: unknowns.clone(),
            tearing_var: tearing_var.clone(),
            equations: equations
                .iter()
                .map(|e| inline_equation(e, loader, cache, max_depth))
                .collect(),
            residuals: residuals
                .iter()
                .map(|r| inline_expr(r, loader, cache, 0, max_depth))
                .collect(),
        },
        Equation::If(cond, then_eqs, elseif_list, else_eqs) => Equation::If(
            inline_expr(cond, loader, cache, 0, max_depth),
            then_eqs
                .iter()
                .map(|e| inline_equation(e, loader, cache, max_depth))
                .collect(),
            elseif_list
                .iter()
                .map(|(c, eb)| {
                    (
                        inline_expr(c, loader, cache, 0, max_depth),
                        eb.iter()
                            .map(|e| inline_equation(e, loader, cache, max_depth))
                            .collect(),
                    )
                })
                .collect(),
            else_eqs.as_ref().map(|eqs| {
                eqs.iter()
                    .map(|e| inline_equation(e, loader, cache, max_depth))
                    .collect()
            }),
        ),
        Equation::MultiAssign(lhss, rhs) => Equation::MultiAssign(
            lhss.iter()
                .map(|e| inline_expr(e, loader, cache, 0, max_depth))
                .collect(),
            inline_expr(rhs, loader, cache, 0, max_depth),
        ),
    }
}

pub(super) fn inline_algorithm(
    stmt: &AlgorithmStatement,
    loader: &mut ModelLoader,
    cache: &mut HashMap<String, Arc<Model>>,
    max_depth: u32,
) -> AlgorithmStatement {
    match stmt {
        AlgorithmStatement::Assignment(lhs, rhs) => {
            AlgorithmStatement::Assignment(lhs.clone(), inline_expr(rhs, loader, cache, 0, max_depth))
        }
        AlgorithmStatement::CallStmt(expr) => {
            AlgorithmStatement::CallStmt(inline_expr(expr, loader, cache, 0, max_depth))
        }
        AlgorithmStatement::NoOp => AlgorithmStatement::NoOp,
        AlgorithmStatement::MultiAssign(lhss, rhs) => AlgorithmStatement::MultiAssign(
            lhss.iter()
                .map(|e| inline_expr(e, loader, cache, 0, max_depth))
                .collect(),
            inline_expr(rhs, loader, cache, 0, max_depth),
        ),
        AlgorithmStatement::Reinit(var, e) => {
            AlgorithmStatement::Reinit(var.clone(), inline_expr(e, loader, cache, 0, max_depth))
        }
        AlgorithmStatement::Assert(cond, msg) => AlgorithmStatement::Assert(
            inline_expr(cond, loader, cache, 0, max_depth),
            inline_expr(msg, loader, cache, 0, max_depth),
        ),
        AlgorithmStatement::Terminate(msg) => {
            AlgorithmStatement::Terminate(inline_expr(msg, loader, cache, 0, max_depth))
        }
        AlgorithmStatement::If(cond, t, eifs, els) => AlgorithmStatement::If(
            inline_expr(cond, loader, cache, 0, max_depth),
            t.iter()
                .map(|s| inline_algorithm(s, loader, cache, max_depth))
                .collect(),
            eifs.iter()
                .map(|(c, b)| {
                    (
                        inline_expr(c, loader, cache, 0, max_depth),
                        b.iter()
                            .map(|s| inline_algorithm(s, loader, cache, max_depth))
                            .collect(),
                    )
                })
                .collect(),
            els.as_ref().map(|b| {
                b.iter()
                    .map(|s| inline_algorithm(s, loader, cache, max_depth))
                    .collect()
            }),
        ),
        AlgorithmStatement::For(var, range, body) => AlgorithmStatement::For(
            var.clone(),
            Box::new(inline_expr(range, loader, cache, 0, max_depth)),
            body.iter()
                .map(|s| inline_algorithm(s, loader, cache, max_depth))
                .collect(),
        ),
        AlgorithmStatement::While(cond, body) => AlgorithmStatement::While(
            inline_expr(cond, loader, cache, 0, max_depth),
            body.iter()
                .map(|s| inline_algorithm(s, loader, cache, max_depth))
                .collect(),
        ),
        AlgorithmStatement::When(cond, body, elses) => AlgorithmStatement::When(
            inline_expr(cond, loader, cache, 0, max_depth),
            body.iter()
                .map(|s| inline_algorithm(s, loader, cache, max_depth))
                .collect(),
            elses
                .iter()
                .map(|(c, b)| {
                    (
                        inline_expr(c, loader, cache, 0, max_depth),
                        b.iter()
                            .map(|s| inline_algorithm(s, loader, cache, max_depth))
                            .collect(),
                    )
                })
                .collect(),
        ),
    }
}

pub(super) fn inline_function_calls_in_model(
    flat: &mut FlattenedModel,
    loader: &mut ModelLoader,
    max_depth: u32,
) {
    let mut cache: HashMap<String, Arc<Model>> = HashMap::new();
    for decl in &mut flat.declarations {
        if let Some(ref sv) = decl.start_value {
            decl.start_value = Some(inline_expr(sv, loader, &mut cache, 0, max_depth));
        }
    }
    flat.equations = flat
        .equations
        .iter()
        .map(|e| inline_equation(e, loader, &mut cache, max_depth))
        .collect();
    flat.initial_equations = flat
        .initial_equations
        .iter()
        .map(|e| inline_equation(e, loader, &mut cache, max_depth))
        .collect();
    flat.algorithms = flat
        .algorithms
        .iter()
        .map(|s| inline_algorithm(s, loader, &mut cache, max_depth))
        .collect();
    flat.initial_algorithms = flat
        .initial_algorithms
        .iter()
        .map(|s| inline_algorithm(s, loader, &mut cache, max_depth))
        .collect();
}
