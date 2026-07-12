use crate::ast::{AlgorithmStatement, Expression};
use crate::jit::context::TranslationContext;
use crate::jit::translator::expr::{compile_expression, compile_zero_crossing_store};
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;

type CompileStmt = fn(
    &AlgorithmStatement,
    &mut TranslationContext,
    &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String>;

pub(super) fn compile_if_stmt(
    cond: &Expression,
    true_stmts: &[AlgorithmStatement],
    else_ifs: &[(Expression, Vec<AlgorithmStatement>)],
    else_stmts: Option<&Vec<AlgorithmStatement>>,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_stmt: CompileStmt,
) -> Result<(), String> {
    let true_block = builder.create_block();
    let mut next_block = builder.create_block();
    let end_block = builder.create_block();
    let cond_val = compile_expression(cond, ctx, builder)?;
    let zero = builder.ins().f64const(0.0);
    let cond_bool = builder.ins().fcmp(FloatCC::NotEqual, cond_val, zero);
    builder
        .ins()
        .brif(cond_bool, true_block, &[], next_block, &[]);
    builder.switch_to_block(true_block);
    for s in true_stmts {
        compile_stmt(s, ctx, builder)?;
    }
    builder.ins().jump(end_block, &[]);
    builder.seal_block(true_block);
    for (cond, stmts) in else_ifs {
        let check_block = next_block;
        let body_block = builder.create_block();
        next_block = builder.create_block();
        builder.switch_to_block(check_block);
        let c_val = compile_expression(cond, ctx, builder)?;
        let c_bool = builder.ins().fcmp(FloatCC::NotEqual, c_val, zero);
        builder.ins().brif(c_bool, body_block, &[], next_block, &[]);
        builder.seal_block(check_block);
        builder.switch_to_block(body_block);
        for s in stmts {
            compile_stmt(s, ctx, builder)?;
        }
        builder.ins().jump(end_block, &[]);
        builder.seal_block(body_block);
    }
    builder.switch_to_block(next_block);
    if let Some(stmts) = else_stmts {
        for s in stmts {
            compile_stmt(s, ctx, builder)?;
        }
    }
    builder.ins().jump(end_block, &[]);
    builder.seal_block(next_block);
    builder.switch_to_block(end_block);
    Ok(())
}

pub(super) fn compile_while_stmt(
    cond: &Expression,
    body: &[AlgorithmStatement],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_stmt: CompileStmt,
) -> Result<(), String> {
    let header_block = builder.create_block();
    let body_block = builder.create_block();
    let exit_block = builder.create_block();
    let after_while = builder.create_block();
    builder.ins().jump(header_block, &[]);
    builder.switch_to_block(header_block);
    let c_val = compile_expression(cond, ctx, builder)?;
    let zero = builder.ins().f64const(0.0);
    let c_bool = builder.ins().fcmp(FloatCC::NotEqual, c_val, zero);
    builder.ins().brif(c_bool, body_block, &[], exit_block, &[]);
    builder.switch_to_block(body_block);
    ctx.loop_break_stack.push(after_while);
    for s in body {
        compile_stmt(s, ctx, builder)?;
    }
    let _ = ctx.loop_break_stack.pop();
    builder.ins().jump(header_block, &[]);
    builder.seal_block(body_block);
    builder.switch_to_block(exit_block);
    builder.ins().jump(after_while, &[]);
    builder.seal_block(header_block);
    builder.seal_block(exit_block);
    builder.switch_to_block(after_while);
    Ok(())
}

pub(super) fn compile_for_stmt(
    var_name: &str,
    range_expr: &Expression,
    body: &[AlgorithmStatement],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_stmt: CompileStmt,
) -> Result<(), String> {
    let (start_val, step_val, end_val) = if let Expression::Range(start, step, end) = range_expr {
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
        return Err(format!(
            "Loop variable '{}' stack slot not found.",
            var_name
        ));
    };
    builder.ins().stack_store(start_val, loop_var_slot, 0);
    let header_block = builder.create_block();
    let body_block = builder.create_block();
    let exit_block = builder.create_block();
    let after_for = builder.create_block();
    builder.ins().jump(header_block, &[]);
    builder.switch_to_block(header_block);
    let curr_i = builder.ins().stack_load(cl_types::F64, loop_var_slot, 0);
    // Loop continuation depends on step sign: `i <= end` when ascending,
    // `i >= end` when descending. A fixed `<=` made every negative-step range
    // (e.g. `for i in 3:-1:1`) skip the body entirely.
    let zero = builder.ins().f64const(0.0);
    let step_nonneg = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, step_val, zero);
    let le = builder.ins().fcmp(FloatCC::LessThanOrEqual, curr_i, end_val);
    let ge = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, curr_i, end_val);
    let cmp = builder.ins().select(step_nonneg, le, ge);
    builder.ins().brif(cmp, body_block, &[], exit_block, &[]);
    builder.switch_to_block(body_block);
    ctx.loop_break_stack.push(after_for);
    for s in body {
        compile_stmt(s, ctx, builder)?;
    }
    let _ = ctx.loop_break_stack.pop();
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

