use crate::ast::Expression;
use crate::jit::context::TranslationContext;
use crate::jit::types::ArrayType;
use crate::jit::translator::expr::helpers::{
    jit_builtin_fallback_warn_once, jit_strict_placeholders_enabled,
};
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::Module;

pub(super) fn compile_first_true_index(
    args: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    _compile_rec: fn(
        &Expression,
        &mut TranslationContext,
        &mut cranelift::frontend::FunctionBuilder<'_>,
    ) -> Result<Value, String>,
) -> Result<Value, String> {
    if args.len() != 1 {
        return Err(format!(
            "firstTrueIndex() expects 1 argument (Boolean vector), got {}",
            args.len()
        ));
    }
    if let Expression::Variable(id) = &args[0] {
        let vec_name = crate::string_intern::resolve_id(*id);
        if let Some(info) = ctx.array_info.get(&vec_name) {
            if info.size == 0 {
                return Ok(builder.ins().f64const(0.0));
            }
            let base_ptr = match info.array_type {
                ArrayType::State => ctx.states_ptr,
                ArrayType::Discrete => ctx.discrete_ptr,
                ArrayType::Parameter => ctx.params_ptr,
                ArrayType::Output => ctx.outputs_ptr,
                ArrayType::Derivative => ctx.derivs_ptr,
            };
            let zero = builder.ins().f64const(0.0);
            let start_idx = builder.ins().iconst(cl_types::I64, 0);
            let end_idx = builder.ins().iconst(cl_types::I64, info.size as i64);
            let loop_slot = builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
                cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
                8,
                0,
            ));
            let result_slot = builder.create_sized_stack_slot(cranelift::codegen::ir::StackSlotData::new(
                cranelift::codegen::ir::StackSlotKind::ExplicitSlot,
                8,
                0,
            ));
            builder.ins().stack_store(start_idx, loop_slot, 0);
            builder.ins().stack_store(zero, result_slot, 0);
            let header = builder.create_block();
            let body_block = builder.create_block();
            let found_block = builder.create_block();
            let next_block = builder.create_block();
            let exit_block = builder.create_block();
            let after_loop = builder.create_block();
            builder.ins().jump(header, &[]);
            builder.switch_to_block(header);
            let i_val = builder.ins().stack_load(cl_types::I64, loop_slot, 0);
            let cmp = builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, i_val, end_idx);
            builder.ins().brif(cmp, exit_block, &[], body_block, &[]);
            builder.switch_to_block(body_block);
            let i_int = builder.ins().stack_load(cl_types::I64, loop_slot, 0);
            let eight = builder.ins().iconst(cl_types::I64, 8);
            let offset_bytes = builder.ins().imul(i_int, eight);
            let base_offset = builder.ins().iconst(cl_types::I64, (info.start_index * 8) as i64);
            let offset_sum = builder.ins().iadd(base_offset, offset_bytes);
            let addr = builder.ins().iadd(base_ptr, offset_sum);
            let elem = builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0);
            let is_true = builder.ins().fcmp(FloatCC::NotEqual, elem, zero);
            builder.ins().brif(is_true, found_block, &[], next_block, &[]);
            builder.switch_to_block(next_block);
            let one_i = builder.ins().iconst(cl_types::I64, 1);
            let next_i = builder.ins().iadd(i_int, one_i);
            builder.ins().stack_store(next_i, loop_slot, 0);
            builder.ins().jump(header, &[]);
            builder.switch_to_block(found_block);
            let one_i2 = builder.ins().iconst(cl_types::I64, 1);
            let i_plus_one = builder.ins().iadd(i_int, one_i2);
            let idx_f64 = builder.ins().fcvt_from_sint(cl_types::F64, i_plus_one);
            builder.ins().stack_store(idx_f64, result_slot, 0);
            builder.ins().jump(exit_block, &[]);
            builder.switch_to_block(exit_block);
            let result_val = builder.ins().stack_load(cl_types::F64, result_slot, 0);
            builder.ins().jump(after_loop, &[]);
            builder.seal_block(exit_block);
            builder.switch_to_block(after_loop);
            builder.seal_block(header);
            builder.seal_block(body_block);
            builder.seal_block(next_block);
            builder.seal_block(found_block);
            return Ok(result_val);
        }
    }
    if jit_strict_placeholders_enabled() {
        return Err("JIT strict placeholders: firstTrueIndex() requires a resolved Boolean array variable".to_string());
    }
    jit_builtin_fallback_warn_once("firstTrueIndex", "firstTrueIndex-non-array");
    Ok(builder.ins().f64const(0.0))
}

