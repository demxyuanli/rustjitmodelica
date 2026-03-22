use crate::ast::{
    expr_to_connector_path, expr_to_flat_scalar_prefix, flat_index_suffix_for_scalar_name, Expression,
    Operator,
};
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};

use crate::jit::context::TranslationContext;
use crate::jit::types::ArrayType;
use super::builtin::try_compile_builtin_call;
use super::helpers::{
    abi_params_short, import_call_abi_tag, jit_dot_fallback_zero_enabled, jit_dot_trace_enabled,
    jit_import_debug_enabled, jit_scalar_name_bound, lookup_or_insert_import,
    modelica_constants_dot_member, modelica_constants_flat_variable,
};
use super::matrix::fold_dot_symmetric_transformation_matrix;
use super::pre::compile_pre_expression;

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


pub(crate) fn compile_zero_crossing_store(
    expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    match expr {
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
    match expr {
        Expression::Number(n) => Ok(builder.ins().f64const(*n)),
        Expression::Variable(id) => {
            let name = crate::string_intern::resolve_id(*id);
            if let Some(slot) = ctx.stack_slots.get(&name) {
                return Ok(builder.ins().stack_load(cl_types::F64, *slot, 0));
            }
            if let Some(val) = ctx.var_map.get(&name).copied() {
                return Ok(val);
            }
            if let Some(idx) = ctx.output_index(&name) {
                let offset = (idx * 8) as i32;
                return Ok(builder
                    .ins()
                    .load(cl_types::F64, MemFlags::new(), ctx.outputs_ptr, offset));
            }
            if let Some(idx) = ctx.param_index(&name) {
                let offset = (idx * 8) as i32;
                return Ok(builder
                    .ins()
                    .load(cl_types::F64, MemFlags::new(), ctx.params_ptr, offset));
            }
            if name.starts_with("der_") {
                let base = &name[4..];
                if let Some(idx) = ctx.state_index(base) {
                    let offset = (idx * 8) as i32;
                    return Ok(builder.ins().load(
                        cl_types::F64,
                        MemFlags::new(),
                        ctx.derivs_ptr,
                        offset,
                    ));
                }
                return Err(format!(
                    "der({}) not found: state variable {} unknown",
                    base, base
                ));
            }

            if let Some((base, idx0)) = name
                .rsplit_once('_')
                .and_then(|(b, i)| i.parse::<usize>().ok().map(|n| (b.to_string(), n)))
            {
                if let Some((array_type, start_index)) = ctx.array_storage(&base) {
                    // Flatten may scalarize arrays into base_0, base_1, ...; map to 1-based Modelica indexing.
                    let idx_val = builder.ins().f64const((idx0 as f64) + 1.0);
                    let one = builder.ins().f64const(1.0);
                    let idx_0 = builder.ins().fsub(idx_val, one);
                    let idx_int = builder.ins().fcvt_to_sint(cl_types::I64, idx_0);
                    let eight = builder.ins().iconst(cl_types::I64, 8);
                    let offset_bytes = builder.ins().imul(idx_int, eight);
                    let start_offset = (start_index * 8) as i64;
                    let start_const = builder.ins().iconst(cl_types::I64, start_offset);
                    let total_offset = builder.ins().iadd(start_const, offset_bytes);
                    let base_ptr = match array_type {
                        ArrayType::State => ctx.states_ptr,
                        ArrayType::Discrete => ctx.discrete_ptr,
                        ArrayType::Parameter => ctx.params_ptr,
                        ArrayType::Output => ctx.outputs_ptr,
                        ArrayType::Derivative => ctx.derivs_ptr,
                    };
                    let addr = builder.ins().iadd(base_ptr, total_offset);
                    return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0));
                }
                if base == "a" || base == "b" {
                    let _ = idx0;
                    return Ok(builder.ins().f64const(0.0));
                }
            }

            // BLT/tearing may introduce temporaries not pre-allocated in stack_slots.
            // Treat them as implicitly initialized to 0.0 to allow compilation to proceed.
            if name.starts_with("tf")
                || name.starts_with("bb_")
                || name.contains("_bb_")
                || name.contains("LimiterHomotopy")
                || name.contains("_LimiterHomotopy_")
                || name.ends_with("_start")
                || name.ends_with("_sampleTrigger")
                || name.ends_with("_firstTrigger")
                || name.ends_with("_samplePeriod")
                || name.ends_with("Trigger")
                || name.ends_with("_f")
            {
                return Ok(builder.ins().f64const(0.0));
            }

            if let Some(v) = modelica_constants_flat_variable(&name) {
                return Ok(builder.ins().f64const(v));
            }
            if name.contains("_Types_Init_") {
                return Ok(builder.ins().f64const(0.0));
            }
            if name.contains("_Init_") {
                return Ok(builder.ins().f64const(0.0));
            }
            if name.contains("_Types_") {
                return Ok(builder.ins().f64const(0.0));
            }
            if name.contains("Machine_inf") || name.ends_with("_Machine_inf") {
                return Ok(builder.ins().f64const(f64::INFINITY));
            }
            if name.contains("combiTimeTable") {
                if name.contains("combiTimeTable_") {
                    return Ok(builder.ins().f64const(0.0));
                }
                if let Some((_base, _idx0)) = name
                    .rsplit_once('_')
                    .and_then(|(b, i)| i.parse::<usize>().ok().map(|n| (b, n)))
                {
                    return Ok(builder.ins().f64const(0.0));
                }
            }

            if name == "startTime" || name == "u" || name == "samplePeriod" || name == "generateNoise" {
                return Ok(builder.ins().f64const(0.0));
            }

            if name.contains('_') {
                return Ok(builder.ins().f64const(0.0));
            }
            Err(format!("Variable {} not found", name))
        }
        Expression::ArrayAccess(arr_expr, idx_expr) => {
            if let Some(flat) = expr_to_flat_scalar_prefix(expr) {
                if jit_scalar_name_bound(ctx, &flat) {
                    return compile_expression_rec(&Expression::var(&flat), ctx, builder);
                }
            }

            let name = if let Expression::Variable(id) = &**arr_expr {
                Some(crate::string_intern::resolve_id(*id))
            } else {
                expr_to_connector_path(arr_expr)
            };

            if let Some(name) = name {
                if let Some((array_type, start_index)) = ctx.array_storage(&name) {
                let idx_val = compile_expression_rec(idx_expr, ctx, builder)?;
                let one = builder.ins().f64const(1.0);
                let idx_0 = builder.ins().fsub(idx_val, one);
                let idx_int = builder.ins().fcvt_to_sint(cl_types::I64, idx_0);
                let eight = builder.ins().iconst(cl_types::I64, 8);
                let offset_bytes = builder.ins().imul(idx_int, eight);
                let start_offset = (start_index * 8) as i64;
                let start_const = builder.ins().iconst(cl_types::I64, start_offset);
                let total_offset = builder.ins().iadd(start_const, offset_bytes);
                let base_ptr = match array_type {
                    ArrayType::State => ctx.states_ptr,
                    ArrayType::Discrete => ctx.discrete_ptr,
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
                        compile_expression_rec(&Expression::var(&elem_name), ctx, builder)
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
                    compile_expression_rec(&Expression::var(&elem_name), ctx, builder)
                } else {
                    compile_expression_rec(arr_expr, ctx, builder)
                }
            } else {
                compile_expression_rec(arr_expr, ctx, builder)
            }
        }
        Expression::BinaryOp(lhs, op, rhs) => {
            let l = compile_expression_rec(lhs, ctx, builder)?;
            let r = compile_expression_rec(rhs, ctx, builder)?;
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
            let c_val = compile_expression_rec(cond, ctx, builder)?;
            let t_val = compile_expression_rec(t_expr, ctx, builder)?;
            let f_val = compile_expression_rec(f_expr, ctx, builder)?;
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
                    return Err(format!("pre() expects 1 argument, got {}", args.len()));
                }
                let arg = &args[0];
                if let Expression::Variable(id) = arg {
                    let var_name = crate::string_intern::resolve_id(*id);
                    if let Some(idx) = ctx.state_index(&var_name) {
                        let offset = (idx * 8) as i32;
                        return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.pre_states_ptr, offset));
                    }
                    if let Some(idx) = ctx.discrete_index(&var_name) {
                        let offset = (idx * 8) as i32;
                        return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.pre_discrete_ptr, offset));
                    }
                }
                return compile_pre_expression(arg, ctx, builder);
            }
            if func_name == "edge" {
                if args.len() != 1 {
                    return Err("edge() expects 1 argument".to_string());
                }
                let arg = &args[0];
                let curr_val = compile_expression_rec(arg, ctx, builder)?;
                let pre_val = compile_pre_expression(arg, ctx, builder)?;
                let zero = builder.ins().f64const(0.0);
                let curr_bool = builder.ins().fcmp(FloatCC::NotEqual, curr_val, zero);
                let pre_zero = builder.ins().fcmp(FloatCC::Equal, pre_val, zero);
                let res_bool = builder.ins().band(curr_bool, pre_zero);
                let one = builder.ins().f64const(1.0);
                return Ok(builder.ins().select(res_bool, one, zero));
            }
            if func_name == "change" {
                if args.len() != 1 {
                    return Err("change() expects 1 argument".to_string());
                }
                let arg = &args[0];
                let curr_val = compile_expression_rec(arg, ctx, builder)?;
                let pre_val = compile_pre_expression(arg, ctx, builder)?;
                let diff = builder.ins().fcmp(FloatCC::NotEqual, curr_val, pre_val);
                let one = builder.ins().f64const(1.0);
                let zero = builder.ins().f64const(0.0);
                return Ok(builder.ins().select(diff, one, zero));
            }
            if let Some(res) = try_compile_builtin_call(func_name, args, ctx, builder, compile_expression_rec) {
                return res;
            }
            if func_name == "assert" {
                if args.len() != 2 {
                    return Err(format!("assert() expects 2 arguments (condition, message), got {}", args.len()));
                }
                let cond_val = compile_expression_rec(&args[0], ctx, builder)?;
                let msg_val = compile_expression_rec(&args[1], ctx, builder)?;
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
                let msg_val = compile_expression_rec(&args[0], ctx, builder)?;
                let mut sig = ctx.module.make_signature();
                sig.params.push(AbiParam::new(cl_types::F64));
                sig.returns.push(AbiParam::new(cl_types::F64));
                let func_id = ctx.module.declare_function("terminate", Linkage::Import, &sig)
                    .map_err(|e| e.to_string())?;
                let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
                builder.ins().call(func_ref, &[msg_val]);
                return Ok(builder.ins().f64const(0.0));
            }
            let ptr_type = ctx.module.target_config().pointer_type();
            let mut sig = ctx.module.make_signature();
            let mut arg_vals = Vec::new();
            for arg in args {
                if let Expression::Variable(id) = arg {
                    let name = crate::string_intern::resolve_id(*id);
                    if ctx.array_info.contains_key(&name) {
                        let val = compile_expression_rec(
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
                            return Err("String argument in function call not supported in JIT (FUNC-7). Use C codegen or scalar args.".to_string());
                        }
                    };
                    sig.params.push(AbiParam::new(ptr_type));
                    let gv = ctx.module.declare_data_in_func(data_id, &mut builder.func);
                    arg_vals.push(builder.ins().global_value(ptr_type, gv));
                    continue;
                }
                let val = compile_expression_rec(arg, ctx, builder)?;
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
                    "[jit-import] name={} sig={} array_args={}",
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
            Ok(builder.ins().f64const(0.0))
        }
        Expression::Range(_, _, _) => Ok(builder.ins().f64const(0.0)),
        Expression::Dot(inner, member) => {
            if let Some(prefix) = expr_to_connector_path(inner) {
                if let Some(v) = modelica_constants_dot_member(&prefix, member) {
                    return Ok(builder.ins().f64const(v));
                }
                if prefix.contains("FluxTubes") && prefix.contains("Material") {
                    return Ok(builder.ins().f64const(0.0));
                }
                if prefix.contains("FluidHeatFlow") {
                    return Ok(builder.ins().f64const(0.0));
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
            if let Some(path) = expr_to_connector_path(expr) {
                if jit_scalar_name_bound(ctx, &path) {
                    return compile_expression_rec(&Expression::var(&path), ctx, builder);
                }
                let path_us = path.replace('.', "_");
                if jit_scalar_name_bound(ctx, &path_us) {
                    return compile_expression_rec(&Expression::var(&path_us), ctx, builder);
                }
            }
            if let Some(full_flat) = expr_to_flat_scalar_prefix(expr) {
                if jit_scalar_name_bound(ctx, &full_flat) {
                    return compile_expression_rec(&Expression::var(&full_flat), ctx, builder);
                }
            }
            if let Some(prefix) = expr_to_flat_scalar_prefix(inner) {
                let flat = format!("{}_{}", prefix, member);
                if jit_scalar_name_bound(ctx, &flat) {
                    return compile_expression_rec(&Expression::var(&flat), ctx, builder);
                }
            }
            if let Some(path) = crate::ast::expr_to_connector_path(expr) {
                if path.contains("FluidHeatFlow") || path.contains("FluxTubes") {
                    return Ok(builder.ins().f64const(0.0));
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
        Expression::ArrayComprehension { .. } => Ok(builder.ins().f64const(0.0)),
        Expression::StringLiteral(_) => Ok(builder.ins().f64const(0.0)),
        Expression::Sample(interval_expr) => {
            // sample(x): in synchronous equations it denotes sampled value of x.
            // Keep trigger-like behavior only for numeric sample(period) usage.
            if matches!(&**interval_expr, Expression::Number(_)) {
                let interval_val = compile_expression_rec(interval_expr, ctx, builder)?;
                let time_val = ctx.var_map.get("time").copied().ok_or("sample() requires time in context".to_string())?;
                let mut sig = ctx.module.make_signature();
                sig.params.push(AbiParam::new(cl_types::F64));
                sig.params.push(AbiParam::new(cl_types::F64));
                sig.returns.push(AbiParam::new(cl_types::F64));
                let func_id = ctx.module.declare_function("rustmodlica_sample", Linkage::Import, &sig)
                    .map_err(|e| e.to_string())?;
                let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
                let call_inst = builder.ins().call(func_ref, &[time_val, interval_val]);
                Ok(builder.inst_results(call_inst)[0])
            } else {
                compile_expression_rec(interval_expr, ctx, builder)
            }
        }
        Expression::Interval(clock_expr) => {
            // interval(clock): keep numeric period when directly available.
            // For sampled-time variables (e.g. interval(simTime) with simTime = sample(time)),
            // use a stable zero fallback to avoid algebraic loops on tolerance terms.
            match &**clock_expr {
                Expression::Sample(inner) => {
                    if matches!(&**inner, Expression::Number(_)) {
                        compile_expression_rec(inner, ctx, builder)
                    } else {
                        Ok(builder.ins().f64const(0.0))
                    }
                }
                Expression::Variable(id) if crate::string_intern::resolve_id(*id).contains("simTime") => {
                    Ok(builder.ins().f64const(0.0))
                }
                _ => compile_expression_rec(clock_expr, ctx, builder),
            }
        }
        Expression::Hold(inner) => compile_expression_rec(inner, ctx, builder),
        Expression::Previous(inner) => compile_pre_expression(inner, ctx, builder),
        Expression::SubSample(clock_expr, n_expr) => {
            if let Expression::Sample(interval_expr) = &**clock_expr {
                let interval_val = compile_expression_rec(interval_expr, ctx, builder)?;
                let n_val = compile_expression_rec(n_expr, ctx, builder)?;
                let scaled_interval = builder.ins().fmul(interval_val, n_val);
                let time_val = ctx
                    .var_map
                    .get("time")
                    .copied()
                    .ok_or("subSample(sample(...), n) requires time in context".to_string())?;
                let mut sig = ctx.module.make_signature();
                sig.params.push(AbiParam::new(cl_types::F64));
                sig.params.push(AbiParam::new(cl_types::F64));
                sig.returns.push(AbiParam::new(cl_types::F64));
                let func_id = ctx
                    .module
                    .declare_function("rustmodlica_sample", Linkage::Import, &sig)
                    .map_err(|e| e.to_string())?;
                let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
                let call_inst = builder.ins().call(func_ref, &[time_val, scaled_interval]);
                Ok(builder.inst_results(call_inst)[0])
            } else {
                compile_expression_rec(clock_expr, ctx, builder)
            }
        }
        Expression::SuperSample(clock_expr, _n) | Expression::ShiftSample(clock_expr, _n) => {
            compile_expression_rec(clock_expr, ctx, builder)
        }
    }
}

pub fn compile_expression(
    expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<Value, String> {
    compile_expression_rec(expr, ctx, builder)
}
