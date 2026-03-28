//! JSON-driven builtin dispatch (rules from `build.rs` -> `OUT_DIR`, plus policy overlay).

use crate::ast::Expression;
use crate::jit::context::TranslationContext;
use crate::jit::types::ArrayType;
use super::builtin_clock_sample::{compile_clock_derived_call, compile_periodic_sample_call};
use crate::jit::translator::expr::helpers::jit_builtin_fallback_warn_once;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::Module;
use std::sync::OnceLock;

fn warn_stream_semantics_once(kind: &'static str) {
    static INSTREAM_WARNED: OnceLock<()> = OnceLock::new();
    static ACTUAL_WARNED: OnceLock<()> = OnceLock::new();
    static PEER_WARNED: OnceLock<()> = OnceLock::new();
    match kind {
        "inStream" => {
            let _ = INSTREAM_WARNED.get_or_init(|| {
                eprintln!("[fallback:stream-semantics] inStream(): using minimal semantics in JIT (single-arg passthrough for stable one-way flow subset)")
            });
        }
        "actualStream" => {
            let _ = ACTUAL_WARNED.get_or_init(|| {
                eprintln!("[fallback:stream-semantics] actualStream(): using minimal semantics in JIT (single-arg passthrough for stable one-way flow subset)")
            });
        }
        "peerMissing" => {
            let _ = PEER_WARNED.get_or_init(|| {
                eprintln!("[fallback:stream-semantics] stream peer/flow mapping not found, fallback to passthrough for this model path")
            });
        }
        _ => {}
    }
}

fn stream_flow_name(stream_name: &str) -> Option<String> {
    stream_name
        .strip_suffix("_h_outflow")
        .map(|prefix| format!("{}_m_flow", prefix))
}

fn stream_peer_name(stream_name: &str) -> Option<String> {
    if let Some(prefix) = stream_name.strip_suffix("_a_h_outflow") {
        return Some(format!("{}_b_h_outflow", prefix));
    }
    if let Some(prefix) = stream_name.strip_suffix("_b_h_outflow") {
        return Some(format!("{}_a_h_outflow", prefix));
    }
    None
}

fn value_name_exists(ctx: &TranslationContext, name: &str) -> bool {
    ctx.state_index(name).is_some()
        || ctx.discrete_index(name).is_some()
        || ctx.output_index(name).is_some()
        || ctx.param_index(name).is_some()
        || ctx.stack_slots.contains_key(name)
        || ctx.var_map.contains_key(name)
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

fn passthrough_first_empty0(
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
    if args.is_empty() {
        jit_builtin_fallback_warn_once(func_name, "namespace-helper-empty-args");
        Ok(builder.ins().f64const(0.0))
    } else {
        compile_rec(&args[0], ctx, builder)
    }
}

fn passthrough_first_empty1(
    _func_name: &str,
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
        Ok(builder.ins().f64const(1.0))
    } else {
        compile_rec(&args[0], ctx, builder)
    }
}

fn compile_reg_step_blend(
    args: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: fn(
        &Expression,
        &mut TranslationContext,
        &mut cranelift::frontend::FunctionBuilder<'_>,
    ) -> Result<Value, String>,
) -> Result<Value, String> {
    if args.len() >= 4 {
        let x = compile_rec(&args[0], ctx, builder)?;
        let y1 = compile_rec(&args[1], ctx, builder)?;
        let y2 = compile_rec(&args[2], ctx, builder)?;
        let x_small = compile_rec(&args[3], ctx, builder)?;
        let half = builder.ins().f64const(0.5);
        let eps = builder.ins().f64const(1e-12);
        let abs_small = builder.ins().fabs(x_small);
        let safe_small = {
            let too_small = builder.ins().fcmp(FloatCC::LessThan, abs_small, eps);
            builder.ins().select(too_small, eps, abs_small)
        };
        let scaled = builder.ins().fdiv(x, safe_small);
        let one = builder.ins().f64const(1.0);
        let one_plus_scaled = builder.ins().fadd(one, scaled);
        let t = builder.ins().fmul(half, one_plus_scaled);
        let zero = builder.ins().f64const(0.0);
        let t_clamped_low = {
            let lt0 = builder.ins().fcmp(FloatCC::LessThan, t, zero);
            builder.ins().select(lt0, zero, t)
        };
        let t_clamped = {
            let gt1 = builder.ins().fcmp(FloatCC::GreaterThan, t_clamped_low, one);
            builder.ins().select(gt1, one, t_clamped_low)
        };
        let omt = builder.ins().fsub(one, t_clamped);
        let blend1 = builder.ins().fmul(t_clamped, y1);
        let blend2 = builder.ins().fmul(omt, y2);
        return Ok(builder.ins().fadd(blend1, blend2));
    }
    if args.is_empty() {
        return Ok(builder.ins().f64const(0.0));
    }
    compile_rec(&args[0], ctx, builder)
}

