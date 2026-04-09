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
    abi_params_short, import_call_abi_tag, jit_builtin_fallback_warn_once, jit_dot_trace_enabled,
    jit_import_debug_enabled, jit_import_strict_enabled, jit_strict_placeholders_enabled,
    jit_var_fallback_trace, lookup_or_insert_import, modelica_constants_dot_member,
    modelica_constants_flat_variable, pre_scalar_name_bound,
};
use super::matrix::fold_dot_symmetric_transformation_matrix;
use crate::jit::jit_policy::{hysteresis_record_value, lookup_pre_variable_fallback};

fn pre_call_name_matches(func_name: &str, plain: &str) -> bool {
    func_name == plain || func_name.ends_with(&format!(".{}", plain))
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
                } else if let Some((v, trace_tag)) = lookup_pre_variable_fallback(&name) {
                    if !trace_tag.is_empty() {
                        jit_var_fallback_trace(&name, trace_tag.as_str());
                    }
                    Ok(builder.ins().f64const(v))
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
                        ArrayType::Output => ctx.outputs_ptr,
                        ArrayType::Derivative => ctx.derivs_ptr,
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
            if pre_call_name_matches(func_name, "cardinality") {
                if args.len() != 1 {
                    return Err(format!(
                        "cardinality() expects 1 argument, got {}",
                        args.len()
                    ));
                }
                if let Expression::Variable(id) = &args[0] {
                    let path = crate::string_intern::resolve_id(*id);
                    if let Some(&d) = ctx.connector_connection_degree.get(&path) {
                        return Ok(builder.ins().f64const(d as f64));
                    }
                }
                if jit_strict_placeholders_enabled() {
                    return Err(format!(
                        "cardinality (pre): no flatten degree for '{}'",
                        func_name
                    ));
                }
                return Ok(builder.ins().f64const(0.0));
            }
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
                // Do not fall through to generic external import path for size().
                return Ok(builder.ins().f64const(1.0));
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

            // In validate-mode JIT, avoid importing unknown call-site names (can panic if the
            // host symbol is missing). Degrade to a placeholder result instead.
            let is_external_modelica = ctx
                .external_modelica_names
                .map(|s| s.contains(func_name))
                .unwrap_or(false);
            if !is_external_modelica
                && !crate::jit::native::builtin_jit_symbol_names()
                    .iter()
                    .any(|&n| n == func_name)
            {
                if jit_import_strict_enabled() {
                    return Err(format!(
                        "JIT import strict (pre): '{}' is not a linked builtin or external symbol",
                        func_name
                    ));
                }
                jit_builtin_fallback_warn_once(func_name, "unknown-import-pre");
                if args.is_empty() {
                    return Ok(builder.ins().f64const(0.0));
                }
                return compile_pre_expression(&args[0], ctx, builder);
            }
            let ptr_type = ctx.module.target_config().pointer_type();
            let mut sig = ctx.module.make_signature();
            let mut arg_vals = Vec::new();
            for arg in args {
                if let Expression::Variable(id) = arg {
                    let name = crate::string_intern::resolve_id(*id);
                    // FUNC-1: Array argument ABI in pre() context - pass ptr + size as dual parameters
                    if let Some(info) = ctx.array_info.get(&name) {
                        // Get base pointer for the array (use pre_* buffers for state/discrete)
                        let base_ptr = match info.array_type {
                            crate::jit::types::ArrayType::State => ctx.pre_states_ptr,
                            crate::jit::types::ArrayType::Discrete => ctx.pre_discrete_ptr,
                            crate::jit::types::ArrayType::Parameter => ctx.params_ptr,
                            crate::jit::types::ArrayType::Output => ctx.outputs_ptr,
                            crate::jit::types::ArrayType::Derivative => ctx.derivs_ptr,
                        };
                        let start_offset = (info.start_index * 8) as i64;
                        let start_const = builder.ins().iconst(ptr_type, start_offset);
                        let array_ptr = builder.ins().iadd(base_ptr, start_const);
                        
                        // Array size as f64 (Modelica convention)
                        let size_val = builder.ins().f64const(info.size as f64);
                        
                        // Push ptr then size (C ABI: double* ptr, double size)
                        sig.params.push(AbiParam::new(ptr_type));
                        arg_vals.push(array_ptr);
                        sig.params.push(AbiParam::new(cl_types::F64));
                        arg_vals.push(size_val);
                        continue;
                    }
                }
                if let Expression::ArrayLiteral(items) = arg {
                    if items.iter().all(|it| matches!(it, Expression::Number(_))) {
                        let mut elems: Vec<f64> = Vec::with_capacity(items.len());
                        for it in items {
                            if let Expression::Number(n) = it {
                                elems.push(*n);
                            }
                        }
                        let data_id = match ctx.get_or_create_f64_array_literal_data(&elems)? {
                            Some(id) => id,
                            None => {
                                return Err(
                                    "Array literal in pre() external call requires JIT data context (EXT-3)."
                                        .to_string(),
                                );
                            }
                        };
                        sig.params.push(AbiParam::new(ptr_type));
                        let gv = ctx.module.declare_data_in_func(data_id, &mut builder.func);
                        arg_vals.push(builder.ins().global_value(ptr_type, gv));
                        sig.params.push(AbiParam::new(cl_types::F64));
                        arg_vals.push(builder.ins().f64const(elems.len() as f64));
                        continue;
                    }

                    let byte_len = (items.len() * 8) as u32;
                    let tmp_slot = builder.create_sized_stack_slot(StackSlotData::new(
                        StackSlotKind::ExplicitSlot,
                        byte_len,
                        8,
                    ));
                    for (i, it) in items.iter().enumerate() {
                        let item_val = compile_pre_expression(it, ctx, builder)?;
                        let off_i64 = (i as i64) * 8;
                        let off = i32::try_from(off_i64).map_err(|_| {
                            format!(
                                "pre() external call '{}': array literal too large (EXT-3).",
                                func_name
                            )
                        })?;
                        builder.ins().stack_store(item_val, tmp_slot, off);
                    }
                    sig.params.push(AbiParam::new(ptr_type));
                    arg_vals.push(builder.ins().stack_addr(ptr_type, tmp_slot, 0));
                    sig.params.push(AbiParam::new(cl_types::F64));
                    arg_vals.push(builder.ins().f64const(items.len() as f64));
                    continue;
                }
                if let Expression::StringLiteral(s) = arg {
                    let data_id = match ctx.get_or_create_string_data(s)? {
                        Some(id) => id,
                        None => {
                            return Err("String argument in pre() function call requires string data context (FUNC-7).".to_string());
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
                    if let Some(v) = hysteresis_record_value(func_name, member) {
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
        Expression::ArrayComprehension { expr, iter_var, iter_range } => {
            // pre() context expects a scalar value. For array comprehension,
            // use the first iterator point when the range is non-empty.
            let (start_val, step_val, end_val) = match iter_range.as_ref() {
                Expression::Range(start, step, end) => {
                    let s = match compile_pre_expression(start.as_ref(), ctx, builder) {
                        Ok(v) => v,
                        Err(_) => return Ok(builder.ins().f64const(0.0)),
                    };
                    let st = match compile_pre_expression(step.as_ref(), ctx, builder) {
                        Ok(v) => v,
                        Err(_) => return Ok(builder.ins().f64const(0.0)),
                    };
                    let e = match compile_pre_expression(end.as_ref(), ctx, builder) {
                        Ok(v) => v,
                        Err(_) => return Ok(builder.ins().f64const(0.0)),
                    };
                    (s, st, e)
                }
                Expression::Number(n) => {
                    let one = builder.ins().f64const(1.0);
                    let end = builder.ins().f64const(*n);
                    (one, one, end)
                }
                _ => return Ok(builder.ins().f64const(0.0)),
            };

            let old_val = ctx.var_map.get(iter_var).copied();
            ctx.var_map.insert(iter_var.clone(), start_val);
            let result = compile_pre_expression(expr, ctx, builder);
            match old_val {
                Some(v) => {
                    ctx.var_map.insert(iter_var.clone(), v);
                }
                None => {
                    ctx.var_map.remove(iter_var);
                }
            }

            let zero = builder.ins().f64const(0.0);
            let step_pos = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, step_val, zero);
            let ge_cond = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, end_val, start_val);
            let le_cond = builder.ins().fcmp(FloatCC::LessThanOrEqual, end_val, start_val);
            let non_empty = builder.ins().select(step_pos, ge_cond, le_cond);
            let result_val = result.unwrap_or(zero);
            Ok(builder.ins().select(non_empty, result_val, zero))
        }
        Expression::StringLiteral(_) => Ok(builder.ins().f64const(0.0)),
        Expression::Sample(_) => Err("sample() not supported in pre() (SYNC-1)".to_string()),
        Expression::Interval(_) => Err("interval() not supported in pre() (SYNC-1)".to_string()),
        Expression::Hold(inner) => compile_pre_expression(inner, ctx, builder),
        Expression::Previous(inner) => compile_pre_expression(inner, ctx, builder),
        Expression::SubSample(c, _)
        | Expression::SuperSample(c, _)
        | Expression::ShiftSample(c, _)
        | Expression::BackSample(c, _) => compile_pre_expression(c, ctx, builder),
    }
}
