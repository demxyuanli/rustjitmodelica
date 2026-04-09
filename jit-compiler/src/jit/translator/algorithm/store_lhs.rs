use crate::jit::context::TranslationContext;
use crate::jit::types::ArrayType;
use super::super::expr::compile_expression;
use crate::ast::{Expression, Operator};
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::Module;

pub(super) fn scalar_f64_ptr_for_assign(
    ctx: &TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    name: &str,
) -> Result<Value, String> {
    let ptr_ty = ctx.module.target_config().pointer_type();
    if let Some(i) = ctx.discrete_index(name) {
        let off = builder.ins().iconst(ptr_ty, (i * 8) as i64);
        return Ok(builder.ins().iadd(ctx.discrete_ptr, off));
    }
    if let Some(i) = ctx.output_index(name) {
        let off = builder.ins().iconst(ptr_ty, (i * 8) as i64);
        return Ok(builder.ins().iadd(ctx.outputs_ptr, off));
    }
    Err(format!(
        "realFFT: scalar output '{}' has no discrete/output slot",
        name
    ))
}

pub(super) fn compile_store_to_lhs(
    lhs: &Expression,
    val: Value,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    if matches!(lhs, Expression::Number(_)) {
        return Ok(());
    }
    if let Expression::BinaryOp(l, Operator::Sub, r) = lhs {
        if matches!(&**l, Expression::Number(n) if *n == 0.0) {
            let zero = builder.ins().f64const(0.0);
            let neg_val = builder.ins().fsub(zero, val);
            return compile_store_to_lhs(r, neg_val, ctx, builder);
        }
    }
    if matches!(lhs, Expression::Dot(_, _)) {
        return Err(format!(
            "LHS field-store target is unsupported in JIT backend for multi-assign: {:?}. Use scalar variable/array access target instead.",
            lhs
        ));
    }
    if let Expression::ArrayAccess(arr_expr, idx_expr) = lhs {
        if matches!(&**idx_expr, Expression::Range(_, _, _)) {
            return Ok(());
        }
        if let Expression::Variable(id) = &**arr_expr {
            let name = crate::string_intern::resolve_id(*id);
            if let Some(info) = ctx.array_info.get(&name) {
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
    } else if let Expression::Variable(id) = lhs {
        let name = crate::string_intern::resolve_id(*id);
        if let Some(slot) = ctx.stack_slots.get(&name) {
            builder.ins().stack_store(val, *slot, 0);
        } else {
            ctx.var_map.insert(name.clone(), val);
        }
        if let Some(idx) = ctx.output_index(&name) {
            let offset = (idx * 8) as i32;
            builder
                .ins()
                .store(MemFlags::new(), val, ctx.outputs_ptr, offset);
        }
        if let Some(idx) = ctx.discrete_index(&name) {
            let offset = (idx * 8) as i32;
            builder
                .ins()
                .store(MemFlags::new(), val, ctx.discrete_ptr, offset);
        }
        return Ok(());
    }
    Err(format!(
        "LHS of assignment must be a variable or array access, got {:?}",
        lhs
    ))
}

pub(super) fn compile_fill_array_variable(
    lhs_name: &str,
    fill_value: Value,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<bool, String> {
    let Some(info) = ctx.array_info.get(lhs_name) else {
        return Ok(false);
    };
    let base_ptr = match info.array_type {
        ArrayType::State => ctx.states_ptr,
        ArrayType::Discrete => ctx.discrete_ptr,
        ArrayType::Parameter => ctx.params_ptr,
        ArrayType::Output => ctx.outputs_ptr,
        ArrayType::Derivative => ctx.derivs_ptr,
    };
    for i in 0..info.size {
        let offset = ((info.start_index + i) * 8) as i32;
        builder
            .ins()
            .store(MemFlags::new(), fill_value, base_ptr, offset);
    }
    Ok(true)
}

pub(super) fn compile_cat_into_array_variable(
    lhs_name: &str,
    args: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<bool, String> {
    let Some(lhs_info) = ctx.array_info.get(lhs_name) else {
        return Ok(false);
    };
    if args.len() < 2 {
        return Ok(false);
    }
    let lhs_base = match lhs_info.array_type {
        ArrayType::State => ctx.states_ptr,
        ArrayType::Discrete => ctx.discrete_ptr,
        ArrayType::Parameter => ctx.params_ptr,
        ArrayType::Output => ctx.outputs_ptr,
        ArrayType::Derivative => ctx.derivs_ptr,
    };
    let mut write_pos = 0usize;
    for src_expr in args.iter().skip(1) {
        let Expression::Variable(src_id) = src_expr else {
            continue;
        };
        let src_name = crate::string_intern::resolve_id(*src_id);
        let Some(src_info) = ctx.array_info.get(&src_name) else {
            continue;
        };
        let src_base = match src_info.array_type {
            ArrayType::State => ctx.states_ptr,
            ArrayType::Discrete => ctx.discrete_ptr,
            ArrayType::Parameter => ctx.params_ptr,
            ArrayType::Output => ctx.outputs_ptr,
            ArrayType::Derivative => ctx.derivs_ptr,
        };
        for i in 0..src_info.size {
            if write_pos >= lhs_info.size {
                return Ok(true);
            }
            let src_offset = ((src_info.start_index + i) * 8) as i32;
            let v = builder
                .ins()
                .load(cl_types::F64, MemFlags::new(), src_base, src_offset);
            let dst_offset = ((lhs_info.start_index + write_pos) * 8) as i32;
            builder.ins().store(MemFlags::new(), v, lhs_base, dst_offset);
            write_pos += 1;
        }
    }
    Ok(true)
}
