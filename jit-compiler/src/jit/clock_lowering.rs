//! Clock sample triggers and guarded partition lowering.

use crate::ast::*;
use crate::compiler::ClockPartitionScheduleEntry;
use crate::jit::context::TranslationContext;
use crate::jit::translator::{compile_algorithm_stmt, compile_equation};
#[cfg(not(windows))]
use cranelift::codegen::binemit::Reloc;
use cranelift::prelude::{types as cl_types, *};
use cranelift_module::{Linkage, Module};

/// Veneer tail allocation per out-of-range `Arm64Call` in cranelift-jit `CompiledBlob` (must match JIT).
#[cfg(not(windows))]
const JIT_ARM64_VENEER_SIZE: usize = 24;

/// Executable allocation size for the last `define_function` on `ctx`: body + AArch64 veneers.
pub(crate) fn jit_executable_allocation_len(ctx: &codegen::Context) -> Option<usize> {
    let cc = ctx.compiled_code()?;
    let body_len = cc.code_buffer().len();
    #[cfg(windows)]
    {
        Some(body_len)
    }
    #[cfg(not(windows))]
    {
        let veneer_slots = cc
            .buffer
            .relocs()
            .iter()
            .filter(|r| r.kind == Reloc::Arm64Call)
            .count();
        Some(body_len + veneer_slots * JIT_ARM64_VENEER_SIZE)
    }
}

pub(crate) fn emit_sample_trigger(
    start: f64,
    interval: f64,
    t_ctx: &mut TranslationContext,
    builder: &mut FunctionBuilder,
) -> Result<Value, String> {
    let time_val = t_ctx
        .var_map
        .get("time")
        .copied()
        .ok_or_else(|| "sample trigger requires time".to_string())?;
    let mut sig = t_ctx.module.make_signature();
    sig.params.push(AbiParam::new(cl_types::F64));
    sig.params.push(AbiParam::new(cl_types::F64));
    sig.returns.push(AbiParam::new(cl_types::F64));
    let func_id = t_ctx
        .module
        .declare_function("rustmodlica_sample", Linkage::Import, &sig)
        .map_err(|e| e.to_string())?;
    let func_ref = t_ctx.module.declare_func_in_func(func_id, &mut builder.func);
    let start_val = builder.ins().f64const(start);
    let interval_val = builder.ins().f64const(interval);
    let t_rel = builder.ins().fsub(time_val, start_val);
    let call = builder.ins().call(func_ref, &[t_rel, interval_val]);
    Ok(builder.inst_results(call)[0])
}

pub(crate) fn compile_guarded_partition(
    trigger_val: Value,
    t_ctx: &mut TranslationContext,
    builder: &mut FunctionBuilder,
    algorithms: &[AlgorithmStatement],
    alg_equations: &[Equation],
    diff_equations: &[Equation],
    entry: &ClockPartitionScheduleEntry,
) -> Result<(), String> {
    let zero = builder.ins().f64const(0.0);
    let active = builder.ins().fcmp(FloatCC::NotEqual, trigger_val, zero);
    let then_block = builder.create_block();
    let cont_block = builder.create_block();
    builder.ins().brif(active, then_block, &[], cont_block, &[]);

    builder.switch_to_block(then_block);
    for idx in &entry.algorithm_indices {
        if let Some(stmt) = algorithms.get(*idx) {
            compile_algorithm_stmt(stmt, t_ctx, builder)?;
        }
    }
    for idx in &entry.alg_equation_indices {
        if let Some(eq) = alg_equations.get(*idx) {
            compile_equation(eq, t_ctx, builder)?;
        }
    }
    for idx in &entry.diff_equation_indices {
        if let Some(eq) = diff_equations.get(*idx) {
            compile_equation(eq, t_ctx, builder)?;
        }
    }
    builder.ins().jump(cont_block, &[]);
    builder.seal_block(then_block);

    builder.switch_to_block(cont_block);
    builder.seal_block(cont_block);
    Ok(())
}
