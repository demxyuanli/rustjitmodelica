//! Periodic `sample` / `interval` and clock-derived operators (shared by policy dispatch and expr lowering).

use crate::ast::Expression;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};

use crate::jit::context::TranslationContext;

pub(super) fn compile_periodic_sample_call(
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
    if args.len() != 1 && args.len() != 2 {
        return Err(format!(
            "sample/interval expect 0, 1 or 2 arguments, got {}",
            args.len()
        ));
    }
    let time_val = ctx
        .var_map
        .get("time")
        .copied()
        .ok_or_else(|| "sample/interval requires time in context".to_string())?;
    let mut sig = ctx.module.make_signature();
    sig.params.push(AbiParam::new(cl_types::F64));
    sig.params.push(AbiParam::new(cl_types::F64));
    sig.returns.push(AbiParam::new(cl_types::F64));
    let func_id = ctx
        .module
        .declare_function("rustmodlica_sample", Linkage::Import, &sig)
        .map_err(|e| e.to_string())?;
    let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
    let (t_arg, interval_arg) = if args.len() == 1 {
        (time_val, compile_rec(&args[0], ctx, builder)?)
    } else {
        let start_val = compile_rec(&args[0], ctx, builder)?;
        let interval_val = compile_rec(&args[1], ctx, builder)?;
        let t_rel = builder.ins().fsub(time_val, start_val);
        (t_rel, interval_val)
    };
    let call_inst = builder.ins().call(func_ref, &[t_arg, interval_arg]);
    Ok(builder.inst_results(call_inst)[0])
}

pub(super) fn compile_clock_derived_call(
    op_name: &str,
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
    if args.len() < 2 {
        return compile_rec(&args[0], ctx, builder);
    }
    let zero = builder.ins().f64const(0.0);
    let one = builder.ins().f64const(1.0);
    let sample_pair = match &args[0] {
        Expression::Sample(interval_expr) => Some((None, interval_expr.as_ref())),
        Expression::Call(name, cargs)
            if (name.eq_ignore_ascii_case("sample") || name.ends_with(".sample"))
                && (cargs.len() == 1 || cargs.len() == 2) =>
        {
            if cargs.len() == 2 {
                Some((Some(&cargs[0]), &cargs[1]))
            } else {
                Some((None, &cargs[0]))
            }
        }
        _ => None,
    };

    match (sample_pair, op_name) {
        (Some((start_expr, interval_expr)), "subSample") => {
            let interval_val = compile_rec(interval_expr, ctx, builder)?;
            let n_val = compile_rec(&args[1], ctx, builder)?;
            let n_pos = builder.ins().fcmp(FloatCC::GreaterThan, n_val, zero);
            let n_safe = builder.ins().select(n_pos, n_val, one);
            let scaled_interval = builder.ins().fmul(interval_val, n_safe);
            let time_val = ctx
                .var_map
                .get("time")
                .copied()
                .ok_or_else(|| "subSample(sample(...), n) requires time in context".to_string())?;
            let t_arg = if let Some(se) = start_expr {
                let start_val = compile_rec(se, ctx, builder)?;
                builder.ins().fsub(time_val, start_val)
            } else {
                time_val
            };
            let mut sig = ctx.module.make_signature();
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.returns.push(AbiParam::new(cl_types::F64));
            let func_id = ctx
                .module
                .declare_function("rustmodlica_sample", Linkage::Import, &sig)
                .map_err(|e| e.to_string())?;
            let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
            let call_inst = builder.ins().call(func_ref, &[t_arg, scaled_interval]);
            Ok(builder.inst_results(call_inst)[0])
        }
        (Some((start_expr, interval_expr)), "superSample") => {
            let interval_val = compile_rec(interval_expr, ctx, builder)?;
            let n_val = compile_rec(&args[1], ctx, builder)?;
            let n_pos = builder.ins().fcmp(FloatCC::GreaterThan, n_val, zero);
            let n_safe = builder.ins().select(n_pos, n_val, one);
            let scaled_interval = builder.ins().fdiv(interval_val, n_safe);
            let time_val = ctx
                .var_map
                .get("time")
                .copied()
                .ok_or_else(|| "superSample(sample(...), n) requires time in context".to_string())?;
            let t_arg = if let Some(se) = start_expr {
                let start_val = compile_rec(se, ctx, builder)?;
                builder.ins().fsub(time_val, start_val)
            } else {
                time_val
            };
            let mut sig = ctx.module.make_signature();
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.returns.push(AbiParam::new(cl_types::F64));
            let func_id = ctx
                .module
                .declare_function("rustmodlica_sample", Linkage::Import, &sig)
                .map_err(|e| e.to_string())?;
            let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
            let call_inst = builder.ins().call(func_ref, &[t_arg, scaled_interval]);
            Ok(builder.inst_results(call_inst)[0])
        }
        (Some((start_expr, interval_expr)), "shiftSample") => {
            let interval_val = compile_rec(interval_expr, ctx, builder)?;
            let n_val = compile_rec(&args[1], ctx, builder)?;
            let shift = builder.ins().fmul(interval_val, n_val);
            let time_val = ctx
                .var_map
                .get("time")
                .copied()
                .ok_or_else(|| "shiftSample(sample(...), n) requires time in context".to_string())?;
            let t_arg = if let Some(se) = start_expr {
                let start_val = compile_rec(se, ctx, builder)?;
                builder.ins().fsub(time_val, start_val)
            } else {
                time_val
            };
            let shifted_t = builder.ins().fsub(t_arg, shift);
            let mut sig = ctx.module.make_signature();
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.returns.push(AbiParam::new(cl_types::F64));
            let func_id = ctx
                .module
                .declare_function("rustmodlica_sample", Linkage::Import, &sig)
                .map_err(|e| e.to_string())?;
            let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
            let call_inst = builder.ins().call(func_ref, &[shifted_t, interval_val]);
            Ok(builder.inst_results(call_inst)[0])
        }
        (Some((start_expr, interval_expr)), "backSample") => {
            let interval_val = compile_rec(interval_expr, ctx, builder)?;
            let n_val = compile_rec(&args[1], ctx, builder)?;
            let n_pos = builder.ins().fcmp(FloatCC::GreaterThan, n_val, zero);
            let n_safe = builder.ins().select(n_pos, n_val, one);
            let n_minus_one = builder.ins().fsub(n_safe, one);
            let slow_period = builder.ins().fmul(n_safe, interval_val);
            let t0 = builder.ins().fmul(n_minus_one, interval_val);
            let time_val = ctx
                .var_map
                .get("time")
                .copied()
                .ok_or_else(|| "backSample(sample(...), n) requires time in context".to_string())?;
            let t_arg = if let Some(se) = start_expr {
                let start_val = compile_rec(se, ctx, builder)?;
                builder.ins().fsub(time_val, start_val)
            } else {
                time_val
            };
            let shifted_t = builder.ins().fsub(t_arg, t0);
            let mut sig = ctx.module.make_signature();
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.returns.push(AbiParam::new(cl_types::F64));
            let func_id = ctx
                .module
                .declare_function("rustmodlica_sample", Linkage::Import, &sig)
                .map_err(|e| e.to_string())?;
            let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
            let call_inst = builder.ins().call(func_ref, &[shifted_t, slow_period]);
            Ok(builder.inst_results(call_inst)[0])
        }
        _ => compile_rec(&args[0], ctx, builder),
    }
}
