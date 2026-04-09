use crate::ast::Expression;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};

use crate::diag::fallback_counter;
use crate::jit::context::TranslationContext;

pub(super) fn compile_base_sample_trigger(
    clock_expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: fn(
        &Expression,
        &mut TranslationContext,
        &mut cranelift::frontend::FunctionBuilder<'_>,
    ) -> Result<Value, String>,
) -> Result<Option<(Value, Value)>, String> {
    let time_val = ctx
        .var_map
        .get("time")
        .copied()
        .ok_or("sample-based clock requires time in context".to_string())?;
    match clock_expr {
        Expression::Sample(interval_expr) => {
            let interval_val = compile_rec(interval_expr, ctx, builder)?;
            Ok(Some((time_val, interval_val)))
        }
        Expression::Call(name, args)
            if (name.eq_ignore_ascii_case("sample") || name.ends_with(".sample"))
                && (args.len() == 1 || args.len() == 2) =>
        {
            let interval_val = compile_rec(args.last().unwrap(), ctx, builder)?;
            if args.len() == 2 {
                let start_val = compile_rec(&args[0], ctx, builder)?;
                let t_rel = builder.ins().fsub(time_val, start_val);
                Ok(Some((t_rel, interval_val)))
            } else {
                Ok(Some((time_val, interval_val)))
            }
        }
        Expression::Call(name, args)
            if (name.eq_ignore_ascii_case("clock") || name.ends_with(".Clock"))
                && !args.is_empty() =>
        {
            let interval_val = compile_rec(args.last().unwrap(), ctx, builder)?;
            Ok(Some((time_val, interval_val)))
        }
        _ => Ok(None),
    }
}

use std::sync::OnceLock;

