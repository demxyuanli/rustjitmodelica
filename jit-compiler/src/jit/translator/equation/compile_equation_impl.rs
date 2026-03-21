use crate::analysis::collect_vars_expr;
use crate::ast::{Equation, Expression};
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};
use std::collections::HashSet;

use crate::jit::context::TranslationContext;
use crate::jit::translator::expr::compile_expression;
use crate::jit::types::ArrayType;

use super::solvable::{
    compile_solvable_block_general_n, emit_assert_suppress_begin, emit_assert_suppress_end,
};

pub fn compile_equation(
    eq: &Equation,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    match eq {
        Equation::CallStmt(_) => {}
        Equation::Simple(lhs, rhs) => {
            if let Expression::ArrayAccess(arr_expr, idx_expr) = lhs {
                if let Expression::Variable(name) = &**arr_expr {
                    let val = compile_expression(rhs, ctx, builder)?;
                    if let Some((array_type, start_index)) = ctx.array_storage(name) {
                        let idx_val = compile_expression(idx_expr, ctx, builder)?;
                        let one = builder.ins().f64const(1.0);
                        let idx_0 = builder.ins().fsub(idx_val, one);
                        let idx_int = builder.ins().fcvt_to_sint(cl_types::I64, idx_0);
                        let eight = builder.ins().iconst(cl_types::I64, 8);
                        let offset_bytes = builder.ins().imul(idx_int, eight);
                        let start_offset = (start_index * 8) as i64;
                        let start_const = builder.ins().iconst(cl_types::I64, start_offset);
                        let total_offset = builder.ins().iadd(start_const, offset_bytes);
                        let base_ptr = match array_type {
                            ArrayType::State => ctx.states_ptr,
                            ArrayType::Discrete => ctx.discrete_ptr,
                            ArrayType::Parameter => ctx.params_ptr,
                            ArrayType::Output => ctx.outputs_ptr,
                            ArrayType::Derivative => ctx.derivs_ptr,
                        };
                        let addr = builder.ins().iadd(base_ptr, total_offset);
                        builder.ins().store(MemFlags::new(), val, addr, 0);
                    } else {
                        if let Expression::Number(n) = &**idx_expr {
                            let elem_name = format!("{}_{}", name, *n as i64);
                            if let Some(slot) = ctx.stack_slots.get(&elem_name) {
                                builder.ins().stack_store(val, *slot, 0);
                            } else {
                                ctx.var_map.insert(elem_name.clone(), val);
                            }
                            if let Some(idx) = ctx.output_index(&elem_name) {
                                let offset = (idx * 8) as i32;
                                builder
                                    .ins()
                                    .store(MemFlags::new(), val, ctx.outputs_ptr, offset);
                            }
                            if let Some(idx) = ctx.discrete_index(&elem_name) {
                                let offset = (idx * 8) as i32;
                                builder
                                    .ins()
                                    .store(MemFlags::new(), val, ctx.discrete_ptr, offset);
                            }
                            if let Some(idx) = ctx.param_index(&elem_name) {
                                let offset = (idx * 8) as i32;
                                builder
                                    .ins()
                                    .store(MemFlags::new(), val, ctx.params_ptr, offset);
                            }
                        } else {
                            return Err(format!("Array {} not found in array_info", name));
                        }
                    }
                }
            } else if let Expression::Variable(var_name) = lhs {
                let val = compile_expression(rhs, ctx, builder)?;
                if let Some(slot) = ctx.stack_slots.get(var_name) {
                    builder.ins().stack_store(val, *slot, 0);
                } else {
                    ctx.var_map.insert(var_name.clone(), val);
                }
                if let Some(state_name) = var_name.strip_prefix("der_") {
                    if let Some(idx) = ctx.state_index(state_name) {
                        let offset = (idx * 8) as i32;
                        builder
                            .ins()
                            .store(MemFlags::new(), val, ctx.derivs_ptr, offset);
                    }
                }
                if let Some(idx) = ctx.output_index(var_name) {
                    let offset = (idx * 8) as i32;
                    builder
                        .ins()
                        .store(MemFlags::new(), val, ctx.outputs_ptr, offset);
                }
                if let Some(idx) = ctx.discrete_index(var_name) {
                    let offset = (idx * 8) as i32;
                    builder
                        .ins()
                        .store(MemFlags::new(), val, ctx.discrete_ptr, offset);
                }
            } else if let Expression::Der(arg) = lhs {
                if let Expression::Variable(var_name) = &**arg {
                    if let Some(idx) = ctx.state_index(var_name) {
                        let val = compile_expression(rhs, ctx, builder)?;
                        let offset = (idx * 8) as i32;
                        builder
                            .ins()
                            .store(MemFlags::new(), val, ctx.derivs_ptr, offset);
                    }
                } else if let Expression::ArrayAccess(arr_expr, idx_expr) = &**arg {
                    if let Expression::Variable(name) = &**arr_expr {
                        if let Some((array_type, start_index)) = ctx.array_storage(name) {
                            if matches!(array_type, ArrayType::State) {
                                let val = compile_expression(rhs, ctx, builder)?;
                                let idx_val = compile_expression(idx_expr, ctx, builder)?;
                                let one = builder.ins().f64const(1.0);
                                let idx_0 = builder.ins().fsub(idx_val, one);
                                let idx_int = builder.ins().fcvt_to_sint(cl_types::I64, idx_0);
                                let eight = builder.ins().iconst(cl_types::I64, 8);
                                let offset_bytes = builder.ins().imul(idx_int, eight);
                                let start_offset = (start_index * 8) as i64;
                                let start_const = builder.ins().iconst(cl_types::I64, start_offset);
                                let total_offset = builder.ins().iadd(start_const, offset_bytes);
                                let addr = builder.ins().iadd(ctx.derivs_ptr, total_offset);
                                builder.ins().store(MemFlags::new(), val, addr, 0);
                            }
                        }
                    }
                }
            }
        }
        Equation::For(loop_var, start_expr, end_expr, body) => {
            // Loop var and bounds: slot allocated in analysis; bounds may be non-const (runtime).
            let start_val = compile_expression(start_expr, ctx, builder)?;
            let end_val = compile_expression(end_expr, ctx, builder)?;
            let step_val = builder.ins().f64const(1.0);
            let loop_var_slot = if let Some(slot) = ctx.stack_slots.get(loop_var) {
                *slot
            } else {
                return Err(format!(
                    "Loop variable '{}' stack slot not found.",
                    loop_var
                ));
            };
            builder.ins().stack_store(start_val, loop_var_slot, 0);
            let header_block = builder.create_block();
            let body_block = builder.create_block();
            let exit_block = builder.create_block();
            let after_for = builder.create_block();
            builder.ins().jump(header_block, &[]);
            builder.switch_to_block(header_block);
            let curr_i = builder.ins().stack_load(cl_types::F64, loop_var_slot, 0);
            let cmp = builder
                .ins()
                .fcmp(FloatCC::LessThanOrEqual, curr_i, end_val);
            builder.ins().brif(cmp, body_block, &[], exit_block, &[]);
            builder.switch_to_block(body_block);
            for sub_eq in body {
                compile_equation(sub_eq, ctx, builder)?;
            }
            let curr_i_2 = builder.ins().stack_load(cl_types::F64, loop_var_slot, 0);
            let next_i = builder.ins().fadd(curr_i_2, step_val);
            builder.ins().stack_store(next_i, loop_var_slot, 0);
            builder.ins().jump(header_block, &[]);
            builder.seal_block(body_block);
            builder.switch_to_block(exit_block);
            builder.ins().jump(after_for, &[]);
            builder.seal_block(header_block);
            builder.seal_block(exit_block);
            builder.switch_to_block(after_for);
        }
        Equation::SolvableBlock {
            unknowns,
            tearing_var,
            equations: inner_eqs,
            residuals,
        } => {
            if residuals.len() >= 2
                && residuals.len() <= 32
                && unknowns.len() == residuals.len()
            {
                compile_solvable_block_general_n(unknowns, residuals, ctx, builder)?;
            } else if residuals.len() == 2 && unknowns.len() >= 2 {
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
                if let Some(idx) = ctx.output_index(v0) {
                    let offset = (idx * 8) as i32;
                    let init0 =
                        builder
                            .ins()
                            .load(cl_types::F64, MemFlags::new(), ctx.outputs_ptr, offset);
                    builder.ins().stack_store(init0, slot0, 0);
                }
                if let Some(idx) = ctx.output_index(v1) {
                    let offset = (idx * 8) as i32;
                    let init1 =
                        builder
                            .ins()
                            .load(cl_types::F64, MemFlags::new(), ctx.outputs_ptr, offset);
                    builder.ins().stack_store(init1, slot1, 0);
                }
                ctx.var_map.remove(v0);
                ctx.var_map.remove(v1);
                let iter_slot =
                    builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
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
                let still_bad =
                    builder.ins().fcmp(FloatCC::LessThan, det_d_abs, min_det_d);
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
                for (var, slot) in [(v0, slot0), (v1, slot1)] {
                    let val = builder.ins().stack_load(cl_types::F64, slot, 0);
                    if let Some(idx) = ctx.output_index(var) {
                        let offset = (idx * 8) as i32;
                        builder
                            .ins()
                            .store(MemFlags::new(), val, ctx.outputs_ptr, offset);
                    }
                }
                builder.ins().jump(after_newton_2, &[]);
                builder.seal_block(exit_block);
                builder.switch_to_block(after_newton_2);
            } else if residuals.len() == 3 && unknowns.len() >= 3 {
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
                    if let Some(idx) = ctx.output_index(var) {
                        let offset = (idx * 8) as i32;
                        let init_val = builder.ins().load(
                            cl_types::F64,
                            MemFlags::new(),
                            ctx.outputs_ptr,
                            offset,
                        );
                        builder.ins().stack_store(init_val, slot, 0);
                    }
                }
                ctx.var_map.remove(v0);
                ctx.var_map.remove(v1);
                ctx.var_map.remove(v2);
                let iter_slot =
                    builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
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
                let lm3_buf = builder.create_sized_stack_slot(
                    cranelift::codegen::ir::StackSlotData::new(
                        cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
                        lm3_buf_size as u32,
                        0,
                    ),
                );
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
                    let off =
                        builder.ins().iconst(ptr_type_3, (r3_base + idx * 8) as i64);
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
                let func_ref_3 =
                    ctx.module.declare_func_in_func(func_id_3, &mut builder.func);
                let solve_res_3 =
                    builder
                        .ins()
                        .call(func_ref_3, &[n_3, lm3_base, r3_ptr, dx3_ptr]);
                let solve_status_3 = builder.inst_results(solve_res_3)[0];
                let zero_i32_3 = builder.ins().iconst(cl_types::I32, 0);
                let solve_ok_3 =
                    builder.ins().icmp(IntCC::Equal, solve_status_3, zero_i32_3);
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
                    let off =
                        builder.ins().iconst(ptr_type_3, (dx3_base + i * 8) as i64);
                    let addr = builder.ins().iadd(lm3_base, off);
                    let dxi =
                        builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0);
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
                for (var, slot) in [(v0, slot0), (v1, slot1), (v2, slot2)] {
                    let val = builder.ins().stack_load(cl_types::F64, slot, 0);
                    if let Some(idx) = ctx.output_index(var) {
                        let offset = (idx * 8) as i32;
                        builder
                            .ins()
                            .store(MemFlags::new(), val, ctx.outputs_ptr, offset);
                    }
                }
                builder.ins().jump(after_newton_3, &[]);
                builder.seal_block(exit_block);
                builder.switch_to_block(after_newton_3);
            } else if residuals.len() >= 4
                && residuals.len() <= 32
                && unknowns.len() >= residuals.len()
            {
                compile_solvable_block_general_n(unknowns, residuals, ctx, builder)?;
            } else if (residuals.len() == 1 && (tearing_var.is_some() || !unknowns.is_empty()))
                || (residuals.len() >= 2 && residuals.len() <= 32 && unknowns.len() == 1)
            {
                let t_var = tearing_var
                    .as_ref()
                    .cloned()
                    .unwrap_or_else(|| unknowns[0].clone());
                {
                    ctx.var_map.remove(&t_var);
                    let t_slot = *ctx
                        .stack_slots
                        .get(&t_var)
                        .expect("Tearing var must have stack slot");
                    if let Some(idx) = ctx.output_index(&t_var) {
                        let offset = (idx * 8) as i32;
                        let init_val = builder.ins().load(
                            cl_types::F64,
                            MemFlags::new(),
                            ctx.outputs_ptr,
                            offset,
                        );
                        builder.ins().stack_store(init_val, t_slot, 0);
                    } else {
                        let zero = builder.ins().f64const(0.0);
                        builder.ins().stack_store(zero, t_slot, 0);
                    }
                    let iter_slot = builder.create_sized_stack_slot(
                        cranelift::codegen::ir::StackSlotData::new(
                            cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
                            8,
                            0,
                        ),
                    );
                    let jac_retry_slot = builder.create_sized_stack_slot(
                        cranelift::codegen::ir::StackSlotData::new(
                            cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
                            8,
                            0,
                        ),
                    );
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
                    let max_iter = builder.ins().f64const(100.0);
                    let iter_cond = builder.ins().fcmp(FloatCC::LessThan, iter_val, max_iter);
                    let error_block = builder.create_block();
                    builder
                        .ins()
                        .brif(iter_cond, body_block, &[], error_block, &[]);
                    builder.switch_to_block(error_block);
                    emit_assert_suppress_end(ctx, builder)?;
                    let error_code = builder.ins().iconst(cl_types::I32, 2);
                    builder.ins().return_(&[error_code]);
                    builder.seal_block(error_block);
                    builder.switch_to_block(body_block);
                    let x = builder.ins().stack_load(cl_types::F64, t_slot, 0);
                    for ieq in inner_eqs {
                        if let Equation::Simple(lhs, rhs) = ieq {
                            if let Expression::Variable(name) = lhs {
                                let val = compile_expression(rhs, ctx, builder)?;
                                if let Some(slot) = ctx.stack_slots.get(name) {
                                    builder.ins().stack_store(val, *slot, 0);
                                }
                            }
                        }
                    }
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
                    for ieq in inner_eqs {
                        if let Equation::Simple(lhs, rhs) = ieq {
                            if let Expression::Variable(name) = lhs {
                                let val = compile_expression(rhs, ctx, builder)?;
                                if let Some(slot) = ctx.stack_slots.get(name) {
                                    builder.ins().stack_store(val, *slot, 0);
                                }
                            }
                        }
                    }
                    let res_p = compile_expression(&residuals[0], ctx, builder)?;
                    let diff_res = builder.ins().fsub(res_p, res_val);
                    let j_val = builder.ins().fdiv(diff_res, epsilon);
                    // Guard against tiny Jacobian (ill-conditioned or flat residual).
                    let j_abs = builder.ins().fabs(j_val);
                    let j_min = builder.ins().f64const(1e-12);
                    let bad_jac = builder.ins().fcmp(FloatCC::LessThan, j_abs, j_min);
                    let jac_error_block = builder.create_block();
                    let update_block = builder.create_block();
                    builder
                        .ins()
                        .brif(bad_jac, jac_error_block, &[], update_block, &[]);
                    builder.switch_to_block(jac_error_block);
                    if let (Some(pr), Some(px)) = (ctx.diag_residual_ptr, ctx.diag_x_ptr) {
                        builder.ins().store(MemFlags::new(), res_val, pr, 0);
                        builder.ins().store(MemFlags::new(), x, px, 0);
                    }
                    let retry_val = builder.ins().stack_load(cl_types::F64, jac_retry_slot, 0);
                    let retry_limit = builder.ins().f64const(12.0);
                    let can_retry = builder.ins().fcmp(FloatCC::LessThan, retry_val, retry_limit);
                    let retry_block = builder.create_block();
                    let hard_fail_block = builder.create_block();
                    builder
                        .ins()
                        .brif(can_retry, retry_block, &[], hard_fail_block, &[]);
                    builder.switch_to_block(retry_block);
                    let nudge_count: usize = 12;
                    let nudge_table_slot = builder.create_sized_stack_slot(
                        cranelift::codegen::ir::StackSlotData::new(
                            cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
                            (nudge_count * 8) as u32,
                            0,
                        ),
                    );
                    let nudges: [f64; 12] = [
                        1e-4, -1e-4, 1e-3, -1e-3, 1e-2, -1e-2,
                        1e-1, -1e-1, 1.0, -1.0, 10.0, -10.0,
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
                    emit_assert_suppress_end(ctx, builder)?;
                    let error_code2 = builder.ins().iconst(cl_types::I32, 2);
                    builder.ins().return_(&[error_code2]);
                    builder.seal_block(hard_fail_block);
                    builder.seal_block(jac_error_block);
                    builder.switch_to_block(update_block);
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
                    let step_slot = builder.create_sized_stack_slot(
                        cranelift::codegen::ir::StackSlotData::new(
                            cranelift::codegen::ir::StackSlotKind::ExplicitSlot, 8, 0,
                        ),
                    );
                    let abs_res_slot = builder.create_sized_stack_slot(
                        cranelift::codegen::ir::StackSlotData::new(
                            cranelift::codegen::ir::StackSlotKind::ExplicitSlot, 8, 0,
                        ),
                    );
                    let x_save_slot = builder.create_sized_stack_slot(
                        cranelift::codegen::ir::StackSlotData::new(
                            cranelift::codegen::ir::StackSlotKind::ExplicitSlot, 8, 0,
                        ),
                    );
                    let alpha_slot = builder.create_sized_stack_slot(
                        cranelift::codegen::ir::StackSlotData::new(
                            cranelift::codegen::ir::StackSlotKind::ExplicitSlot, 8, 0,
                        ),
                    );
                    let ls_count_slot = builder.create_sized_stack_slot(
                        cranelift::codegen::ir::StackSlotData::new(
                            cranelift::codegen::ir::StackSlotKind::ExplicitSlot, 8, 0,
                        ),
                    );
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
                    let ls_continue =
                        builder.ins().fcmp(FloatCC::LessThan, ls_cnt, ls_max);
                    builder
                        .ins()
                        .brif(ls_continue, ls_body, &[], ls_fail, &[]);
                    builder.switch_to_block(ls_body);
                    let step_v =
                        builder.ins().stack_load(cl_types::F64, step_slot, 0);
                    let x_orig =
                        builder.ins().stack_load(cl_types::F64, x_save_slot, 0);
                    let scaled_step = builder.ins().fmul(ls_alpha, step_v);
                    let x_try = builder.ins().fsub(x_orig, scaled_step);
                    builder.ins().stack_store(x_try, t_slot, 0);
                    for ieq in inner_eqs {
                        if let Equation::Simple(lhs, rhs) = ieq {
                            if let Expression::Variable(name) = lhs {
                                let val = compile_expression(rhs, ctx, builder)?;
                                if let Some(slot) = ctx.stack_slots.get(name) {
                                    builder.ins().stack_store(val, *slot, 0);
                                }
                            }
                        }
                    }
                    let r_ls = compile_expression(&residuals[0], ctx, builder)?;
                    let abs_r_ls = builder.ins().fabs(r_ls);
                    let old_abs =
                        builder.ins().stack_load(cl_types::F64, abs_res_slot, 0);
                    let c_armijo = builder.ins().f64const(1e-4);
                    let ca = builder.ins().fmul(c_armijo, ls_alpha);
                    let descent = builder.ins().fmul(ca, old_abs);
                    let threshold = builder.ins().fsub(old_abs, descent);
                    let better =
                        builder.ins().fcmp(FloatCC::LessThan, abs_r_ls, threshold);
                    builder
                        .ins()
                        .brif(better, ls_accept, &[], ls_halve, &[]);
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
                    emit_assert_suppress_end(ctx, builder)?;
                    let ls_err = builder.ins().iconst(cl_types::I32, 2);
                    builder.ins().return_(&[ls_err]);
                    builder.seal_block(ls_fail);
                    builder.switch_to_block(ls_accept);
                    if let (Some(pr), Some(px)) = (ctx.diag_residual_ptr, ctx.diag_x_ptr) {
                        builder.ins().store(MemFlags::new(), abs_r_ls, pr, 0);
                        let accepted_x =
                            builder.ins().stack_load(cl_types::F64, t_slot, 0);
                        builder.ins().store(MemFlags::new(), accepted_x, px, 0);
                    }
                    let one = builder.ins().f64const(1.0);
                    let next_iter = builder.ins().fadd(iter_val, one);
                    builder.ins().stack_store(next_iter, iter_slot, 0);
                    builder.ins().jump(header_block, &[]);
                    builder.seal_block(ls_body);
                    builder.seal_block(ls_header);
                    builder.seal_block(ls_accept);
                    builder.seal_block(update_block);
                    builder.seal_block(header_block);
                    builder.seal_block(body_block);
                    builder.seal_block(perturb_block);
                    builder.switch_to_block(exit_block);
                    emit_assert_suppress_end(ctx, builder)?;
                    for ieq in inner_eqs {
                        if let Equation::Simple(lhs, rhs) = ieq {
                            if let Expression::Variable(name) = lhs {
                                let val = compile_expression(rhs, ctx, builder)?;
                                if let Some(slot) = ctx.stack_slots.get(name) {
                                    builder.ins().stack_store(val, *slot, 0);
                                }
                                if let Some(idx) = ctx.output_index(name) {
                                    let offset = (idx * 8) as i32;
                                    builder.ins().store(
                                        MemFlags::new(),
                                        val,
                                        ctx.outputs_ptr,
                                        offset,
                                    );
                                }
                            }
                        }
                    }
                    if let Some(idx) = ctx.output_index(&t_var) {
                        let val = builder.ins().stack_load(cl_types::F64, t_slot, 0);
                        let offset = (idx * 8) as i32;
                        builder
                            .ins()
                            .store(MemFlags::new(), val, ctx.outputs_ptr, offset);
                    }
                    builder.ins().jump(after_tearing_1, &[]);
                    builder.seal_block(exit_block);
                    builder.switch_to_block(after_tearing_1);
                }
            } else if residuals.len() == 1 {
                let mut u = unknowns.clone();
                if u.is_empty() {
                    if let Some(ref t) = tearing_var {
                        u.push(t.clone());
                    } else {
                        let mut hs = HashSet::new();
                        collect_vars_expr(&residuals[0], &mut hs);
                        let mut vars: Vec<String> = hs.into_iter().collect();
                        vars.sort();
                        if let Some(p) = vars
                            .iter()
                            .find(|v| !v.starts_with("__dummy"))
                            .cloned()
                            .or_else(|| vars.first().cloned())
                        {
                            u.push(p);
                        }
                    }
                }
                if u.len() == 1 {
                    compile_solvable_block_general_n(&u, residuals, ctx, builder)?;
                } else if u.is_empty() {
                    for ieq in inner_eqs {
                        compile_equation(ieq, ctx, builder)?;
                    }
                } else {
                    return Err(format!(
                        "SolvableBlock with 1 residual needs one unknown (synthesized len {})",
                        u.len()
                    ));
                }
            } else {
                return Err(format!(
                    "SolvableBlock with {} residuals is not supported (1 to 32 allowed)",
                    residuals.len()
                ));
            }
        }
        Equation::If(..) => {
            return Err("if-equation not yet supported in JIT; use algorithm if(cond) then ... end if instead".to_string());
        }
        Equation::MultiAssign(_, _) => {
            return Err("MultiAssign should not reach JIT (expand in flatten)".to_string());
        }
        _ => {}
    }
    Ok(())
}
