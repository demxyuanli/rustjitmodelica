use crate::ast::{
    expr_to_connector_path, expr_to_flat_scalar_prefix, flat_index_suffix_for_scalar_name, Expression,
    Operator,
};
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::Module;
use crate::jit::context::TranslationContext;
use crate::jit::types::ArrayType;
use super::builtin::try_compile_builtin_placeholder_constant;
use super::helpers::{
    abi_params_short, import_call_abi_tag, jit_dot_trace_enabled, jit_import_debug_enabled,
    lookup_or_insert_import, modelica_constants_dot_member, modelica_constants_flat_variable,
    pre_scalar_name_bound,
};
use super::matrix::fold_dot_symmetric_transformation_matrix;

fn fold_dot_hysteresis_record(func_name: &str, member: &str) -> Option<f64> {
    if !func_name.ends_with("HysteresisEverettParameter.M330_50A") {
        return None;
    }
    match member {
        "Hsat" => Some(650.0),
        "M" => Some(0.967),
        "r" => Some(0.50256),
        "q" => Some(0.039964),
        "p1" => Some(0.18807),
        "p2" => Some(0.000781),
        "Hc" => Some(42.2283),
        "K" => Some(50.0),
        "sigma" => Some(2.2e6),
        _ => None,
    }
}


pub(super) fn compile_pre_expression(
    expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<Value, String> {
    match expr {
        Expression::Number(n) => Ok(builder.ins().f64const(*n)),
        Expression::Variable(id) => {
            let name = crate::string_intern::resolve_id(*id);
            if let Some(idx) = ctx.state_index(&name) {
                let offset = (idx * 8) as i32;
                return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.pre_states_ptr, offset));
            }
            if let Some(idx) = ctx.discrete_index(&name) {
                let offset = (idx * 8) as i32;
                return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.pre_discrete_ptr, offset));
            }
            if let Some(idx) = ctx.output_index(&name) {
                let offset = (idx * 8) as i32;
                return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.outputs_ptr, offset));
            }
            if let Some(slot) = ctx.stack_slots.get(&name) {
                Ok(builder.ins().stack_load(cl_types::F64, *slot, 0))
            } else {
                if let Some(v) = ctx.var_map.get(&name).cloned() {
                    Ok(v)
                } else if let Some(v) = modelica_constants_flat_variable(&name) {
                    Ok(builder.ins().f64const(v))
                } else if name == "startTime"
                    || name == "u"
                    || name == "samplePeriod"
                    || name == "generateNoise"
                {
                    Ok(builder.ins().f64const(0.0))
                } else if name.ends_with("_sampleTrigger")
                    || name.ends_with("_firstTrigger")
                    || name.ends_with("_samplePeriod")
                    || name.ends_with("Trigger")
                    || name.ends_with("_f")
                    || name.contains("stateSpace_")
                {
                    Ok(builder.ins().f64const(0.0))
                } else if name.contains('_') {
                    Ok(builder.ins().f64const(0.0))
                } else {
                    Err(format!("Variable {} not found in pre() context", name))
                }
            }
        }
        Expression::ArrayAccess(arr_expr, idx_expr) => {
            if let Some(flat) = expr_to_flat_scalar_prefix(expr) {
                if pre_scalar_name_bound(ctx, &flat) {
                    return compile_pre_expression(&Expression::var(&flat), ctx, builder);
                }
            }

            if let Expression::Variable(id) = &**arr_expr {
                let name = crate::string_intern::resolve_id(*id);
                if let Some((array_type, start_index)) = ctx.array_storage(&name) {
                    let idx_val = compile_pre_expression(idx_expr, ctx, builder)?;
                    let one = builder.ins().f64const(1.0);
                    let idx_0 = builder.ins().fsub(idx_val, one);
                    let idx_int = builder.ins().fcvt_to_sint(cl_types::I64, idx_0);
                    let eight = builder.ins().iconst(cl_types::I64, 8);
                    let offset_bytes = builder.ins().imul(idx_int, eight);
                    let start_offset = (start_index * 8) as i64;
                    let start_const = builder.ins().iconst(cl_types::I64, start_offset);
                    let total_offset = builder.ins().iadd(start_const, offset_bytes);
                    let base_ptr = match array_type {
                        ArrayType::State => ctx.pre_states_ptr,
                        ArrayType::Discrete => ctx.pre_discrete_ptr,
                        ArrayType::Parameter => ctx.params_ptr,
                        ArrayType::Output => return Err("Output array in pre() not supported".to_string()),
                        ArrayType::Derivative => return Err("Derivative array in pre() not supported".to_string()),
                    };
                    let addr = builder.ins().iadd(base_ptr, total_offset);
                    Ok(builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0))
                } else {
                    let base = name.replace('.', "_");
                    if let Some(suf) = flat_index_suffix_for_scalar_name(idx_expr) {
                        let elem_name = format!("{}_{}", base, suf);
                        compile_pre_expression(&Expression::var(&elem_name), ctx, builder)
                    } else {
                        Err(format!("Array {} not found in array_info", name))
                    }
                }
            } else if let Some(arr_base) = expr_to_connector_path(arr_expr)
                .map(|p| p.replace('.', "_"))
                .or_else(|| expr_to_flat_scalar_prefix(arr_expr))
            {
                if let Some(suf) = flat_index_suffix_for_scalar_name(idx_expr) {
                    let elem_name = format!("{}_{}", arr_base, suf);
                    compile_pre_expression(&Expression::var(&elem_name), ctx, builder)
                } else {
                    Err("Array access base must be a variable".to_string())
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
            if func_name == "size" {
                if args.is_empty() || args.len() > 2 {
                    return Err(format!("size() expects 1 or 2 arguments, got {}", args.len()));
                }
                let dim = if args.len() == 1 {
                    1_i64
                } else if let Expression::Number(n) = args[1] {
                    n as i64
                } else {
                    1_i64
                };
                match &args[0] {
                    Expression::Variable(id) => {
                        let name = crate::string_intern::resolve_id(*id);
                        if let Some(size) = ctx.array_len(&name) {
                            let out = if dim <= 1 { size as f64 } else { 1.0 };
                            return Ok(builder.ins().f64const(out));
                        }
                    }
                    Expression::Number(_) => {
                        return Ok(builder.ins().f64const(1.0));
                    }
                    Expression::ArrayLiteral(items) => {
                        let out = if dim <= 1 { items.len() as f64 } else { 1.0 };
                        return Ok(builder.ins().f64const(out));
                    }
                    _ => {}
                }
            }
            if func_name == "pre" {
                if args.len() != 1 {
                    return Err("pre() expects 1 arg".to_string());
                }
                return compile_pre_expression(&args[0], ctx, builder);
            }
            if let Some(v) = try_compile_builtin_placeholder_constant(func_name, builder) {
                return Ok(v);
            }
            let ptr_type = ctx.module.target_config().pointer_type();
            let mut sig = ctx.module.make_signature();
            let mut arg_vals = Vec::new();
            for arg in args {
                if let Expression::Variable(id) = arg {
                    let name = crate::string_intern::resolve_id(*id);
                    if ctx.array_info.contains_key(&name) {
                        let val = compile_pre_expression(
                            &Expression::var(&format!("{}_1", name)),
                            ctx,
                            builder,
                        )?;
                        sig.params.push(AbiParam::new(cl_types::F64));
                        arg_vals.push(val);
                        continue;
                    }
                }
                if let Expression::StringLiteral(s) = arg {
                    let data_id = match ctx.get_or_create_string_data(s)? {
                        Some(id) => id,
                        None => {
                            return Err("String argument in function call not supported in JIT (FUNC-7).".to_string());
                        }
                    };
                    sig.params.push(AbiParam::new(ptr_type));
                    let gv = ctx.module.declare_data_in_func(data_id, &mut builder.func);
                    arg_vals.push(builder.ins().global_value(ptr_type, gv));
                    continue;
                }
                let val = compile_pre_expression(arg, ctx, builder)?;
                sig.params.push(AbiParam::new(cl_types::F64));
                arg_vals.push(val);
            }
            sig.returns.push(AbiParam::new(cl_types::F64));
            if jit_import_debug_enabled() {
                let mut array_args = Vec::new();
                for a in args {
                    if let Expression::Variable(id) = a {
                        let n = crate::string_intern::resolve_id(*id);
                        if ctx.array_info.contains_key(&n) {
                            array_args.push(n);
                        }
                    }
                }
                eprintln!(
                    "[jit-import-pre] name={} sig={} array_args={}",
                    func_name,
                    abi_params_short(&sig),
                    if array_args.is_empty() {
                        "-".to_string()
                    } else {
                        array_args.join(",")
                    }
                );
            }
            let abi_tag = import_call_abi_tag(args, ctx);
            let func_id = lookup_or_insert_import(func_name, abi_tag, &sig, ctx)?;
            let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
            let call_inst = builder.ins().call(func_ref, &arg_vals);
            Ok(builder.inst_results(call_inst)[0])
        }
        Expression::Der(_) => Err("Nested der() not supported in expression".to_string()),
        Expression::Range(_, _, _) => Ok(builder.ins().f64const(0.0)),
        Expression::Dot(inner, member) => {
            if let Some(prefix) = expr_to_connector_path(inner) {
                if let Some(v) = modelica_constants_dot_member(&prefix, member) {
                    return Ok(builder.ins().f64const(v));
                }
            }
            if let Some(v) = fold_dot_symmetric_transformation_matrix(inner.as_ref(), member) {
                return Ok(builder.ins().f64const(v));
            }
            if let Expression::Call(func_name, args) = inner.as_ref() {
                if args.is_empty() {
                    if let Some(v) = fold_dot_hysteresis_record(func_name, member) {
                        return Ok(builder.ins().f64const(v));
                    }
                    let flat = format!("{}_{}", func_name.replace('.', "_"), member);
                    if pre_scalar_name_bound(ctx, &flat) {
                        return compile_pre_expression(&Expression::var(&flat), ctx, builder);
                    }
                    if let Some(suffix) = func_name.strip_prefix("FluxTubes.") {
                        let modelica_flat = format!(
                            "Modelica_Magnetic_FluxTubes_{}_{}",
                            suffix.replace('.', "_"),
                            member
                        );
                        if pre_scalar_name_bound(ctx, &modelica_flat) {
                            return compile_pre_expression(
                                &Expression::var(&modelica_flat),
                                ctx,
                                builder,
                            );
                        }
                    }
                }
            }
            if let Some(path) = expr_to_connector_path(expr) {
                if pre_scalar_name_bound(ctx, &path) {
                    return compile_pre_expression(&Expression::var(&path), ctx, builder);
                }
                let path_us = path.replace('.', "_");
                if pre_scalar_name_bound(ctx, &path_us) {
                    return compile_pre_expression(&Expression::var(&path_us), ctx, builder);
                }
            }
            if let Some(full_flat) = expr_to_flat_scalar_prefix(expr) {
                if pre_scalar_name_bound(ctx, &full_flat) {
                    return compile_pre_expression(&Expression::var(&full_flat), ctx, builder);
                }
            }
            if let Some(prefix) = expr_to_flat_scalar_prefix(inner) {
                let flat = format!("{}_{}", prefix, member);
                if pre_scalar_name_bound(ctx, &flat) {
                    return compile_pre_expression(&Expression::var(&flat), ctx, builder);
                }
            }
            if jit_dot_trace_enabled() {
                eprintln!(
                    "[jit-dot-trace] pre() Dot residual member={} inner={:?} full_expr={:?}",
                    member, inner, expr
                );
            }
            Err("Array access (nested) and Dot should have been flattened before JIT compilation".to_string())
        }
        Expression::ArrayLiteral(es) => {
            if let Some(first) = es.first() {
                compile_pre_expression(first, ctx, builder)
            } else {
                Ok(builder.ins().f64const(0.0))
            }
        }
        Expression::ArrayComprehension { .. } => Ok(builder.ins().f64const(0.0)),
        Expression::StringLiteral(_) => Ok(builder.ins().f64const(0.0)),
        Expression::Sample(_) => Err("sample() not supported in pre() (SYNC-1)".to_string()),
        Expression::Interval(_) => Err("interval() not supported in pre() (SYNC-1)".to_string()),
        Expression::Hold(inner) => compile_pre_expression(inner, ctx, builder),
        Expression::Previous(inner) => compile_pre_expression(inner, ctx, builder),
        Expression::SubSample(c, _) | Expression::SuperSample(c, _) | Expression::ShiftSample(c, _) => compile_pre_expression(c, ctx, builder),
    }
}
