use crate::analysis::build_solvable_block_sparse_pattern;
use crate::ast::Expression;
use cranelift::codegen::ir::StackSlot;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};

use crate::jit::context::TranslationContext;
use crate::jit::translator::expr::compile_expression;
use crate::solvable_limits::{
    newton_sparse_policy_from_env, should_use_newton_sparse_path, validate_solvable_residual_count,
    JIT_STACK_BUFFER_BYTES_MAX,
};

use super::solvable_assert::{emit_assert_suppress_begin, emit_assert_suppress_end};
use super::solvable::SymbolicJacobianPlan;

fn sparse_debug_enabled() -> bool {
    use std::sync::OnceLock;
    static CACHED: OnceLock<bool> = OnceLock::new();
    *CACHED.get_or_init(|| {
        std::env::var("RUSTMODLICA_NEWTON_SPARSE_DEBUG")
            .ok()
            .map(|v| {
                let t = v.trim().to_ascii_lowercase();
                t == "1" || t == "true" || t == "on" || t == "yes"
            })
            .unwrap_or(false)
    })
}

fn align_up(v: usize, align: usize) -> usize {
    debug_assert!(align.is_power_of_two());
    (v + (align - 1)) & !(align - 1)
}

#[derive(Debug, Clone)]
pub(super) struct SparseJacobianPattern {
    row_ptr: Vec<i32>,
    col_idx: Vec<i32>,
    entries: Vec<(usize, usize)>,
}

impl SparseJacobianPattern {
    fn from_analysis_pattern(p: crate::analysis::SolvableBlockSparsePattern) -> Self {
        Self {
            row_ptr: p.row_ptr,
            col_idx: p.col_idx,
            entries: p.entries,
        }
    }

    pub(super) fn nnz(&self) -> usize {
        self.entries.len()
    }

    pub(super) fn density(&self, n: usize) -> f64 {
        let total = n.saturating_mul(n);
        if total == 0 {
            0.0
        } else {
            self.entries.len() as f64 / total as f64
        }
    }
}

pub(crate) fn solvable_block_uses_sparse_jacobian_path(
    unknowns: &[String],
    residuals: &[Expression],
) -> bool {
    build_sparse_jacobian_pattern(unknowns, residuals).is_some()
}

pub(super) fn build_sparse_jacobian_pattern(
    unknowns: &[String],
    residuals: &[Expression],
) -> Option<SparseJacobianPattern> {
    let policy = newton_sparse_policy_from_env();
    let n = residuals.len();
    if n < 3 || unknowns.len() < n {
        return None;
    }
    // Prefer shared analysis-stage sparsity metadata builder to keep backend/JIT aligned.
    let pattern = build_solvable_block_sparse_pattern(unknowns, residuals)
        .map(SparseJacobianPattern::from_analysis_pattern)?;

    let nnz = pattern.nnz();
    if !should_use_newton_sparse_path(policy, n, nnz, unknowns.len()) {
        return None;
    }
    if sparse_debug_enabled() {
        eprintln!(
            "[newton-sparse] select pattern n={} nnz={} density={:.2}% policy={:?}",
            n,
            nnz,
            pattern.density(n) * 100.0,
            policy
        );
    }
    Some(pattern)
}

pub(super) fn compile_solvable_block_general_sparse_n(
    unknowns: &[String],
    residuals: &[Expression],
    slots: &[StackSlot],
    symbolic_plan: &SymbolicJacobianPlan,
    pattern: &SparseJacobianPattern,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    let n = residuals.len();
    validate_solvable_residual_count(n)?;
    let nnz = pattern.entries.len();
    let ptr_type = ctx.module.target_config().pointer_type();
    let row_ptr_bytes = pattern.row_ptr.len() * 4;
    let col_idx_bytes = pattern.col_idx.len() * 4;
    // values/r/dx are f64 buffers and must stay 8-byte aligned.
    let values_offset = align_up(row_ptr_bytes + col_idx_bytes, 8);
    let r_offset = align_up(values_offset + nnz * 8, 8);
    let dx_offset = align_up(r_offset + n * 8, 8);
    let buf_size = dx_offset + n * 8;
    let use_heap_workspace = buf_size > JIT_STACK_BUFFER_BYTES_MAX;
    let buf_slot_opt = if use_heap_workspace {
        None
    } else {
        Some(builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
            cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
            buf_size as u32,
            0,
        )))
    };
    let iter_slot = builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
        cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
        8,
        0,
    ));
    let zero = builder.ins().f64const(0.0);
    builder.ins().stack_store(zero, iter_slot, 0);
    let base_ptr = if let Some(buf_slot) = buf_slot_opt {
        builder.ins().stack_addr(ptr_type, buf_slot, 0)
    } else {
        let bsz = i32::try_from(buf_size).map_err(|_| {
            "sparse Newton workspace size exceeds i32 (reduce SolvableBlock size)".to_string()
        })?;
        let bsz_val = builder.ins().iconst(cl_types::I32, i64::from(bsz));
        let mut sig_b = ctx.module.make_signature();
        sig_b.params.push(AbiParam::new(cl_types::I32));
        sig_b.returns.push(AbiParam::new(ptr_type));
        let fid_b = ctx
            .module
            .declare_function("rustmodlica_jit_workspace_bytes", Linkage::Import, &sig_b)
            .map_err(|e| e.to_string())?;
        let fr_b = ctx.module.declare_func_in_func(fid_b, &mut builder.func);
        let call_b = builder.ins().call(fr_b, &[bsz_val]);
        builder.inst_results(call_b)[0]
    };

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

    let row_ptr_ptr = base_ptr;
    let col_idx_offset_val = builder.ins().iconst(ptr_type, row_ptr_bytes as i64);
    let col_idx_ptr = builder.ins().iadd(base_ptr, col_idx_offset_val);
    let values_offset_val = builder.ins().iconst(ptr_type, values_offset as i64);
    let values_ptr = builder.ins().iadd(base_ptr, values_offset_val);
    let r_offset_val = builder.ins().iconst(ptr_type, r_offset as i64);
    let r_ptr = builder.ins().iadd(base_ptr, r_offset_val);
    let dx_offset_val = builder.ins().iconst(ptr_type, dx_offset as i64);
    let dx_ptr = builder.ins().iadd(base_ptr, dx_offset_val);

    let value_ptrs: Vec<Value> = (0..nnz)
        .map(|entry_idx| {
            let off_val = builder
                .ins()
                .iconst(ptr_type, (values_offset + entry_idx * 8) as i64);
            builder.ins().iadd(base_ptr, off_val)
        })
        .collect();

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
        let jac = if let Some(d_expr) = symbolic_plan.get(*row, *col) {
            compile_expression(d_expr, ctx, builder)?
        } else {
            let rp = compile_expression(&residuals[*row], ctx, builder)?;
            let dr = builder.ins().fsub(rp, r_vals[*row]);
            builder.ins().fdiv(dr, eps_val)
        };
        builder
            .ins()
            .store(MemFlags::new(), jac, value_ptrs[entry_idx], 0);
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
    builder.seal_block(header_block);
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