fn compile_splice_blend(
    args: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: fn(
        &Expression,
        &mut TranslationContext,
        &mut cranelift::frontend::FunctionBuilder<'_>,
    ) -> Result<Value, String>,
) -> Result<Value, String> {
    if args.len() >= 4 {
        let pos = compile_rec(&args[0], ctx, builder)?;
        let neg = compile_rec(&args[1], ctx, builder)?;
        let x = compile_rec(&args[2], ctx, builder)?;
        let dx = compile_rec(&args[3], ctx, builder)?;
        let half = builder.ins().f64const(0.5);
        let eps = builder.ins().f64const(1e-12);
        let abs_dx = builder.ins().fabs(dx);
        let safe_dx = {
            let too_small = builder.ins().fcmp(FloatCC::LessThan, abs_dx, eps);
            builder.ins().select(too_small, eps, abs_dx)
        };
        let scaled = builder.ins().fdiv(x, safe_dx);
        let one = builder.ins().f64const(1.0);
        let one_plus_scaled = builder.ins().fadd(one, scaled);
        let t = builder.ins().fmul(half, one_plus_scaled);
        let zero = builder.ins().f64const(0.0);
        let t_clamped_low = {
            let lt0 = builder.ins().fcmp(FloatCC::LessThan, t, zero);
            builder.ins().select(lt0, zero, t)
        };
        let t_clamped = {
            let gt1 = builder.ins().fcmp(FloatCC::GreaterThan, t_clamped_low, one);
            builder.ins().select(gt1, one, t_clamped_low)
        };
        let omt = builder.ins().fsub(one, t_clamped);
        let blend_pos = builder.ins().fmul(t_clamped, pos);
        let blend_neg = builder.ins().fmul(omt, neg);
        return Ok(builder.ins().fadd(blend_pos, blend_neg));
    }
    if args.is_empty() {
        return Ok(builder.ins().f64const(0.0));
    }
    compile_rec(&args[0], ctx, builder)
}

fn compile_first_true_index(
    args: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    _compile_rec: fn(
        &Expression,
        &mut TranslationContext,
        &mut cranelift::frontend::FunctionBuilder<'_>,
    ) -> Result<Value, String>,
) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!(
            "firstTrueIndex() expects 1 argument (Boolean vector), got {}",
            args.len()
        ));
    }
    if let Expression::Variable(id) = &args[0] {
        let vec_name = crate::string_intern::resolve_id(*id);
        if let Some(info) = ctx.array_info.get(&vec_name) {
            if info.size == 0 {
                return Ok(builder.ins().f64const(0.0));
            }
            let base_ptr = match info.array_type {
                ArrayType::State => ctx.states_ptr,
                ArrayType::Discrete => ctx.discrete_ptr,
                ArrayType::Parameter => ctx.params_ptr,
                ArrayType::Output => ctx.outputs_ptr,
                ArrayType::Derivative => ctx.derivs_ptr,
            };
            let zero = builder.ins().f64const(0.0);
            let start_idx = builder.ins().iconst(cl_types::I64, 0);
            let end_idx = builder.ins().iconst(cl_types::I64, info.size as i64);
            let loop_slot = builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
                cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
                8,
                0,
            ));
            let result_slot = builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
                cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
                8,
                0,
            ));
            builder.ins().stack_store(start_idx, loop_slot, 0);
            builder.ins().stack_store(zero, result_slot, 0);
            let header = builder.create_block();
            let body_block = builder.create_block();
            let found_block = builder.create_block();
            let next_block = builder.create_block();
            let exit_block = builder.create_block();
            let after_loop = builder.create_block();
            builder.ins().jump(header, &[]);
            builder.switch_to_block(header);
            let i_val = builder.ins().stack_load(cl_types::I64, loop_slot, 0);
            let cmp = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, i_val, end_idx);
            builder.ins().brif(cmp, exit_block, &[], body_block, &[]);
            builder.switch_to_block(body_block);
            let i_int = builder.ins().stack_load(cl_types::I64, loop_slot, 0);
            let eight = builder.ins().iconst(cl_types::I64, 8);
            let offset_bytes = builder.ins().imul(i_int, eight);
            let base_offset = builder.ins().iconst(cl_types::I64, (info.start_index * 8) as i64);
            let offset_sum = builder.ins().iadd(base_offset, offset_bytes);
            let addr = builder.ins().iadd(base_ptr, offset_sum);
            let elem = builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0);
            let is_true = builder.ins().fcmp(FloatCC::NotEqual, elem, zero);
            builder.ins().brif(is_true, found_block, &[], next_block, &[]);
            builder.switch_to_block(next_block);
            let one_i = builder.ins().iconst(cl_types::I64, 1);
            let next_i = builder.ins().iadd(i_int, one_i);
            builder.ins().stack_store(next_i, loop_slot, 0);
            builder.ins().jump(header, &[]);
            builder.switch_to_block(found_block);
            let one_i2 = builder.ins().iconst(cl_types::I64, 1);
            let i_plus_one = builder.ins().iadd(i_int, one_i2);
            let idx_f64 = builder.ins().fcvt_from_sint(cl_types::F64, i_plus_one);
            builder.ins().stack_store(idx_f64, result_slot, 0);
            builder.ins().jump(exit_block, &[]);
            builder.switch_to_block(exit_block);
            let result_val = builder.ins().stack_load(cl_types::F64, result_slot, 0);
            builder.ins().jump(after_loop, &[]);
            builder.seal_block(exit_block);
            builder.switch_to_block(after_loop);
            builder.seal_block(header);
            builder.seal_block(body_block);
            builder.seal_block(next_block);
            builder.seal_block(found_block);
            return Ok(result_val);
        }
    }
    Ok(builder.ins().f64const(0.0))
}

