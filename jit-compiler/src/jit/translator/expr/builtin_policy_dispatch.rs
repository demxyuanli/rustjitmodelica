//! JSON-driven builtin dispatch (rules from `build.rs` -> `OUT_DIR`, plus policy overlay).

use crate::ast::Expression;
use crate::jit::context::TranslationContext;
use super::builtin_clock_sample::{compile_clock_derived_call, compile_periodic_sample_call};
use super::builtin_policy_blend::{
    compile_reg_step_blend, compile_splice_blend, passthrough_first_empty0, passthrough_first_empty1,
};
use super::builtin_policy_interpolate::{
    compile_first_true_index, compile_interp_coef, compile_interpolate_vectors,
};
use super::builtin_policy_stream::{
    stream_flow_name_for, stream_peer_names, value_name_exists, warn_stream_semantics_once,
};
use crate::jit::translator::expr::helpers::{
    jit_builtin_fallback_warn_once, jit_strict_placeholders_enabled,
};
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};

fn strict_placeholder_zero(
    func_name: &str,
    reason: &str,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<Value, String> {
    if jit_strict_placeholders_enabled() {
        return Err(format!(
            "JIT strict placeholders: function '{}' ({})",
            func_name, reason
        ));
    }
    jit_builtin_fallback_warn_once(func_name, reason);
    Ok(builder.ins().f64const(0.0))
}

fn clock_derived_op(func_name: &str) -> &'static str {
    if func_name.ends_with(".backSample") || func_name == "backSample" {
        "backSample"
    } else if func_name.ends_with(".subSample") || func_name == "subSample" {
        "subSample"
    } else if func_name.ends_with(".superSample") || func_name == "superSample" {
        "superSample"
    } else {
        "shiftSample"
    }
}

fn compile_table_lookup(
    args: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: fn(
        &Expression,
        &mut TranslationContext,
        &mut cranelift::frontend::FunctionBuilder<'_>,
    ) -> Result<Value, String>,
) -> Result<Value, String> {
    if args.is_empty() {
        return Ok(builder.ins().f64const(0.0));
    }
    let handle_f = compile_rec(&args[0], ctx, builder)?;
    let handle_i = builder.ins().fcvt_to_sint(cl_types::I64, handle_f);
    let time_v = if args.len() >= 2 {
        compile_rec(&args[1], ctx, builder)?
    } else {
        ctx.var_map
            .get("time")
            .copied()
            .unwrap_or_else(|| builder.ins().f64const(0.0))
    };
    let col_i = if args.len() >= 3 {
        let col_f = compile_rec(&args[2], ctx, builder)?;
        builder.ins().fcvt_to_sint(cl_types::I64, col_f)
    } else {
        builder.ins().iconst(cl_types::I64, 1)
    };
    let mut sig = ctx.module.make_signature();
    sig.params.push(AbiParam::new(cl_types::I64));
    sig.params.push(AbiParam::new(cl_types::F64));
    sig.params.push(AbiParam::new(cl_types::I64));
    sig.returns.push(AbiParam::new(cl_types::F64));
    let fid = ctx
        .module
        .declare_function("rustmodlica_table_get_value", Linkage::Import, &sig)
        .map_err(|e| e.to_string())?;
    let fref = ctx.module.declare_func_in_func(fid, &mut builder.func);
    let inst = builder.ins().call(fref, &[handle_i, time_v, col_i]);
    Ok(builder.inst_results(inst)[0])
}

