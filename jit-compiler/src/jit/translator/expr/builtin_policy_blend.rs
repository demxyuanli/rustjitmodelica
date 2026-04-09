use crate::ast::Expression;
use crate::jit::context::TranslationContext;
use crate::jit::translator::expr::helpers::jit_builtin_fallback_warn_once;
use cranelift::prelude::*;

pub(super) fn passthrough_first_empty0(
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

pub(super) fn passthrough_first_empty1(
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

pub(super) fn compile_reg_step_blend(
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

pub(super) fn compile_splice_blend(
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
