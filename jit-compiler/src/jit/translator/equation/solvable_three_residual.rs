use crate::ast::Expression;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift::codegen::ir::StackSlot;
use cranelift_module::{Linkage, Module};

use crate::jit::context::TranslationContext;
use crate::jit::translator::expr::compile_expression;

use super::helpers::{init_unknown_slot_from_output_or_default, write_unknown_outputs};
use super::solvable::{emit_assert_suppress_begin, emit_assert_suppress_end};

#[allow(clippy::too_many_arguments)]
fn emit_three_residual_newton_loop(
    v0: &str,
    v1: &str,
    v2: &str,
    slot0: StackSlot,
    slot1: StackSlot,
    slot2: StackSlot,
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
    let eps_val = builder.ins().f64const(eps);
    let header_block = builder.create_block();
    let body_block = builder.create_block();
    let perturb_block = builder.create_block();
    let exit_block = builder.create_block();
    let iter_error_block = builder.create_block();
    let det_error_block = builder.create_block();
    let after_newton_3 = builder.create_block();
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
    let x2 = builder.ins().stack_load(cl_types::F64, slot2, 0);
    let r0 = compile_expression(&residuals[0], ctx, builder)?;
    let r1 = compile_expression(&residuals[1], ctx, builder)?;
    let r2 = compile_expression(&residuals[2], ctx, builder)?;
    let tol = builder.ins().f64const(1e-8);
    let ar0 = builder.ins().fabs(r0);
    let ar1 = builder.ins().fabs(r1);
    let ar2 = builder.ins().fabs(r2);
    let c0 = builder.ins().fcmp(FloatCC::LessThan, ar0, tol);
    let c1 = builder.ins().fcmp(FloatCC::LessThan, ar1, tol);
    let c2 = builder.ins().fcmp(FloatCC::LessThan, ar2, tol);
    let check_c1_block = builder.create_block();
    let check_c2_block = builder.create_block();
    builder
        .ins()
        .brif(c0, check_c1_block, &[], perturb_block, &[]);
    builder.switch_to_block(check_c1_block);
    builder
        .ins()
        .brif(c1, check_c2_block, &[], perturb_block, &[]);
    builder.switch_to_block(check_c2_block);
    builder.ins().brif(c2, exit_block, &[], perturb_block, &[]);
    builder.switch_to_block(perturb_block);
    let x0p = builder.ins().fadd(x0, eps_val);
    builder.ins().stack_store(x0p, slot0, 0);
    let r0p0 = compile_expression(&residuals[0], ctx, builder)?;
    let r1p0 = compile_expression(&residuals[1], ctx, builder)?;
    let r2p0 = compile_expression(&residuals[2], ctx, builder)?;
    builder.ins().stack_store(x0, slot0, 0);
    let x1p = builder.ins().fadd(x1, eps_val);
    builder.ins().stack_store(x1p, slot1, 0);
    let r0p1 = compile_expression(&residuals[0], ctx, builder)?;
    let r1p1 = compile_expression(&residuals[1], ctx, builder)?;
    let r2p1 = compile_expression(&residuals[2], ctx, builder)?;
    builder.ins().stack_store(x1, slot1, 0);
    let x2p = builder.ins().fadd(x2, eps_val);
    builder.ins().stack_store(x2p, slot2, 0);
    let r0p2 = compile_expression(&residuals[0], ctx, builder)?;
    let r1p2 = compile_expression(&residuals[1], ctx, builder)?;
    let r2p2 = compile_expression(&residuals[2], ctx, builder)?;
    builder.ins().stack_store(x2, slot2, 0);
    let dr0_0 = builder.ins().fsub(r0p0, r0);
    let dr1_0 = builder.ins().fsub(r1p0, r1);
    let dr2_0 = builder.ins().fsub(r2p0, r2);
    let dr0_1 = builder.ins().fsub(r0p1, r0);
    let dr1_1 = builder.ins().fsub(r1p1, r1);
    let dr2_1 = builder.ins().fsub(r2p1, r2);
    let dr0_2 = builder.ins().fsub(r0p2, r0);
    let dr1_2 = builder.ins().fsub(r1p2, r1);
    let dr2_2 = builder.ins().fsub(r2p2, r2);
    let j00 = builder.ins().fdiv(dr0_0, eps_val);
    let j10 = builder.ins().fdiv(dr1_0, eps_val);
    let j20 = builder.ins().fdiv(dr2_0, eps_val);
    let j01 = builder.ins().fdiv(dr0_1, eps_val);
    let j11 = builder.ins().fdiv(dr1_1, eps_val);
    let j21 = builder.ins().fdiv(dr2_1, eps_val);
    let j02 = builder.ins().fdiv(dr0_2, eps_val);
    let j12 = builder.ins().fdiv(dr1_2, eps_val);
    let j22 = builder.ins().fdiv(dr2_2, eps_val);
    let j11_j22 = builder.ins().fmul(j11, j22);
    let j12_j21 = builder.ins().fmul(j12, j21);
    let j10_j22 = builder.ins().fmul(j10, j22);
    let j12_j20 = builder.ins().fmul(j12, j20);
    let j10_j21 = builder.ins().fmul(j10, j21);
    let j11_j20 = builder.ins().fmul(j11, j20);
    let c0_det = builder.ins().fsub(j11_j22, j12_j21);
    let c1_det = builder.ins().fsub(j10_j22, j12_j20);
    let c2_det = builder.ins().fsub(j10_j21, j11_j20);
    let t0 = builder.ins().fmul(j00, c0_det);
    let t1 = builder.ins().fmul(j01, c1_det);
    let t2 = builder.ins().fmul(j02, c2_det);
    let t12 = builder.ins().fadd(t1, t2);
    let det_fixed = builder.ins().fsub(t0, t12);
    let det_abs = builder.ins().fabs(det_fixed);
    let min_det = builder.ins().f64const(1e-12);
    let bad_det = builder.ins().fcmp(FloatCC::LessThan, det_abs, min_det);
    let update_block = builder.create_block();
    builder
        .ins()
        .brif(bad_det, det_error_block, &[], update_block, &[]);
    builder.switch_to_block(update_block);
    let j01_j10 = builder.ins().fmul(j01, j10);
    let j00_j22 = builder.ins().fmul(j00, j22);
    let j02_j20 = builder.ins().fmul(j02, j20);
    let j00_j12 = builder.ins().fmul(j00, j12);
    let zero_f = builder.ins().f64const(0.0);
    let neg_r0 = builder.ins().fsub(zero_f, r0);
    let neg_r1 = builder.ins().fsub(zero_f, r1);
    let _neg_r2 = builder.ins().fsub(zero_f, r2);
    let j01_j22 = builder.ins().fmul(j01, j22);
    let j02_j21 = builder.ins().fmul(j02, j21);
    let j01_j12 = builder.ins().fmul(j01, j12);
    let j02_j11 = builder.ins().fmul(j02, j11);
    let n0a = builder.ins().fmul(neg_r0, c0_det);
    let n0b = builder.ins().fsub(j01_j22, j02_j21);
    let n0c = builder.ins().fsub(j01_j12, j02_j11);
    let n0d = builder.ins().fmul(r1, n0b);
    let n0e = builder.ins().fmul(r2, n0c);
    let n0f = builder.ins().fadd(n0d, n0e);
    let num0_correct = builder.ins().fadd(n0a, n0f);
    let dx0 = builder.ins().fdiv(num0_correct, det_fixed);
    let j02_j10 = builder.ins().fmul(j02, j10);
    let neg_j01_j12 = builder.ins().fsub(zero_f, j01_j12);
    let n1a = builder.ins().fmul(r0, neg_j01_j12);
    let n1b = builder.ins().fadd(j00_j22, j02_j10);
    let n1c_inner = builder.ins().fadd(j00_j12, j01_j10);
    let n1c = builder.ins().fadd(n1c_inner, j02_j20);
    let n1d = builder.ins().fmul(neg_r1, n1b);
    let n1e = builder.ins().fmul(r2, n1c);
    let n1f = builder.ins().fadd(n1d, n1e);
    let num1_correct = builder.ins().fadd(n1a, n1f);
    let dx1 = builder.ins().fdiv(num1_correct, det_fixed);
    let n2a = builder.ins().fmul(neg_r0, c2_det);
    let n2b_l = builder.ins().fmul(r1, j12);
    let n2b_r = builder.ins().fmul(r2, j11);
    let n2b = builder.ins().fsub(n2b_l, n2b_r);
    let n2c_l = builder.ins().fmul(r2, j10);
    let n2c_r = builder.ins().fmul(r0, j12);
    let n2c = builder.ins().fsub(n2c_l, n2c_r);
    let n2d = builder.ins().fmul(j00, n2b);
    let n2e = builder.ins().fmul(j01, n2c);
    let n2f = builder.ins().fadd(n2d, n2e);
    let num2_correct = builder.ins().fadd(n2a, n2f);
    let dx2 = builder.ins().fdiv(num2_correct, det_fixed);
    let x0_new = builder.ins().fadd(x0, dx0);
    let x1_new = builder.ins().fadd(x1, dx1);
    let x2_new = builder.ins().fadd(x2, dx2);
    builder.ins().stack_store(x0_new, slot0, 0);
    builder.ins().stack_store(x1_new, slot1, 0);
    builder.ins().stack_store(x2_new, slot2, 0);
    let one = builder.ins().f64const(1.0);
    let next_iter = builder.ins().fadd(iter_val, one);
    builder.ins().stack_store(next_iter, iter_slot, 0);
    builder.ins().jump(header_block, &[]);
    builder.seal_block(update_block);
    builder.seal_block(body_block);
    builder.seal_block(check_c1_block);
    builder.seal_block(check_c2_block);
    builder.seal_block(perturb_block);
    builder.switch_to_block(det_error_block);
    let ptr_type_3 = ctx.module.target_config().pointer_type();
    let lm3_buf_size = (9 + 3 + 3) * 8;
    let lm3_buf = builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
        cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
        lm3_buf_size as u32,
        0,
    ));
    let lm3_base = builder.ins().stack_addr(ptr_type_3, lm3_buf, 0);
    let jac_vals = [j00, j01, j02, j10, j11, j12, j20, j21, j22];
    for (idx, jval) in jac_vals.iter().enumerate() {
        let off = builder.ins().iconst(ptr_type_3, (idx * 8) as i64);
        let addr = builder.ins().iadd(lm3_base, off);
        builder.ins().store(MemFlags::new(), *jval, addr, 0);
    }
    let r3_base = 9 * 8;
    let r3_vals = [r0, r1, r2];
    for (idx, rval) in r3_vals.iter().enumerate() {
        let off = builder.ins().iconst(ptr_type_3, (r3_base + idx * 8) as i64);
        let addr = builder.ins().iadd(lm3_base, off);
        builder.ins().store(MemFlags::new(), *rval, addr, 0);
    }
    let dx3_base = (9 + 3) * 8;
    let r3_off_v = builder.ins().iconst(ptr_type_3, r3_base as i64);
    let r3_ptr = builder.ins().iadd(lm3_base, r3_off_v);
    let dx3_off_v = builder.ins().iconst(ptr_type_3, dx3_base as i64);
    let dx3_ptr = builder.ins().iadd(lm3_base, dx3_off_v);
    let n_3 = builder.ins().iconst(cl_types::I32, 3);
    let mut sig3 = ctx.module.make_signature();
    sig3.params.push(AbiParam::new(cl_types::I32));
    sig3.params.push(AbiParam::new(ptr_type_3));
    sig3.params.push(AbiParam::new(ptr_type_3));
    sig3.params.push(AbiParam::new(ptr_type_3));
    sig3.returns.push(AbiParam::new(cl_types::I32));
    let func_id_3 = ctx
        .module
        .declare_function("rustmodlica_solve_linear_n", Linkage::Import, &sig3)
        .map_err(|e| e.to_string())?;
    let func_ref_3 = ctx.module.declare_func_in_func(func_id_3, &mut builder.func);
    let solve_res_3 = builder
        .ins()
        .call(func_ref_3, &[n_3, lm3_base, r3_ptr, dx3_ptr]);
    let solve_status_3 = builder.inst_results(solve_res_3)[0];
    let zero_i32_3 = builder.ins().iconst(cl_types::I32, 0);
    let solve_ok_3 = builder.ins().icmp(IntCC::Equal, solve_status_3, zero_i32_3);
    let lm3_update = builder.create_block();
    let lm3_fail = builder.create_block();
    builder
        .ins()
        .brif(solve_ok_3, lm3_update, &[], lm3_fail, &[]);
    builder.switch_to_block(lm3_fail);
    emit_assert_suppress_end(ctx, builder)?;
    let det_err3 = builder.ins().iconst(cl_types::I32, 2);
    builder.ins().return_(&[det_err3]);
    builder.seal_block(lm3_fail);
    builder.switch_to_block(lm3_update);
    for (i, slt) in [slot0, slot1, slot2].iter().enumerate() {
        let off = builder.ins().iconst(ptr_type_3, (dx3_base + i * 8) as i64);
        let addr = builder.ins().iadd(lm3_base, off);
        let dxi = builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0);
        let xi = builder.ins().stack_load(cl_types::F64, *slt, 0);
        let xi_new = builder.ins().fadd(xi, dxi);
        builder.ins().stack_store(xi_new, *slt, 0);
    }
    let one_lm3 = builder.ins().f64const(1.0);
    let next_iter_lm3 = builder.ins().fadd(iter_val, one_lm3);
    builder.ins().stack_store(next_iter_lm3, iter_slot, 0);
    builder.ins().jump(header_block, &[]);
    builder.seal_block(lm3_update);
    builder.seal_block(det_error_block);
    builder.seal_block(header_block);
    builder.switch_to_block(exit_block);
    emit_assert_suppress_end(ctx, builder)?;
    write_unknown_outputs(&[(v0, slot0), (v1, slot1), (v2, slot2)], ctx, builder);
    builder.ins().jump(after_newton_3, &[]);
    builder.seal_block(exit_block);
    builder.switch_to_block(after_newton_3);
    Ok(())
}

pub(super) fn compile_three_residual_solvable_block(
    unknowns: &[String],
    residuals: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    let v0 = &unknowns[0];
    let v1 = &unknowns[1];
    let v2 = &unknowns[2];
    let slot0 = *ctx
        .stack_slots
        .get(v0)
        .ok_or_else(|| format!("SolvableBlock unknown {} missing stack slot", v0))?;
    let slot1 = *ctx
        .stack_slots
        .get(v1)
        .ok_or_else(|| format!("SolvableBlock unknown {} missing stack slot", v1))?;
    let slot2 = *ctx
        .stack_slots
        .get(v2)
        .ok_or_else(|| format!("SolvableBlock unknown {} missing stack slot", v2))?;
    for (var, slot) in [(v0, slot0), (v1, slot1), (v2, slot2)] {
        init_unknown_slot_from_output_or_default(var, slot, ctx, builder);
    }
    ctx.var_map.remove(v0);
    ctx.var_map.remove(v1);
    ctx.var_map.remove(v2);
    emit_three_residual_newton_loop(v0, v1, v2, slot0, slot1, slot2, residuals, ctx, builder)
}