pub(super) fn dispatch_named_builtin_policy(
    handler_id: &str,
    func_name: &str,
    args: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: fn(
        &Expression,
        &mut TranslationContext,
        &mut cranelift::frontend::FunctionBuilder<'_>,
    ) -> Result<Value, String>,
) -> Result<Value, String> {
    match handler_id {
        "sample_interval" => compile_periodic_sample_call(args, ctx, builder, compile_rec),
        "passthrough_first_empty0" => passthrough_first_empty0(func_name, args, ctx, builder, compile_rec),
        "passthrough_first_empty1" => passthrough_first_empty1(func_name, args, ctx, builder, compile_rec),
        "const0_warn_gravity" => {
            strict_placeholder_zero(func_name, "gravity-placeholder", builder)
        }
        "const0_warn_medium" => {
            strict_placeholder_zero(func_name, "medium-package-placeholder", builder)
        }
        "const0_warn_internal" => {
            strict_placeholder_zero(func_name, "internal-package-placeholder", builder)
        }
        "reg_step_blend" => compile_reg_step_blend(args, ctx, builder, compile_rec),
        "splice_blend" => compile_splice_blend(args, ctx, builder, compile_rec),
        "const0_warn_connections" => {
            strict_placeholder_zero(func_name, "connections-placeholder", builder)
        }
        "const0_warn_noise" => {
            strict_placeholder_zero(func_name, "generate-noise-placeholder", builder)
        }
        "interp_coef" => compile_interp_coef(func_name, args, ctx, builder, compile_rec),
        "semi_linear" => {
            if args.len() >= 3 {
                let x = compile_rec(&args[0], ctx, builder)?;
                let k_pos = compile_rec(&args[1], ctx, builder)?;
                let k_neg = compile_rec(&args[2], ctx, builder)?;
                let zero = builder.ins().f64const(0.0);
                let branch = builder
                    .ins()
                    .fcmp(FloatCC::GreaterThanOrEqual, x, zero);
                let x_k_pos = builder.ins().fmul(x, k_pos);
                let x_k_neg = builder.ins().fmul(x, k_neg);
                return Ok(builder.ins().select(branch, x_k_pos, x_k_neg));
            }
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "outer_product" => {
            if args.len() >= 2 {
                let u_val = match compile_rec(&args[0], ctx, builder) {
                    Ok(v) => v,
                    Err(_) => return Ok(builder.ins().f64const(0.0)),
                };
                let v_val = match compile_rec(&args[1], ctx, builder) {
                    Ok(v) => v,
                    Err(_) => return Ok(builder.ins().f64const(0.0)),
                };
                return Ok(builder.ins().fmul(u_val, v_val));
            }
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "identity_jit" => {
            if args.len() >= 1 {
                return Ok(builder.ins().f64const(1.0));
            }
            Ok(builder.ins().f64const(0.0))
        }
        "skew_jit" => {
            if args.len() >= 1 {
                let w_val = match compile_rec(&args[0], ctx, builder) {
                    Ok(v) => v,
                    Err(_) => return Ok(builder.ins().f64const(0.0)),
                };
                return Ok(w_val);
            }
            Ok(builder.ins().f64const(0.0))
        }
        "const0_warn_baseclasses" => {
            strict_placeholder_zero(func_name, "baseclasses-placeholder", builder)
        }
        "const0_warn_frames" => {
            strict_placeholder_zero(func_name, "frames-placeholder", builder)
        }
        "noevent_1" => {
            if args.len() != 1 {
                return Err(format!("noEvent() expects 1 argument, got {}", args.len()));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "instream" => {
            if args.len() != 1 {
                return Err(format!(
                    "inStream() minimal JIT semantics expects exactly 1 argument, got {}",
                    args.len()
                ));
            }
            warn_stream_semantics_once("inStream");
            if let Expression::Variable(id) = &args[0] {
                let stream_name = crate::string_intern::resolve_id(*id);
                if let Some(self_flow_name) = stream_flow_name_for(ctx, &stream_name) {
                    if value_name_exists(ctx, &self_flow_name) {
                        let self_v = compile_rec(&args[0], ctx, builder)?;
                        let peers = stream_peer_names(ctx, &stream_name);
                        if peers.is_empty() {
                            warn_stream_semantics_once("peerMissing");
                            return Ok(self_v);
                        }
                        let mut numerator = builder.ins().f64const(0.0);
                        let mut denominator = builder.ins().f64const(0.0);
                        for peer_name in peers {
                            let Some(peer_flow_name) = stream_flow_name_for(ctx, &peer_name) else {
                                continue;
                            };
                            if !value_name_exists(ctx, &peer_flow_name) || !value_name_exists(ctx, &peer_name) {
                                continue;
                            }
                            let m_peer = compile_rec(&Expression::var(&peer_flow_name), ctx, builder)?;
                            let h_peer = compile_rec(&Expression::var(&peer_name), ctx, builder)?;
                            let zero = builder.ins().f64const(0.0);
                            let neg_m = builder.ins().fsub(zero, m_peer);
                            let active = builder.ins().fcmp(FloatCC::GreaterThan, neg_m, zero);
                            let w = builder.ins().select(active, neg_m, zero);
                            let contrib = builder.ins().fmul(w, h_peer);
                            numerator = builder.ins().fadd(numerator, contrib);
                            denominator = builder.ins().fadd(denominator, w);
                        }
                        let eps = builder.ins().f64const(1e-12);
                        let has_mix = builder.ins().fcmp(FloatCC::GreaterThan, denominator, eps);
                        let mixed = builder.ins().fdiv(numerator, denominator);
                        return Ok(builder.ins().select(has_mix, mixed, self_v));
                    }
                }
                warn_stream_semantics_once("peerMissing");
            }
            compile_rec(&args[0], ctx, builder)
        }
        "actualstream" => {
            if args.len() != 1 {
                return Err(format!(
                    "actualStream() minimal JIT semantics expects exactly 1 argument, got {}",
                    args.len()
                ));
            }
            warn_stream_semantics_once("actualStream");
            if let Expression::Variable(id) = &args[0] {
                let stream_name = crate::string_intern::resolve_id(*id);
                if let Some(flow_name) = stream_flow_name_for(ctx, &stream_name) {
                    if value_name_exists(ctx, &flow_name) {
                        let flow_v = compile_rec(&Expression::var(&flow_name), ctx, builder)?;
                        let self_v = compile_rec(&args[0], ctx, builder)?;
                        let instream_v = {
                            let peers = stream_peer_names(ctx, &stream_name);
                            if peers.is_empty() {
                                self_v
                            } else {
                                let mut numerator = builder.ins().f64const(0.0);
                                let mut denominator = builder.ins().f64const(0.0);
                                for peer_name in peers {
                                    let Some(peer_flow_name) = stream_flow_name_for(ctx, &peer_name) else {
                                        continue;
                                    };
                                    if !value_name_exists(ctx, &peer_flow_name)
                                        || !value_name_exists(ctx, &peer_name)
                                    {
                                        continue;
                                    }
                                    let m_peer = compile_rec(&Expression::var(&peer_flow_name), ctx, builder)?;
                                    let h_peer = compile_rec(&Expression::var(&peer_name), ctx, builder)?;
                                    let zero = builder.ins().f64const(0.0);
                                    let neg_m = builder.ins().fsub(zero, m_peer);
                                    let active = builder.ins().fcmp(FloatCC::GreaterThan, neg_m, zero);
                                    let w = builder.ins().select(active, neg_m, zero);
                                    let contrib = builder.ins().fmul(w, h_peer);
                                    numerator = builder.ins().fadd(numerator, contrib);
                                    denominator = builder.ins().fadd(denominator, w);
                                }
                                let eps = builder.ins().f64const(1e-12);
                                let has_mix = builder.ins().fcmp(FloatCC::GreaterThan, denominator, eps);
                                let mixed = builder.ins().fdiv(numerator, denominator);
                                builder.ins().select(has_mix, mixed, self_v)
                            }
                        };
                        let eps = builder.ins().f64const(1e-12);
                        let outflow = builder.ins().fcmp(FloatCC::GreaterThan, flow_v, eps);
                        return Ok(builder.ins().select(outflow, self_v, instream_v));
                    }
                }
                warn_stream_semantics_once("peerMissing");
            }
            compile_rec(&args[0], ctx, builder)
        }
        "valve_char_1" => {
            if args.len() != 1 {
                return Err(format!(
                    "valveCharacteristic() expects 1 argument, got {}",
                    args.len()
                ));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "imag_zero" => {
            if args.len() != 1 {
                return Err(format!(
                    "imag() expects 1 argument in scalar JIT, got {}",
                    args.len()
                ));
            }
            let _ = compile_rec(&args[0], ctx, builder)?;
            Ok(builder.ins().f64const(0.0))
        }
        "cardinality_zero" => {
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
                    "JIT strict placeholders: cardinality (no flatten degree) at '{}'",
                    func_name
                ));
            }
            jit_builtin_fallback_warn_once(func_name, "cardinality-placeholder");
            Ok(builder.ins().f64const(0.0))
        }
        "initial_fn" => {
            if !args.is_empty() {
                return Err(format!("initial() expects 0 arguments, got {}", args.len()));
            }
            if let Some(&t_val) = ctx.var_map.get("time") {
                let zero = builder.ins().f64const(0.0);
                let diff = builder.ins().fsub(t_val, zero);
                let abs = builder.ins().fabs(diff);
                let eps = builder.ins().f64const(1e-9);
                let is_initial = builder.ins().fcmp(FloatCC::LessThanOrEqual, abs, eps);
                let one = builder.ins().f64const(1.0);
                let z = builder.ins().f64const(0.0);
                return Ok(builder.ins().select(is_initial, one, z));
            }
            if jit_strict_placeholders_enabled() {
                return Err(format!(
                    "JIT strict placeholders: initial() without time in var_map ({})",
                    func_name
                ));
            }
            jit_builtin_fallback_warn_once(func_name, "initial-without-time");
            Ok(builder.ins().f64const(0.0))
        }
        "terminal_fn" => {
            if !args.is_empty() {
                return Err(format!("terminal() expects 0 arguments, got {}", args.len()));
            }
            if let (Some(&t_val), Some(&t_end_val)) =
                (ctx.var_map.get("time"), ctx.var_map.get("t_end"))
            {
                let diff = builder.ins().fsub(t_end_val, t_val);
                let abs = builder.ins().fabs(diff);
                let eps = builder.ins().f64const(1e-9);
                let is_terminal = builder.ins().fcmp(FloatCC::LessThanOrEqual, abs, eps);
                let one = builder.ins().f64const(1.0);
                let z = builder.ins().f64const(0.0);
                return Ok(builder.ins().select(is_terminal, one, z));
            }
            if jit_strict_placeholders_enabled() {
                return Err(format!(
                    "JIT strict placeholders: terminal() without time/t_end ({})",
                    func_name
                ));
            }
            jit_builtin_fallback_warn_once(func_name, "terminal-without-time");
            Ok(builder.ins().f64const(0.0))
        }
        "boolean_1" => {
            if args.len() != 1 {
                return Err(format!("Boolean() expects 1 argument, got {}", args.len()));
            }
            let x = compile_rec(&args[0], ctx, builder)?;
            let zero = builder.ins().f64const(0.0);
            let one = builder.ins().f64const(1.0);
            let cmp = builder.ins().fcmp(FloatCC::NotEqual, x, zero);
            Ok(builder.ins().select(cmp, one, zero))
        }
        "abs_1" => {
            if args.len() != 1 {
                return Err(format!("abs() expects 1 argument, got {}", args.len()));
            }
            let v = compile_rec(&args[0], ctx, builder)?;
            Ok(builder.ins().fabs(v))
        }
        "max_2" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            if args.len() == 1 {
                return compile_rec(&args[0], ctx, builder);
            }
            let a = compile_rec(&args[0], ctx, builder)?;
            let b = compile_rec(&args[1], ctx, builder)?;
            let cc = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, a, b);
            Ok(builder.ins().select(cc, a, b))
        }
        "min_2" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            if args.len() == 1 {
                return compile_rec(&args[0], ctx, builder);
            }
            let a = compile_rec(&args[0], ctx, builder)?;
            let b = compile_rec(&args[1], ctx, builder)?;
            let cc = builder.ins().fcmp(FloatCC::LessThanOrEqual, a, b);
            Ok(builder.ins().select(cc, a, b))
        }
        "integer_1" => {
            if args.len() != 1 {
                return Err(format!("integer() expects 1 argument, got {}", args.len()));
            }
            let v = compile_rec(&args[0], ctx, builder)?;
            Ok(builder.ins().floor(v))
        }
        "homotopy_var" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            let actual = compile_rec(&args[0], ctx, builder)?;
            if args.len() < 2 {
                return Ok(actual);
            }
            let simplified = compile_rec(&args[1], ctx, builder)?;
            let lambda = builder.ins().load(
                cl_types::F64,
                MemFlags::new(),
                ctx.homotopy_lambda_ptr,
                0,
            );
            let one = builder.ins().f64const(1.0);
            let one_minus_lambda = builder.ins().fsub(one, lambda);
            let term1 = builder.ins().fmul(lambda, actual);
            let term2 = builder.ins().fmul(one_minus_lambda, simplified);
            Ok(builder.ins().fadd(term1, term2))
        }
        "size_jit" => {
            if args.is_empty() {
                return Err("size() requires at least 1 argument (array)".to_string());
            }
            if let Expression::Variable(id) = &args[0] {
                let arr_name = crate::string_intern::resolve_id(*id);
                if let Some(info) = ctx.array_info.get(&arr_name) {
                    let dim = if args.len() >= 2 {
                        if let Expression::Number(d) = &args[1] {
                            (*d as i64).max(1).min(info.size as i64) as usize
                        } else {
                            1
                        }
                    } else {
                        1
                    };
                    let size_val = if dim == 1 { info.size } else { 1 };
                    return Ok(builder.ins().f64const(size_val as f64));
                }
            }
            Ok(builder.ins().f64const(1.0))
        }
        "first_tick" => {
            if !args.is_empty() {
                return Err(format!("firstTick() expects 0 arguments, got {}", args.len()));
            }
            if let Some(&t_val) = ctx.var_map.get("time") {
                let zero = builder.ins().f64const(0.0);
                let diff = builder.ins().fsub(t_val, zero);
                let abs = builder.ins().fabs(diff);
                let eps = builder.ins().f64const(1e-9);
                let is_first = builder.ins().fcmp(FloatCC::LessThanOrEqual, abs, eps);
                let one = builder.ins().f64const(1.0);
                let z = builder.ins().f64const(0.0);
                return Ok(builder.ins().select(is_first, one, z));
            }
            Ok(builder.ins().f64const(0.0))
        }
        "first_true_index" => compile_first_true_index(args, ctx, builder, compile_rec),
        "interpolate" => compile_interpolate_vectors(args, ctx, builder, compile_rec),
        "get_next_time_event" => {
            if !args.is_empty() {
                return Err(format!(
                    "getNextTimeEvent() expects 0 arguments, got {}",
                    args.len()
                ));
            }
            Ok(builder.ins().f64const(0.0))
        }
        "is_empty_one" => {
            if args.len() != 1 {
                return Err(format!(
                    "isEmpty() expects 1 argument (string), got {}",
                    args.len()
                ));
            }
            if let Expression::StringLiteral(s) = &args[0] {
                return Ok(builder.ins().f64const(if s.is_empty() { 1.0 } else { 0.0 }));
            }
            Err("isEmpty() requires string literal in JIT context".to_string())
        }
        "named_last" => {
            if let Some(last) = args.last() {
                compile_rec(last, ctx, builder)
            } else {
                Ok(builder.ins().f64const(0.0))
            }
        }
        "cat" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            if args.len() >= 2 {
                return compile_rec(&args[1], ctx, builder);
            }
            compile_rec(&args[0], ctx, builder)
        }
        "modelicatest_one" => Ok(builder.ins().f64const(1.0)),
        "not_1" => {
            if args.len() != 1 {
                return Err(format!("not() expects 1 argument, got {}", args.len()));
            }
            let v = compile_rec(&args[0], ctx, builder)?;
            let zero = builder.ins().f64const(0.0);
            let one = builder.ins().f64const(1.0);
            let is_zero = builder.ins().fcmp(FloatCC::Equal, v, zero);
            Ok(builder.ins().select(is_zero, one, zero))
        }
        "clock_derived" => {
            let op = clock_derived_op(func_name);
            compile_clock_derived_call(op, args, ctx, builder, compile_rec)
        }
        "number_symmetric" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(1.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "combitable_err0" => {
            if args.is_empty() {
                return Err(format!(
                    "[JIT_TABLE_CONFIG] {} expects at least 1 argument (table or handle), got 0",
                    func_name
                ));
            }
            compile_table_lookup(args, ctx, builder, compile_rec)
        }
        "ext_object_err0" => {
            if !args.is_empty() {
                return Err(format!(
                    "[JIT_EXTERNAL_OBJECT] {} in validate-only JIT does not accept runtime arguments (got {})",
                    func_name,
                    args.len()
                ));
            }
            Ok(builder.ins().f64const(0.0))
        }
        "ext_combitimetable_warn0" => {
            jit_builtin_fallback_warn_once(func_name, "external-combitimetable-placeholder");
            compile_table_lookup(args, ctx, builder, compile_rec)
        }
        "loadresource_warn0" => {
            if jit_strict_placeholders_enabled() {
                return Err(format!(
                    "JIT strict placeholders: loadResource not supported ({})",
                    func_name
                ));
            }
            if let Some(Expression::StringLiteral(uri)) = args.first() {
                let path = uri
                    .strip_prefix("file://")
                    .or_else(|| uri.strip_prefix("file:"))
                    .unwrap_or(uri.as_str());
                let exists = std::path::Path::new(path).exists();
                let v = if exists { 1.0 } else { 0.0 };
                jit_builtin_fallback_warn_once(func_name, "loadresource-path-probe");
                return Ok(builder.ins().f64const(v));
            }
            strict_placeholder_zero(func_name, "loadresource-placeholder", builder)
        }
        "type_conv_pf0" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "product_fn" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(1.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "sum_fn" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "zeros_fn" => {
            strict_placeholder_zero(func_name, "zeros-placeholder", builder)
        }
        "ones_fn" => Ok(builder.ins().f64const(1.0)),
        _ => Err(format!("unknown JIT builtin handler_id: {}", handler_id)),
    }
}
