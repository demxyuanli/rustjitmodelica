use crate::ast::{Expression, Operator};
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;

use crate::diag::fallback_counter;
use crate::jit::context::TranslationContext;
use crate::jit::jit_policy::{
    dot_flat_path_yields_zero, dot_prefix_yields_zero, hysteresis_record_value,
};
use super::call::compile_call;
use super::clock_sample::compile_sample_interval_clock_arms;
use super::helpers::{
    jit_dot_fallback_zero_enabled, jit_dot_trace_enabled, jit_scalar_name_bound,
    modelica_constants_dot_member,
};
use super::matrix::fold_dot_symmetric_transformation_matrix;
use super::pre::compile_pre_expression;
use super::variable::{compile_array_access, compile_variable_load};

pub(crate) fn compile_zero_crossing_store(
    expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    if ctx.suppress_zero_crossings {
        return Ok(());
    }
    match expr {
        Expression::Call(name, args) if (name == "noEvent" || name.ends_with(".noEvent")) && args.len() == 1 => {
            let prev = ctx.suppress_zero_crossings;
            ctx.suppress_zero_crossings = true;
            let out = compile_zero_crossing_store(&args[0], ctx, builder);
            ctx.suppress_zero_crossings = prev;
            out?;
        }
        Expression::BinaryOp(lhs, op, rhs) => match op {
            Operator::Less | Operator::Greater | Operator::LessEq | Operator::GreaterEq => {
                let l = compile_expression(lhs, ctx, builder)?;
                let r = compile_expression(rhs, ctx, builder)?;
                let diff = builder.ins().fsub(l, r);
                let offset = (*ctx.crossings_idx * 8) as i32;
                builder
                    .ins()
                    .store(MemFlags::new(), diff, ctx.crossings_ptr, offset);
                *ctx.crossings_idx += 1;
            }
            Operator::And | Operator::Or => {
                compile_zero_crossing_store(lhs, ctx, builder)?;
                compile_zero_crossing_store(rhs, ctx, builder)?;
            }
            _ => {}
        },
        _ => {}
    }
    Ok(())
}