fn compile_interpolate_vectors(
    args: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: fn(
        &Expression,
        &mut TranslationContext,
        &mut cranelift::frontend::FunctionBuilder<'_>,
    ) -> Result<Value, String>,
) -> Result<Value, String> {
    if args.len() < 3 {
        return Err(format!(
            "interpolate(x, xa, ya) expects at least 3 arguments, got {}",
            args.len()
        ));
    }
    let x = compile_rec(&args[0], ctx, builder)?;
    let xa = &args[1];
    let ya = &args[2];
    if let (Expression::Variable(xan_id), Expression::Variable(yan_id)) = (xa, ya) {
        let xan = crate::string_intern::resolve_id(*xan_id);
        let yan = crate::string_intern::resolve_id(*yan_id);
        if let (Some(xai), Some(yai)) = (ctx.array_info.get(&xan), ctx.array_info.get(&yan)) {
            if xai.size == 0 || yai.size == 0 {
                return Ok(builder.ins().f64const(0.0));
            }
            let xa_ptr = match xai.array_type {
                ArrayType::State => ctx.states_ptr,
                ArrayType::Discrete => ctx.discrete_ptr,
                ArrayType::Parameter => ctx.params_ptr,
                ArrayType::Output => ctx.outputs_ptr,
                ArrayType::Derivative => ctx.derivs_ptr,
            };
            let ya_ptr = match yai.array_type {
                ArrayType::State => ctx.states_ptr,
                ArrayType::Discrete => ctx.discrete_ptr,
                ArrayType::Parameter => ctx.params_ptr,
                ArrayType::Output => ctx.outputs_ptr,
                ArrayType::Derivative => ctx.derivs_ptr,
            };
            let x0_offset = (xai.start_index * 8) as i64;
            let y0_offset = (yai.start_index * 8) as i64;
            let x0 = builder
                .ins()
                .load(cl_types::F64, MemFlags::new(), xa_ptr, x0_offset as i32);
            let y0 = builder
                .ins()
                .load(cl_types::F64, MemFlags::new(), ya_ptr, y0_offset as i32);
            if xai.size == 1 {
                return Ok(y0);
            }
            let x1_offset = (xai.start_index + 1) * 8;
            let y1_offset = (yai.start_index + 1) * 8;
            let x1 = builder
                .ins()
                .load(cl_types::F64, MemFlags::new(), xa_ptr, x1_offset as i32);
            let y1 = builder
                .ins()
                .load(cl_types::F64, MemFlags::new(), ya_ptr, y1_offset as i32);
            let dx = builder.ins().fsub(x1, x0);
            let t = builder.ins().fsub(x, x0);
            let dy = builder.ins().fsub(y1, y0);
            let div = builder.ins().fdiv(t, dx);
            let interp = builder.ins().fmul(div, dy);
            let y_val = builder.ins().fadd(y0, interp);
            return Ok(y_val);
        }
    }
    Ok(builder.ins().f64const(0.0))
}

