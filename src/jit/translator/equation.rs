use cranelift::prelude::*;
use cranelift::prelude::types as cl_types;
use cranelift_module::{Linkage, Module};
use crate::ast::{Equation, Expression};
use super::super::types::ArrayType;
use super::super::context::TranslationContext;
use super::expr::compile_expression;

pub fn compile_equation(
    eq: &Equation,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    match eq {
        Equation::Simple(lhs, rhs) => {
            if let Expression::ArrayAccess(arr_expr, idx_expr) = lhs {
                 if let Expression::Variable(name) = &**arr_expr {
                     let val = compile_expression(rhs, ctx, builder)?;
                     if let Some(info) = ctx.array_info.get(name) {
                         let idx_val = compile_expression(idx_expr, ctx, builder)?;
                         let one = builder.ins().f64const(1.0);
                         let idx_0 = builder.ins().fsub(idx_val, one);
                         let idx_int = builder.ins().fcvt_to_sint(cl_types::I64, idx_0);
                         let eight = builder.ins().iconst(cl_types::I64, 8);
                         let offset_bytes = builder.ins().imul(idx_int, eight);
                         let start_offset = (info.start_index * 8) as i64;
                         let start_const = builder.ins().iconst(cl_types::I64, start_offset);
                         let total_offset = builder.ins().iadd(start_const, offset_bytes);
                         let base_ptr = match info.array_type {
                             ArrayType::State => ctx.states_ptr,
                             ArrayType::Discrete => ctx.discrete_ptr,
                             ArrayType::Parameter => ctx.params_ptr,
                             ArrayType::Output => ctx.outputs_ptr,
                             ArrayType::Derivative => ctx.derivs_ptr,
                         };
                         let addr = builder.ins().iadd(base_ptr, total_offset);
                         builder.ins().store(MemFlags::new(), val, addr, 0);
                     } else {
                         return Err(format!("Array {} not found in array_info", name));
                     }
                 }
            } else if let Expression::Variable(var_name) = lhs {
                let val = compile_expression(rhs, ctx, builder)?;
                if let Some(slot) = ctx.stack_slots.get(var_name) {
                    builder.ins().stack_store(val, *slot, 0);
                } else {
                    ctx.var_map.insert(var_name.clone(), val);
                }
                if let Some(idx) = ctx.output_index(var_name) {
                    let offset = (idx * 8) as i32;
                    builder.ins().store(MemFlags::new(), val, ctx.outputs_ptr, offset);
                }
                if let Some(idx) = ctx.discrete_index(var_name) {
                     let offset = (idx * 8) as i32;
                     builder.ins().store(MemFlags::new(), val, ctx.discrete_ptr, offset);
                }
            }
            else if let Expression::Der(arg) = lhs {
                if let Expression::Variable(var_name) = &**arg {
                    if let Some(idx) = ctx.state_index(var_name) {
                        let val = compile_expression(rhs, ctx, builder)?;
                        let offset = (idx * 8) as i32;
                        builder.ins().store(MemFlags::new(), val, ctx.derivs_ptr, offset);
                    }
                } else if let Expression::ArrayAccess(arr_expr, idx_expr) = &**arg {
                    if let Expression::Variable(name) = &**arr_expr {
                         if let Some(info) = ctx.array_info.get(name) {
                             if matches!(info.array_type, ArrayType::State) {
                                 let val = compile_expression(rhs, ctx, builder)?;
                                 let idx_val = compile_expression(idx_expr, ctx, builder)?;
                                 let one = builder.ins().f64const(1.0);
                                 let idx_0 = builder.ins().fsub(idx_val, one);
                                 let idx_int = builder.ins().fcvt_to_sint(cl_types::I64, idx_0);
                                 let eight = builder.ins().iconst(cl_types::I64, 8);
                                 let offset_bytes = builder.ins().imul(idx_int, eight);
                                 let start_offset = (info.start_index * 8) as i64;
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
             let start_val = compile_expression(start_expr, ctx, builder)?;
             let end_val = compile_expression(end_expr, ctx, builder)?;
             let step_val = builder.ins().f64const(1.0);
             let loop_var_slot = if let Some(slot) = ctx.stack_slots.get(loop_var) {
                 *slot
             } else {
                 return Err(format!("Loop variable '{}' stack slot not found.", loop_var));
             };
             builder.ins().stack_store(start_val, loop_var_slot, 0);
             let header_block = builder.create_block();
             let body_block = builder.create_block();
             let exit_block = builder.create_block();
             builder.ins().jump(header_block, &[]);
             builder.switch_to_block(header_block);
             let curr_i = builder.ins().stack_load(cl_types::F64, loop_var_slot, 0);
             let cmp = builder.ins().fcmp(FloatCC::LessThanOrEqual, curr_i, end_val);
             builder.ins().brif(cmp, body_block, &[], exit_block, &[]);
             builder.switch_to_block(body_block);
             for sub_eq in body {
                 compile_equation(sub_eq, ctx, builder)?;
             }
             let curr_i_2 = builder.ins().stack_load(cl_types::F64, loop_var_slot, 0);
             let next_i = builder.ins().fadd(curr_i_2, step_val);
             builder.ins().stack_store(next_i, loop_var_slot, 0);
             builder.ins().jump(header_block, &[]);
             builder.switch_to_block(exit_block);
             builder.seal_block(header_block);
             builder.seal_block(body_block);
             builder.seal_block(exit_block);
        }
        Equation::SolvableBlock { unknowns, tearing_var, equations: inner_eqs, residuals } => {
            if residuals.len() == 2 && unknowns.len() >= 2 {
                let v0 = &unknowns[0];
                let v1 = &unknowns[1];
                let slot0 = *ctx.stack_slots.get(v0).ok_or_else(|| format!("SolvableBlock unknown {} missing stack slot", v0))?;
                let slot1 = *ctx.stack_slots.get(v1).ok_or_else(|| format!("SolvableBlock unknown {} missing stack slot", v1))?;
                if let Some(idx) = ctx.output_index(v0) {
                    let offset = (idx * 8) as i32;
                    let init0 = builder.ins().load(cl_types::F64, MemFlags::new(), ctx.outputs_ptr, offset);
                    builder.ins().stack_store(init0, slot0, 0);
                }
                if let Some(idx) = ctx.output_index(v1) {
                    let offset = (idx * 8) as i32;
                    let init1 = builder.ins().load(cl_types::F64, MemFlags::new(), ctx.outputs_ptr, offset);
                    builder.ins().stack_store(init1, slot1, 0);
                }
                ctx.var_map.remove(v0);
                ctx.var_map.remove(v1);
                let iter_slot = builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(cranelift::codegen::ir::StackSlotKind::ExplicitSlot, 8, 0));
                let zero = builder.ins().f64const(0.0);
                builder.ins().stack_store(zero, iter_slot, 0);
                let eps = 1e-6_f64;
                let header_block = builder.create_block();
                let body_block = builder.create_block();
                let perturb_block = builder.create_block();
                let exit_block = builder.create_block();
                let error_block = builder.create_block();
                builder.ins().jump(header_block, &[]);
                builder.switch_to_block(header_block);
                let iter_val = builder.ins().stack_load(cl_types::F64, iter_slot, 0);
                let max_iter = builder.ins().f64const(50.0);
                let iter_cond = builder.ins().fcmp(FloatCC::LessThan, iter_val, max_iter);
                builder.ins().brif(iter_cond, body_block, &[], error_block, &[]);
                builder.switch_to_block(error_block);
                let err_code = builder.ins().iconst(cl_types::I32, 2);
                builder.ins().return_(&[err_code]);
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
                builder.ins().brif(c0, check_c1_block, &[], perturb_block, &[]);
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
                builder.ins().brif(bad_det, error_block, &[], update_block, &[]);
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
                builder.seal_block(header_block);
                builder.seal_block(body_block);
                builder.seal_block(check_c1_block);
                builder.seal_block(perturb_block);
                builder.seal_block(error_block);
                builder.switch_to_block(exit_block);
                for (var, slot) in [(v0, slot0), (v1, slot1)] {
                    let val = builder.ins().stack_load(cl_types::F64, slot, 0);
                    if let Some(idx) = ctx.output_index(var) {
                        let offset = (idx * 8) as i32;
                        builder.ins().store(MemFlags::new(), val, ctx.outputs_ptr, offset);
                    }
                }
                builder.seal_block(exit_block);
            } else if residuals.len() == 3 && unknowns.len() >= 3 {
                let v0 = &unknowns[0];
                let v1 = &unknowns[1];
                let v2 = &unknowns[2];
                let slot0 = *ctx.stack_slots.get(v0).ok_or_else(|| format!("SolvableBlock unknown {} missing stack slot", v0))?;
                let slot1 = *ctx.stack_slots.get(v1).ok_or_else(|| format!("SolvableBlock unknown {} missing stack slot", v1))?;
                let slot2 = *ctx.stack_slots.get(v2).ok_or_else(|| format!("SolvableBlock unknown {} missing stack slot", v2))?;
                for (var, slot) in [(v0, slot0), (v1, slot1), (v2, slot2)] {
                    if let Some(idx) = ctx.output_index(var) {
                        let offset = (idx * 8) as i32;
                        let init_val = builder.ins().load(cl_types::F64, MemFlags::new(), ctx.outputs_ptr, offset);
                        builder.ins().stack_store(init_val, slot, 0);
                    }
                }
                ctx.var_map.remove(v0);
                ctx.var_map.remove(v1);
                ctx.var_map.remove(v2);
                let iter_slot = builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(cranelift::codegen::ir::StackSlotKind::ExplicitSlot, 8, 0));
                let zero = builder.ins().f64const(0.0);
                builder.ins().stack_store(zero, iter_slot, 0);
                let eps = 1e-6_f64;
                let eps_val = builder.ins().f64const(eps);
                let header_block = builder.create_block();
                let body_block = builder.create_block();
                let perturb_block = builder.create_block();
                let exit_block = builder.create_block();
                let error_block = builder.create_block();
                builder.ins().jump(header_block, &[]);
                builder.switch_to_block(header_block);
                let iter_val = builder.ins().stack_load(cl_types::F64, iter_slot, 0);
                let max_iter = builder.ins().f64const(50.0);
                let iter_cond = builder.ins().fcmp(FloatCC::LessThan, iter_val, max_iter);
                builder.ins().brif(iter_cond, body_block, &[], error_block, &[]);
                builder.switch_to_block(error_block);
                let err_code = builder.ins().iconst(cl_types::I32, 2);
                builder.ins().return_(&[err_code]);
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
                builder.ins().brif(c0, check_c1_block, &[], perturb_block, &[]);
                builder.switch_to_block(check_c1_block);
                builder.ins().brif(c1, check_c2_block, &[], perturb_block, &[]);
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
                builder.ins().brif(bad_det, error_block, &[], update_block, &[]);
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
                builder.seal_block(header_block);
                builder.seal_block(body_block);
                builder.seal_block(check_c1_block);
                builder.seal_block(check_c2_block);
                builder.seal_block(perturb_block);
                builder.seal_block(error_block);
                builder.switch_to_block(exit_block);
                for (var, slot) in [(v0, slot0), (v1, slot1), (v2, slot2)] {
                    let val = builder.ins().stack_load(cl_types::F64, slot, 0);
                    if let Some(idx) = ctx.output_index(var) {
                        let offset = (idx * 8) as i32;
                        builder.ins().store(MemFlags::new(), val, ctx.outputs_ptr, offset);
                    }
                }
                builder.seal_block(exit_block);
            } else if residuals.len() >= 4 && residuals.len() <= 32 && unknowns.len() >= residuals.len() {
                compile_solvable_block_general_n(unknowns, residuals, ctx, builder)?;
            } else if residuals.len() == 1 {
            if let Some(t_var) = tearing_var {
                 ctx.var_map.remove(t_var);
                 let t_slot = *ctx.stack_slots.get(t_var).expect("Tearing var must have stack slot");
                 if let Some(idx) = ctx.output_index(t_var) {
                     let offset = (idx * 8) as i32;
                     let init_val = builder.ins().load(cl_types::F64, MemFlags::new(), ctx.outputs_ptr, offset);
                     builder.ins().stack_store(init_val, t_slot, 0);
                 } else {
                     let zero = builder.ins().f64const(0.0);
                     builder.ins().stack_store(zero, t_slot, 0);
                 }
                 let iter_slot = builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(cranelift::codegen::ir::StackSlotKind::ExplicitSlot, 8, 0));
                 let zero = builder.ins().f64const(0.0);
                 builder.ins().stack_store(zero, iter_slot, 0);
                 let header_block = builder.create_block();
                 let body_block = builder.create_block();
                 let perturb_block = builder.create_block();
                 let exit_block = builder.create_block();
                 builder.ins().jump(header_block, &[]);
                 builder.switch_to_block(header_block);
                 let iter_val = builder.ins().stack_load(cl_types::F64, iter_slot, 0);
                 let max_iter = builder.ins().f64const(50.0);
                 let iter_cond = builder.ins().fcmp(FloatCC::LessThan, iter_val, max_iter);
                 let error_block = builder.create_block();
                 builder.ins().brif(iter_cond, body_block, &[], error_block, &[]);
                 builder.switch_to_block(error_block);
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
                 builder.ins().brif(converged, exit_block, &[], perturb_block, &[]);
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
                 let error_code2 = builder.ins().iconst(cl_types::I32, 2);
                 builder.ins().return_(&[error_code2]);
                 builder.seal_block(jac_error_block);
                 builder.switch_to_block(update_block);
                 let step = builder.ins().fdiv(res_val, j_val);
                 let x_new = builder.ins().fsub(x, step);
                 builder.ins().stack_store(x_new, t_slot, 0);
                 if let (Some(pr), Some(px)) = (ctx.diag_residual_ptr, ctx.diag_x_ptr) {
                     builder.ins().store(MemFlags::new(), res_val, pr, 0);
                     builder.ins().store(MemFlags::new(), x, px, 0);
                 }
                 let one = builder.ins().f64const(1.0);
                 let next_iter = builder.ins().fadd(iter_val, one);
                 builder.ins().stack_store(next_iter, iter_slot, 0);
                 builder.ins().jump(header_block, &[]);
                 builder.seal_block(update_block);
                 builder.switch_to_block(exit_block);
                 builder.seal_block(header_block);
                 builder.seal_block(body_block);
                 builder.seal_block(perturb_block);
                 builder.seal_block(exit_block);
                 for ieq in inner_eqs {
                     if let Equation::Simple(lhs, rhs) = ieq {
                         if let Expression::Variable(name) = lhs {
                             let val = compile_expression(rhs, ctx, builder)?;
                             if let Some(slot) = ctx.stack_slots.get(name) {
                                 builder.ins().stack_store(val, *slot, 0);
                             }
                             if let Some(idx) = ctx.output_index(name) {
                                let offset = (idx * 8) as i32;
                                builder.ins().store(MemFlags::new(), val, ctx.outputs_ptr, offset);
                            }
                         }
                     }
                 }
                 if let Some(idx) = ctx.output_index(t_var) {
                     let val = builder.ins().stack_load(cl_types::F64, t_slot, 0);
                     let offset = (idx * 8) as i32;
                     builder.ins().store(MemFlags::new(), val, ctx.outputs_ptr, offset);
                 }
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

fn compile_solvable_block_general_n(
    unknowns: &[String],
    residuals: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    let n = residuals.len();
    let ptr_type = ctx.module.target_config().pointer_type();
    let slots: Vec<_> = unknowns.iter().take(n)
        .map(|v| -> Result<_, String> {
            Ok(*ctx.stack_slots.get(v).ok_or_else(|| format!("SolvableBlock unknown {} missing stack slot", v))?)
        })
        .collect::<Result<Vec<_>, _>>()?;
    for v in unknowns.iter().take(n) {
        ctx.var_map.remove(v);
    }
    for (var, slot) in unknowns.iter().take(n).zip(&slots) {
        if let Some(idx) = ctx.output_index(var) {
            let offset = (idx * 8) as i32;
            let init_val = builder.ins().load(cl_types::F64, MemFlags::new(), ctx.outputs_ptr, offset);
            builder.ins().stack_store(init_val, *slot, 0);
        }
    }
    let buf_size = (n * n + n + n) * 8;
    let buf_slot = builder.create_sized_stack_slot(
        cranelift::codegen::ir::StackSlotData::new(
            cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
            buf_size as u32,
            0,
        ),
    );
    let iter_slot = builder.create_sized_stack_slot(
        cranelift::codegen::ir::StackSlotData::new(
            cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
            8,
            0,
        ),
    );
    let zero = builder.ins().f64const(0.0);
    builder.ins().stack_store(zero, iter_slot, 0);
    let eps = 1e-6_f64;
    let eps_val = builder.ins().f64const(eps);
    let header_block = builder.create_block();
    let body_block = builder.create_block();
    let exit_block = builder.create_block();
    let error_block = builder.create_block();
    builder.ins().jump(header_block, &[]);
    builder.switch_to_block(header_block);
    let iter_val = builder.ins().stack_load(cl_types::F64, iter_slot, 0);
    let max_iter = builder.ins().f64const(50.0);
    let iter_cond = builder.ins().fcmp(FloatCC::LessThan, iter_val, max_iter);
    builder.ins().brif(iter_cond, body_block, &[], error_block, &[]);
    builder.switch_to_block(error_block);
    let err_code = builder.ins().iconst(cl_types::I32, 2);
    builder.ins().return_(&[err_code]);
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
    builder.ins().brif(conv_cond, exit_block, &[], perturb_block, &[]);
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
    let func_id = ctx.module.declare_function("rustmodlica_solve_linear_n", Linkage::Import, &sig)
        .map_err(|e| e.to_string())?;
    let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
    let solve_result = builder.ins().call(func_ref, &[n_i32, jac_ptr, r_ptr, dx_ptr]);
    let status = builder.inst_results(solve_result)[0];
    let zero_i32 = builder.ins().iconst(cl_types::I32, 0);
    let status_ok = builder.ins().icmp(IntCC::Equal, status, zero_i32);
    let update_block = builder.create_block();
    builder.ins().brif(status_ok, update_block, &[], error_block, &[]);
    builder.switch_to_block(update_block);
    for i in 0..n {
        let off = dx_offset + (i * 8) as i32;
        let off_val = builder.ins().iconst(ptr_type, off as i64);
        let addr = builder.ins().iadd(base_ptr, off_val);
        let dxi = builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0);
        let xi = builder.ins().stack_load(cl_types::F64, slots[i], 0);
        let xi_new = builder.ins().fadd(xi, dxi);
        builder.ins().stack_store(xi_new, slots[i], 0);
    }
    let one = builder.ins().f64const(1.0);
    let next_iter = builder.ins().fadd(iter_val, one);
    builder.ins().stack_store(next_iter, iter_slot, 0);
    builder.ins().jump(header_block, &[]);
    builder.seal_block(update_block);
    builder.seal_block(header_block);
    builder.seal_block(body_block);
    builder.seal_block(perturb_block);
    builder.seal_block(error_block);
    builder.switch_to_block(exit_block);
    for (var, slot) in unknowns.iter().take(n).zip(&slots) {
        let val = builder.ins().stack_load(cl_types::F64, *slot, 0);
        if let Some(idx) = ctx.output_index(var) {
            let offset = (idx * 8) as i32;
            builder.ins().store(MemFlags::new(), val, ctx.outputs_ptr, offset);
        }
    }
    builder.seal_block(exit_block);
    Ok(())
}
