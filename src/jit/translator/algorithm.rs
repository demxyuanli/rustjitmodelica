use cranelift::prelude::*;
use cranelift::prelude::types as cl_types;
use crate::ast::{AlgorithmStatement, Expression};
use super::super::types::ArrayType;
use super::super::context::TranslationContext;
use super::expr::{compile_expression, compile_zero_crossing_store};

pub fn compile_algorithm_stmt(
    stmt: &AlgorithmStatement,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    match stmt {
        AlgorithmStatement::Assignment(lhs, rhs) => {
             let val = compile_expression(rhs, ctx, builder)?;
             if let Expression::ArrayAccess(arr_expr, idx_expr) = lhs {
                 if let Expression::Variable(name) = &**arr_expr {
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
                         return Ok(());
                     }
                 }
             } else if let Expression::Variable(name) = lhs {
                 if let Some(slot) = ctx.stack_slots.get(name) {
                     builder.ins().stack_store(val, *slot, 0);
                 } else {
                     ctx.var_map.insert(name.clone(), val);
                 }
                 if let Some(idx) = ctx.output_index(name) {
                     let offset = (idx * 8) as i32;
                     builder.ins().store(MemFlags::new(), val, ctx.outputs_ptr, offset);
                 }
                 if !ctx.stack_slots.contains_key(name) {
                     if let Some(idx) = ctx.discrete_index(name) {
                         let offset = (idx * 8) as i32;
                         builder.ins().store(MemFlags::new(), val, ctx.discrete_ptr, offset);
                     }
                 }
             } else {
                 return Err(format!("LHS of assignment must be a variable, got {:?}", lhs));
             }
        }
        AlgorithmStatement::Reinit(var_name, val_expr) => {
             let val = compile_expression(val_expr, ctx, builder)?;
             if let Some(idx) = ctx.state_index(var_name) {
                 let offset = (idx * 8) as i32;
                 builder.ins().store(MemFlags::new(), val, ctx.states_ptr, offset);
             } else {
                 return Err(format!("reinit() target '{}' is not a state variable", var_name));
             }
        }
        AlgorithmStatement::If(cond, true_stmts, else_ifs, else_stmts) => {
            let true_block = builder.create_block();
            let mut next_block = builder.create_block();
            let end_block = builder.create_block();
            let cond_val = compile_expression(cond, ctx, builder)?;
            let zero = builder.ins().f64const(0.0);
            let cond_bool = builder.ins().fcmp(FloatCC::NotEqual, cond_val, zero);
            builder.ins().brif(cond_bool, true_block, &[], next_block, &[]);
            builder.switch_to_block(true_block);
            for s in true_stmts {
                compile_algorithm_stmt(s, ctx, builder)?;
            }
            builder.ins().jump(end_block, &[]);
            for (cond, stmts) in else_ifs {
                let check_block = next_block;
                let body_block = builder.create_block();
                next_block = builder.create_block();
                builder.switch_to_block(check_block);
                let c_val = compile_expression(cond, ctx, builder)?;
                let c_bool = builder.ins().fcmp(FloatCC::NotEqual, c_val, zero);
                builder.ins().brif(c_bool, body_block, &[], next_block, &[]);
                builder.switch_to_block(body_block);
                for s in stmts {
                    compile_algorithm_stmt(s, ctx, builder)?;
                }
                builder.ins().jump(end_block, &[]);
            }
            builder.switch_to_block(next_block);
            if let Some(stmts) = else_stmts {
                for s in stmts {
                    compile_algorithm_stmt(s, ctx, builder)?;
                }
            }
            builder.ins().jump(end_block, &[]);
            builder.switch_to_block(end_block);
            builder.seal_block(true_block);
            builder.seal_block(end_block);
        }
        AlgorithmStatement::While(cond, body) => {
            let header_block = builder.create_block();
            let body_block = builder.create_block();
            let exit_block = builder.create_block();
            builder.ins().jump(header_block, &[]);
            builder.switch_to_block(header_block);
            let c_val = compile_expression(cond, ctx, builder)?;
            let zero = builder.ins().f64const(0.0);
            let c_bool = builder.ins().fcmp(FloatCC::NotEqual, c_val, zero);
            builder.ins().brif(c_bool, body_block, &[], exit_block, &[]);
            builder.switch_to_block(body_block);
            for s in body {
                compile_algorithm_stmt(s, ctx, builder)?;
            }
            builder.ins().jump(header_block, &[]);
            builder.switch_to_block(exit_block);
            builder.seal_block(header_block);
            builder.seal_block(body_block);
            builder.seal_block(exit_block);
        }
        AlgorithmStatement::For(var_name, range_expr, body) => {
            let (start_val, step_val, end_val) = if let Expression::Range(start, step, end) = &**range_expr {
                let s = compile_expression(start, ctx, builder)?;
                let st = compile_expression(step, ctx, builder)?;
                let e = compile_expression(end, ctx, builder)?;
                (s, st, e)
            } else {
                let e = compile_expression(range_expr, ctx, builder)?;
                let s = builder.ins().f64const(1.0);
                let st = builder.ins().f64const(1.0);
                (s, st, e)
            };
            let loop_var_slot = if let Some(slot) = ctx.stack_slots.get(var_name) {
                *slot
            } else {
                return Err(format!("Loop variable '{}' stack slot not found.", var_name));
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
            for s in body {
                compile_algorithm_stmt(s, ctx, builder)?;
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
        AlgorithmStatement::When(cond, body, else_whens) => {
            compile_zero_crossing_store(cond, ctx, builder)?;
            let true_block = builder.create_block();
            let mut next_block = builder.create_block();
            let end_block = builder.create_block();
            let cond_val = compile_expression(cond, ctx, builder)?;
            let zero = builder.ins().f64const(0.0);
            let cond_bool = builder.ins().fcmp(FloatCC::NotEqual, cond_val, zero);
            let current_when_idx = *ctx.when_idx;
            *ctx.when_idx += 1;
            let offset_pre = (current_when_idx * 16) as i32;
            let offset_new = (current_when_idx * 16 + 8) as i32;
            let pre_cond_val = builder.ins().load(cl_types::F64, MemFlags::new(), ctx.when_states_ptr, offset_pre);
            let one = builder.ins().f64const(1.0);
            let cond_norm = builder.ins().select(cond_bool, one, zero);
            builder.ins().store(MemFlags::new(), cond_norm, ctx.when_states_ptr, offset_new);
            let pre_zero = builder.ins().fcmp(FloatCC::Equal, pre_cond_val, zero);
            let edge = builder.ins().band(cond_bool, pre_zero);
            builder.ins().brif(edge, true_block, &[], next_block, &[]);
            builder.switch_to_block(true_block);
            for s in body {
                compile_algorithm_stmt(s, ctx, builder)?;
            }
            builder.ins().jump(end_block, &[]);
            for (cond, stmts) in else_whens {
                compile_zero_crossing_store(cond, ctx, builder)?;
                let check_block = next_block;
                let body_block = builder.create_block();
                next_block = builder.create_block();
                builder.switch_to_block(check_block);
                let c_val = compile_expression(cond, ctx, builder)?;
                let curr_idx = *ctx.when_idx;
                *ctx.when_idx += 1;
                let offset_pre = (curr_idx * 16) as i32;
                let offset_new = (curr_idx * 16 + 8) as i32;
                let pre_c = builder.ins().load(cl_types::F64, MemFlags::new(), ctx.when_states_ptr, offset_pre);
                let c_bool = builder.ins().fcmp(FloatCC::NotEqual, c_val, zero);
                let one = builder.ins().f64const(1.0);
                let c_norm = builder.ins().select(c_bool, one, zero);
                builder.ins().store(MemFlags::new(), c_norm, ctx.when_states_ptr, offset_new);
                let pre_c_zero = builder.ins().fcmp(FloatCC::Equal, pre_c, zero);
                let c_edge = builder.ins().band(c_bool, pre_c_zero);
                builder.ins().brif(c_edge, body_block, &[], next_block, &[]);
                builder.switch_to_block(body_block);
                for s in stmts {
                    compile_algorithm_stmt(s, ctx, builder)?;
                }
                builder.ins().jump(end_block, &[]);
            }
            builder.switch_to_block(next_block);
            builder.ins().jump(end_block, &[]);
            builder.switch_to_block(end_block);
            builder.seal_block(true_block);
            builder.seal_block(end_block);
        }
    }
    Ok(())
}
