use crate::ast::{AlgorithmStatement, Equation};
use crate::compiler::pipeline::log_stage_timing;
use crate::loader::ModelLoader;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use crate::ast::Model;
use crate::ast::Expression;
use crate::flatten::FlattenedModel;

use super::rewrite::{inline_expr, ResolveMemoEntry};

fn expr_may_need_inline(expr: &Expression) -> bool {
    use Expression::*;
    match expr {
        Call(_, _) => true,
        Variable(_) | Number(_) | StringLiteral(_) => false,
        BinaryOp(l, _, r) => expr_may_need_inline(l) || expr_may_need_inline(r),
        Der(i) | Sample(i) | Interval(i) | Hold(i) | Previous(i) => expr_may_need_inline(i),
        SubSample(c, n) | SuperSample(c, n) | ShiftSample(c, n) | BackSample(c, n) => {
            expr_may_need_inline(c) || expr_may_need_inline(n)
        }
        ArrayAccess(a, i) => expr_may_need_inline(a) || expr_may_need_inline(i),
        If(c, t, f) => expr_may_need_inline(c) || expr_may_need_inline(t) || expr_may_need_inline(f),
        Range(s, st, e) => expr_may_need_inline(s) || expr_may_need_inline(st) || expr_may_need_inline(e),
        ArrayLiteral(items) => items.iter().any(expr_may_need_inline),
        ArrayComprehension {
            expr: inner,
            iter_range,
            ..
        } => expr_may_need_inline(inner) || expr_may_need_inline(iter_range),
        Dot(base, _) => expr_may_need_inline(base),
    }
}

pub(super) fn inline_equation(
    eq: &Equation,
    loader: &mut ModelLoader,
    cache: &mut HashMap<String, Arc<Model>>,
    no_inline: &mut HashSet<String>,
    resolve_memo: &mut HashMap<String, ResolveMemoEntry>,
    max_depth: u32,
) -> Equation {
    match eq {
        Equation::Simple(lhs, rhs) => Equation::Simple(
            inline_expr(lhs, loader, cache, no_inline, resolve_memo, 0, max_depth),
            inline_expr(rhs, loader, cache, no_inline, resolve_memo, 0, max_depth),
        ),
        Equation::For(var, start, end, body) => Equation::For(
            var.clone(),
            Box::new(inline_expr(start, loader, cache, no_inline, resolve_memo, 0, max_depth)),
            Box::new(inline_expr(end, loader, cache, no_inline, resolve_memo, 0, max_depth)),
            body.iter()
                .map(|e| inline_equation(e, loader, cache, no_inline, resolve_memo, max_depth))
                .collect(),
        ),
        Equation::When(cond, body, elses) => Equation::When(
            inline_expr(cond, loader, cache, no_inline, resolve_memo, 0, max_depth),
            body.iter()
                .map(|e| inline_equation(e, loader, cache, no_inline, resolve_memo, max_depth))
                .collect(),
            elses
                .iter()
                .map(|(c, b)| {
                    (
                        inline_expr(c, loader, cache, no_inline, resolve_memo, 0, max_depth),
                        b.iter()
                            .map(|e| inline_equation(e, loader, cache, no_inline, resolve_memo, max_depth))
                            .collect(),
                    )
                })
                .collect(),
        ),
        Equation::Reinit(var, e) => {
            Equation::Reinit(var.clone(), inline_expr(e, loader, cache, no_inline, resolve_memo, 0, max_depth))
        }
        Equation::Connect(a, b) => Equation::Connect(
            inline_expr(a, loader, cache, no_inline, resolve_memo, 0, max_depth),
            inline_expr(b, loader, cache, no_inline, resolve_memo, 0, max_depth),
        ),
        Equation::Assert(cond, msg) => Equation::Assert(
            inline_expr(cond, loader, cache, no_inline, resolve_memo, 0, max_depth),
            inline_expr(msg, loader, cache, no_inline, resolve_memo, 0, max_depth),
        ),
        Equation::Terminate(msg) => {
            Equation::Terminate(inline_expr(msg, loader, cache, no_inline, resolve_memo, 0, max_depth))
        }
        Equation::CallStmt(expr) => {
            Equation::CallStmt(inline_expr(expr, loader, cache, no_inline, resolve_memo, 0, max_depth))
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
                .map(|e| inline_equation(e, loader, cache, no_inline, resolve_memo, max_depth))
                .collect(),
            residuals: residuals
                .iter()
                .map(|r| inline_expr(r, loader, cache, no_inline, resolve_memo, 0, max_depth))
                .collect(),
        },
        Equation::If(cond, then_eqs, elseif_list, else_eqs) => Equation::If(
            inline_expr(cond, loader, cache, no_inline, resolve_memo, 0, max_depth),
            then_eqs
                .iter()
                .map(|e| inline_equation(e, loader, cache, no_inline, resolve_memo, max_depth))
                .collect(),
            elseif_list
                .iter()
                .map(|(c, eb)| {
                    (
                        inline_expr(c, loader, cache, no_inline, resolve_memo, 0, max_depth),
                        eb.iter()
                            .map(|e| inline_equation(e, loader, cache, no_inline, resolve_memo, max_depth))
                            .collect(),
                    )
                })
                .collect(),
            else_eqs.as_ref().map(|eqs| {
                eqs.iter()
                    .map(|e| inline_equation(e, loader, cache, no_inline, resolve_memo, max_depth))
                    .collect()
            }),
        ),
        Equation::MultiAssign(lhss, rhs) => Equation::MultiAssign(
            lhss.iter()
                .map(|e| inline_expr(e, loader, cache, no_inline, resolve_memo, 0, max_depth))
                .collect(),
            inline_expr(rhs, loader, cache, no_inline, resolve_memo, 0, max_depth),
        ),
    }
}

