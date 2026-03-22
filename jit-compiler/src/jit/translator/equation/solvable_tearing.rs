use crate::ast::{Equation, Expression};
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift::codegen::ir::StackSlot;
use cranelift_module::Module;

use crate::jit::context::TranslationContext;
use crate::jit::translator::expr::compile_expression;

use super::helpers::{
    compile_inner_simple_assignments, store_diag_residual_and_x,
    write_back_inner_simple_equations_and_tearing_output,
};
use super::solvable::{emit_assert_suppress_begin, emit_assert_suppress_end};

#[allow(clippy::too_many_arguments)]
fn emit_bad_jacobian_retry_or_exit(
    ctx: &TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    t_slot: StackSlot,
    iter_slot: StackSlot,
    jac_retry_slot: StackSlot,
    iter_val: Value,
    retry_val: Value,
    x: Value,
    res_val: Value,
    header_block: Block,
    exit_block: Block,
) {
    store_diag_residual_and_x(ctx, builder, res_val, x);
    let retry_limit = builder.ins().f64const(12.0);
    let can_retry = builder.ins().fcmp(FloatCC::LessThan, retry_val, retry_limit);
    let retry_block = builder.create_block();
    let hard_fail_block = builder.create_block();
    builder
        .ins()
        .brif(can_retry, retry_block, &[], hard_fail_block, &[]);
    builder.switch_to_block(retry_block);
    let nudge_count: usize = 12;
    let nudge_table_slot = builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
        cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
        (nudge_count * 8) as u32,
        0,
    ));
    let nudges: [f64; 12] = [
        1e-4, -1e-4, 1e-3, -1e-3, 1e-2, -1e-2, 1e-1, -1e-1, 1.0, -1.0, 10.0, -10.0,
    ];
    for (ni, nv) in nudges.iter().enumerate() {
        let v = builder.ins().f64const(*nv);
        builder.ins().stack_store(v, nudge_table_slot, (ni * 8) as i32);
    }
    let ptr_ty = ctx.module.target_config().pointer_type();
    let retry_int = builder.ins().fcvt_to_sint(cl_types::I64, retry_val);
    let eight = builder.ins().iconst(cl_types::I64, 8);
    let byte_off = builder.ins().imul(retry_int, eight);
    let table_base = builder.ins().stack_addr(ptr_ty, nudge_table_slot, 0);
    let nudge_addr = builder.ins().iadd(table_base, byte_off);
    let nudge = builder.ins().load(cl_types::F64, MemFlags::new(), nudge_addr, 0);
    let x_nudged = builder.ins().fadd(x, nudge);
    builder.ins().stack_store(x_nudged, t_slot, 0);
    let one = builder.ins().f64const(1.0);
    let next_iter_bad = builder.ins().fadd(iter_val, one);
    builder.ins().stack_store(next_iter_bad, iter_slot, 0);
    let next_retry = builder.ins().fadd(retry_val, one);
    builder.ins().stack_store(next_retry, jac_retry_slot, 0);
    builder.ins().jump(header_block, &[]);
    builder.seal_block(retry_block);
    builder.switch_to_block(hard_fail_block);
    builder.ins().jump(exit_block, &[]);
    builder.seal_block(hard_fail_block);
}

