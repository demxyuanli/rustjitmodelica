use crate::ast::Expression;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;

use crate::jit::context::TranslationContext;

pub(super) use super::solvable_assert::{emit_assert_suppress_begin, emit_assert_suppress_end};
use super::solvable_general_dense::compile_solvable_block_general_dense_n;
use super::solvable_general_sparse::{
    build_sparse_jacobian_pattern, compile_solvable_block_general_sparse_n,
};

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
        } else {
            let default_val = crate::compiler::geometric_default_for_name(var);
            let fallback_f = if default_val != 0.0 { default_val } else { 1e-3 };
            let fallback = builder.ins().f64const(fallback_f);
            builder.ins().stack_store(fallback, *slot, 0);
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