pub(super) fn compile_expression_rec(
    expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<Value, String> {
    if let Some(v) =
        compile_sample_interval_clock_arms(expr, ctx, builder, compile_expression_rec)?
    {
        return Ok(v);
    }
    match expr {
        Expression::Number(n) => Ok(builder.ins().f64const(*n)),
        Expression::Variable(id) => compile_variable_load(*id, ctx, builder),
        Expression::ArrayAccess(arr_expr, idx_expr) => {
            compile_array_access(expr, arr_expr, idx_expr, ctx, builder, compile_expression_rec)
        }
        Expression::BinaryOp(lhs, op, rhs) => {
            let l = compile_expression_rec(lhs, ctx, builder)?;
            let r = compile_expression_rec(rhs, ctx, builder)?;
            match op {
                Operator::Add => Ok(builder.ins().fadd(l, r)),
                Operator::Sub => Ok(builder.ins().fsub(l, r)),
                Operator::Mul => Ok(builder.ins().fmul(l, r)),
                Operator::Div => {
                    let min_den = builder.ins().f64const(1e-12);
                    let r_safe = builder.ins().fmax(r, min_den);
                    Ok(builder.ins().fdiv(l, r_safe))
                }
                Operator::Less
                | Operator::Greater
                | Operator::LessEq
                | Operator::GreaterEq
                | Operator::Equal
                | Operator::NotEqual => {
                    let cc = match op {
                        Operator::Less => FloatCC::LessThan,
                        Operator::Greater => FloatCC::GreaterThan,
                        Operator::LessEq => FloatCC::LessThanOrEqual,
                        Operator::GreaterEq => FloatCC::GreaterThanOrEqual,
                        Operator::Equal => FloatCC::Equal,
                        Operator::NotEqual => FloatCC::NotEqual,
                        _ => unreachable!(),
                    };
                    let cmp = builder.ins().fcmp(cc, l, r);
                    let one = builder.ins().f64const(1.0);
                    let zero = builder.ins().f64const(0.0);
                    Ok(builder.ins().select(cmp, one, zero))
                }
                Operator::And => {
                    let zero = builder.ins().f64const(0.0);
                    let l_bool = builder.ins().fcmp(FloatCC::NotEqual, l, zero);
                    let r_bool = builder.ins().fcmp(FloatCC::NotEqual, r, zero);
                    let res_bool = builder.ins().band(l_bool, r_bool);
                    let one = builder.ins().f64const(1.0);
                    Ok(builder.ins().select(res_bool, one, zero))
                }
                Operator::Or => {
                    let zero = builder.ins().f64const(0.0);
                    let l_bool = builder.ins().fcmp(FloatCC::NotEqual, l, zero);
                    let r_bool = builder.ins().fcmp(FloatCC::NotEqual, r, zero);
                    let res_bool = builder.ins().bor(l_bool, r_bool);
                    let one = builder.ins().f64const(1.0);
                    Ok(builder.ins().select(res_bool, one, zero))
                }
            }
        }
        Expression::If(cond, t_expr, f_expr) => {
            let c_val = compile_expression_rec(cond, ctx, builder)?;
            let t_val = compile_expression_rec(t_expr, ctx, builder)?;
            let f_val = compile_expression_rec(f_expr, ctx, builder)?;
            let zero = builder.ins().f64const(0.0);
            let cmp = builder.ins().fcmp(FloatCC::NotEqual, c_val, zero);
            Ok(builder.ins().select(cmp, t_val, f_val))
        }
        Expression::Call(func_name, args) => compile_call(func_name, args, ctx, builder, compile_expression_rec),
        Expression::Der(inner) => {
            if let Some(expanded) = crate::analysis::derivative::expand_der_linear(inner) {
                return compile_expression_rec(&expanded, ctx, builder);
            }
            if let Expression::Variable(id) = &**inner {
                let name = crate::string_intern::resolve_id(*id);
                if let Some(idx) = ctx.state_index(&name) {
                    let offset = (idx * 8) as i32;
                    return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.derivs_ptr, offset));
                }
            }
            let flat_name = crate::analysis::derivative::flatten_dot_to_name(inner);
            if let Some(ref flat) = flat_name {
                if let Some(idx) = ctx.state_index(flat) {
                    let offset = (idx * 8) as i32;
                    return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.derivs_ptr, offset));
                }
            }
            if jit_dot_trace_enabled() {
                fallback_counter::inc_jit_derivative();
                eprintln!(
                    "[fallback:jit-derivative] der fallback to 0.0 for expression {:?}",
                    inner
                );
            }
            Ok(builder.ins().f64const(0.0))
        }
        Expression::Range(_, _, _) => Ok(builder.ins().f64const(0.0)),
        Expression::Dot(inner, member) => {
            if let Some(prefix) = crate::ast::expr_to_connector_path(inner) {
                if let Some(v) = modelica_constants_dot_member(&prefix, member) {
                    return Ok(builder.ins().f64const(v));
                }
                if dot_prefix_yields_zero(&prefix) {
                    return Ok(builder.ins().f64const(0.0));
                }
            }
            if let Some(v) = fold_dot_symmetric_transformation_matrix(inner.as_ref(), member) {
                return Ok(builder.ins().f64const(v));
            }
            if let Expression::Call(func_name, args) = inner.as_ref() {
                if args.is_empty() {
                    if let Some(v) = hysteresis_record_value(func_name, member) {
                        return Ok(builder.ins().f64const(v));
                    }
                    let flat = format!("{}_{}", func_name.replace('.', "_"), member);
                    if jit_scalar_name_bound(ctx, &flat) {
                        return compile_expression_rec(&Expression::var(&flat), ctx, builder);
                    }
                    if let Some(suffix) = func_name.strip_prefix("FluxTubes.") {
                        let modelica_flat = format!(
                            "Modelica_Magnetic_FluxTubes_{}_{}",
                            suffix.replace('.', "_"),
                            member
                        );
                        if jit_scalar_name_bound(ctx, &modelica_flat) {
                            return compile_expression_rec(
                                &Expression::var(&modelica_flat),
                                ctx,
                                builder,
                            );
                        }
                    }
                }
            }
            if let Some(path) = crate::ast::expr_to_connector_path(expr) {
                if jit_scalar_name_bound(ctx, &path) {
                    return compile_expression_rec(&Expression::var(&path), ctx, builder);
                }
                let path_us = path.replace('.', "_");
                if jit_scalar_name_bound(ctx, &path_us) {
                    return compile_expression_rec(&Expression::var(&path_us), ctx, builder);
                }
            }
            if let Some(full_flat) = crate::ast::expr_to_flat_scalar_prefix(expr) {
                if jit_scalar_name_bound(ctx, &full_flat) {
                    return compile_expression_rec(&Expression::var(&full_flat), ctx, builder);
                }
            }
            if let Some(prefix) = crate::ast::expr_to_flat_scalar_prefix(inner) {
                let flat = format!("{}_{}", prefix, member);
                if jit_scalar_name_bound(ctx, &flat) {
                    return compile_expression_rec(&Expression::var(&flat), ctx, builder);
                }
            }
            if let Some(path) = crate::ast::expr_to_connector_path(expr) {
                if dot_flat_path_yields_zero(&path) {
                    return Ok(builder.ins().f64const(0.0));
                }
            }
            // Enumeration literal access: E.field → integer index.
            if let Expression::Variable(type_id) = inner.as_ref() {
                let type_name = crate::string_intern::resolve_id(*type_id);
                if let Some(literals) = ctx.enumerations.get(&type_name) {
                    if let Some(idx) = literals.iter().position(|l| l == member) {
                        return Ok(builder.ins().f64const(idx as f64));
                    }
                }
            }
            if jit_dot_trace_enabled() {
                eprintln!(
                    "[jit-dot-trace] JIT_DOT_RESIDUAL member={} inner={:?} full_expr={:?}",
                    member, inner, expr
                );
            }
            if jit_dot_fallback_zero_enabled() {
                return Ok(builder.ins().f64const(0.0));
            }
            Err("Array access (nested) and Dot should have been flattened before JIT compilation"
                .to_string())
        }
        Expression::ArrayLiteral(es) => {
            if let Some(first) = es.first() {
                compile_expression_rec(first, ctx, builder)
            } else {
                Ok(builder.ins().f64const(0.0))
            }
        }
        Expression::ArrayComprehension { expr, iter_var, iter_range } => {
            let (start_val, step_val, end_val) = match iter_range.as_ref() {
                Expression::Range(start, step, end) => (
                    compile_expression_rec(start.as_ref(), ctx, builder)?,
                    compile_expression_rec(step.as_ref(), ctx, builder)?,
                    compile_expression_rec(end.as_ref(), ctx, builder)?,
                ),
                Expression::Number(n) => (
                    builder.ins().f64const(1.0),
                    builder.ins().f64const(1.0),
                    builder.ins().f64const(*n),
                ),
                _ => {
                    return Err(format!(
                        "Unsupported array comprehension range in scalar JIT path: {:?}",
                        iter_range
                    ))
                }
            };

            let old_val = ctx.var_map.get(iter_var).copied();
            ctx.var_map.insert(iter_var.clone(), start_val);
            let result = compile_expression_rec(expr, ctx, builder);
            match old_val {
                Some(v) => {
                    ctx.var_map.insert(iter_var.clone(), v);
                }
                None => {
                    ctx.var_map.remove(iter_var);
                }
            }

            let zero = builder.ins().f64const(0.0);
            let step_pos = builder.ins().fcmp(FloatCC::GreaterThan, step_val, zero);
            let step_neg = builder.ins().fcmp(FloatCC::LessThan, step_val, zero);
            let step_nonzero = builder.ins().fcmp(FloatCC::NotEqual, step_val, zero);
            let cond_fwd = builder.ins().fcmp(FloatCC::LessThanOrEqual, start_val, end_val);
            let cond_bwd = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, start_val, end_val);
            let valid_fwd = builder.ins().band(step_pos, cond_fwd);
            let valid_bwd = builder.ins().band(step_neg, cond_bwd);
            let has_direction = builder.ins().bor(valid_fwd, valid_bwd);
            let range_valid = builder.ins().band(step_nonzero, has_direction);

            let zero = builder.ins().f64const(0.0);
            let result_val = result.unwrap_or(zero);
            Ok(builder.ins().select(range_valid, result_val, zero))
        }
        Expression::StringLiteral(_) => Ok(builder.ins().f64const(0.0)),
        Expression::Previous(inner) => compile_pre_expression(inner, ctx, builder),
        _ => Err(format!(
            "unsupported expression variant in scalar JIT path: {:?}",
            expr
        )),
    }
}

pub fn compile_expression(
    expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<Value, String> {
    compile_expression_rec(expr, ctx, builder)
}