#[allow(clippy::too_many_arguments)]
fn emit_line_search_update(
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    inner_eqs: &[Equation],
    residuals: &[Expression],
    t_slot: StackSlot,
    iter_slot: StackSlot,
    iter_val: Value,
    x: Value,
    res_val: Value,
    abs_res: Value,
    j_val: Value,
    header_block: Block,
    exit_block: Block,
) -> Result<(), String> {
    let eps = builder.ins().f64const(1e-12);
    let j_abs = builder.ins().fabs(j_val);
    let is_small = builder.ins().fcmp(FloatCC::LessThan, j_abs, eps);
    let pos_eps = builder.ins().f64const(1e-12);
    let neg_eps = builder.ins().f64const(-1e-12);
    let zero = builder.ins().f64const(0.0);
    let sign_non_neg = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, j_val, zero);
    let eps_signed = builder.ins().select(sign_non_neg, pos_eps, neg_eps);
    let j_safe = builder.ins().select(is_small, eps_signed, j_val);
    let step = builder.ins().fdiv(res_val, j_safe);
    let step_slot = builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
        cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
        8,
        0,
    ));
    let abs_res_slot =
        builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
            cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
            8,
            0,
        ));
    let x_save_slot = builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
        cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
        8,
        0,
    ));
    let alpha_slot = builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
        cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
        8,
        0,
    ));
    let ls_count_slot =
        builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
            cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
            8,
            0,
        ));
    builder.ins().stack_store(step, step_slot, 0);
    builder.ins().stack_store(abs_res, abs_res_slot, 0);
    builder.ins().stack_store(x, x_save_slot, 0);
    let ls_init_alpha = builder.ins().f64const(1.0);
    builder.ins().stack_store(ls_init_alpha, alpha_slot, 0);
    let ls_init_count = builder.ins().f64const(0.0);
    builder.ins().stack_store(ls_init_count, ls_count_slot, 0);
    let ls_header = builder.create_block();
    let ls_body = builder.create_block();
    let ls_accept = builder.create_block();
    let ls_halve = builder.create_block();
    let ls_fail = builder.create_block();
    builder.ins().jump(ls_header, &[]);
    builder.switch_to_block(ls_header);
    let ls_alpha = builder.ins().stack_load(cl_types::F64, alpha_slot, 0);
    let ls_cnt = builder.ins().stack_load(cl_types::F64, ls_count_slot, 0);
    let ls_max = builder.ins().f64const(8.0);
    let ls_continue = builder.ins().fcmp(FloatCC::LessThan, ls_cnt, ls_max);
    builder
        .ins()
        .brif(ls_continue, ls_body, &[], ls_fail, &[]);
    builder.switch_to_block(ls_body);
    let step_v = builder.ins().stack_load(cl_types::F64, step_slot, 0);
    let x_orig = builder.ins().stack_load(cl_types::F64, x_save_slot, 0);
    let scaled_step = builder.ins().fmul(ls_alpha, step_v);
    let x_try = builder.ins().fsub(x_orig, scaled_step);
    builder.ins().stack_store(x_try, t_slot, 0);
    compile_inner_simple_assignments(inner_eqs, ctx, builder)?;
    let r_ls = compile_expression(&residuals[0], ctx, builder)?;
    let abs_r_ls = builder.ins().fabs(r_ls);
    let old_abs = builder.ins().stack_load(cl_types::F64, abs_res_slot, 0);
    let c_armijo = builder.ins().f64const(1e-4);
    let ca = builder.ins().fmul(c_armijo, ls_alpha);
    let descent = builder.ins().fmul(ca, old_abs);
    let threshold = builder.ins().fsub(old_abs, descent);
    let better = builder.ins().fcmp(FloatCC::LessThan, abs_r_ls, threshold);
    builder.ins().brif(better, ls_accept, &[], ls_halve, &[]);
    builder.switch_to_block(ls_halve);
    let half = builder.ins().f64const(0.5);
    let new_alpha = builder.ins().fmul(ls_alpha, half);
    builder.ins().stack_store(new_alpha, alpha_slot, 0);
    let one_ls = builder.ins().f64const(1.0);
    let new_count = builder.ins().fadd(ls_cnt, one_ls);
    builder.ins().stack_store(new_count, ls_count_slot, 0);
    builder.ins().jump(ls_header, &[]);
    builder.seal_block(ls_halve);
    builder.switch_to_block(ls_fail);
    builder.ins().jump(exit_block, &[]);
    builder.seal_block(ls_fail);
    builder.switch_to_block(ls_accept);
    if let (Some(pr), Some(px)) = (ctx.diag_residual_ptr, ctx.diag_x_ptr) {
        builder.ins().store(MemFlags::new(), abs_r_ls, pr, 0);
        let accepted_x = builder.ins().stack_load(cl_types::F64, t_slot, 0);
        builder.ins().store(MemFlags::new(), accepted_x, px, 0);
    }
    let one = builder.ins().f64const(1.0);
    let next_iter = builder.ins().fadd(iter_val, one);
    builder.ins().stack_store(next_iter, iter_slot, 0);
    builder.ins().jump(header_block, &[]);
    builder.seal_block(ls_body);
    builder.seal_block(ls_header);
    builder.seal_block(ls_accept);
    Ok(())
}

