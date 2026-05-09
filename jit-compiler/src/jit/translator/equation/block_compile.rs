//! Independent solvable-block compilation.
//!
//! When enabled (RUSTMODLICA_BLOCK_COMPILE=1), each solvable block is compiled
//! into its own Cranelift function instead of being inlined into calc_derivs.
//! This enables per-block recompilation and hot-swapping.
//!
//! Block functions share the same signature as calc_derivs so they receive
//! direct pointer access to all state arrays.

use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::{FuncId, Linkage, Module};

use crate::jit::context::TranslationContext;

/// Whether independent block compilation is enabled.
pub fn block_compile_enabled() -> bool {
    std::env::var("RUSTMODLICA_BLOCK_COMPILE")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE"))
        .unwrap_or(false)
}

/// A compiled solvable block stored as a separate Cranelift function.
pub struct CompiledBlock {
    /// Cranelift function ID.
    pub func_id: FuncId,
    /// Block index for tracking/recompilation.
    pub block_index: usize,
}

/// Build the calc_derivs signature for block functions (identical to main).
pub fn make_block_signature(module: &dyn Module) -> Signature {
    let mut sig = module.make_signature();
    let ptr_ty = module.target_config().pointer_type();
    sig.params.push(AbiParam::new(cl_types::F64));  // time
    sig.params.push(AbiParam::new(ptr_ty));          // states
    sig.params.push(AbiParam::new(ptr_ty));          // discrete
    sig.params.push(AbiParam::new(ptr_ty));          // derivs
    sig.params.push(AbiParam::new(ptr_ty));          // params
    sig.params.push(AbiParam::new(ptr_ty));          // outputs
    sig.params.push(AbiParam::new(ptr_ty));          // when_states
    sig.params.push(AbiParam::new(ptr_ty));          // crossings
    sig.params.push(AbiParam::new(ptr_ty));          // pre_states
    sig.params.push(AbiParam::new(ptr_ty));          // pre_discrete
    sig.params.push(AbiParam::new(cl_types::F64));   // t_end
    sig.params.push(AbiParam::new(ptr_ty));          // diag_residual
    sig.params.push(AbiParam::new(ptr_ty));          // diag_x
    sig.params.push(AbiParam::new(ptr_ty));          // homotopy_lambda
    sig.returns.push(AbiParam::new(cl_types::I32));
    sig
}

/// Declare a new block function in the module and return its ID.
pub fn declare_block_function(
    ctx: &mut TranslationContext,
    block_index: usize,
) -> Result<(FuncId, Signature), String> {
    let sig = make_block_signature(ctx.module);
    let name = format!("__block_{}", block_index);
    let func_id = ctx
        .module
        .declare_function(&name, Linkage::Local, &sig)
        .map_err(|e| e.to_string())?;
    Ok((func_id, sig))
}

/// Build a block function entry block with block params matching calc_derivs signature.
/// Caller must have already declared the function and created the FunctionBuilder.
pub fn setup_block_entry(builder: &mut FunctionBuilder) -> Block {
    let entry = builder.create_block();
    builder.append_block_params_for_function_params(entry);
    builder.switch_to_block(entry);
    entry
}

/// Argument values for a block function call, matching calc_derivs signature.
pub struct BlockCallArgs {
    pub time: Value,
    pub states_ptr: Value,
    pub discrete_ptr: Value,
    pub derivs_ptr: Value,
    pub params_ptr: Value,
    pub outputs_ptr: Value,
    pub when_states_ptr: Value,
    pub crossings_ptr: Value,
    pub pre_states_ptr: Value,
    pub pre_discrete_ptr: Value,
    pub t_end: Value,
    pub diag_residual_ptr: Value,
    pub diag_x_ptr: Value,
    pub homotopy_lambda_ptr: Value,
}

/// Emit a call to a block function from within calc_derivs.
/// Passes through all calc_derivs arguments.
pub fn emit_block_call(
    ctx: &mut TranslationContext,
    builder: &mut FunctionBuilder,
    block_func_id: FuncId,
    args: &BlockCallArgs,
) -> Result<Value, String> {
    let func_ref = ctx.module.declare_func_in_func(block_func_id, &mut builder.func);
    let call_args = vec![
        args.time,
        args.states_ptr,
        args.discrete_ptr,
        args.derivs_ptr,
        args.params_ptr,
        args.outputs_ptr,
        args.when_states_ptr,
        args.crossings_ptr,
        args.pre_states_ptr,
        args.pre_discrete_ptr,
        args.t_end,
        args.diag_residual_ptr,
        args.diag_x_ptr,
        args.homotopy_lambda_ptr,
    ];
    let call_inst = builder.ins().call(func_ref, &call_args);
    Ok(builder.inst_results(call_inst)[0])
}

/// Deferred block data preserved for hot-swap recompilation.
pub struct DeferredBlockData {
    pub func_id: FuncId,
    pub unknowns: Vec<String>,
    pub tearing_var: Option<String>,
    pub equations: Vec<crate::ast::Equation>,
    pub residuals: Vec<crate::ast::Expression>,
}

/// Registry of compiled blocks for potential recompilation and hot-swapping.
pub struct BlockRegistry {
    pub blocks: Vec<CompiledBlock>,
    /// Preserved block data for recompilation (hot-swap, tier-up).
    pub deferred: Vec<DeferredBlockData>,
}

impl BlockRegistry {
    pub fn new() -> Self {
        Self { blocks: Vec::new(), deferred: Vec::new() }
    }

    pub fn register(&mut self, func_id: FuncId, block_index: usize) {
        self.blocks.push(CompiledBlock {
            func_id,
            block_index,
        });
    }

    /// Store deferred block data for potential recompilation.
    pub fn defer_block(
        &mut self,
        func_id: FuncId,
        unknowns: Vec<String>,
        tearing_var: Option<String>,
        equations: Vec<crate::ast::Equation>,
        residuals: Vec<crate::ast::Expression>,
    ) {
        self.deferred.push(DeferredBlockData {
            func_id,
            unknowns,
            tearing_var,
            equations,
            residuals,
        });
    }

    /// Get deferred blocks that can be recompiled.
    pub fn recompilable_blocks(&self) -> &[DeferredBlockData] {
        &self.deferred
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_compile_disabled_by_default() {
        assert!(!block_compile_enabled());
    }

    #[test]
    fn test_block_registry_empty() {
        let reg = BlockRegistry::new();
        assert!(reg.blocks.is_empty());
    }
}
