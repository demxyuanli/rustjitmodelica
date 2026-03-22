use crate::ast::Expression;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift::codegen::ir::StackSlot;

use crate::jit::context::TranslationContext;
use crate::jit::translator::expr::compile_expression;

use super::helpers::{init_unknown_slot_from_output_or_default, write_unknown_outputs};
use super::solvable::{emit_assert_suppress_begin, emit_assert_suppress_end};

#[allow(clippy::too_many_arguments)]
fn emit_two_residual_newton_loop(
    v0: &str,
    v1: &str,
    slot0: StackSlot,
    slot1: StackSlot,
    residuals: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    let iter_slot = builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
        cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
        8,
        0,
    ));
    let zero = builder.ins().f64const(0.0);
    builder.ins().stack_store(zero, iter_slot, 0);
    let eps = 1e-6_f64;
    let header_block = builder.create_block();
    let body_block = builder.create_block();
    let perturb_block = builder.create_block();
    let exit_block = builder.create_block();
    let iter_error_block = builder.create_block();
    let det_error_block = builder.create_block();
    let after_newton_2 = builder.create_block();
    emit_assert_suppress_begin(ctx, builder)?;
    builder.ins().jump(header_block, &[]);
    builder.switch_to_block(header_block);
    let iter_val = builder.ins().stack_load(cl_types::F64, iter_slot, 0);
    let max_iter = builder.ins().f64const(200.0);
    let iter_cond = builder.ins().fcmp(FloatCC::LessThan, iter_val, max_iter);
    builder
        .ins()
        .brif(iter_cond, body_block, &[], iter_error_block, &[]);
    builder.switch_to_block(iter_error_block);
    emit_assert_suppress_end(ctx, builder)?;
    let err_code = builder.ins().iconst(cl_types::I32, 2);
    builder.ins().return_(&[err_code]);
    builder.seal_block(iter_error_block);
    builder.switch_to_block(body_block);
    let x0 = builder.ins().stack_load(cl_types::F64, slot0, 0);
    let x1 = builder.ins().stack_load(cl_types::F64, slot1, 0);
    let r0 = compile_expression(&residuals[0], ctx, builder)?;
    let r1 = compile_expression(&residuals[1], ctx, builder)?;
    let tol = builder.ins().f64const(1e-8);
    let ar0 = builder.ins().fabs(r0);
    let ar1 = builder.ins().fabs(r1);
    let c0 = builder.ins().fcmp(FloatCC::LessThan, ar0, tol);
    let c1 = builder.ins().fcmp(FloatCC::LessThan, ar1, tol);
    let check_c1_block = builder.create_block();
    builder
        .ins()
        .brif(c0, check_c1_block, &[], perturb_block, &[]);
    builder.switch_to_block(check_c1_block);
    builder.ins().brif(c1, exit_block, &[], perturb_block, &[]);
    builder.switch_to_block(perturb_block);
    let eps_val = builder.ins().f64const(eps);
    let x0p = builder.ins().fadd(x0, eps_val);
    builder.ins().stack_store(x0p, slot0, 0);
    let r0p0 = compile_expression(&residuals[0], ctx, builder)?;
    let r1p0 = compile_expression(&residuals[1], ctx, builder)?;
    builder.ins().stack_store(x0, slot0, 0);
    let x1p = builder.ins().fadd(x1, eps_val);
    builder.ins().stack_store(x1p, slot1, 0);
    let r0p1 = compile_expression(&residuals[0], ctx, builder)?;
    let r1p1 = compile_expression(&residuals[1], ctx, builder)?;
    builder.ins().stack_store(x1, slot1, 0);
    let dr0_0 = builder.ins().fsub(r0p0, r0);
    let dr1_0 = builder.ins().fsub(r1p0, r1);
    let dr0_1 = builder.ins().fsub(r0p1, r0);
    let dr1_1 = builder.ins().fsub(r1p1, r1);
    let j00 = builder.ins().fdiv(dr0_0, eps_val);
    let j10 = builder.ins().fdiv(dr1_0, eps_val);
    let j01 = builder.ins().fdiv(dr0_1, eps_val);
    let j11 = builder.ins().fdiv(dr1_1, eps_val);
    let j00_j11 = builder.ins().fmul(j00, j11);
    let j01_j10 = builder.ins().fmul(j01, j10);
    let det = builder.ins().fsub(j00_j11, j01_j10);
    let det_abs = builder.ins().fabs(det);
    let min_det = builder.ins().f64const(1e-12);
    let bad_det = builder.ins().fcmp(FloatCC::LessThan, det_abs, min_det);
    let update_block = builder.create_block();
    builder
        .ins()
        .brif(bad_det, det_error_block, &[], update_block, &[]);
    builder.switch_to_block(update_block);
    let zero_f = builder.ins().f64const(0.0);
    let neg_r0 = builder.ins().fsub(zero_f, r0);
    let num0_a = builder.ins().fmul(neg_r0, j11);
    let num0_b = builder.ins().fmul(r1, j01);
    let num0 = builder.ins().fadd(num0_a, num0_b);
    let dx0 = builder.ins().fdiv(num0, det);
    let num1_a = builder.ins().fmul(r0, j10);
    let num1_b = builder.ins().fmul(r1, j00);
    let num1 = builder.ins().fsub(num1_a, num1_b);
    let dx1 = builder.ins().fdiv(num1, det);
    let x0_new = builder.ins().fadd(x0, dx0);
    let x1_new = builder.ins().fadd(x1, dx1);
    builder.ins().stack_store(x0_new, slot0, 0);
    builder.ins().stack_store(x1_new, slot1, 0);
    let one = builder.ins().f64const(1.0);
    let next_iter = builder.ins().fadd(iter_val, one);
    builder.ins().stack_store(next_iter, iter_slot, 0);
    builder.ins().jump(header_block, &[]);
    builder.seal_block(update_block);
    builder.seal_block(body_block);
    builder.seal_block(check_c1_block);
    builder.seal_block(perturb_block);
    builder.switch_to_block(det_error_block);
    let lm_lambda2 = builder.ins().f64const(1e-6);
    let j00_d = builder.ins().fadd(j00, lm_lambda2);
    let j11_d = builder.ins().fadd(j11, lm_lambda2);
    let det_d_prod = builder.ins().fmul(j00_d, j11_d);
    let det_d_cross = builder.ins().fmul(j01, j10);
    let det_damped = builder.ins().fsub(det_d_prod, det_d_cross);
    let det_d_abs = builder.ins().fabs(det_damped);
    let min_det_d = builder.ins().f64const(1e-14);
    let still_bad = builder.ins().fcmp(FloatCC::LessThan, det_d_abs, min_det_d);
    let lm2_solve = builder.create_block();
    let lm2_fail = builder.create_block();
    builder
        .ins()
        .brif(still_bad, lm2_fail, &[], lm2_solve, &[]);
    builder.switch_to_block(lm2_fail);
    emit_assert_suppress_end(ctx, builder)?;
    let det_err = builder.ins().iconst(cl_types::I32, 2);
    builder.ins().return_(&[det_err]);
    builder.seal_block(lm2_fail);
    builder.switch_to_block(lm2_solve);
    let zero_d = builder.ins().f64const(0.0);
    let neg_r0_d = builder.ins().fsub(zero_d, r0);
    let n0a_d = builder.ins().fmul(neg_r0_d, j11_d);
    let n0b_d = builder.ins().fmul(r1, j01);
    let n0_d = builder.ins().fadd(n0a_d, n0b_d);
    let dx0_d = builder.ins().fdiv(n0_d, det_damped);
    let n1a_d = builder.ins().fmul(r0, j10);
    let n1b_d = builder.ins().fmul(r1, j00_d);
    let n1_d = builder.ins().fsub(n1a_d, n1b_d);
    let dx1_d = builder.ins().fdiv(n1_d, det_damped);
    let x0_lm = builder.ins().fadd(x0, dx0_d);
    let x1_lm = builder.ins().fadd(x1, dx1_d);
    builder.ins().stack_store(x0_lm, slot0, 0);
    builder.ins().stack_store(x1_lm, slot1, 0);
    let one_lm = builder.ins().f64const(1.0);
    let next_iter_lm = builder.ins().fadd(iter_val, one_lm);
    builder.ins().stack_store(next_iter_lm, iter_slot, 0);
    builder.ins().jump(header_block, &[]);
    builder.seal_block(lm2_solve);
    builder.seal_block(det_error_block);
    builder.seal_block(header_block);
    builder.switch_to_block(exit_block);
    emit_assert_suppress_end(ctx, builder)?;
    write_unknown_outputs(&[(v0, slot0), (v1, slot1)], ctx, builder);
    builder.ins().jump(after_newton_2, &[]);
    builder.seal_block(exit_block);
    builder.switch_to_block(after_newton_2);
    Ok(())
}

pub(super) fn compile_two_residual_solvable_block(
    unknowns: &[String],
    residuals: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    let v0 = &unknowns[0];
    let v1 = &unknowns[1];
    let slot0 = *ctx
        .stack_slots
        .get(v0)
        .ok_or_else(|| format!("SolvableBlock unknown {} missing stack slot", v0))?;
    let slot1 = *ctx
        .stack_slots
        .get(v1)
        .ok_or_else(|| format!("SolvableBlock unknown {} missing stack slot", v1))?;
    init_unknown_slot_from_output_or_default(v0, slot0, ctx, builder);
    init_unknown_slot_from_output_or_default(v1, slot1, ctx, builder);
    ctx.var_map.remove(v0);
    ctx.var_map.remove(v1);
    emit_two_residual_newton_loop(v0, v1, slot0, slot1, residuals, ctx, builder)
}