pub(super) fn warn_clock_degrade_once(kind: &'static str) {
    static SUPER_WARNED: OnceLock<()> = OnceLock::new();
    static SHIFT_WARNED: OnceLock<()> = OnceLock::new();
    static SUB_WARNED: OnceLock<()> = OnceLock::new();
    static BACK_WARNED: OnceLock<()> = OnceLock::new();
    static INTERVAL_WARNED: OnceLock<()> = OnceLock::new();
    static SYNC_WARN_ENABLED: OnceLock<bool> = OnceLock::new();
    let enabled = *SYNC_WARN_ENABLED.get_or_init(|| {
        std::env::var("RUSTMODLICA_SYNC_WARN")
            .ok()
            .map(|v| {
                let t = v.trim();
                t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(true)
    });
    if !enabled {
        return;
    }
    match kind {
        "superSample" => {
            if SUPER_WARNED.set(()).is_ok() {
                fallback_counter::inc_clock_degrade();
                eprintln!("[fallback:clock] superSample currently uses first-version passthrough semantics.");
            }
        }
        "shiftSample" => {
            if SHIFT_WARNED.set(()).is_ok() {
                fallback_counter::inc_clock_degrade();
                eprintln!("[fallback:clock] shiftSample currently uses first-version passthrough semantics.");
            }
        }
        "subSample" => {
            if SUB_WARNED.set(()).is_ok() {
                fallback_counter::inc_clock_degrade();
                eprintln!("[fallback:clock] subSample currently uses first-version passthrough semantics.");
            }
        }
        "backSample" => {
            if BACK_WARNED.set(()).is_ok() {
                fallback_counter::inc_clock_degrade();
                eprintln!("[fallback:clock] backSample currently uses first-version passthrough semantics.");
            }
        }
        "interval" => {
            if INTERVAL_WARNED.set(()).is_ok() {
                fallback_counter::inc_clock_degrade();
                eprintln!("[fallback:clock] interval() fallback currently returns 0.0 for non-numeric sample clock.");
            }
        }
        _ => {}
    }
}

pub(super) fn compile_sample_interval_clock_arms(
    expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: fn(
        &Expression,
        &mut TranslationContext,
        &mut cranelift::frontend::FunctionBuilder<'_>,
    ) -> Result<Value, String>,
) -> Result<Option<Value>, String> {
    match expr {
        Expression::Sample(interval_expr) => {
            if matches!(&**interval_expr, Expression::Number(_)) {
                let interval_val = compile_rec(interval_expr, ctx, builder)?;
                let time_val = ctx
                    .var_map
                    .get("time")
                    .copied()
                    .ok_or_else(|| "sample() requires time in context".to_string())?;
                let mut sig = ctx.module.make_signature();
                sig.params.push(AbiParam::new(cl_types::F64));
                sig.params.push(AbiParam::new(cl_types::F64));
                sig.returns.push(AbiParam::new(cl_types::F64));
                let func_id = ctx
                    .module
                    .declare_function("rustmodlica_sample", Linkage::Import, &sig)
                    .map_err(|e| e.to_string())?;
                let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
                let call_inst = builder.ins().call(func_ref, &[time_val, interval_val]);
                Ok(Some(builder.inst_results(call_inst)[0]))
            } else {
                Ok(Some(compile_rec(interval_expr, ctx, builder)?))
            }
        }
        Expression::Interval(clock_expr) => Ok(Some(match &**clock_expr {
            Expression::Sample(inner) => {
                if matches!(&**inner, Expression::Number(_)) {
                    compile_rec(inner, ctx, builder)?
                } else {
                    warn_clock_degrade_once("interval");
                    builder.ins().f64const(0.0)
                }
            }
            Expression::Call(name, cargs)
                if (name.eq_ignore_ascii_case("clock") || name.ends_with(".clock")
                    || name.eq_ignore_ascii_case("sample") || name.ends_with(".sample"))
                    && !cargs.is_empty() =>
            {
                compile_rec(cargs.last().unwrap(), ctx, builder)?
            }
            Expression::Variable(id) if crate::string_intern::resolve_id(*id).contains("simTime") => {
                warn_clock_degrade_once("interval");
                builder.ins().f64const(0.0)
            }
            _ => compile_rec(clock_expr, ctx, builder)?,
        })),
        Expression::Hold(inner) => Ok(Some(compile_rec(inner, ctx, builder)?)),
        Expression::SubSample(clock_expr, n_expr) => match compile_base_sample_trigger(
            clock_expr,
            ctx,
            builder,
            compile_rec,
        )? {
            Some((time_arg, interval_val)) => {
                let n_val = compile_rec(n_expr, ctx, builder)?;
                let zero = builder.ins().f64const(0.0);
                let one = builder.ins().f64const(1.0);
                let n_pos = builder.ins().fcmp(FloatCC::GreaterThan, n_val, zero);
                let n_safe = builder.ins().select(n_pos, n_val, one);
                let scaled_interval = builder.ins().fmul(interval_val, n_safe);
                let mut sig = ctx.module.make_signature();
                sig.params.push(AbiParam::new(cl_types::F64));
                sig.params.push(AbiParam::new(cl_types::F64));
                sig.returns.push(AbiParam::new(cl_types::F64));
                let func_id = ctx
                    .module
                    .declare_function("rustmodlica_sample", Linkage::Import, &sig)
                    .map_err(|e| e.to_string())?;
                let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
                let call_inst = builder.ins().call(func_ref, &[time_arg, scaled_interval]);
                Ok(Some(builder.inst_results(call_inst)[0]))
            }
            None => {
                warn_clock_degrade_once("subSample");
                Ok(Some(compile_rec(clock_expr, ctx, builder)?))
            }
        },
        Expression::SuperSample(clock_expr, n_expr) => match compile_base_sample_trigger(
            clock_expr,
            ctx,
            builder,
            compile_rec,
        )? {
            Some((time_arg, interval_val)) => {
                let n_val = compile_rec(n_expr, ctx, builder)?;
                let one = builder.ins().f64const(1.0);
                let zero = builder.ins().f64const(0.0);
                let n_pos = builder.ins().fcmp(FloatCC::GreaterThan, n_val, zero);
                let n_safe = builder.ins().select(n_pos, n_val, one);
                let scaled_interval = builder.ins().fdiv(interval_val, n_safe);
                let mut sig = ctx.module.make_signature();
                sig.params.push(AbiParam::new(cl_types::F64));
                sig.params.push(AbiParam::new(cl_types::F64));
                sig.returns.push(AbiParam::new(cl_types::F64));
                let func_id = ctx
                    .module
                    .declare_function("rustmodlica_sample", Linkage::Import, &sig)
                    .map_err(|e| e.to_string())?;
                let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
                let call_inst = builder.ins().call(func_ref, &[time_arg, scaled_interval]);
                Ok(Some(builder.inst_results(call_inst)[0]))
            }
            None => {
                warn_clock_degrade_once("superSample");
                Ok(Some(compile_rec(clock_expr, ctx, builder)?))
            }
        },
        Expression::ShiftSample(clock_expr, n_expr) => match compile_base_sample_trigger(
            clock_expr,
            ctx,
            builder,
            compile_rec,
        )? {
            Some((time_arg, interval_val)) => {
                let n_val = compile_rec(n_expr, ctx, builder)?;
                let shift = builder.ins().fmul(interval_val, n_val);
                let shifted_t = builder.ins().fsub(time_arg, shift);
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
                Ok(Some(builder.inst_results(call_inst)[0]))
            }
            None => {
                warn_clock_degrade_once("shiftSample");
                Ok(Some(compile_rec(clock_expr, ctx, builder)?))
            }
        },
        Expression::BackSample(clock_expr, n_expr) => match compile_base_sample_trigger(
            clock_expr,
            ctx,
            builder,
            compile_rec,
        )? {
            Some((time_arg, interval_val)) => {
                let n_val = compile_rec(n_expr, ctx, builder)?;
                let one = builder.ins().f64const(1.0);
                let zero = builder.ins().f64const(0.0);
                let n_pos = builder.ins().fcmp(FloatCC::GreaterThan, n_val, zero);
                let n_safe = builder.ins().select(n_pos, n_val, one);
                let n_minus_one = builder.ins().fsub(n_safe, one);
                let slow_period = builder.ins().fmul(n_safe, interval_val);
                let t0 = builder.ins().fmul(n_minus_one, interval_val);
                let shifted_t = builder.ins().fsub(time_arg, t0);
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
                Ok(Some(builder.inst_results(call_inst)[0]))
            }
            None => {
                warn_clock_degrade_once("backSample");
                Ok(Some(compile_rec(clock_expr, ctx, builder)?))
            }
        },
        _ => Ok(None),
    }
}
