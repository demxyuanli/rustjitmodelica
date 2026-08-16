use crate::ast::Expression;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::Module;

use crate::jit::context::TranslationContext;
use crate::jit::types::ArrayType;
use super::super::helpers::{
    abi_params_short, import_call_abi_tag, jit_builtin_fallback_warn_once, jit_import_debug_enabled,
    jit_import_strict_enabled, lookup_or_insert_import,
};
use super::name_matches;

pub(super) fn compile_call_tail(
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
    let json_builtin_rule = crate::jit::jit_policy::match_function_builtin_rule(func_name);
    if name_matches(func_name, "position") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "move_position",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if func_name.contains("GenerateStribeckFrictionTable") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "stribeck_table_f0",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "atan2") || func_name.contains("Functions.atan2") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "atan2_fn",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "state2")
        || name_matches(func_name, "state1")
        || func_name.contains("Functions.state")
    {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "passthrough_first_empty0",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "conj") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "conj_fn",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "fromPolar") || func_name.contains("ComplexMath.fromPolar") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "from_polar",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if func_name.contains("ComplexMath.") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "complex_math",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "transpose") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "transpose_fn",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "wrapAngle") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "wrap_angle",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "from_rpm") || func_name.contains("Conversions.from_rpm") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "unit_from_rpm",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "to_rpm") || func_name.contains("Conversions.to_rpm") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "unit_to_rpm",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if func_name.contains("brushVoltageDrop")
        || func_name.contains("quasiRMS")
        || func_name.contains("factorY2DC")
        || func_name.contains("ComplexImpedance")
        || func_name.contains("FCNiout_limit")
    {
        let hid = if func_name.contains("ComplexImpedance") {
            "complex_ctor"
        } else if func_name.contains("FCNiout_limit") {
            "passthrough_first_empty0"
        } else {
            "space_phasor"
        };
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            hid,
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "vector") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "vector_fn",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "exlin") || name_matches(func_name, "exlin2") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "exlin_fn",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if func_name.contains("ToSpacePhasor")
        || func_name.contains("FromSpacePhasor")
        || func_name.contains("symmetricOrientation")
    {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "space_phasor",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if func_name.contains("everett") || func_name.contains("FluxTubes.Utilities") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "flux_everett",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    // zeros(n,...) / ones(n,...): scalar element for equation lowering.
    if name_matches(func_name, "zeros") {
        return Ok(builder.ins().f64const(0.0));
    }
    if name_matches(func_name, "ones") {
        return Ok(builder.ins().f64const(1.0));
    }
    if name_matches(func_name, "cross") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "cross_fn",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if func_name.contains("Vectors.length")
        || (func_name == "length" && !func_name.contains("Strings"))
    {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "vectors_length",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "anyTrue")
        || name_matches(func_name, "allTrue")
        || name_matches(func_name, "oneTrue")
        || func_name.contains("BooleanVectors.")
    {
        let hid = if name_matches(func_name, "allTrue") {
            "bool_vec_all"
        } else if name_matches(func_name, "oneTrue") {
            "bool_vec_one"
        } else {
            "bool_vec_any"
        };
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            hid,
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if func_name.contains("maxWithoutEvent") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "max_without_event",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if func_name.contains("WallFriction.")
        || func_name.contains("massFlowRate_dp")
        || func_name.contains("pressureLoss_m_flow")
    {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "wall_friction",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if func_name.contains("getNextTimeEvent") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "get_next_time_event",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if func_name.contains("Modelica.Utilities.Strings")
        || func_name.starts_with("Strings.")
        || name_matches(func_name, "findLast")
        || name_matches(func_name, "find")
        || name_matches(func_name, "substring")
    {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "strings_util",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if func_name.contains("getTable1DValue")
        || func_name.contains("getTable2DValue")
        || func_name.contains("getTimeTableValue")
    {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "combitable_err0",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "from_degC") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "unit_from_degC",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "to_degC") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "unit_to_degC",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "from_deg") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "unit_from_deg",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "to_deg") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "unit_to_deg",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "from_rad") || name_matches(func_name, "to_rad") {
        if args.is_empty() {
            return Ok(builder.ins().f64const(0.0));
        }
        return compile_rec(&args[0], ctx, builder);
    }
    // cat(dim, a, b, ...): scalar path uses the first array argument (args[1]).
    if name_matches(func_name, "cat") {
        if args.is_empty() {
            return Ok(builder.ins().f64const(0.0));
        }
        if args.len() >= 2 {
            return compile_rec(&args[1], ctx, builder);
        }
        return compile_rec(&args[0], ctx, builder);
    }
    // Clock constructor: expose interval (last arg) for sample-based schedules.
    if name_matches(func_name, "Clock") {
        if args.is_empty() {
            return Ok(builder.ins().f64const(0.0));
        }
        return compile_rec(args.last().unwrap(), ctx, builder);
    }
    // MultiBody Frames.* (short or fully qualified): scalar orientation lowering.
    if func_name.starts_with("Frames.")
        || func_name.contains(".Frames.")
        || func_name.contains("MultiBody.Frames")
    {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "frames_orient",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "valveCharacteristic")
        || name_matches(func_name, "flowCharacteristic")
        || name_matches(func_name, "efficiencyCharacteristic")
        || name_matches(func_name, "powerCharacteristic")
        || func_name.ends_with("Characteristic")
    {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "valve_char_1",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "regRoot")
        || name_matches(func_name, "regRoot2")
        || name_matches(func_name, "regSquare")
        || name_matches(func_name, "regSquare2")
        || name_matches(func_name, "regPow")
        || func_name.starts_with("Utilities.reg")
        || func_name.contains("Fluid.Utilities.reg")
    {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "fluid_reg",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    // Replaceable Fluid Medium.* property calls (short or Modelica.Media.*).
    if func_name.starts_with("Medium.")
        || func_name.contains("Modelica.Media")
        || func_name.contains(".Media.")
    {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "medium_props",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if func_name.contains("gravityAcceleration") || func_name.contains("GravityAcceleration") {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "gravity_accel",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    if name_matches(func_name, "normalize")
        || func_name.contains("normalizeWithAssert")
    {
        return super::super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            "vectors_normalize",
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        );
    }
    // Periodic sample / interval (native rustmodlica_sample).
    if name_matches(func_name, "sample") || name_matches(func_name, "interval") {
        return super::super::builtin_clock_sample::compile_periodic_sample_call(
            args, ctx, builder, compile_rec,
        );
    }
    if name_matches(func_name, "subSample")
        || name_matches(func_name, "superSample")
        || name_matches(func_name, "shiftSample")
        || name_matches(func_name, "backSample")
    {
        let op = if name_matches(func_name, "subSample") {
            "subSample"
        } else if name_matches(func_name, "superSample") {
            "superSample"
        } else if name_matches(func_name, "shiftSample") {
            "shiftSample"
        } else {
            "backSample"
        };
        return super::super::builtin_clock_sample::compile_clock_derived_call(
            op, args, ctx, builder, compile_rec,
        );
    }

    let is_external_modelica = ctx
        .external_modelica_names
        .map(|s| s.contains(func_name))
        .unwrap_or(false);
    if !is_external_modelica
        && !crate::jit::native::builtin_jit_symbol_names()
            .iter()
            .any(|&n| n == func_name)
    {
        // J14: strict function-builtin fails only on the unknown-import passthrough path,
        // after hand-coded handlers (fill/not/assert/...) and try_compile_builtin_call.
        if json_builtin_rule.is_none() && crate::jit::jit_policy::strict_function_builtin() {
            return Err(format!(
                "JIT strict function-builtin: '{}' not found in builtin rules",
                func_name
            ));
        }
        if jit_import_strict_enabled() {
            return Err(format!(
                "JIT import strict: '{}' is not a linked builtin or external symbol",
                func_name
            ));
        }
        jit_builtin_fallback_warn_once(func_name, "unknown-import");
        if args.is_empty() {
            return Ok(builder.ins().f64const(0.0));
        }
        return compile_rec(&args[0], ctx, builder);
    }

    let ptr_type = ctx.module.target_config().pointer_type();
    let mut sig = ctx.module.make_signature();
    let mut arg_vals = Vec::new();
    for arg in args {
        if let Expression::Variable(id) = arg {
            let name = crate::string_intern::resolve_id(*id);
            if let Some(info) = ctx.array_info.get(&name) {
                let base_ptr = match info.array_type {
                    ArrayType::State => ctx.states_ptr,
                    ArrayType::Discrete => ctx.discrete_ptr,
                    ArrayType::Parameter => ctx.params_ptr,
                    ArrayType::Output => ctx.outputs_ptr,
                    ArrayType::Derivative => ctx.derivs_ptr,
                };
                let start_offset = (info.start_index * 8) as i64;
                let start_const = builder.ins().iconst(ptr_type, start_offset);
                let array_ptr = builder.ins().iadd(base_ptr, start_const);

                let size_val = builder.ins().f64const(info.size as f64);

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
                            "Array literal in external call requires JIT data context (EXT-3)."
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
                let item_val = compile_rec(it, ctx, builder)?;
                let off_i64 = (i as i64) * 8;
                let off = i32::try_from(off_i64).map_err(|_| {
                    format!(
                        "JIT external call '{}': array literal too large (EXT-3).",
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
                    return Err("String argument in function call requires string data context (FUNC-7). Ensure JIT compilation is configured with string literal support.".to_string());
                }
            };
            sig.params.push(AbiParam::new(ptr_type));
            let gv = ctx.module.declare_data_in_func(data_id, &mut builder.func);
            arg_vals.push(builder.ins().global_value(ptr_type, gv));
            continue;
        }
        let val = compile_rec(arg, ctx, builder)?;
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
