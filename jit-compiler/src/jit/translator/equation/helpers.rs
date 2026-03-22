use crate::ast::{Equation, Expression};
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift::codegen::ir::StackSlot;

use crate::jit::context::TranslationContext;
use crate::jit::translator::expr::compile_expression;

pub fn compile_inner_simple_assignments(
    inner_eqs: &[Equation],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    for ieq in inner_eqs {
        if let Equation::Simple(lhs, rhs) = ieq {
            if let Expression::Variable(id) = lhs {
                let name = crate::string_intern::resolve_id(*id);
                let val = compile_expression(rhs, ctx, builder)?;
                if let Some(slot) = ctx.stack_slots.get(&name) {
                    builder.ins().stack_store(val, *slot, 0);
                }
            }
        }
    }
    Ok(())
}

pub fn init_unknown_slot_from_output_or_default(
    var: &str,
    slot: StackSlot,
    ctx: &TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) {
    if let Some(idx) = ctx.output_index(var) {
        let offset = (idx * 8) as i32;
        let init_val = builder
            .ins()
            .load(cl_types::F64, MemFlags::new(), ctx.outputs_ptr, offset);
        builder.ins().stack_store(init_val, slot, 0);
    } else {
        let dv = crate::compiler::geometric_default_for_name(var);
        let fv = if dv != 0.0 { dv } else { 1e-3 };
        let fb = builder.ins().f64const(fv);
        builder.ins().stack_store(fb, slot, 0);
    }
}

pub fn write_unknown_outputs(
    vars_and_slots: &[(&str, StackSlot)],
    ctx: &TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) {
    for (var, slot) in vars_and_slots {
        let val = builder.ins().stack_load(cl_types::F64, *slot, 0);
        if let Some(idx) = ctx.output_index(var) {
            let offset = (idx * 8) as i32;
            builder
                .ins()
                .store(MemFlags::new(), val, ctx.outputs_ptr, offset);
        }
    }
}

pub fn store_diag_residual_and_x(
    ctx: &TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    residual: Value,
    x: Value,
) {
    if let (Some(pr), Some(px)) = (ctx.diag_residual_ptr, ctx.diag_x_ptr) {
        builder.ins().store(MemFlags::new(), residual, pr, 0);
        builder.ins().store(MemFlags::new(), x, px, 0);
    }
}

pub fn write_back_inner_simple_equations_and_tearing_output(
    inner_eqs: &[Equation],
    t_var: &str,
    t_slot: StackSlot,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    for ieq in inner_eqs {
        if let Equation::Simple(lhs, rhs) = ieq {
            if let Expression::Variable(id) = lhs {
                let name = crate::string_intern::resolve_id(*id);
                let val = compile_expression(rhs, ctx, builder)?;
                if let Some(slot) = ctx.stack_slots.get(&name) {
                    builder.ins().stack_store(val, *slot, 0);
                }
                if let Some(idx) = ctx.output_index(&name) {
                    let offset = (idx * 8) as i32;
                    builder
                        .ins()
                        .store(MemFlags::new(), val, ctx.outputs_ptr, offset);
                }
            }
        }
    }
    if let Some(idx) = ctx.output_index(t_var) {
        let val = builder.ins().stack_load(cl_types::F64, t_slot, 0);
        let offset = (idx * 8) as i32;
        builder
            .ins()
            .store(MemFlags::new(), val, ctx.outputs_ptr, offset);
    }
    Ok(())
}
