use cranelift::prelude::*;
use cranelift::prelude::types as cl_types;
use cranelift_module::{Linkage, Module};
use crate::ast::{Expression, Operator};
use super::super::types::ArrayType;
use super::super::context::TranslationContext;

pub(crate) fn compile_zero_crossing_store(
    expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    match expr {
        Expression::BinaryOp(lhs, op, rhs) => {
            match op {
                Operator::Less | Operator::Greater | Operator::LessEq | Operator::GreaterEq => {
                    let l = compile_expression(lhs, ctx, builder)?;
                    let r = compile_expression(rhs, ctx, builder)?;
                    let diff = builder.ins().fsub(l, r);
                    let offset = (*ctx.crossings_idx * 8) as i32;
                    builder.ins().store(MemFlags::new(), diff, ctx.crossings_ptr, offset);
                    *ctx.crossings_idx += 1;
                },
                Operator::And | Operator::Or => {
                    compile_zero_crossing_store(lhs, ctx, builder)?;
                    compile_zero_crossing_store(rhs, ctx, builder)?;
                },
                _ => {}
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn compile_expression(
    expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<Value, String> {
    match expr {
        Expression::Number(n) => Ok(builder.ins().f64const(*n)),
        Expression::Variable(name) => {
            if let Some(slot) = ctx.stack_slots.get(name) {
                Ok(builder.ins().stack_load(cl_types::F64, *slot, 0))
            } else if let Some(val) = ctx.var_map.get(name).copied() {
                Ok(val)
            } else if let Some(idx) = ctx.output_index(name) {
                let offset = (idx * 8) as i32;
                let val = builder.ins().load(cl_types::F64, MemFlags::new(), ctx.outputs_ptr, offset);
                ctx.var_map.insert(name.clone(), val);
                Ok(val)
            } else if name.starts_with("der_") {
                let base = &name[4..];
                if let Some(idx) = ctx.state_index(base) {
                    let offset = (idx * 8) as i32;
                    Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.derivs_ptr, offset))
                } else {
                    Err(format!("der({}) not found: state variable {} unknown", base, base))
                }
            } else {
                Err(format!("Variable {} not found", name))
            }
        }
        Expression::ArrayAccess(arr_expr, idx_expr) => {
             if let Expression::Variable(name) = &**arr_expr {
                 if let Some(info) = ctx.array_info.get(name) {
                     let idx_val = compile_expression(idx_expr, ctx, builder)?;
                     let one = builder.ins().f64const(1.0);
                     let idx_0 = builder.ins().fsub(idx_val, one);
                     let idx_int = builder.ins().fcvt_to_sint(cl_types::I64, idx_0);
                     let eight = builder.ins().iconst(cl_types::I64, 8);
                     let offset_bytes = builder.ins().imul(idx_int, eight);
                     let start_offset = (info.start_index * 8) as i64;
                     let start_const = builder.ins().iconst(cl_types::I64, start_offset);
                     let total_offset = builder.ins().iadd(start_const, offset_bytes);
                     let base_ptr = match info.array_type {
                         ArrayType::State => ctx.states_ptr,
                         ArrayType::Discrete => ctx.discrete_ptr,
                         ArrayType::Parameter => ctx.params_ptr,
                         ArrayType::Output => ctx.outputs_ptr,
                         ArrayType::Derivative => ctx.derivs_ptr,
                     };
                     let addr = builder.ins().iadd(base_ptr, total_offset);
                     Ok(builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0))
                 } else {
                      return Err(format!("Array {} not found in array_info", name));
                 }
             } else {
                 Err("Array access base must be a variable".to_string())
             }
        }
        Expression::BinaryOp(lhs, op, rhs) => {
            let l = compile_expression(lhs, ctx, builder)?;
            let r = compile_expression(rhs, ctx, builder)?;
            match op {
                Operator::Add => Ok(builder.ins().fadd(l, r)),
                Operator::Sub => Ok(builder.ins().fsub(l, r)),
                Operator::Mul => Ok(builder.ins().fmul(l, r)),
                Operator::Div => {
                    let eps = builder.ins().f64const(1e-12);
                    let r_abs = builder.ins().fabs(r);
                    let is_small = builder.ins().fcmp(FloatCC::LessThan, r_abs, eps);
                    let pos_eps = builder.ins().f64const(1e-12);
                    let neg_eps = builder.ins().f64const(-1e-12);
                    let zero = builder.ins().f64const(0.0);
                    let sign_non_neg = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, r, zero);
                    let eps_signed = builder.ins().select(sign_non_neg, pos_eps, neg_eps);
                    let r_safe = builder.ins().select(is_small, eps_signed, r);
                    Ok(builder.ins().fdiv(l, r_safe))
                }
                Operator::Less | Operator::Greater | Operator::LessEq | Operator::GreaterEq | Operator::Equal | Operator::NotEqual => {
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
            let c_val = compile_expression(cond, ctx, builder)?;
            let t_val = compile_expression(t_expr, ctx, builder)?;
            let f_val = compile_expression(f_expr, ctx, builder)?;
            let zero = builder.ins().f64const(0.0);
            let cmp = builder.ins().fcmp(FloatCC::NotEqual, c_val, zero);
            Ok(builder.ins().select(cmp, t_val, f_val))
        }
        Expression::Call(func_name, args) => {
            if func_name == "pre" {
                if args.len() != 1 {
                    return Err(format!("pre() expects 1 argument, got {}", args.len()));
                }
                let arg = &args[0];
                if let Expression::Variable(var_name) = arg {
                    if let Some(idx) = ctx.state_index(var_name) {
                        let offset = (idx * 8) as i32;
                        return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.pre_states_ptr, offset));
                    }
                    if let Some(idx) = ctx.discrete_index(var_name) {
                        let offset = (idx * 8) as i32;
                        return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.pre_discrete_ptr, offset));
                    }
                }
                return compile_pre_expression(arg, ctx, builder);
            }
            if func_name == "edge" {
                 if args.len() != 1 { return Err("edge() expects 1 argument".to_string()); }
                 let arg = &args[0];
                 let curr_val = compile_expression(arg, ctx, builder)?;
                 let pre_val = compile_pre_expression(arg, ctx, builder)?;
                 let zero = builder.ins().f64const(0.0);
                 let curr_bool = builder.ins().fcmp(FloatCC::NotEqual, curr_val, zero);
                 let pre_zero = builder.ins().fcmp(FloatCC::Equal, pre_val, zero);
                 let res_bool = builder.ins().band(curr_bool, pre_zero);
                 let one = builder.ins().f64const(1.0);
                 return Ok(builder.ins().select(res_bool, one, zero));
            }
            if func_name == "change" {
                 if args.len() != 1 { return Err("change() expects 1 argument".to_string()); }
                 let arg = &args[0];
                 let curr_val = compile_expression(arg, ctx, builder)?;
                 let pre_val = compile_pre_expression(arg, ctx, builder)?;
                 let diff = builder.ins().fcmp(FloatCC::NotEqual, curr_val, pre_val);
                 let one = builder.ins().f64const(1.0);
                 let zero = builder.ins().f64const(0.0);
                 return Ok(builder.ins().select(diff, one, zero));
            }
            if func_name == "noEvent" {
                if args.len() != 1 {
                    return Err(format!("noEvent() expects 1 argument, got {}", args.len()));
                }
                // Semantics: evaluate expression without generating zero-crossings.
                // Zero-cross detection is driven by compile_zero_crossing_store, which
                // ignores function calls, so here we can just compile the inner expr.
                return compile_expression(&args[0], ctx, builder);
            }
            if func_name == "initial" {
                if !args.is_empty() {
                    return Err(format!("initial() expects 0 arguments, got {}", args.len()));
                }
                // Approximate initial() by checking if time is approximately zero.
                if let Some(&t_val) = ctx.var_map.get("time") {
                    let zero = builder.ins().f64const(0.0);
                    let diff = builder.ins().fsub(t_val, zero);
                    let abs = builder.ins().fabs(diff);
                    let eps = builder.ins().f64const(1e-9);
                    let is_initial = builder.ins().fcmp(FloatCC::LessThanOrEqual, abs, eps);
                    let one = builder.ins().f64const(1.0);
                    let z = builder.ins().f64const(0.0);
                    return Ok(builder.ins().select(is_initial, one, z));
                } else {
                    return Err("initial() requires time variable in context".to_string());
                }
            }
            if func_name == "terminal" {
                if !args.is_empty() {
                    return Err(format!("terminal() expects 0 arguments, got {}", args.len()));
                }
                if let (Some(&t_val), Some(&t_end_val)) = (ctx.var_map.get("time"), ctx.var_map.get("t_end")) {
                    let diff = builder.ins().fsub(t_end_val, t_val);
                    let abs = builder.ins().fabs(diff);
                    let eps = builder.ins().f64const(1e-9);
                    let is_terminal = builder.ins().fcmp(FloatCC::LessThanOrEqual, abs, eps);
                    let one = builder.ins().f64const(1.0);
                    let z = builder.ins().f64const(0.0);
                    return Ok(builder.ins().select(is_terminal, one, z));
                }
                return Ok(builder.ins().f64const(0.0));
            }
            if func_name == "assert" {
                if args.len() != 2 {
                    return Err(format!("assert() expects 2 arguments (condition, message), got {}", args.len()));
                }
                let cond_val = compile_expression(&args[0], ctx, builder)?;
                let msg_val = compile_expression(&args[1], ctx, builder)?;
                let mut sig = ctx.module.make_signature();
                sig.params.push(AbiParam::new(cl_types::F64));
                sig.params.push(AbiParam::new(cl_types::F64));
                sig.returns.push(AbiParam::new(cl_types::F64));
                let func_id = ctx.module.declare_function("assert", Linkage::Import, &sig)
                    .map_err(|e| e.to_string())?;
                let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
                builder.ins().call(func_ref, &[cond_val, msg_val]);
                return Ok(builder.ins().f64const(0.0));
            }
            if func_name == "terminate" {
                if args.len() != 1 {
                    return Err(format!("terminate() expects 1 argument (message), got {}", args.len()));
                }
                let msg_val = compile_expression(&args[0], ctx, builder)?;
                let mut sig = ctx.module.make_signature();
                sig.params.push(AbiParam::new(cl_types::F64));
                sig.returns.push(AbiParam::new(cl_types::F64));
                let func_id = ctx.module.declare_function("terminate", Linkage::Import, &sig)
                    .map_err(|e| e.to_string())?;
                let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
                builder.ins().call(func_ref, &[msg_val]);
                return Ok(builder.ins().f64const(0.0));
            }
            if func_name == "Boolean" {
                if args.len() != 1 {
                    return Err(format!("Boolean() expects 1 argument, got {}", args.len()));
                }
                let x = compile_expression(&args[0], ctx, builder)?;
                let zero = builder.ins().f64const(0.0);
                let one = builder.ins().f64const(1.0);
                let cmp = builder.ins().fcmp(FloatCC::NotEqual, x, zero);
                return Ok(builder.ins().select(cmp, one, zero));
            }
            if func_name == "sample" || func_name == "interval" {
                return Err(format!(
                    "{}() is not supported; clock/synchronous semantics are out of scope (F2-3). Use when/zero-crossing instead.",
                    func_name
                ));
            }
            let mut arg_vals = Vec::new();
            for arg in args {
                arg_vals.push(compile_expression(arg, ctx, builder)?);
            }
            let mut sig = ctx.module.make_signature();
            for _ in 0..args.len() {
                sig.params.push(AbiParam::new(cl_types::F64));
            }
            sig.returns.push(AbiParam::new(cl_types::F64));
            let func_id = ctx.module.declare_function(func_name, Linkage::Import, &sig)
                .map_err(|e| e.to_string())?;
            let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
            let call_inst = builder.ins().call(func_ref, &arg_vals);
            Ok(builder.inst_results(call_inst)[0])
        }
        Expression::Der(inner) => {
            if let Expression::Variable(name) = &**inner {
                if let Some(idx) = ctx.state_index(name) {
                    let offset = (idx * 8) as i32;
                    return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.derivs_ptr, offset));
                }
            }
            Err("der(expr) only supports der(x) for state variable x".to_string())
        }
        Expression::Range(_, _, _) => Err("Range expression not supported as a scalar value. It should be handled by For loop structure.".to_string()),
        Expression::Dot(_, _) => Err("Dot expression should have been flattened before JIT compilation".to_string()),
        Expression::ArrayLiteral(_) => Err("ArrayLiteral should have been flattened before JIT compilation".to_string()),
    }
}

