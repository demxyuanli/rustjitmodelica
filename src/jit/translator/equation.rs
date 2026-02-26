use cranelift::prelude::*;
use cranelift::prelude::types as cl_types;
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
        Equation::SolvableBlock { unknowns: _, tearing_var, equations: inner_eqs, residuals } => {
            if let Some(t_var) = tearing_var {
                 let t_slot = *ctx.stack_slots.get(t_var).expect("Tearing var must have stack slot");
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
                 let max_iter = builder.ins().f64const(20.0);
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
                 let tol = builder.ins().f64const(1e-6);
                 let converged = builder.ins().fcmp(FloatCC::LessThan, abs_res, tol);
                 builder.ins().brif(converged, exit_block, &[], perturb_block, &[]);
                 builder.switch_to_block(perturb_block);
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
                 let step = builder.ins().fdiv(res_val, j_val);
                 let x_new = builder.ins().fsub(x, step);
                 builder.ins().stack_store(x_new, t_slot, 0);
                 let one = builder.ins().f64const(1.0);
                 let next_iter = builder.ins().fadd(iter_val, one);
                 builder.ins().stack_store(next_iter, iter_slot, 0);
                 builder.ins().jump(header_block, &[]);
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
        }
        _ => {}
    }
    Ok(())
}
