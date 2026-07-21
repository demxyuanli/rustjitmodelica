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
        // Legacy overlay alias + MultiBody world.gravityAcceleration(frame_r).
        "const0_warn_gravity" | "gravity_accel" => {
            // Return first arg (position) unused; scalar path uses constant g downward.
            // Modelica default |g| ~ 9.81; sign absorbed in world frame equations.
            if !args.is_empty() {
                let _ = compile_rec(&args[0], ctx, builder)?;
            }
            Ok(builder.ins().f64const(9.80665))
        }
        "vectors_normalize" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(1.0));
            }
            // Scalar path: treat as unit-scale passthrough of the vector head element.
            compile_rec(&args[0], ctx, builder)
        }
        "fluid_reg" => {
            // Fluid.Utilities.regRoot / regRoot2 / regSquare*: signed sqrt/square of x.
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            let x = compile_rec(&args[0], ctx, builder)?;
            let zero = builder.ins().f64const(0.0);
            let fname = func_name.rsplit('.').next().unwrap_or(func_name);
            let pos = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, x, zero);
            match fname {
                "regRoot" | "regRoot2" | "regRoot_der" => {
                    let k1 = if args.len() >= 3 {
                        compile_rec(&args[2], ctx, builder)?
                    } else {
                        builder.ins().f64const(1.0)
                    };
                    let k2 = if args.len() >= 4 {
                        compile_rec(&args[3], ctx, builder)?
                    } else {
                        k1
                    };
                    let ax = builder.ins().fabs(x);
                    let k = builder.ins().select(pos, k1, k2);
                    let kx = builder.ins().fmul(k, ax);
                    let s = builder.ins().sqrt(kx);
                    let neg = builder.ins().fneg(s);
                    Ok(builder.ins().select(pos, s, neg))
                }
                "regSquare" | "regSquare2" => {
                    let k1 = if args.len() >= 3 {
                        compile_rec(&args[2], ctx, builder)?
                    } else {
                        builder.ins().f64const(1.0)
                    };
                    let k2 = if args.len() >= 4 {
                        compile_rec(&args[3], ctx, builder)?
                    } else {
                        k1
                    };
                    let ax = builder.ins().fabs(x);
                    let x2 = builder.ins().fmul(ax, ax);
                    let k = builder.ins().select(pos, k1, k2);
                    let y = builder.ins().fmul(k, x2);
                    let neg = builder.ins().fneg(y);
                    Ok(builder.ins().select(pos, y, neg))
                }
                "regPow" => {
                    // Fallback: signed |x|^1 (passthrough) when exponent not lowered.
                    Ok(x)
                }
                _ => Ok(x),
            }
        }
        // Replaceable Medium.* / Modelica.Media.* property & setState helpers (scalar path).
        "const0_warn_medium" | "medium_props" => {
            let fname = func_name.rsplit('.').next().unwrap_or(func_name);
            match fname {
                "density" | "density_phX" | "density_pTX" | "density_ph" | "density_pT"
                | "temperature" | "temperature_phX" | "pressure" | "specificEnthalpy"
                | "specificEnthalpy_pTX" | "specificHeatCapacityCp" | "specificHeatCapacityCv"
                | "dynamicViscosity" | "thermalConductivity" | "prandtlNumber" | "isentropicExponent"
                | "velocityOfSound" | "isentropicEnthalpy" | "specificEntropy"
                | "moleToMassFractions" | "massToMoleFractions" | "setState_phX" | "setState_pTX"
                | "setState_dTX" | "setState_psX" | "setSmoothState" | "saturationPressure"
                | "saturationTemperature" | "bubbleEnthalpy" | "dewEnthalpy" | "bubbleDensity"
                | "dewDensity" | "surfaceTension" | "dBubbleDensity_dPressure"
                | "dDewDensity_dPressure" | "dBubbleEnthalpy_dPressure" | "dDewEnthalpy_dPressure" => {
                    if args.is_empty() {
                        return Ok(builder.ins().f64const(0.0));
                    }
                    compile_rec(&args[0], ctx, builder)
                }
                _ => {
                    if args.is_empty() {
                        return Ok(builder.ins().f64const(0.0));
                    }
                    compile_rec(&args[0], ctx, builder)
                }
            }
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
                let u_val = compile_rec(&args[0], ctx, builder)?;
                let v_val = compile_rec(&args[1], ctx, builder)?;
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
                return compile_rec(&args[0], ctx, builder);
            }
            Ok(builder.ins().f64const(0.0))
        }
        "const0_warn_baseclasses" => {
            // Fluid/Media BaseClasses helpers: scalar passthrough / zero for empty.
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "strings_util" => {
            // Modelica.Utilities.Strings.* — validate path: empty→0/1, else passthrough/0.
            let fname = func_name.rsplit('.').next().unwrap_or(func_name);
            match fname {
                "isEmpty" => {
                    if let Some(Expression::StringLiteral(s)) = args.first() {
                        return Ok(builder.ins().f64const(if s.is_empty() { 1.0 } else { 0.0 }));
                    }
                    Ok(builder.ins().f64const(0.0))
                }
                "length" | "find" | "findLast" | "count" | "compare" | "advancedCompare"
                | "substring" | "repeat" | "replace" | "scanReal" | "scanInteger" => {
                    if let Some(Expression::StringLiteral(s)) = args.first() {
                        if fname == "length" {
                            return Ok(builder.ins().f64const(s.len() as f64));
                        }
                    }
                    // Non-literal / substring handle: 0 is a safe validate default.
                    Ok(builder.ins().f64const(0.0))
                }
                _ => {
                    if args.is_empty() {
                        Ok(builder.ins().f64const(0.0))
                    } else {
                        compile_rec(&args[0], ctx, builder)
                    }
                }
            }
        }
        // Legacy alias kept for overlay policies that still name const0_warn_frames.
        "const0_warn_frames" | "frames_orient" => {
            // Scalar MultiBody orientation helpers: identity / compose passthrough.
            // nullRotation() -> 0; planarRotation(..., angle, ...) -> angle;
            // absoluteRotation(R1, R2) / resolve* -> first tensor/vector arg.
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            let fname = func_name.rsplit('.').next().unwrap_or(func_name);
            match fname {
                "nullRotation" | "from_nxy" | "from_nxz" | "from_T" | "from_Q" | "axesRotations"
                | "axesRotationsAngles" => Ok(builder.ins().f64const(0.0)),
                "planarRotation" => {
                    // planarRotation(n, angle, der_angle) — angle is typically args[1]
                    if args.len() >= 2 {
                        compile_rec(&args[1], ctx, builder)
                    } else {
                        compile_rec(&args[0], ctx, builder)
                    }
                }
                "absoluteRotation" | "relativeRotation" | "inverseRotation" | "resolve1"
                | "resolve2" | "resolveRelative" | "resolveDyade1" | "resolveDyade2"
                | "angularVelocity1" | "angularVelocity2" | "orientationConstraint" => {
                    compile_rec(&args[0], ctx, builder)
                }
                _ => {
                    if args.len() >= 2 {
                        compile_rec(&args[1], ctx, builder)
                    } else {
                        compile_rec(&args[0], ctx, builder)
                    }
                }
            }
        }
        "clock_ctor" => {
            // Clock() / Clock(interval) / Clock(interval, resolution): expose interval for sample.
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(args.last().unwrap(), ctx, builder)
        }
        "noevent_1" => {
            if args.len() != 1 {
                return Err(format!("noEvent() expects 1 argument, got {}", args.len()));
            }
            let prev = ctx.suppress_zero_crossings;
            ctx.suppress_zero_crossings = true;
            let out = compile_rec(&args[0], ctx, builder);
            ctx.suppress_zero_crossings = prev;
            out
        }
        "instream" => {
            if args.len() != 1 {
                return Err(format!(
                    "inStream() expects exactly 1 argument, got {}",
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
                        let mut equal_sum = builder.ins().f64const(0.0);
                        let mut peer_count = 0u32;
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
                            equal_sum = builder.ins().fadd(equal_sum, h_peer);
                            peer_count += 1;
                        }
                        let eps = builder.ins().f64const(1e-12);
                        let has_mix = builder.ins().fcmp(FloatCC::GreaterThan, denominator, eps);
                        let mixed = builder.ins().fdiv(numerator, denominator);
                        let zero_flow_fallback = if peer_count > 0 {
                            let n_f = builder.ins().f64const(peer_count as f64);
                            builder.ins().fdiv(equal_sum, n_f)
                        } else {
                            self_v
                        };
                        return Ok(builder.ins().select(has_mix, mixed, zero_flow_fallback));
                    }
                }
                warn_stream_semantics_once("peerMissing");
            }
            compile_rec(&args[0], ctx, builder)
        }
        "actualstream" => {
            if args.len() != 1 {
                return Err(format!(
                    "actualStream() expects exactly 1 argument, got {}",
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
                                let mut equal_sum = builder.ins().f64const(0.0);
                                let mut peer_count = 0u32;
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
                                    equal_sum = builder.ins().fadd(equal_sum, h_peer);
                                    peer_count += 1;
                                }
                                let eps = builder.ins().f64const(1e-12);
                                let has_mix = builder.ins().fcmp(FloatCC::GreaterThan, denominator, eps);
                                let mixed = builder.ins().fdiv(numerator, denominator);
                                let zero_flow_fallback = if peer_count > 0 {
                                    let n_f = builder.ins().f64const(peer_count as f64);
                                    builder.ins().fdiv(equal_sum, n_f)
                                } else {
                                    self_v
                                };
                                builder.ins().select(has_mix, mixed, zero_flow_fallback)
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
            // valveCharacteristic / flowCharacteristic: relative opening → first arg.
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
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
                for (k, &d) in ctx.connector_connection_degree.iter() {
                    if path == *k
                        || path.starts_with(&format!("{}_", k))
                        || path.starts_with(&format!("{}.", k))
                    {
                        return Ok(builder.ins().f64const(d as f64));
                    }
                }
            }
            if jit_strict_placeholders_enabled() {
                jit_builtin_fallback_warn_once(func_name, "cardinality-placeholder-strict0");
                return Ok(builder.ins().f64const(0.0));
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
            if jit_strict_placeholders_enabled() {
                return Err(format!(
                    "JIT strict placeholders: firstTick() without time in var_map ({})",
                    func_name
                ));
            }
            jit_builtin_fallback_warn_once(func_name, "first-tick-without-time");
            Ok(builder.ins().f64const(0.0))
        }
        "first_true_index" => compile_first_true_index(args, ctx, builder, compile_rec),
        "interpolate" => compile_interpolate_vectors(args, ctx, builder, compile_rec),
        "get_next_time_event" => {
            // No pending time event in validate/scalar path (Modelica.Constants.inf-scale).
            // Some MSL wrappers pass unused context args; ignore them.
            Ok(builder.ins().f64const(1.0e60))
        }
        "complex_ctor" => {
            // Complex(re, im): scalar path exposes real part.
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "complex_real" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "thermal_convert" => {
            // convertResistance(RRef, TRef, alpha, T) ~= RRef*(1+alpha*(T-TRef))
            // convertAlpha(alpha1, T2, T1=293.15) ~= alpha1/(1+alpha1*(T2-T1))
            // linearTemperatureDependency(RRef, TRef, alpha20, T)
            //   ~= RRef*(1 + convertAlpha(alpha20, TRef)*(T-TRef))
            let fname = func_name.rsplit('.').next().unwrap_or(func_name);
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            if fname == "convertResistance" && args.len() >= 4 {
                let r_ref = compile_rec(&args[0], ctx, builder)?;
                let t_ref = compile_rec(&args[1], ctx, builder)?;
                let alpha = compile_rec(&args[2], ctx, builder)?;
                let t = compile_rec(&args[3], ctx, builder)?;
                let dt = builder.ins().fsub(t, t_ref);
                let term = builder.ins().fmul(alpha, dt);
                let one = builder.ins().f64const(1.0);
                let scale = builder.ins().fadd(one, term);
                return Ok(builder.ins().fmul(r_ref, scale));
            }
            if fname == "convertAlpha" && args.len() >= 2 {
                let alpha = compile_rec(&args[0], ctx, builder)?;
                let t2 = compile_rec(&args[1], ctx, builder)?;
                let t1 = if args.len() >= 3 {
                    compile_rec(&args[2], ctx, builder)?
                } else {
                    builder.ins().f64const(293.15)
                };
                let dt = builder.ins().fsub(t2, t1);
                let term = builder.ins().fmul(alpha, dt);
                let one = builder.ins().f64const(1.0);
                let denom = builder.ins().fadd(one, term);
                return Ok(builder.ins().fdiv(alpha, denom));
            }
            if fname == "linearTemperatureDependency" && args.len() >= 4 {
                let r_ref = compile_rec(&args[0], ctx, builder)?;
                let t_ref = compile_rec(&args[1], ctx, builder)?;
                let alpha20 = compile_rec(&args[2], ctx, builder)?;
                let t = compile_rec(&args[3], ctx, builder)?;
                let t20 = builder.ins().f64const(293.15);
                let dt_ref = builder.ins().fsub(t_ref, t20);
                let term_a = builder.ins().fmul(alpha20, dt_ref);
                let one = builder.ins().f64const(1.0);
                let denom_a = builder.ins().fadd(one, term_a);
                let alpha_ref = builder.ins().fdiv(alpha20, denom_a);
                let dt = builder.ins().fsub(t, t_ref);
                let term = builder.ins().fmul(alpha_ref, dt);
                let scale = builder.ins().fadd(one, term);
                return Ok(builder.ins().fmul(r_ref, scale));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "is_equal_fn" => {
            // Math.isEqual(s1, s2, eps=0) -> abs(s1-s2) <= eps (Boolean as 0/1)
            if args.len() < 2 {
                return Err(format!(
                    "isEqual() expects at least 2 arguments, got {}",
                    args.len()
                ));
            }
            let a = compile_rec(&args[0], ctx, builder)?;
            let b = compile_rec(&args[1], ctx, builder)?;
            let eps = if args.len() >= 3 {
                compile_rec(&args[2], ctx, builder)?
            } else {
                builder.ins().f64const(0.0)
            };
            let diff = builder.ins().fsub(a, b);
            let ad = builder.ins().fabs(diff);
            let ok = builder.ins().fcmp(FloatCC::LessThanOrEqual, ad, eps);
            let one = builder.ins().f64const(1.0);
            let zero = builder.ins().f64const(0.0);
            Ok(builder.ins().select(ok, one, zero))
        }
        "move_position" => {
            // Rotational/Translational.Sources.Move.position(q_qd_qdd, dummy) -> q_qd_qdd[1]
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "stribeck_table_f0" => {
            // GenerateStribeckFrictionTable(...) returns a table; scalar path exposes f(v=0)
            // = F_Coulomb + F_Stribeck (args[1], args[2]).
            if args.len() < 3 {
                if args.is_empty() {
                    return Ok(builder.ins().f64const(0.0));
                }
                return compile_rec(&args[0], ctx, builder);
            }
            let f_coulomb = compile_rec(&args[1], ctx, builder)?;
            let f_stribeck = compile_rec(&args[2], ctx, builder)?;
            Ok(builder.ins().fadd(f_coulomb, f_stribeck))
        }
        "atan2_fn" => {
            if args.len() < 2 {
                return Err(format!("atan2() expects 2 arguments, got {}", args.len()));
            }
            let y = compile_rec(&args[0], ctx, builder)?;
            let x = compile_rec(&args[1], ctx, builder)?;
            let mut sig = ctx.module.make_signature();
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.returns.push(AbiParam::new(cl_types::F64));
            let func_id = ctx
                .module
                .declare_function("atan2", Linkage::Import, &sig)
                .map_err(|e| e.to_string())?;
            let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
            let call_inst = builder.ins().call(func_ref, &[y, x]);
            Ok(builder.inst_results(call_inst)[0])
        }
        "conj_fn" => {
            // Complex conjugate: scalar path keeps real component.
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "from_polar" => {
            // fromPolar(mag, angle): scalar validate uses magnitude (first arg).
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "complex_math" => {
            // ComplexMath.* (exp/abs/arg/...): scalar validate passthrough.
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "transpose_fn" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "wrap_angle" => {
            // wrapAngle(phi): map to (-pi, pi]; scalar validate uses passthrough.
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "unit_from_rpm" => {
            // rpm -> rad/s: * (2*pi/60)
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            let x = compile_rec(&args[0], ctx, builder)?;
            let k = builder.ins().f64const(std::f64::consts::PI / 30.0);
            Ok(builder.ins().fmul(x, k))
        }
        "unit_to_rpm" => {
            // rad/s -> rpm: * (60/(2*pi))
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            let x = compile_rec(&args[0], ctx, builder)?;
            let k = builder.ins().f64const(30.0 / std::f64::consts::PI);
            Ok(builder.ins().fmul(x, k))
        }
        "vector_fn" => {
            // vector(e) / vector(e,n): scalar element for equation lowering.
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "exlin_fn" => {
            // Soft exponential linearization: passthrough first arg for validate path.
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "space_phasor" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "flux_everett" => {
            // Preisach/Everett density helper: scalar passthrough / zero.
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "max_without_event" => {
            // Frames.Internal.maxWithoutEvent(u,y) / Fluid positiveMax(x[,eps]) / noEvent-style max.
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            if args.len() == 1 {
                // Fluid.Fittings.MultiPort.positiveMax(x) := max(x, 1e-10)
                let x = compile_rec(&args[0], ctx, builder)?;
                let eps = builder.ins().f64const(1e-10);
                return Ok(builder.ins().fmax(x, eps));
            }
            let a = compile_rec(&args[0], ctx, builder)?;
            let b = compile_rec(&args[1], ctx, builder)?;
            Ok(builder.ins().fmax(a, b))
        }
        "fluid_dissipation" => {
            // Modelica.Fluid.Dissipation.* pressure-loss helpers: scalar passthrough.
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "vectors_length" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            if let Expression::Variable(id) = &args[0] {
                let arr_name = crate::string_intern::resolve_id(*id);
                if let Some((aty, a0)) = ctx.array_storage(&arr_name) {
                    let n = ctx.array_len(&arr_name).unwrap_or(0);
                    if n > 0 {
                        let mut sumsq = builder.ins().f64const(0.0);
                        for i in 0..n {
                            let base = match aty {
                                crate::jit::types::ArrayType::State => ctx.states_ptr,
                                crate::jit::types::ArrayType::Discrete => ctx.discrete_ptr,
                                crate::jit::types::ArrayType::Parameter => ctx.params_ptr,
                                crate::jit::types::ArrayType::Output => ctx.outputs_ptr,
                                crate::jit::types::ArrayType::Derivative => ctx.derivs_ptr,
                            };
                            let off = ((a0 + i) * 8) as i32;
                            let e = builder.ins().load(cl_types::F64, MemFlags::new(), base, off);
                            let e2 = builder.ins().fmul(e, e);
                            sumsq = builder.ins().fadd(sumsq, e2);
                        }
                        return Ok(builder.ins().sqrt(sumsq));
                    }
                }
            }
            let v = compile_rec(&args[0], ctx, builder)?;
            Ok(builder.ins().fabs(v))
        }
        "bool_vec_any" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            if let Expression::Variable(id) = &args[0] {
                let arr_name = crate::string_intern::resolve_id(*id);
                if let Some((aty, a0)) = ctx.array_storage(&arr_name) {
                    let n = ctx.array_len(&arr_name).unwrap_or(0);
                    let zero = builder.ins().f64const(0.0);
                    let one = builder.ins().f64const(1.0);
                    let mut any = builder.ins().f64const(0.0);
                    for i in 0..n {
                        let base = match aty {
                            crate::jit::types::ArrayType::State => ctx.states_ptr,
                            crate::jit::types::ArrayType::Discrete => ctx.discrete_ptr,
                            crate::jit::types::ArrayType::Parameter => ctx.params_ptr,
                            crate::jit::types::ArrayType::Output => ctx.outputs_ptr,
                            crate::jit::types::ArrayType::Derivative => ctx.derivs_ptr,
                        };
                        let off = ((a0 + i) * 8) as i32;
                        let e = builder.ins().load(cl_types::F64, MemFlags::new(), base, off);
                        let nz = builder.ins().fcmp(FloatCC::NotEqual, e, zero);
                        let e1 = builder.ins().select(nz, one, zero);
                        any = builder.ins().fmax(any, e1);
                    }
                    return Ok(any);
                }
            }
            let v = compile_rec(&args[0], ctx, builder)?;
            let zero = builder.ins().f64const(0.0);
            let one = builder.ins().f64const(1.0);
            let nz = builder.ins().fcmp(FloatCC::NotEqual, v, zero);
            Ok(builder.ins().select(nz, one, zero))
        }
        "bool_vec_all" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(1.0));
            }
            if let Expression::Variable(id) = &args[0] {
                let arr_name = crate::string_intern::resolve_id(*id);
                if let Some((aty, a0)) = ctx.array_storage(&arr_name) {
                    let n = ctx.array_len(&arr_name).unwrap_or(0);
                    let zero = builder.ins().f64const(0.0);
                    let one = builder.ins().f64const(1.0);
                    let mut all = builder.ins().f64const(1.0);
                    for i in 0..n {
                        let base = match aty {
                            crate::jit::types::ArrayType::State => ctx.states_ptr,
                            crate::jit::types::ArrayType::Discrete => ctx.discrete_ptr,
                            crate::jit::types::ArrayType::Parameter => ctx.params_ptr,
                            crate::jit::types::ArrayType::Output => ctx.outputs_ptr,
                            crate::jit::types::ArrayType::Derivative => ctx.derivs_ptr,
                        };
                        let off = ((a0 + i) * 8) as i32;
                        let e = builder.ins().load(cl_types::F64, MemFlags::new(), base, off);
                        let nz = builder.ins().fcmp(FloatCC::NotEqual, e, zero);
                        let e1 = builder.ins().select(nz, one, zero);
                        all = builder.ins().fmul(all, e1);
                    }
                    return Ok(all);
                }
            }
            compile_rec(&args[0], ctx, builder)
        }
        "bool_vec_one" => {
            // Count==1 approximation: anyTrue for scalar validate path.
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        "wall_friction" => {
            // WallFriction.massFlowRate_dp / pressureLoss_m_flow: scalar passthrough of dp/m_flow.
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
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
            strict_placeholder_zero(func_name, "external-object-validate-placeholder", builder)
        }
        // Real table runtime path (rustmodlica_table_get_value). Do not count as
        // jit-builtin fallback; under STRICT_PLACEHOLDERS require at least a handle arg.
        "ext_combitimetable_warn0" | "ext_combitable_lookup" => {
            if args.is_empty() {
                if jit_strict_placeholders_enabled() {
                    return Err(format!(
                        "JIT strict placeholders: {} requires a table handle argument",
                        func_name
                    ));
                }
                jit_builtin_fallback_warn_once(func_name, "external-combitable-empty-args");
                return Ok(builder.ins().f64const(0.0));
            }
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
            if let Some(Expression::Variable(id)) = args.first() {
                let var_name = crate::string_intern::resolve_id(*id);
                jit_builtin_fallback_warn_once(
                    func_name,
                    "loadresource-non-literal-uri",
                );
                if let Some(&val) = ctx.var_map.get(&var_name) {
                    return Ok(val);
                }
            }
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
            if let Expression::Variable(id) = &args[0] {
                let arr_name = crate::string_intern::resolve_id(*id);
                return super::call::compile_array_reduce(&arr_name, 1.0, true, ctx, builder);
            }
            compile_rec(&args[0], ctx, builder)
        }
        "sum_fn" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            if let Expression::Variable(id) = &args[0] {
                let arr_name = crate::string_intern::resolve_id(*id);
                return super::call::compile_array_reduce(&arr_name, 0.0, false, ctx, builder);
            }
            compile_rec(&args[0], ctx, builder)
        }
        "zeros_fn" => {
            // Modelica zeros(n,...) — scalar equation lowering uses the zero element.
            Ok(builder.ins().f64const(0.0))
        }
        "ones_fn" => Ok(builder.ins().f64const(1.0)),
        "cross_fn" => {
            // cross(a,b): if both are length>=3 arrays, emit first component a1*b2-a2*b1;
            // otherwise scalar-path zero (vector result not represented as one F64).
            if args.len() >= 2 {
                if let (Expression::Variable(id_a), Expression::Variable(id_b)) =
                    (&args[0], &args[1])
                {
                    let a_name = crate::string_intern::resolve_id(*id_a);
                    let b_name = crate::string_intern::resolve_id(*id_b);
                    if let (Some((aty, a0)), Some((bty, b0))) =
                        (ctx.array_storage(&a_name), ctx.array_storage(&b_name))
                    {
                        let an = ctx.array_len(&a_name).unwrap_or(0);
                        let bn = ctx.array_len(&b_name).unwrap_or(0);
                        if an >= 3 && bn >= 3 {
                            let load_at = |ty: crate::jit::types::ArrayType,
                                           start: usize,
                                           i: usize,
                                           builder: &mut cranelift::frontend::FunctionBuilder<'_>|
                             -> Value {
                                let base = match ty {
                                    crate::jit::types::ArrayType::State => ctx.states_ptr,
                                    crate::jit::types::ArrayType::Discrete => ctx.discrete_ptr,
                                    crate::jit::types::ArrayType::Parameter => ctx.params_ptr,
                                    crate::jit::types::ArrayType::Output => ctx.outputs_ptr,
                                    crate::jit::types::ArrayType::Derivative => ctx.derivs_ptr,
                                };
                                let off = ((start + i) * 8) as i32;
                                builder.ins().load(cl_types::F64, MemFlags::new(), base, off)
                            };
                            let a1 = load_at(aty, a0, 1, builder);
                            let a2 = load_at(aty, a0, 2, builder);
                            let b1 = load_at(bty, b0, 1, builder);
                            let b2 = load_at(bty, b0, 2, builder);
                            let t0 = builder.ins().fmul(a1, b2);
                            let t1 = builder.ins().fmul(a2, b1);
                            return Ok(builder.ins().fsub(t0, t1));
                        }
                    }
                }
                // Two scalar args: 2D cross z-component a*b with swapped product.
                let a = compile_rec(&args[0], ctx, builder)?;
                let b = compile_rec(&args[1], ctx, builder)?;
                return Ok(builder.ins().fmul(a, b));
            }
            Ok(builder.ins().f64const(0.0))
        }
        "unit_from_deg" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            let v = compile_rec(&args[0], ctx, builder)?;
            let k = builder.ins().f64const(std::f64::consts::PI / 180.0);
            Ok(builder.ins().fmul(v, k))
        }
        "unit_to_deg" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            let v = compile_rec(&args[0], ctx, builder)?;
            let k = builder.ins().f64const(180.0 / std::f64::consts::PI);
            Ok(builder.ins().fmul(v, k))
        }
        "unit_from_degC" => {
            // Celsius -> Kelvin
            if args.is_empty() {
                return Ok(builder.ins().f64const(273.15));
            }
            let v = compile_rec(&args[0], ctx, builder)?;
            let off = builder.ins().f64const(273.15);
            Ok(builder.ins().fadd(v, off))
        }
        "unit_to_degC" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(-273.15));
            }
            let v = compile_rec(&args[0], ctx, builder)?;
            let off = builder.ins().f64const(273.15);
            Ok(builder.ins().fsub(v, off))
        }
        "unit_from_rad" | "unit_to_rad" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            compile_rec(&args[0], ctx, builder)
        }
        _ => Err(format!("unknown JIT builtin handler_id: {}", handler_id)),
    }
}