pub(super) fn compile_single_unknown_or_tearing_solvable_block(
    unknowns: &[String],
    tearing_var: &Option<String>,
    inner_eqs: &[Equation],
    residuals: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    let t_var = tearing_var
        .as_ref()
        .cloned()
        .unwrap_or_else(|| unknowns[0].clone());
    ctx.var_map.remove(&t_var);
    let t_slot = *ctx
        .stack_slots
        .get(&t_var)
        .expect("Tearing var must have stack slot");
    if let Some(idx) = ctx.output_index(&t_var) {
        let offset = (idx * 8) as i32;
        let init_val = builder
            .ins()
            .load(cl_types::F64, MemFlags::new(), ctx.outputs_ptr, offset);
        builder.ins().stack_store(init_val, t_slot, 0);
    } else {
        let default_val = crate::compiler::geometric_default_for_name(&t_var);
        let fallback_f = if default_val != 0.0 { default_val } else { 1e-3 };
        let fallback = builder.ins().f64const(fallback_f);
        builder.ins().stack_store(fallback, t_slot, 0);
    }
    let iter_slot = builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
        cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
        8,
        0,
    ));
    let jac_retry_slot =
        builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
            cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
            8,
            0,
        ));
    let zero = builder.ins().f64const(0.0);
    builder.ins().stack_store(zero, iter_slot, 0);
    builder.ins().stack_store(zero, jac_retry_slot, 0);
    let header_block = builder.create_block();
    let body_block = builder.create_block();
    let perturb_block = builder.create_block();
    let exit_block = builder.create_block();
    let after_tearing_1 = builder.create_block();
    emit_assert_suppress_begin(ctx, builder)?;
    builder.ins().jump(header_block, &[]);
    builder.switch_to_block(header_block);
    let iter_val = builder.ins().stack_load(cl_types::F64, iter_slot, 0);
    let max_iter = builder.ins().f64const(200.0);
    let iter_cond = builder.ins().fcmp(FloatCC::LessThan, iter_val, max_iter);
    let error_block = builder.create_block();
    builder
        .ins()
        .brif(iter_cond, body_block, &[], error_block, &[]);
    builder.switch_to_block(error_block);
    if let (Some(pr), Some(_px)) = (ctx.diag_residual_ptr, ctx.diag_x_ptr) {
        let neg_one = builder.ins().f64const(-1.0);
        builder.ins().store(MemFlags::new(), neg_one, pr, 0);
    }
    builder.ins().jump(exit_block, &[]);
    builder.seal_block(error_block);
    builder.switch_to_block(body_block);
    let x = builder.ins().stack_load(cl_types::F64, t_slot, 0);
    compile_inner_simple_assignments(inner_eqs, ctx, builder)?;
    let res_val = compile_expression(&residuals[0], ctx, builder)?;
    let abs_res = builder.ins().fabs(res_val);
    let tol = builder.ins().f64const(1e-8);
    let converged = builder.ins().fcmp(FloatCC::LessThan, abs_res, tol);
    builder
        .ins()
        .brif(converged, exit_block, &[], perturb_block, &[]);
    builder.switch_to_block(perturb_block);
    if let (Some(pr), Some(px)) = (ctx.diag_residual_ptr, ctx.diag_x_ptr) {
        builder.ins().store(MemFlags::new(), res_val, pr, 0);
        builder.ins().store(MemFlags::new(), x, px, 0);
    }
    let epsilon = builder.ins().f64const(1e-6);
    let x_p = builder.ins().fadd(x, epsilon);
    builder.ins().stack_store(x_p, t_slot, 0);
    compile_inner_simple_assignments(inner_eqs, ctx, builder)?;
    let res_p = compile_expression(&residuals[0], ctx, builder)?;
    let diff_res = builder.ins().fsub(res_p, res_val);
    let j_val = builder.ins().fdiv(diff_res, epsilon);
    let j_abs = builder.ins().fabs(j_val);
    let j_min = builder.ins().f64const(1e-12);
    let bad_jac = builder.ins().fcmp(FloatCC::LessThan, j_abs, j_min);
    let jac_error_block = builder.create_block();
    let update_block = builder.create_block();
    builder
        .ins()
        .brif(bad_jac, jac_error_block, &[], update_block, &[]);
    builder.switch_to_block(jac_error_block);
    let retry_val = builder.ins().stack_load(cl_types::F64, jac_retry_slot, 0);
    emit_bad_jacobian_retry_or_exit(
        ctx,
        builder,
        t_slot,
        iter_slot,
        jac_retry_slot,
        iter_val,
        retry_val,
        x,
        res_val,
        header_block,
        exit_block,
    );
    builder.seal_block(jac_error_block);
    builder.switch_to_block(update_block);
    emit_line_search_update(
        ctx,
        builder,
        inner_eqs,
        residuals,
        t_slot,
        iter_slot,
        iter_val,
        x,
        res_val,
        abs_res,
        j_val,
        header_block,
        exit_block,
    )?;
    builder.seal_block(update_block);
    builder.seal_block(header_block);
    builder.seal_block(body_block);
    builder.seal_block(perturb_block);
    builder.switch_to_block(exit_block);
    emit_assert_suppress_end(ctx, builder)?;
    write_back_inner_simple_equations_and_tearing_output(inner_eqs, &t_var, t_slot, ctx, builder)?;
    builder.ins().jump(after_tearing_1, &[]);
    builder.seal_block(exit_block);
    builder.switch_to_block(after_tearing_1);
    Ok(())
}
