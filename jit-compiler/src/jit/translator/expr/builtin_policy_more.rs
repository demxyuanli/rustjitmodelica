//! Remaining named-builtin policy match arms (split from dispatch).

use crate::ast::Expression;
use crate::jit::context::TranslationContext;
use super::super::builtin_clock_sample::compile_clock_derived_call;
use crate::jit::translator::expr::helpers::{
    jit_builtin_fallback_warn_once, jit_strict_placeholders_enabled,
};
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};

use super::strict_placeholder_zero;

fn connector_member_bool_elems(
    ctx: &TranslationContext<'_>,
    base: &str,
) -> Option<Vec<String>> {
    let (prefix, member) = base.rsplit_once('_')?;
    let mut out = Vec::new();
    for i in 1..=64 {
        let name = format!("{}_{}_{}", prefix, i, member);
        if ctx.output_index(&name).is_some() || ctx.discrete_index(&name).is_some() {
            out.push(name);
        } else {
            break;
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
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

pub(super) fn dispatch_policy_more(
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
                // Connector arrays flatten as `inPort_1_set`, not `inPort_set_1`.
                if let Some(elems) = connector_member_bool_elems(ctx, &arr_name) {
                    let zero = builder.ins().f64const(0.0);
                    let one = builder.ins().f64const(1.0);
                    let mut any = builder.ins().f64const(0.0);
                    for name in elems {
                        let e = compile_rec(&crate::ast::Expression::var(&name), ctx, builder)?;
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
                return super::super::call::compile_array_reduce(&arr_name, 1.0, true, ctx, builder);
            }
            compile_rec(&args[0], ctx, builder)
        }
        "sum_fn" => {
            if args.is_empty() {
                return Ok(builder.ins().f64const(0.0));
            }
            if let Expression::Variable(id) = &args[0] {
                let arr_name = crate::string_intern::resolve_id(*id);
                return super::super::call::compile_array_reduce(&arr_name, 0.0, false, ctx, builder);
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
