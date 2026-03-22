use crate::ast::{Equation, Expression};
use crate::jit::context::TranslationContext;
use crate::jit::translator::expr::compile_expression;
use crate::jit::types::ArrayType;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;

pub(super) fn compile_simple_equation(
    lhs: &Expression,
    rhs: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    if let Expression::ArrayAccess(arr_expr, idx_expr) = lhs {
        if let Expression::Variable(id) = &**arr_expr {
            let name = crate::string_intern::resolve_id(*id);
            let val = compile_expression(rhs, ctx, builder)?;
            if let Some((array_type, start_index)) = ctx.array_storage(&name) {
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
            } else if let Expression::Number(n) = &**idx_expr {
                let elem_name = format!("{}_{}", name, *n as i64);
                if let Some(slot) = ctx.stack_slots.get(&elem_name) {
                    builder.ins().stack_store(val, *slot, 0);
                } else {
                    ctx.var_map.insert(elem_name.clone(), val);
                }
                if let Some(idx) = ctx.output_index(&elem_name) {
                    builder
                        .ins()
                        .store(MemFlags::new(), val, ctx.outputs_ptr, (idx * 8) as i32);
                }
                if let Some(idx) = ctx.discrete_index(&elem_name) {
                    builder
                        .ins()
                        .store(MemFlags::new(), val, ctx.discrete_ptr, (idx * 8) as i32);
                }
                if let Some(idx) = ctx.param_index(&elem_name) {
                    builder
                        .ins()
                        .store(MemFlags::new(), val, ctx.params_ptr, (idx * 8) as i32);
                }
            } else {
                return Err(format!("Array {} not found in array_info", name));
            }
        }
    } else if let Expression::Variable(id) = lhs {
        let var_name = crate::string_intern::resolve_id(*id);
        let val = compile_expression(rhs, ctx, builder)?;
        if let Some(slot) = ctx.stack_slots.get(&var_name) {
            builder.ins().stack_store(val, *slot, 0);
        } else {
            ctx.var_map.insert(var_name.clone(), val);
        }
        if let Some(state_name) = var_name.strip_prefix("der_") {
            if let Some(idx) = ctx.state_index(state_name) {
                builder
                    .ins()
                    .store(MemFlags::new(), val, ctx.derivs_ptr, (idx * 8) as i32);
            }
        }
        if let Some(idx) = ctx.output_index(&var_name) {
            builder
                .ins()
                .store(MemFlags::new(), val, ctx.outputs_ptr, (idx * 8) as i32);
        }
        if let Some(idx) = ctx.discrete_index(&var_name) {
            builder
                .ins()
                .store(MemFlags::new(), val, ctx.discrete_ptr, (idx * 8) as i32);
        }
    } else if let Expression::Der(arg) = lhs {
        if let Expression::Variable(id) = &**arg {
            let var_name = crate::string_intern::resolve_id(*id);
            if let Some(idx) = ctx.state_index(&var_name) {
                let val = compile_expression(rhs, ctx, builder)?;
                builder
                    .ins()
                    .store(MemFlags::new(), val, ctx.derivs_ptr, (idx * 8) as i32);
            }
        } else if let Expression::ArrayAccess(arr_expr, idx_expr) = &**arg {
            if let Expression::Variable(id) = &**arr_expr {
                let name = crate::string_intern::resolve_id(*id);
                if let Some((array_type, start_index)) = ctx.array_storage(&name) {
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
    Ok(())
}

pub(super) fn compile_for_equation(
    loop_var: &String,
    start_expr: &Expression,
    end_expr: &Expression,
    body: &[Equation],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
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
    let after_for = builder.create_block();
    builder.ins().jump(header_block, &[]);
    builder.switch_to_block(header_block);
    let curr_i = builder.ins().stack_load(cl_types::F64, loop_var_slot, 0);
    let cmp = builder.ins().fcmp(FloatCC::LessThanOrEqual, curr_i, end_val);
    builder.ins().brif(cmp, body_block, &[], exit_block, &[]);
    builder.switch_to_block(body_block);
    for sub_eq in body {
        super::compile_equation_impl::compile_equation(sub_eq, ctx, builder)?;
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
    Ok(())
}
