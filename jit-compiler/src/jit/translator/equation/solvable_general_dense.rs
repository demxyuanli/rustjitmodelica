use crate::ast::Expression;
use cranelift::codegen::ir::StackSlot;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};

use crate::jit::context::TranslationContext;
use crate::jit::translator::expr::compile_expression;

use super::solvable_assert::{emit_assert_suppress_begin, emit_assert_suppress_end};

pub(super) fn compile_solvable_block_general_dense_n(
    unknowns: &[String],
    residuals: &[Expression],
    slots: &[StackSlot],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    let n = residuals.len();
    let ptr_type = ctx.module.target_config().pointer_type();
    let buf_size = (n * n + n + n) * 8;
    let buf_slot = builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
        cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
        buf_size as u32,
        0,
    ));
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
    let exit_block = builder.create_block();
    let iter_error_block = builder.create_block();
    let solve_error_block = builder.create_block();
    let after_dense_n = builder.create_block();
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
    let base_ptr = builder.ins().stack_addr(ptr_type, buf_slot, 0);
    let _jac_offset = 0i32;
    let r_offset = (n * n * 8) as i32;
    let dx_offset = ((n * n + n) * 8) as i32;
    let r_off_val = builder.ins().iconst(ptr_type, r_offset as i64);
    let r_ptr = builder.ins().iadd(base_ptr, r_off_val);
    let dx_off_val = builder.ins().iconst(ptr_type, dx_offset as i64);
    let dx_ptr = builder.ins().iadd(base_ptr, dx_off_val);
    let mut r_vals = Vec::with_capacity(n);
    for i in 0..n {
        let rv = compile_expression(&residuals[i], ctx, builder)?;
        r_vals.push(rv);
        let off = r_offset + (i * 8) as i32;
        let off_val = builder.ins().iconst(ptr_type, off as i64);
        let addr = builder.ins().iadd(base_ptr, off_val);
        builder.ins().store(MemFlags::new(), rv, addr, 0);
    }
    let tol = builder.ins().f64const(1e-8);
    let mut max_abs = builder.ins().f64const(0.0);
    for rv in &r_vals {
        let ar = builder.ins().fabs(*rv);
        max_abs = builder.ins().fmax(max_abs, ar);
    }
    let perturb_block = builder.create_block();
    let conv_cond = builder.ins().fcmp(FloatCC::LessThan, max_abs, tol);
    builder
        .ins()
        .brif(conv_cond, exit_block, &[], perturb_block, &[]);
    builder.switch_to_block(perturb_block);
    for j in 0..n {
        let xj = builder.ins().stack_load(cl_types::F64, slots[j], 0);
        let xjp = builder.ins().fadd(xj, eps_val);
        builder.ins().stack_store(xjp, slots[j], 0);
        for i in 0..n {
            let rp = compile_expression(&residuals[i], ctx, builder)?;
            let r_orig = r_vals[i];
            let dr = builder.ins().fsub(rp, r_orig);
            let jac_ij = builder.ins().fdiv(dr, eps_val);
            let off = (i * n + j) * 8;
            let off_val = builder.ins().iconst(ptr_type, off as i64);
            let addr = builder.ins().iadd(base_ptr, off_val);
            builder.ins().store(MemFlags::new(), jac_ij, addr, 0);
        }
        builder.ins().stack_store(xj, slots[j], 0);
    }
    let n_i32 = builder.ins().iconst(cl_types::I32, n as i64);
    let jac_ptr = base_ptr;
    let mut sig = ctx.module.make_signature();
    sig.params.push(AbiParam::new(cl_types::I32));
    sig.params.push(AbiParam::new(ptr_type));
    sig.params.push(AbiParam::new(ptr_type));
    sig.params.push(AbiParam::new(ptr_type));
    sig.returns.push(AbiParam::new(cl_types::I32));
    let func_id = ctx
        .module
        .declare_function("rustmodlica_solve_linear_n", Linkage::Import, &sig)
        .map_err(|e| e.to_string())?;
    let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
    let solve_result = builder
        .ins()
        .call(func_ref, &[n_i32, jac_ptr, r_ptr, dx_ptr]);
    let status = builder.inst_results(solve_result)[0];
    let zero_i32 = builder.ins().iconst(cl_types::I32, 0);
    let status_ok = builder.ins().icmp(IntCC::Equal, status, zero_i32);
    let update_block = builder.create_block();
    builder
        .ins()
        .brif(status_ok, update_block, &[], solve_error_block, &[]);
    builder.switch_to_block(update_block);
    let ls_alpha_slot_n = builder.create_sized_stack_slot(
        cranelift::codegen::ir::StackSlotData::new(
            cranelift::codegen::ir::StackSlotKind::ExplicitSlot, 8, 0,
        ),
    );
    let ls_count_slot_n = builder.create_sized_stack_slot(
        cranelift::codegen::ir::StackSlotData::new(
            cranelift::codegen::ir::StackSlotKind::ExplicitSlot, 8, 0,
        ),
    );
    let ls_old_norm_slot = builder.create_sized_stack_slot(
        cranelift::codegen::ir::StackSlotData::new(
            cranelift::codegen::ir::StackSlotKind::ExplicitSlot, 8, 0,
        ),
    );
    let x_save_slots: Vec<_> = (0..n)
        .map(|_| {
            builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
                cranelift::codegen::ir::StackSlotKind::ExplicitSlot, 8, 0,
            ))
        })
        .collect();
    builder.ins().stack_store(max_abs, ls_old_norm_slot, 0);
    for i in 0..n {
        let xi = builder.ins().stack_load(cl_types::F64, slots[i], 0);
        builder.ins().stack_store(xi, x_save_slots[i], 0);
    }
    let ls_init_a = builder.ins().f64const(1.0);
    builder.ins().stack_store(ls_init_a, ls_alpha_slot_n, 0);
    let ls_init_c = builder.ins().f64const(0.0);
    builder.ins().stack_store(ls_init_c, ls_count_slot_n, 0);
    let ls_hdr_n = builder.create_block();
    let ls_body_n = builder.create_block();
    let ls_accept_n = builder.create_block();
    let ls_halve_n = builder.create_block();
    let ls_fail_n = builder.create_block();
    builder.ins().jump(ls_hdr_n, &[]);
    builder.switch_to_block(ls_hdr_n);
    let ls_a_n = builder.ins().stack_load(cl_types::F64, ls_alpha_slot_n, 0);
    let ls_c_n = builder.ins().stack_load(cl_types::F64, ls_count_slot_n, 0);
    let ls_max_n = builder.ins().f64const(8.0);
    let ls_ok_n = builder.ins().fcmp(FloatCC::LessThan, ls_c_n, ls_max_n);
    builder
        .ins()
        .brif(ls_ok_n, ls_body_n, &[], ls_fail_n, &[]);
    builder.switch_to_block(ls_body_n);
    for i in 0..n {
        let off = dx_offset + (i * 8) as i32;
        let off_val = builder.ins().iconst(ptr_type, off as i64);
        let addr = builder.ins().iadd(base_ptr, off_val);
        let dxi = builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0);
        let xi_orig = builder.ins().stack_load(cl_types::F64, x_save_slots[i], 0);
        let scaled = builder.ins().fmul(ls_a_n, dxi);
        let xi_new = builder.ins().fadd(xi_orig, scaled);
        builder.ins().stack_store(xi_new, slots[i], 0);
    }
    let mut ls_max_abs_n = builder.ins().f64const(0.0);
    for i in 0..n {
        let rv = compile_expression(&residuals[i], ctx, builder)?;
        let arv = builder.ins().fabs(rv);
        ls_max_abs_n = builder.ins().fmax(ls_max_abs_n, arv);
    }
    let ls_old_n = builder.ins().stack_load(cl_types::F64, ls_old_norm_slot, 0);
    let ls_better_n = builder.ins().fcmp(FloatCC::LessThan, ls_max_abs_n, ls_old_n);
    builder
        .ins()
        .brif(ls_better_n, ls_accept_n, &[], ls_halve_n, &[]);
    builder.switch_to_block(ls_halve_n);
    let half_n = builder.ins().f64const(0.5);
    let new_a_n = builder.ins().fmul(ls_a_n, half_n);
    builder.ins().stack_store(new_a_n, ls_alpha_slot_n, 0);
    let one_ls_n = builder.ins().f64const(1.0);
    let new_c_n = builder.ins().fadd(ls_c_n, one_ls_n);
    builder.ins().stack_store(new_c_n, ls_count_slot_n, 0);
    builder.ins().jump(ls_hdr_n, &[]);
    builder.seal_block(ls_halve_n);
    builder.switch_to_block(ls_fail_n);
    for i in 0..n {
        let off = dx_offset + (i * 8) as i32;
        let off_val = builder.ins().iconst(ptr_type, off as i64);
        let addr = builder.ins().iadd(base_ptr, off_val);
        let dxi = builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0);
        let xi_orig = builder.ins().stack_load(cl_types::F64, x_save_slots[i], 0);
        let xi_new = builder.ins().fadd(xi_orig, dxi);
        builder.ins().stack_store(xi_new, slots[i], 0);
    }
    let one_fb = builder.ins().f64const(1.0);
    let next_iter_fb = builder.ins().fadd(iter_val, one_fb);
    builder.ins().stack_store(next_iter_fb, iter_slot, 0);
    builder.ins().jump(header_block, &[]);
    builder.seal_block(ls_fail_n);
    builder.switch_to_block(ls_accept_n);
    let one = builder.ins().f64const(1.0);
    let next_iter = builder.ins().fadd(iter_val, one);
    builder.ins().stack_store(next_iter, iter_slot, 0);
    builder.ins().jump(header_block, &[]);
    builder.seal_block(ls_body_n);
    builder.seal_block(ls_hdr_n);
    builder.seal_block(ls_accept_n);
    builder.seal_block(update_block);
    builder.seal_block(header_block);
    builder.seal_block(body_block);
    builder.seal_block(perturb_block);
    builder.switch_to_block(solve_error_block);
    {
        let sd_scale = builder.ins().f64const(1e-4);
        for i in 0..n {
            let xi = builder.ins().stack_load(cl_types::F64, slots[i], 0);
            let ri = compile_expression(&residuals[i], ctx, builder)?;
            let step = builder.ins().fmul(ri, sd_scale);
            let xi_new = builder.ins().fsub(xi, step);
            builder.ins().stack_store(xi_new, slots[i], 0);
        }
        let one_sd = builder.ins().f64const(1.0);
        let next_sd = builder.ins().fadd(iter_val, one_sd);
        builder.ins().stack_store(next_sd, iter_slot, 0);
        builder.ins().jump(header_block, &[]);
    }
    builder.seal_block(solve_error_block);
    builder.switch_to_block(exit_block);
    emit_assert_suppress_end(ctx, builder)?;
    for (var, slot) in unknowns.iter().take(n).zip(slots.iter()) {
        let val = builder.ins().stack_load(cl_types::F64, *slot, 0);
        if let Some(idx) = ctx.output_index(var) {
            let offset = (idx * 8) as i32;
            builder
                .ins()
                .store(MemFlags::new(), val, ctx.outputs_ptr, offset);
        }
    }
    builder.ins().jump(after_dense_n, &[]);
    builder.seal_block(exit_block);
    builder.switch_to_block(after_dense_n);
    Ok(())
}