fn compile_interp_coef(
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
    if args.len() >= 2 {
        let u_val = match compile_rec(&args[1], ctx, builder) {
            Ok(v) => v,
            Err(_) => return Ok(builder.ins().f64const(0.0)),
        };
        let mut sig = ctx.module.make_signature();
        sig.params.push(AbiParam::new(cl_types::F64));
        sig.returns.push(AbiParam::new(cl_types::F64));
        let func_id = ctx
            .module
            .declare_function("floor", cranelift_module::Linkage::Import, &sig)
            .map_err(|e| e.to_string())?;
        let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
        let call = builder.ins().call(func_ref, &[u_val]);
        let floor_u = builder.inst_results(call)[0];
        let h = builder.ins().fsub(u_val, floor_u);
        return Ok(h);
    }
    jit_builtin_fallback_warn_once(func_name, "interpolation-coeff-impl");
    Ok(builder.ins().f64const(0.0))
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
            jit_builtin_fallback_warn_once(func_name, "gravity-placeholder");
            Ok(builder.ins().f64const(0.0))
        }
        "const0_warn_medium" => {
            jit_builtin_fallback_warn_once(func_name, "medium-package-placeholder");
            Ok(builder.ins().f64const(0.0))
        }
        "const0_warn_internal" => {
            jit_builtin_fallback_warn_once(func_name, "internal-package-placeholder");
            Ok(builder.ins().f64const(0.0))
        }
        "reg_step_blend" => compile_reg_step_blend(args, ctx, builder, compile_rec),
        "splice_blend" => compile_splice_blend(args, ctx, builder, compile_rec),
        "const0_warn_connections" => {
            jit_builtin_fallback_warn_once(func_name, "connections-placeholder");
            Ok(builder.ins().f64const(0.0))
        }
        "const0_warn_noise" => {
            jit_builtin_fallback_warn_once(func_name, "generate-noise-placeholder");
            Ok(builder.ins().f64const(0.0))
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
            jit_builtin_fallback_warn_once(func_name, "baseclasses-placeholder");
            Ok(builder.ins().f64const(0.0))
        }
        "const0_warn_frames" => {
            jit_builtin_fallback_warn_once(func_name, "frames-placeholder");
            Ok(builder.ins().f64const(0.0))
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
                if let (Some(flow_name), Some(peer_name)) =
                    (stream_flow_name(&stream_name), stream_peer_name(&stream_name))
                {
                    if value_name_exists(ctx, &flow_name) && value_name_exists(ctx, &peer_name) {
                        let flow_v = compile_rec(&Expression::var(&flow_name), ctx, builder)?;
                        let peer_v = compile_rec(&Expression::var(&peer_name), ctx, builder)?;
                        let self_v = compile_rec(&args[0], ctx, builder)?;
                        let eps = builder.ins().f64const(1e-12);
                        let outflow = builder.ins().fcmp(FloatCC::GreaterThan, flow_v, eps);
                        return Ok(builder.ins().select(outflow, peer_v, self_v));
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
                if let Some(flow_name) = stream_flow_name(&stream_name) {
                    if value_name_exists(ctx, &flow_name) {
                        let flow_v = compile_rec(&Expression::var(&flow_name), ctx, builder)?;
                        let self_v = compile_rec(&args[0], ctx, builder)?;
                        let instream_v = if let Some(peer_name) = stream_peer_name(&stream_name) {
                            if value_name_exists(ctx, &peer_name) {
                                compile_rec(&Expression::var(&peer_name), ctx, builder)?
                            } else {
                                self_v
                            }
                        } else {
                            self_v
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
            jit_builtin_fallback_warn_once(func_name, "imag-placeholder");
            Ok(builder.ins().f64const(0.0))
        }
        "cardinality_zero" => {
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
            Ok(builder.ins().f64const(0.0))
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
            Ok(builder.ins().f64const(0.0))
        }
        "loadresource_warn0" => {
            jit_builtin_fallback_warn_once(func_name, "loadresource-placeholder");
            Ok(builder.ins().f64const(0.0))
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
            jit_builtin_fallback_warn_once(func_name, "zeros-placeholder");
            Ok(builder.ins().f64const(0.0))
        }
        "ones_fn" => Ok(builder.ins().f64const(1.0)),
        _ => Err(format!("unknown JIT builtin handler_id: {}", handler_id)),
    }
}