fn compile_pre_expression(
    expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<Value, String> {
    match expr {
        Expression::Number(n) => Ok(builder.ins().f64const(*n)),
        Expression::Variable(name) => {
            if let Some(idx) = ctx.state_index(name) {
                let offset = (idx * 8) as i32;
                return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.pre_states_ptr, offset));
            }
            if let Some(idx) = ctx.discrete_index(name) {
                let offset = (idx * 8) as i32;
                return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.pre_discrete_ptr, offset));
            }
            if let Some(slot) = ctx.stack_slots.get(name) {
                Ok(builder.ins().stack_load(cl_types::F64, *slot, 0))
            } else {
                ctx.var_map.get(name).cloned().ok_or_else(|| format!("Variable {} not found in pre() context", name))
            }
        }
        Expression::ArrayAccess(arr_expr, idx_expr) => {
            if let Expression::Variable(name) = &**arr_expr {
                if let Some(info) = ctx.array_info.get(name) {
                    let idx_val = compile_pre_expression(idx_expr, ctx, builder)?;
                    let one = builder.ins().f64const(1.0);
                    let idx_0 = builder.ins().fsub(idx_val, one);
                    let idx_int = builder.ins().fcvt_to_sint(cl_types::I64, idx_0);
                    let eight = builder.ins().iconst(cl_types::I64, 8);
                    let offset_bytes = builder.ins().imul(idx_int, eight);
                    let start_offset = (info.start_index * 8) as i64;
                    let start_const = builder.ins().iconst(cl_types::I64, start_offset);
                    let total_offset = builder.ins().iadd(start_const, offset_bytes);
                    let base_ptr = match info.array_type {
                        ArrayType::State => ctx.pre_states_ptr,
                        ArrayType::Discrete => ctx.pre_discrete_ptr,
                        ArrayType::Parameter => ctx.params_ptr,
                        ArrayType::Output => return Err("Output array in pre() not supported".to_string()),
                        ArrayType::Derivative => return Err("Derivative array in pre() not supported".to_string()),
                    };
                    let addr = builder.ins().iadd(base_ptr, total_offset);
                    Ok(builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0))
                } else {
                     return Err(format!("Array {} not found in array_info", name));
                }
            } else {
                Err("Array access base must be a variable".to_string())
            }
       }
        Expression::BinaryOp(lhs, op, rhs) => {
            let l = compile_pre_expression(lhs, ctx, builder)?;
            let r = compile_pre_expression(rhs, ctx, builder)?;
            match op {
                Operator::Add => Ok(builder.ins().fadd(l, r)),
                Operator::Sub => Ok(builder.ins().fsub(l, r)),
                Operator::Mul => Ok(builder.ins().fmul(l, r)),
                Operator::Div => {
                    let eps = builder.ins().f64const(1e-12);
                    let r_abs = builder.ins().fabs(r);
                    let is_small = builder.ins().fcmp(FloatCC::LessThan, r_abs, eps);
                    let pos_eps = builder.ins().f64const(1e-12);
                    let neg_eps = builder.ins().f64const(-1e-12);
                    let zero = builder.ins().f64const(0.0);
                    let sign_non_neg = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, r, zero);
                    let eps_signed = builder.ins().select(sign_non_neg, pos_eps, neg_eps);
                    let r_safe = builder.ins().select(is_small, eps_signed, r);
                    Ok(builder.ins().fdiv(l, r_safe))
                }
                Operator::Less | Operator::Greater | Operator::LessEq | Operator::GreaterEq | Operator::Equal | Operator::NotEqual => {
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
            let c_val = compile_pre_expression(cond, ctx, builder)?;
            let t_val = compile_pre_expression(t_expr, ctx, builder)?;
            let f_val = compile_pre_expression(f_expr, ctx, builder)?;
            let zero = builder.ins().f64const(0.0);
            let cmp = builder.ins().fcmp(FloatCC::NotEqual, c_val, zero);
            Ok(builder.ins().select(cmp, t_val, f_val))
        }
        Expression::Call(func_name, args) => {
            if func_name == "pre" {
                if args.len() != 1 { return Err("pre() expects 1 arg".to_string()); }
                return compile_pre_expression(&args[0], ctx, builder);
            }
            let mut arg_vals = Vec::new();
            for arg in args {
                arg_vals.push(compile_pre_expression(arg, ctx, builder)?);
            }
            let mut sig = ctx.module.make_signature();
            for _ in 0..args.len() {
                sig.params.push(AbiParam::new(cl_types::F64));
            }
            sig.returns.push(AbiParam::new(cl_types::F64));
            let func_id = ctx.module.declare_function(func_name, Linkage::Import, &sig)
                .map_err(|e| e.to_string())?;
            let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
            let call_inst = builder.ins().call(func_ref, &arg_vals);
            Ok(builder.inst_results(call_inst)[0])
        }
        Expression::Der(_) => Err("Nested der() not supported in expression".to_string()),
        Expression::Range(_, _, _) => Err("Range expression not supported as a scalar value. It should be handled by For loop structure.".to_string()),
        Expression::Dot(_, _) => Err("Array access (nested) and Dot should have been flattened before JIT compilation".to_string()),
        Expression::ArrayLiteral(_) => Err("ArrayLiteral should have been flattened before JIT compilation".to_string()),
    }
}
