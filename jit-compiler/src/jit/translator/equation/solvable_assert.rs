use cranelift::prelude::InstBuilder;
use cranelift_module::{Linkage, Module};

use crate::jit::context::TranslationContext;

pub(super) fn emit_assert_suppress_begin(
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    let sig = ctx.module.make_signature();
    let func_id = ctx
        .module
        .declare_function("rustmodlica_assert_suppress_begin", Linkage::Import, &sig)
        .map_err(|e| e.to_string())?;
    let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
    builder.ins().call(func_ref, &[]);
    Ok(())
}

pub(super) fn emit_assert_suppress_end(
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    let sig = ctx.module.make_signature();
    let func_id = ctx
        .module
        .declare_function("rustmodlica_assert_suppress_end", Linkage::Import, &sig)
        .map_err(|e| e.to_string())?;
    let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
    builder.ins().call(func_ref, &[]);
    Ok(())
}
