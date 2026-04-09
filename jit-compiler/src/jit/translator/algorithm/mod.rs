mod control_flow;
mod helpers;
mod multiassign_special;
mod store_lhs;

use crate::jit::context::TranslationContext;
use control_flow::{compile_for_stmt, compile_if_stmt, compile_when_stmt, compile_while_stmt};
use super::expr::compile_expression;
use super::expr::helpers::jit_strict_placeholders_enabled as alg_strict_placeholders;
use crate::ast::{AlgorithmStatement, Expression};
use crate::diag::fallback_counter;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};

use helpers::{
    expand_array_comprehension_values, expr_contains_array_literal, is_record_constructor_call_name,
    is_store_target_expr,
};
use multiassign_special::{try_compile_msl_random_multiassign, try_compile_real_fft_multiassign};
use store_lhs::{compile_cat_into_array_variable, compile_fill_array_variable, compile_store_to_lhs};

pub fn compile_algorithm_stmt(
    stmt: &AlgorithmStatement,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    match stmt {
        AlgorithmStatement::Assignment(lhs, rhs) => {
            if let (Expression::Variable(lhs_id), Expression::Call(func_name, args)) = (lhs, rhs) {
                let lhs_name = crate::string_intern::resolve_id(*lhs_id);
                if func_name == "zeros" || func_name.ends_with(".zeros") {
                    let zero = builder.ins().f64const(0.0);
                    if compile_fill_array_variable(&lhs_name, zero, ctx, builder)? {
                        return Ok(());
                    }
                }
                if func_name == "ones" || func_name.ends_with(".ones") {
                    let one = builder.ins().f64const(1.0);
                    if compile_fill_array_variable(&lhs_name, one, ctx, builder)? {
                        return Ok(());
                    }
                }
                if func_name == "fill" || func_name.ends_with(".fill") {
                    if let Some(fill_expr) = args.first() {
                        let fill_v = compile_expression(fill_expr, ctx, builder)?;
                        if compile_fill_array_variable(&lhs_name, fill_v, ctx, builder)? {
                            return Ok(());
                        }
                    }
                }
                if func_name == "cat" || func_name.ends_with(".cat") {
                    if compile_cat_into_array_variable(&lhs_name, args, ctx, builder)? {
                        return Ok(());
                    }
                }
            }
            if !is_store_target_expr(lhs) && is_store_target_expr(rhs) {
                let val = compile_expression(lhs, ctx, builder)?;
                compile_store_to_lhs(rhs, val, ctx, builder)?;
                return Ok(());
            }
            if !is_store_target_expr(lhs) && !is_store_target_expr(rhs) {
                let _ = compile_expression(lhs, ctx, builder)?;
                let _ = compile_expression(rhs, ctx, builder)?;
                return Ok(());
            }
            let val = compile_expression(rhs, ctx, builder)?;
            compile_store_to_lhs(lhs, val, ctx, builder)?;
        }
        AlgorithmStatement::MultiAssign(lhss, rhs) => {
            if try_compile_real_fft_multiassign(lhss, rhs, ctx, builder)? {
                return Ok(());
            }
            if try_compile_msl_random_multiassign(lhss, rhs, ctx, builder)? {
                return Ok(());
            }
            if let Expression::Call(name, args) = rhs {
                if is_record_constructor_call_name(name) {
                    if args.len() != lhss.len() {
                        return Err(format!(
                            "Record constructor multi-assign arity mismatch for '{}': {} LHS targets but {} constructor args",
                            name,
                            lhss.len(),
                            args.len()
                        ));
                    }
                    for (lhs, arg) in lhss.iter().zip(args.iter()) {
                        let v = compile_expression(arg, ctx, builder)?;
                        compile_store_to_lhs(lhs, v, ctx, builder)?;
                    }
                    return Ok(());
                }
            }
            if let Some(values) = expand_array_comprehension_values(rhs, lhss.len(), ctx, builder)? {
                for (lhs, v) in lhss.iter().zip(values.into_iter()) {
                    compile_store_to_lhs(lhs, v, ctx, builder)?;
                }
                return Ok(());
            }
            if let Expression::ArrayLiteral(items) = rhs {
                let mut expanded_values: Vec<Value> = Vec::new();
                for item in items.iter() {
                    if let Some(mut values) = expand_array_comprehension_values(
                        item,
                        lhss.len().saturating_sub(expanded_values.len()),
                        ctx,
                        builder,
                    )? {
                        expanded_values.append(&mut values);
                        continue;
                    }
                    if expr_contains_array_literal(item) {
                        return Err(format!(
                            "Multi-assign output item has array-valued shape, which is unsupported for scalar targets: {:?}",
                            item
                        ));
                    }
                    let v = compile_expression(item, ctx, builder)?;
                    expanded_values.push(v);
                }
                if expanded_values.len() != lhss.len() {
                    return Err(format!(
                        "Multi-assign arity mismatch after literal/comprehension expansion: {} LHS targets but {} RHS items",
                        lhss.len(),
                        expanded_values.len()
                    ));
                }
                for (lhs, v) in lhss.iter().zip(expanded_values.into_iter()) {
                    compile_store_to_lhs(lhs, v, ctx, builder)?;
                }
                return Ok(());
            }
            if let Expression::Variable(id) = rhs {
                let arr_name = crate::string_intern::resolve_id(*id);
                if let Some(n) = ctx.array_len(&arr_name) {
                    if n == lhss.len() {
                        for (i, lhs) in lhss.iter().enumerate() {
                            let elem = Expression::ArrayAccess(
                                Box::new(Expression::Variable(*id)),
                                Box::new(Expression::Number((i + 1) as f64)),
                            );
                            let v = compile_expression(&elem, ctx, builder)?;
                            compile_store_to_lhs(lhs, v, ctx, builder)?;
                        }
                        return Ok(());
                    }
                    return Err(format!(
                        "Multi-assign array arity mismatch for '{}': {} LHS targets but array length is {}",
                        arr_name,
                        lhss.len(),
                        n
                    ));
                }
            }
            if lhss.len() == 1 {
                let v = compile_expression(rhs, ctx, builder)?;
                compile_store_to_lhs(&lhss[0], v, ctx, builder)?;
                return Ok(());
            }

            let rhs_hint = if let Expression::Call(name, args) = rhs {
                format!("function call '{}({} args)'", name, args.len())
            } else {
                match rhs {
                    Expression::ArrayLiteral(items) => format!("array literal ({} items)", items.len()),
                    Expression::Variable(id) => format!("variable '{}'", crate::string_intern::resolve_id(*id)),
                    Expression::ArrayComprehension { .. } => {
                        format!("array comprehension (not supported in multi-assign)")
                    }
                    Expression::Dot(base, member) => {
                        let base_name = match base.as_ref() {
                            Expression::Variable(id) => crate::string_intern::resolve_id(*id),
                            _ => "unknown".to_string(),
                        };
                        format!(
                            "record field access '{}.{}' (record should be flattened)",
                            base_name, member
                        )
                    }
                    _ => format!("{:?}", rhs),
                }
            };

            let lhs_targets: Vec<String> = lhss
                .iter()
                .enumerate()
                .map(|(i, lhs)| format!("#{}={:?}", i + 1, lhs))
                .collect();

            if alg_strict_placeholders() {
                return Err(format!(
                    "JIT strict placeholders: unresolved MultiAssign with {} target(s), RHS = {}; targets: {}",
                    lhss.len(),
                    rhs_hint,
                    lhs_targets.join(", ")
                ));
            }

            eprintln!(
                "[fallback:jit-multi-assign] writes zero to {} target(s) for unsupported RHS {}.",
                lhss.len(),
                rhs_hint
            );
            eprintln!(
                "[fallback:jit-multi-assign] LHS targets: {}",
                lhs_targets.join(", ")
            );

            fallback_counter::inc_jit_multi_assign();
            let zero = builder.ins().f64const(0.0);
            for lhs in lhss {
                compile_store_to_lhs(lhs, zero, ctx, builder)?;
            }
            return Ok(());
        }
        AlgorithmStatement::CallStmt(expr) => {
            let _ = compile_expression(expr, ctx, builder)?;
        }
        AlgorithmStatement::NoOp => {}
        AlgorithmStatement::Break => {
            let Some(&target) = ctx.loop_break_stack.last() else {
                return Err("break used outside of loop".to_string());
            };
            builder.ins().jump(target, &[]);
            let cont = builder.create_block();
            builder.switch_to_block(cont);
        }
        AlgorithmStatement::Return(_) => {
            let Some(target) = ctx.function_return_block else {
                return Err("return used outside of function context".to_string());
            };
            builder.ins().jump(target, &[]);
            let cont = builder.create_block();
            builder.switch_to_block(cont);
        }
        AlgorithmStatement::Assert(cond, msg) => {
            let cond_val = compile_expression(cond, ctx, builder)?;
            let msg_val = compile_expression(msg, ctx, builder)?;
            let mut sig = ctx.module.make_signature();
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.returns.push(AbiParam::new(cl_types::F64));
            let func_id = ctx
                .module
                .declare_function("assert", Linkage::Import, &sig)
                .map_err(|e| e.to_string())?;
            let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
            builder.ins().call(func_ref, &[cond_val, msg_val]);
        }
        AlgorithmStatement::Terminate(msg) => {
            let msg_val = compile_expression(msg, ctx, builder)?;
            let mut sig = ctx.module.make_signature();
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.returns.push(AbiParam::new(cl_types::F64));
            let func_id = ctx
                .module
                .declare_function("terminate", Linkage::Import, &sig)
                .map_err(|e| e.to_string())?;
            let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
            builder.ins().call(func_ref, &[msg_val]);
        }
        AlgorithmStatement::Reinit(var_name, val_expr) => {
            let val = compile_expression(val_expr, ctx, builder)?;
            if let Some(idx) = ctx.state_index(var_name) {
                let offset = (idx * 8) as i32;
                builder
                    .ins()
                    .store(MemFlags::new(), val, ctx.states_ptr, offset);
            } else {
                return Err(format!(
                    "reinit() target '{}' is not a state variable",
                    var_name
                ));
            }
        }
        AlgorithmStatement::If(cond, true_stmts, else_ifs, else_stmts) => {
            compile_if_stmt(
                cond,
                true_stmts,
                else_ifs,
                else_stmts.as_ref(),
                ctx,
                builder,
                compile_algorithm_stmt,
            )?;
        }
        AlgorithmStatement::While(cond, body) => {
            compile_while_stmt(cond, body, ctx, builder, compile_algorithm_stmt)?;
        }
        AlgorithmStatement::For(var_name, range_expr, body) => {
            compile_for_stmt(
                var_name,
                range_expr.as_ref(),
                body,
                ctx,
                builder,
                compile_algorithm_stmt,
            )?;
        }
        AlgorithmStatement::When(cond, body, else_whens) => {
            compile_when_stmt(cond, body, else_whens, ctx, builder, compile_algorithm_stmt)?;
        }
    }
    Ok(())
}