pub(super) fn compile_interpolate_vectors(
    args: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: fn(
        &Expression,
        &mut TranslationContext,
        &mut cranelift::frontend::FunctionBuilder<'_>,
    ) -> Result<Value, String>,
) -> Result<Value, String> {
    if args.len() < 3 {
        return Err(format!(
            "interpolate(x, xa, ya) expects at least 3 arguments, got {}",
            args.len()
        ));
    }
    let x = compile_rec(&args[0], ctx, builder)?;
    let xa = &args[1];
    let ya = &args[2];
    if let (Expression::Variable(xan_id), Expression::Variable(yan_id)) = (xa, ya) {
        let xan = crate::string_intern::resolve_id(*xan_id);
        let yan = crate::string_intern::resolve_id(*yan_id);
        if let (Some(xai), Some(yai)) = (ctx.array_info.get(&xan), ctx.array_info.get(&yan)) {
            if xai.size == 0 || yai.size == 0 {
                jit_builtin_fallback_warn_once("interpolate", "interpolate-empty-array");
                return Ok(builder.ins().f64const(0.0));
            }
            let xa_ptr = match xai.array_type {
                ArrayType::State => ctx.states_ptr,
                ArrayType::Discrete => ctx.discrete_ptr,
                ArrayType::Parameter => ctx.params_ptr,
                ArrayType::Output => ctx.outputs_ptr,
                ArrayType::Derivative => ctx.derivs_ptr,
            };
            let ya_ptr = match yai.array_type {
                ArrayType::State => ctx.states_ptr,
                ArrayType::Discrete => ctx.discrete_ptr,
                ArrayType::Parameter => ctx.params_ptr,
                ArrayType::Output => ctx.outputs_ptr,
                ArrayType::Derivative => ctx.derivs_ptr,
            };
            let x0_offset = (xai.start_index * 8) as i64;
            let y0_offset = (yai.start_index * 8) as i64;
            let x0 = builder
                .ins()
                .load(cl_types::F64, MemFlags::new(), xa_ptr, x0_offset as i32);
            let y0 = builder
                .ins()
                .load(cl_types::F64, MemFlags::new(), ya_ptr, y0_offset as i32);
            if xai.size == 1 {
                return Ok(y0);
            }
            let x1_offset = (xai.start_index + 1) * 8;
            let y1_offset = (yai.start_index + 1) * 8;
            let x1 = builder
                .ins()
                .load(cl_types::F64, MemFlags::new(), xa_ptr, x1_offset as i32);
            let y1 = builder
                .ins()
                .load(cl_types::F64, MemFlags::new(), ya_ptr, y1_offset as i32);
            let dx = builder.ins().fsub(x1, x0);
            let t = builder.ins().fsub(x, x0);
            let dy = builder.ins().fsub(y1, y0);
            let div = builder.ins().fdiv(t, dx);
            let interp = builder.ins().fmul(div, dy);
            let y_val = builder.ins().fadd(y0, interp);
            return Ok(y_val);
        }
    }
    if jit_strict_placeholders_enabled() {
        return Err("JIT strict placeholders: interpolate() requires resolved array variables".to_string());
    }
    jit_builtin_fallback_warn_once("interpolate", "interpolate-non-array-fallback");
    Ok(builder.ins().f64const(0.0))
}

pub(super) fn compile_interp_coef(
    func_name: &str,
    args: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: fn(
        &Expression,
        &mut TranslationContext,
        &mut cranelift::frontend::FunctionBuilder<'_>,
    ) -> Result<Value, String>,
) -> Result<Value, String> {
    if args.len() >= 2 {
        let u_val = match compile_rec(&args[1], ctx, builder) {
            Ok(v) => v,
            Err(_) => return Ok(builder.ins().f64const(0.0)),
        };
        let mut sig = ctx.module.make_signature();
        sig.params.push(AbiParam::new(cl_types::F64));
        sig.returns.push(AbiParam::new(cl_types::F64));
        let func_id = ctx
            .module
            .declare_function("floor", cranelift_module::Linkage::Import, &sig)
            .map_err(|e| e.to_string())?;
        let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
        let call = builder.ins().call(func_ref, &[u_val]);
        let floor_u = builder.inst_results(call)[0];
        let h = builder.ins().fsub(u_val, floor_u);
        return Ok(h);
    }
    if jit_strict_placeholders_enabled() {
        return Err(format!(
            "JIT strict placeholders: interpolation coefficient needs 2+ args ({})",
            func_name
        ));
    }
    jit_builtin_fallback_warn_once(func_name, "interpolation-coeff-impl");
    Ok(builder.ins().f64const(0.0))
}