pub(super) fn compile_when_stmt(
    cond: &Expression,
    body: &[AlgorithmStatement],
    else_whens: &[(Expression, Vec<AlgorithmStatement>)],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_stmt: CompileStmt,
) -> Result<(), String> {
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
    let pre_cond_val = builder.ins().load(
        cl_types::F64,
        MemFlags::new(),
        ctx.when_states_ptr,
        offset_pre,
    );
    let one = builder.ins().f64const(1.0);
    let cond_norm = builder.ins().select(cond_bool, one, zero);
    builder
        .ins()
        .store(MemFlags::new(), cond_norm, ctx.when_states_ptr, offset_new);
    let pre_zero = builder.ins().fcmp(FloatCC::Equal, pre_cond_val, zero);
    let edge = builder.ins().band(cond_bool, pre_zero);
    builder.ins().brif(edge, true_block, &[], next_block, &[]);
    builder.switch_to_block(true_block);
    for s in body {
        compile_stmt(s, ctx, builder)?;
    }
    builder.ins().jump(end_block, &[]);
    builder.seal_block(true_block);
    for (cond, stmts) in else_whens {
        let check_block = next_block;
        let body_block = builder.create_block();
        next_block = builder.create_block();
        builder.switch_to_block(check_block);
        compile_zero_crossing_store(cond, ctx, builder)?;
        let c_val = compile_expression(cond, ctx, builder)?;
        let curr_idx = *ctx.when_idx;
        *ctx.when_idx += 1;
        let offset_pre = (curr_idx * 16) as i32;
        let offset_new = (curr_idx * 16 + 8) as i32;
        let pre_c = builder.ins().load(
            cl_types::F64,
            MemFlags::new(),
            ctx.when_states_ptr,
            offset_pre,
        );
        let c_bool = builder.ins().fcmp(FloatCC::NotEqual, c_val, zero);
        let one = builder.ins().f64const(1.0);
        let c_norm = builder.ins().select(c_bool, one, zero);
        builder
            .ins()
            .store(MemFlags::new(), c_norm, ctx.when_states_ptr, offset_new);
        let pre_c_zero = builder.ins().fcmp(FloatCC::Equal, pre_c, zero);
        let c_edge = builder.ins().band(c_bool, pre_c_zero);
        builder.ins().brif(c_edge, body_block, &[], next_block, &[]);
        builder.seal_block(check_block);
        builder.switch_to_block(body_block);
        for s in stmts {
            compile_stmt(s, ctx, builder)?;
        }
        builder.ins().jump(end_block, &[]);
        builder.seal_block(body_block);
    }
    builder.switch_to_block(next_block);
    builder.ins().jump(end_block, &[]);
    builder.seal_block(next_block);
    builder.switch_to_block(end_block);
    Ok(())
}
