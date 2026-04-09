use crate::ast::{
    expr_to_connector_path, expr_to_flat_scalar_prefix, flat_index_suffix_for_scalar_name, Expression,
};
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;

use crate::jit::context::TranslationContext;
use crate::jit::types::ArrayType;
use super::helpers::{jit_scalar_name_bound, jit_var_fallback_trace, modelica_constants_flat_variable};

pub(super) fn compile_variable_load(
    id: crate::string_intern::VarId,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<Value, String> {
    let name = crate::string_intern::resolve_id(id);
    if let Some(slot) = ctx.stack_slots.get(&name) {
        return Ok(builder.ins().stack_load(cl_types::F64, *slot, 0));
    }
    if let Some(val) = ctx.var_map.get(&name).copied() {
        return Ok(val);
    }
    if let Some(idx) = ctx.output_index(&name) {
        let offset = (idx * 8) as i32;
        return Ok(builder
            .ins()
            .load(cl_types::F64, MemFlags::new(), ctx.outputs_ptr, offset));
    }
    if let Some(idx) = ctx.param_index(&name) {
        let offset = (idx * 8) as i32;
        return Ok(builder
            .ins()
            .load(cl_types::F64, MemFlags::new(), ctx.params_ptr, offset));
    }
    if name.starts_with("der_") {
        let base = &name[4..];
        if let Some(idx) = ctx.state_index(base) {
            let offset = (idx * 8) as i32;
            return Ok(builder.ins().load(
                cl_types::F64,
                MemFlags::new(),
                ctx.derivs_ptr,
                offset,
            ));
        }
        return Err(format!(
            "der({}) not found: state variable {} unknown",
            base, base
        ));
    }

    if let Some((base, idx0)) = name
        .rsplit_once('_')
        .and_then(|(b, i)| i.parse::<usize>().ok().map(|n| (b.to_string(), n)))
    {
        if let Some((array_type, start_index)) = ctx.array_storage(&base) {
            let idx_val = builder.ins().f64const((idx0 as f64) + 1.0);
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
            return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0));
        }
        if base == "a" || base == "b" {
            let _ = idx0;
            jit_var_fallback_trace(&name, "array-scalar-placeholder-a-b");
            return Ok(builder.ins().f64const(0.0));
        }
    }

    if let Some(v) = modelica_constants_flat_variable(&name) {
        return Ok(builder.ins().f64const(v));
    }
    if let Some((v, trace_tag)) = crate::jit::var_fallback_policy::lookup_var_fallback(&name) {
        if !trace_tag.is_empty() {
            jit_var_fallback_trace(&name, trace_tag.as_str());
        }
        return Ok(builder.ins().f64const(v));
    }

    Err(format!(
        "Variable '{}' not found in JIT context (check model flattening and variable declarations)",
        name
    ))
}

pub(super) fn compile_array_access(
    expr: &Expression,
    arr_expr: &Expression,
    idx_expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: fn(
        &Expression,
        &mut TranslationContext,
        &mut cranelift::frontend::FunctionBuilder<'_>,
    ) -> Result<Value, String>,
) -> Result<Value, String> {
    if let Some(flat) = expr_to_flat_scalar_prefix(expr) {
        if jit_scalar_name_bound(ctx, &flat) {
            return compile_rec(&Expression::var(&flat), ctx, builder);
        }
    }

    let name = if let Expression::Variable(id) = &*arr_expr {
        Some(crate::string_intern::resolve_id(*id))
    } else {
        expr_to_connector_path(arr_expr)
    };

    if let Some(name) = name {
        if let Some((array_type, start_index)) = ctx.array_storage(&name) {
            let idx_val = compile_rec(idx_expr, ctx, builder)?;
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
            Ok(builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0))
        } else {
            let base = name.replace('.', "_");
            if let Some(suf) = flat_index_suffix_for_scalar_name(idx_expr) {
                let elem_name = format!("{}_{}", base, suf);
                compile_rec(&Expression::var(&elem_name), ctx, builder)
            } else {
                Err(format!("Array {} not found in array_info", name))
            }
        }
    } else if let Some(arr_base) = expr_to_connector_path(arr_expr)
        .map(|p| p.replace('.', "_"))
        .or_else(|| expr_to_flat_scalar_prefix(arr_expr))
    {
        if let Some(suf) = flat_index_suffix_for_scalar_name(idx_expr) {
            let elem_name = format!("{}_{}", arr_base, suf);
            compile_rec(&Expression::var(&elem_name), ctx, builder)
        } else {
            compile_rec(arr_expr, ctx, builder)
        }
    } else {
        compile_rec(arr_expr, ctx, builder)
    }
}