pub(super) fn inline_algorithm(
    stmt: &AlgorithmStatement,
    loader: &mut ModelLoader,
    cache: &mut HashMap<String, Arc<Model>>,
    no_inline: &mut HashSet<String>,
    resolve_memo: &mut HashMap<String, ResolveMemoEntry>,
    max_depth: u32,
) -> AlgorithmStatement {
    match stmt {
        AlgorithmStatement::Assignment(lhs, rhs) => {
            AlgorithmStatement::Assignment(lhs.clone(), inline_expr(rhs, loader, cache, no_inline, resolve_memo, 0, max_depth))
        }
        AlgorithmStatement::CallStmt(expr) => {
            AlgorithmStatement::CallStmt(inline_expr(expr, loader, cache, no_inline, resolve_memo, 0, max_depth))
        }
        AlgorithmStatement::NoOp => AlgorithmStatement::NoOp,
        AlgorithmStatement::MultiAssign(lhss, rhs) => AlgorithmStatement::MultiAssign(
            lhss.iter()
                .map(|e| inline_expr(e, loader, cache, no_inline, resolve_memo, 0, max_depth))
                .collect(),
            inline_expr(rhs, loader, cache, no_inline, resolve_memo, 0, max_depth),
        ),
        AlgorithmStatement::Reinit(var, e) => {
            AlgorithmStatement::Reinit(var.clone(), inline_expr(e, loader, cache, no_inline, resolve_memo, 0, max_depth))
        }
        AlgorithmStatement::Assert(cond, msg) => AlgorithmStatement::Assert(
            inline_expr(cond, loader, cache, no_inline, resolve_memo, 0, max_depth),
            inline_expr(msg, loader, cache, no_inline, resolve_memo, 0, max_depth),
        ),
        AlgorithmStatement::Terminate(msg) => {
            AlgorithmStatement::Terminate(inline_expr(msg, loader, cache, no_inline, resolve_memo, 0, max_depth))
        }
        AlgorithmStatement::If(cond, t, eifs, els) => AlgorithmStatement::If(
            inline_expr(cond, loader, cache, no_inline, resolve_memo, 0, max_depth),
            t.iter()
                .map(|s| inline_algorithm(s, loader, cache, no_inline, resolve_memo, max_depth))
                .collect(),
            eifs.iter()
                .map(|(c, b)| {
                    (
                        inline_expr(c, loader, cache, no_inline, resolve_memo, 0, max_depth),
                        b.iter()
                            .map(|s| inline_algorithm(s, loader, cache, no_inline, resolve_memo, max_depth))
                            .collect(),
                    )
                })
                .collect(),
            els.as_ref().map(|b| {
                b.iter()
                    .map(|s| inline_algorithm(s, loader, cache, no_inline, resolve_memo, max_depth))
                    .collect()
            }),
        ),
        AlgorithmStatement::For(var, range, body) => AlgorithmStatement::For(
            var.clone(),
            Box::new(inline_expr(range, loader, cache, no_inline, resolve_memo, 0, max_depth)),
            body.iter()
                .map(|s| inline_algorithm(s, loader, cache, no_inline, resolve_memo, max_depth))
                .collect(),
        ),
        AlgorithmStatement::While(cond, body) => AlgorithmStatement::While(
            inline_expr(cond, loader, cache, no_inline, resolve_memo, 0, max_depth),
            body.iter()
                .map(|s| inline_algorithm(s, loader, cache, no_inline, resolve_memo, max_depth))
                .collect(),
        ),
        AlgorithmStatement::When(cond, body, elses) => AlgorithmStatement::When(
            inline_expr(cond, loader, cache, no_inline, resolve_memo, 0, max_depth),
            body.iter()
                .map(|s| inline_algorithm(s, loader, cache, no_inline, resolve_memo, max_depth))
                .collect(),
            elses
                .iter()
                .map(|(c, b)| {
                    (
                        inline_expr(c, loader, cache, no_inline, resolve_memo, 0, max_depth),
                        b.iter()
                            .map(|s| inline_algorithm(s, loader, cache, no_inline, resolve_memo, max_depth))
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
    resolve_memo: &mut HashMap<String, ResolveMemoEntry>,
    max_depth: u32,
    stage_trace: bool,
) {
    let n_start = flat
        .declarations
        .iter()
        .filter(|d| d.start_value.is_some())
        .count() as u64;
    crate::query_db::perf_record_add("inline_input_declarations", flat.declarations.len() as u64);
    crate::query_db::perf_record_add("inline_input_equations", flat.equations.len() as u64);
    crate::query_db::perf_record_add(
        "inline_input_initial_equations",
        flat.initial_equations.len() as u64,
    );
    crate::query_db::perf_record_add("inline_input_algorithms", flat.algorithms.len() as u64);
    crate::query_db::perf_record_add(
        "inline_input_initial_algorithms",
        flat.initial_algorithms.len() as u64,
    );
    crate::query_db::perf_record_add("inline_declarations_with_start_value", n_start);

    let mut cache: HashMap<String, Arc<Model>> = HashMap::new();
    let mut no_inline: HashSet<String> = HashSet::new();

    let t0 = Instant::now();
    for decl in &mut flat.declarations {
        if let Some(ref sv) = decl.start_value {
            if !expr_may_need_inline(sv) {
                continue;
            }
            decl.start_value = Some(inline_expr(
                sv,
                loader,
                &mut cache,
                &mut no_inline,
                resolve_memo,
                0,
                max_depth,
            ));
        }
    }
    crate::query_db::perf_record_us(
        "inline_pass_decl_start_values_us",
        t0.elapsed().as_micros() as u64,
    );
    log_stage_timing(stage_trace, "inline.decl_start_values", t0);

    let t0 = Instant::now();
    for e in &mut flat.equations {
        *e = inline_equation(e, loader, &mut cache, &mut no_inline, resolve_memo, max_depth);
    }
    crate::query_db::perf_record_us("inline_pass_equations_us", t0.elapsed().as_micros() as u64);
    log_stage_timing(stage_trace, "inline.equations", t0);

    let t0 = Instant::now();
    for e in &mut flat.initial_equations {
        *e = inline_equation(e, loader, &mut cache, &mut no_inline, resolve_memo, max_depth);
    }
    crate::query_db::perf_record_us(
        "inline_pass_initial_equations_us",
        t0.elapsed().as_micros() as u64,
    );
    log_stage_timing(stage_trace, "inline.initial_equations", t0);

    let t0 = Instant::now();
    for s in &mut flat.algorithms {
        *s = inline_algorithm(s, loader, &mut cache, &mut no_inline, resolve_memo, max_depth);
    }
    crate::query_db::perf_record_us(
        "inline_pass_algorithms_us",
        t0.elapsed().as_micros() as u64,
    );
    log_stage_timing(stage_trace, "inline.algorithms", t0);

    let t0 = Instant::now();
    for s in &mut flat.initial_algorithms {
        *s = inline_algorithm(s, loader, &mut cache, &mut no_inline, resolve_memo, max_depth);
    }
    crate::query_db::perf_record_us(
        "inline_pass_initial_algorithms_us",
        t0.elapsed().as_micros() as u64,
    );
    log_stage_timing(stage_trace, "inline.initial_algorithms", t0);
}
