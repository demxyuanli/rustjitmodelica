use crate::analysis::contains_var;
use crate::ast::Expression;
use cranelift::codegen::ir::StackSlot;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};

use crate::jit::context::TranslationContext;
use crate::jit::translator::expr::compile_expression;

pub(super) fn emit_assert_suppress_begin(
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    let sig = ctx.module.make_signature();
    let func_id = ctx
        .module
        .declare_function("rustmodlica_assert_suppress_begin", Linkage::Import, &sig)
        .map_err(|e| e.to_string())?;
    let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
    builder.ins().call(func_ref, &[]);
    Ok(())
}

pub(super) fn emit_assert_suppress_end(
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    let sig = ctx.module.make_signature();
    let func_id = ctx
        .module
        .declare_function("rustmodlica_assert_suppress_end", Linkage::Import, &sig)
        .map_err(|e| e.to_string())?;
    let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
    builder.ins().call(func_ref, &[]);
    Ok(())
}

pub(super) fn compile_solvable_block_general_n(
    unknowns: &[String],
    residuals: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    let n = residuals.len();
    let slots: Vec<_> = unknowns
        .iter()
        .take(n)
        .map(|v| -> Result<_, String> {
            Ok(*ctx
                .stack_slots
                .get(v)
                .ok_or_else(|| format!("SolvableBlock unknown {} missing stack slot", v))?)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for v in unknowns.iter().take(n) {
        ctx.var_map.remove(v);
    }
    for (var, slot) in unknowns.iter().take(n).zip(slots.iter()) {
        if let Some(idx) = ctx.output_index(var) {
            let offset = (idx * 8) as i32;
            let init_val =
                builder
                    .ins()
                    .load(cl_types::F64, MemFlags::new(), ctx.outputs_ptr, offset);
            builder.ins().stack_store(init_val, *slot, 0);
        }
    }
    if let Some(pattern) = build_sparse_jacobian_pattern(&unknowns[..n], residuals) {
        return compile_solvable_block_general_sparse_n(
            unknowns,
            residuals,
            &slots,
            &pattern,
            ctx,
            builder,
        );
    }
    compile_solvable_block_general_dense_n(unknowns, residuals, &slots, ctx, builder)
}

#[derive(Debug, Clone)]
struct SparseJacobianPattern {
    row_ptr: Vec<i32>,
    col_idx: Vec<i32>,
    entries: Vec<(usize, usize)>,
}

fn build_sparse_jacobian_pattern(
    unknowns: &[String],
    residuals: &[Expression],
) -> Option<SparseJacobianPattern> {
    let n = residuals.len();
    if n < 3 || unknowns.len() < n {
        return None;
    }

    let mut row_ptr = Vec::with_capacity(n + 1);
    let mut col_idx = Vec::new();
    let mut entries = Vec::new();
    row_ptr.push(0);

    for residual in residuals {
        let row_start = col_idx.len();
        for (col, unknown) in unknowns.iter().take(n).enumerate() {
            if contains_var(residual, unknown) {
                col_idx.push(col as i32);
                entries.push((row_ptr.len() - 1, col));
            }
        }
        if col_idx.len() == row_start {
            return None;
        }
        row_ptr.push(col_idx.len() as i32);
    }

    let nnz = col_idx.len();
    if nnz == 0 || nnz >= n * n || nnz * 4 > n * n * 3 {
        return None;
    }

    Some(SparseJacobianPattern {
        row_ptr,
        col_idx,
        entries,
    })
}

fn compile_solvable_block_general_dense_n(
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
    let max_iter = builder.ins().f64const(100.0);
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
    emit_assert_suppress_end(ctx, builder)?;
    let solve_err = builder.ins().iconst(cl_types::I32, 2);
    builder.ins().return_(&[solve_err]);
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

fn compile_solvable_block_general_sparse_n(
    unknowns: &[String],
    residuals: &[Expression],
    slots: &[StackSlot],
    pattern: &SparseJacobianPattern,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    let n = residuals.len();
    let nnz = pattern.entries.len();
    let ptr_type = ctx.module.target_config().pointer_type();
    let row_ptr_bytes = pattern.row_ptr.len() * 4;
    let col_idx_bytes = pattern.col_idx.len() * 4;
    let values_offset = row_ptr_bytes + col_idx_bytes;
    let r_offset = values_offset + nnz * 8;
    let dx_offset = r_offset + n * 8;
    let buf_size = dx_offset + n * 8;
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
    let base_ptr = builder.ins().stack_addr(ptr_type, buf_slot, 0);

    for (idx, row) in pattern.row_ptr.iter().enumerate() {
        let off_val = builder.ins().iconst(ptr_type, (idx * 4) as i64);
        let addr = builder.ins().iadd(base_ptr, off_val);
        let row_val = builder.ins().iconst(cl_types::I32, i64::from(*row));
        builder.ins().store(MemFlags::new(), row_val, addr, 0);
    }
    for (idx, col) in pattern.col_idx.iter().enumerate() {
        let off_val = builder
            .ins()
            .iconst(ptr_type, (row_ptr_bytes + idx * 4) as i64);
        let addr = builder.ins().iadd(base_ptr, off_val);
        let col_val = builder.ins().iconst(cl_types::I32, i64::from(*col));
        builder.ins().store(MemFlags::new(), col_val, addr, 0);
    }

    let eps_val = builder.ins().f64const(1e-6);
    let header_block = builder.create_block();
    let body_block = builder.create_block();
    let exit_block = builder.create_block();
    let iter_error_block = builder.create_block();
    let solve_error_block = builder.create_block();
    let after_sparse_n = builder.create_block();
    emit_assert_suppress_begin(ctx, builder)?;
    builder.ins().jump(header_block, &[]);
    builder.switch_to_block(header_block);
    let iter_val = builder.ins().stack_load(cl_types::F64, iter_slot, 0);
    let max_iter = builder.ins().f64const(100.0);
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

    let row_ptr_ptr = base_ptr;
    let col_idx_offset_val = builder.ins().iconst(ptr_type, row_ptr_bytes as i64);
    let col_idx_ptr = builder.ins().iadd(base_ptr, col_idx_offset_val);
    let values_offset_val = builder.ins().iconst(ptr_type, values_offset as i64);
    let values_ptr = builder.ins().iadd(base_ptr, values_offset_val);
    let r_offset_val = builder.ins().iconst(ptr_type, r_offset as i64);
    let r_ptr = builder.ins().iadd(base_ptr, r_offset_val);
    let dx_offset_val = builder.ins().iconst(ptr_type, dx_offset as i64);
    let dx_ptr = builder.ins().iadd(base_ptr, dx_offset_val);

    let mut r_vals = Vec::with_capacity(n);
    for (i, residual) in residuals.iter().enumerate() {
        let rv = compile_expression(residual, ctx, builder)?;
        r_vals.push(rv);
        let off_val = builder
            .ins()
            .iconst(ptr_type, (r_offset + i * 8) as i64);
        let addr = builder.ins().iadd(base_ptr, off_val);
        builder.ins().store(MemFlags::new(), rv, addr, 0);
    }

    let tol = builder.ins().f64const(1e-8);
    let mut max_abs = builder.ins().f64const(0.0);
    for rv in &r_vals {
        let abs_res = builder.ins().fabs(*rv);
        max_abs = builder.ins().fmax(max_abs, abs_res);
    }
    let perturb_block = builder.create_block();
    let converged = builder.ins().fcmp(FloatCC::LessThan, max_abs, tol);
    builder
        .ins()
        .brif(converged, exit_block, &[], perturb_block, &[]);
    builder.switch_to_block(perturb_block);

    for (entry_idx, (row, col)) in pattern.entries.iter().enumerate() {
        let x_col = builder.ins().stack_load(cl_types::F64, slots[*col], 0);
        let x_col_perturbed = builder.ins().fadd(x_col, eps_val);
        builder.ins().stack_store(x_col_perturbed, slots[*col], 0);
        let rp = compile_expression(&residuals[*row], ctx, builder)?;
        let dr = builder.ins().fsub(rp, r_vals[*row]);
        let jac = builder.ins().fdiv(dr, eps_val);
        let off_val = builder
            .ins()
            .iconst(ptr_type, (values_offset + entry_idx * 8) as i64);
        let addr = builder.ins().iadd(base_ptr, off_val);
        builder.ins().store(MemFlags::new(), jac, addr, 0);
        builder.ins().stack_store(x_col, slots[*col], 0);
    }

    let mut sig = ctx.module.make_signature();
    sig.params.push(AbiParam::new(cl_types::I32));
    sig.params.push(AbiParam::new(cl_types::I32));
    sig.params.push(AbiParam::new(ptr_type));
    sig.params.push(AbiParam::new(ptr_type));
    sig.params.push(AbiParam::new(ptr_type));
    sig.params.push(AbiParam::new(ptr_type));
    sig.params.push(AbiParam::new(ptr_type));
    sig.returns.push(AbiParam::new(cl_types::I32));
    let func_id = ctx
        .module
        .declare_function("rustmodlica_solve_linear_csr", Linkage::Import, &sig)
        .map_err(|e| e.to_string())?;
    let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
    let n_i32 = builder.ins().iconst(cl_types::I32, n as i64);
    let nnz_i32 = builder.ins().iconst(cl_types::I32, nnz as i64);
    let solve_result = builder.ins().call(
        func_ref,
        &[
            n_i32,
            nnz_i32,
            row_ptr_ptr,
            col_idx_ptr,
            values_ptr,
            r_ptr,
            dx_ptr,
        ],
    );
    let status = builder.inst_results(solve_result)[0];
    let zero_i32 = builder.ins().iconst(cl_types::I32, 0);
    let status_ok = builder.ins().icmp(IntCC::Equal, status, zero_i32);
    let update_block = builder.create_block();
    builder
        .ins()
        .brif(status_ok, update_block, &[], solve_error_block, &[]);
    builder.switch_to_block(update_block);

    let ls_alpha_slot_s = builder.create_sized_stack_slot(
        cranelift::codegen::ir::StackSlotData::new(
            cranelift::codegen::ir::StackSlotKind::ExplicitSlot, 8, 0,
        ),
    );
    let ls_count_slot_s = builder.create_sized_stack_slot(
        cranelift::codegen::ir::StackSlotData::new(
            cranelift::codegen::ir::StackSlotKind::ExplicitSlot, 8, 0,
        ),
    );
    let ls_old_norm_s = builder.create_sized_stack_slot(
        cranelift::codegen::ir::StackSlotData::new(
            cranelift::codegen::ir::StackSlotKind::ExplicitSlot, 8, 0,
        ),
    );
    let x_save_s: Vec<_> = (0..n)
        .map(|_| {
            builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
                cranelift::codegen::ir::StackSlotKind::ExplicitSlot, 8, 0,
            ))
        })
        .collect();
    builder.ins().stack_store(max_abs, ls_old_norm_s, 0);
    for (i, slot) in slots.iter().enumerate().take(n) {
        let xi = builder.ins().stack_load(cl_types::F64, *slot, 0);
        builder.ins().stack_store(xi, x_save_s[i], 0);
    }
    let ls_init_a_s = builder.ins().f64const(1.0);
    builder.ins().stack_store(ls_init_a_s, ls_alpha_slot_s, 0);
    let ls_init_c_s = builder.ins().f64const(0.0);
    builder.ins().stack_store(ls_init_c_s, ls_count_slot_s, 0);
    let ls_hdr_s = builder.create_block();
    let ls_body_s = builder.create_block();
    let ls_accept_s = builder.create_block();
    let ls_halve_s = builder.create_block();
    let ls_fail_s = builder.create_block();
    builder.ins().jump(ls_hdr_s, &[]);
    builder.switch_to_block(ls_hdr_s);
    let ls_a_s = builder.ins().stack_load(cl_types::F64, ls_alpha_slot_s, 0);
    let ls_c_s = builder.ins().stack_load(cl_types::F64, ls_count_slot_s, 0);
    let ls_max_s = builder.ins().f64const(8.0);
    let ls_ok_s = builder.ins().fcmp(FloatCC::LessThan, ls_c_s, ls_max_s);
    builder
        .ins()
        .brif(ls_ok_s, ls_body_s, &[], ls_fail_s, &[]);
    builder.switch_to_block(ls_body_s);
    for (i, slot) in slots.iter().enumerate().take(n) {
        let off_val = builder
            .ins()
            .iconst(ptr_type, (dx_offset + i * 8) as i64);
        let addr = builder.ins().iadd(base_ptr, off_val);
        let dxi = builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0);
        let xi_orig = builder.ins().stack_load(cl_types::F64, x_save_s[i], 0);
        let scaled = builder.ins().fmul(ls_a_s, dxi);
        let xi_new = builder.ins().fadd(xi_orig, scaled);
        builder.ins().stack_store(xi_new, *slot, 0);
    }
    let mut ls_max_abs_s = builder.ins().f64const(0.0);
    for i in 0..n {
        let rv = compile_expression(&residuals[i], ctx, builder)?;
        let arv = builder.ins().fabs(rv);
        ls_max_abs_s = builder.ins().fmax(ls_max_abs_s, arv);
    }
    let ls_old_s = builder.ins().stack_load(cl_types::F64, ls_old_norm_s, 0);
    let ls_better_s = builder.ins().fcmp(FloatCC::LessThan, ls_max_abs_s, ls_old_s);
    builder
        .ins()
        .brif(ls_better_s, ls_accept_s, &[], ls_halve_s, &[]);
    builder.switch_to_block(ls_halve_s);
    let half_s = builder.ins().f64const(0.5);
    let new_a_s = builder.ins().fmul(ls_a_s, half_s);
    builder.ins().stack_store(new_a_s, ls_alpha_slot_s, 0);
    let one_ls_s = builder.ins().f64const(1.0);
    let new_c_s = builder.ins().fadd(ls_c_s, one_ls_s);
    builder.ins().stack_store(new_c_s, ls_count_slot_s, 0);
    builder.ins().jump(ls_hdr_s, &[]);
    builder.seal_block(ls_halve_s);
    builder.switch_to_block(ls_fail_s);
    for (i, slot) in slots.iter().enumerate().take(n) {
        let off_val = builder
            .ins()
            .iconst(ptr_type, (dx_offset + i * 8) as i64);
        let addr = builder.ins().iadd(base_ptr, off_val);
        let dxi = builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0);
        let xi_orig = builder.ins().stack_load(cl_types::F64, x_save_s[i], 0);
        let xi_new = builder.ins().fadd(xi_orig, dxi);
        builder.ins().stack_store(xi_new, *slot, 0);
    }
    let one_fb_s = builder.ins().f64const(1.0);
    let next_iter_fb_s = builder.ins().fadd(iter_val, one_fb_s);
    builder.ins().stack_store(next_iter_fb_s, iter_slot, 0);
    builder.ins().jump(header_block, &[]);
    builder.seal_block(ls_fail_s);
    builder.switch_to_block(ls_accept_s);
    let one = builder.ins().f64const(1.0);
    let next_iter = builder.ins().fadd(iter_val, one);
    builder.ins().stack_store(next_iter, iter_slot, 0);
    builder.ins().jump(header_block, &[]);
    builder.seal_block(ls_body_s);
    builder.seal_block(ls_hdr_s);
    builder.seal_block(ls_accept_s);
    builder.seal_block(update_block);
    builder.seal_block(header_block);
    builder.seal_block(body_block);
    builder.seal_block(perturb_block);
    builder.switch_to_block(solve_error_block);
    emit_assert_suppress_end(ctx, builder)?;
    let solve_err_csr = builder.ins().iconst(cl_types::I32, 2);
    builder.ins().return_(&[solve_err_csr]);
    builder.seal_block(solve_error_block);
    builder.switch_to_block(exit_block);
    emit_assert_suppress_end(ctx, builder)?;
    for (var, slot) in unknowns.iter().take(n).zip(slots) {
        let val = builder.ins().stack_load(cl_types::F64, *slot, 0);
        if let Some(idx) = ctx.output_index(var) {
            let offset = (idx * 8) as i32;
            builder
                .ins()
                .store(MemFlags::new(), val, ctx.outputs_ptr, offset);
        }
    }
    builder.ins().jump(after_sparse_n, &[]);
    builder.seal_block(exit_block);
    builder.switch_to_block(after_sparse_n);
    Ok(())
}
