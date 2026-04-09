use crate::jit::context::TranslationContext;
use super::super::expr::compile_expression;
use crate::ast::{Expression, Operator};
use cranelift::prelude::*;

pub(super) fn is_store_target_expr(expr: &Expression) -> bool {
    match expr {
        Expression::Variable(_) | Expression::ArrayAccess(_, _) => true,
        Expression::BinaryOp(l, Operator::Sub, r) => {
            matches!(&**l, Expression::Number(n) if *n == 0.0) && is_store_target_expr(r)
        }
        _ => false,
    }
}

pub(super) fn expr_contains_array_literal(expr: &Expression) -> bool {
    match expr {
        Expression::ArrayLiteral(_) => true,
        Expression::BinaryOp(l, _, r) => {
            expr_contains_array_literal(l) || expr_contains_array_literal(r)
        }
        Expression::Call(_, args) => args.iter().any(expr_contains_array_literal),
        Expression::Der(inner)
        | Expression::Sample(inner)
        | Expression::Interval(inner)
        | Expression::Hold(inner)
        | Expression::Previous(inner) => expr_contains_array_literal(inner),
        Expression::SubSample(a, b)
        | Expression::SuperSample(a, b)
        | Expression::ShiftSample(a, b)
        | Expression::BackSample(a, b)
        | Expression::ArrayAccess(a, b) => {
            expr_contains_array_literal(a) || expr_contains_array_literal(b)
        }
        Expression::Dot(base, _) => expr_contains_array_literal(base),
        Expression::If(c, t, f) => {
            expr_contains_array_literal(c)
                || expr_contains_array_literal(t)
                || expr_contains_array_literal(f)
        }
        Expression::Range(s, st, e) => {
            expr_contains_array_literal(s)
                || expr_contains_array_literal(st)
                || expr_contains_array_literal(e)
        }
        Expression::ArrayComprehension {
            expr, iter_range, ..
        } => expr_contains_array_literal(expr) || expr_contains_array_literal(iter_range),
        Expression::Variable(_) | Expression::Number(_) | Expression::StringLiteral(_) => false,
    }
}

pub(super) fn is_record_constructor_call_name(name: &str) -> bool {
    if name.contains('.') {
        return false;
    }
    name.chars()
        .next()
        .map(|c| c.is_ascii_uppercase())
        .unwrap_or(false)
}

pub(super) fn expand_array_comprehension_values(
    rhs: &Expression,
    expected_len: usize,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<Option<Vec<Value>>, String> {
    let Expression::ArrayComprehension {
        expr,
        iter_var,
        iter_range,
    } = rhs
    else {
        return Ok(None);
    };
    let (start_val, step_val, literal_triplet) = match iter_range.as_ref() {
        Expression::Range(s, st, e) => {
            let sv = compile_expression(s.as_ref(), ctx, builder)?;
            let stv = compile_expression(st.as_ref(), ctx, builder)?;
            let lit = match (s.as_ref(), st.as_ref(), e.as_ref()) {
                (Expression::Number(a), Expression::Number(b), Expression::Number(c)) => {
                    Some((*a, *b, *c))
                }
                _ => None,
            };
            (sv, stv, lit)
        }
        Expression::Number(n) => (
            builder.ins().f64const(1.0),
            builder.ins().f64const(1.0),
            Some((1.0, 1.0, *n)),
        ),
        _ => return Ok(None),
    };

    if let Some((start, step, end)) = literal_triplet {
        if !start.is_finite() || !step.is_finite() || !end.is_finite() || step == 0.0 {
            return Ok(None);
        }
        let mut lit_len = 0usize;
        let mut i = start;
        let forward = step > 0.0;
        while (forward && i <= end) || (!forward && i >= end) {
            lit_len += 1;
            if lit_len > expected_len.saturating_mul(4).max(4096) {
                break;
            }
            i += step;
        }
        if lit_len != expected_len {
            return Err(format!(
                "Multi-assign array comprehension arity mismatch: {} LHS targets but {} generated items",
                expected_len,
                lit_len
            ));
        }
    }

    let mut values = Vec::new();
    for idx in 0..expected_len {
        let idx_val = builder.ins().f64const(idx as f64);
        let step_mul_idx = builder.ins().fmul(step_val, idx_val);
        let iter_val = builder.ins().fadd(start_val, step_mul_idx);
        let old = ctx.var_map.insert(iter_var.clone(), iter_val);
        let compiled = compile_expression(expr, ctx, builder);
        if let Some(prev) = old {
            ctx.var_map.insert(iter_var.clone(), prev);
        } else {
            ctx.var_map.remove(iter_var);
        }
        values.push(compiled?);
    }
    Ok(Some(values))
}
