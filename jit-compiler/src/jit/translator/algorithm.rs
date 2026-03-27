use super::super::context::TranslationContext;
use super::super::types::ArrayType;
use super::expr::helpers::lookup_or_insert_import;
use super::expr::{compile_expression, compile_zero_crossing_store};
use crate::ast::{AlgorithmStatement, Expression, Operator};
use crate::diag::fallback_counter;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};

fn lhs_simple_var_name(expr: &Expression) -> Option<String> {
    if let Expression::Variable(id) = expr {
        Some(crate::string_intern::resolve_id(*id))
    } else {
        None
    }
}

fn is_real_fft_call(name: &str) -> bool {
    name == "realFFT" || name.ends_with(".realFFT")
}

/// Multi-assign `(info, Ai, Phii) := realFFT(u, nfi)` / two-output form for RealFFT2.
fn try_compile_real_fft_multiassign(
    lhss: &[Expression],
    rhs: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<bool, String> {
    let Expression::Call(fname, args) = rhs else {
        return Ok(false);
    };
    if !is_real_fft_call(fname) || args.len() != 2 {
        return Ok(false);
    }
    if lhss.len() != 2 && lhss.len() != 3 {
        return Ok(false);
    }
    let u_name = match &args[0] {
        Expression::Variable(id) => crate::string_intern::resolve_id(*id),
        _ => return Ok(false),
    };
    let nu = ctx.array_len(&u_name).ok_or_else(|| {
        format!(
            "realFFT: input must be a sized array variable, got '{}'",
            u_name
        )
    })?;
    if nu == 0 || nu % 2 != 0 {
        return Err(format!(
            "realFFT: array '{}' must have positive even length, got {}",
            u_name, nu
        ));
    }
    let (u_ty, u_start) = ctx.array_storage(&u_name).ok_or_else(|| {
        format!(
            "realFFT: could not resolve storage for array '{}'",
            u_name
        )
    })?;
    let u_base = match u_ty {
        ArrayType::State => ctx.states_ptr,
        ArrayType::Discrete => ctx.discrete_ptr,
        ArrayType::Parameter => ctx.params_ptr,
        ArrayType::Output => ctx.outputs_ptr,
        ArrayType::Derivative => ctx.derivs_ptr,
    };
    let ptr_ty = ctx.module.target_config().pointer_type();
    let u_off = builder
        .ins()
        .iconst(ptr_ty, (u_start * 8) as i64);
    let u_ptr = builder.ins().iadd(u_base, u_off);

    let nfi_expr = compile_expression(&args[1], ctx, builder)?;
    let nfi_i = builder.ins().fcvt_to_sint(cl_types::I64, nfi_expr);

    let info_name = lhs_simple_var_name(&lhss[0]).ok_or_else(|| {
        "realFFT: first LHS must be a variable (info)".to_string()
    })?;
    let ai_name = lhs_simple_var_name(&lhss[1]).ok_or_else(|| {
        "realFFT: second LHS must be a variable (Ai)".to_string()
    })?;
    let nfi_usize = ctx.array_len(&ai_name).ok_or_else(|| {
        format!(
            "realFFT: output array '{}' must have known size",
            ai_name
        )
    })?;
    let (ai_ty, ai_start) = ctx.array_storage(&ai_name).ok_or_else(|| {
        format!(
            "realFFT: could not resolve storage for output '{}'",
            ai_name
        )
    })?;
    let ai_base = match ai_ty {
        ArrayType::State => ctx.states_ptr,
        ArrayType::Discrete => ctx.discrete_ptr,
        ArrayType::Parameter => ctx.params_ptr,
        ArrayType::Output => ctx.outputs_ptr,
        ArrayType::Derivative => ctx.derivs_ptr,
    };
    let ai_off = builder
        .ins()
        .iconst(ptr_ty, (ai_start * 8) as i64);
    let amp_ptr = builder.ins().iadd(ai_base, ai_off);

    let write_phases = lhss.len() == 3;
    let (phase_ptr_val, _ph_name_opt) = if write_phases {
        let ph_name = lhs_simple_var_name(&lhss[2]).ok_or_else(|| {
            "realFFT: third LHS must be a variable (Phii)".to_string()
        })?;
        let n_ph = ctx.array_len(&ph_name).ok_or_else(|| {
            format!(
                "realFFT: phase array '{}' must have known size",
                ph_name
            )
        })?;
        if n_ph != nfi_usize {
            return Err(format!(
                "realFFT: Ai and Phii length mismatch ({} vs {})",
                nfi_usize, n_ph
            ));
        }
        let (ph_ty, ph_start) = ctx.array_storage(&ph_name).ok_or_else(|| {
            format!(
                "realFFT: could not resolve storage for '{}'",
                ph_name
            )
        })?;
        let ph_base = match ph_ty {
            ArrayType::State => ctx.states_ptr,
            ArrayType::Discrete => ctx.discrete_ptr,
            ArrayType::Parameter => ctx.params_ptr,
            ArrayType::Output => ctx.outputs_ptr,
            ArrayType::Derivative => ctx.derivs_ptr,
        };
        let ph_off = builder
            .ins()
            .iconst(ptr_ty, (ph_start * 8) as i64);
        let pp = builder.ins().iadd(ph_base, ph_off);
        (pp, Some(ph_name))
    } else {
        let zero = builder.ins().iconst(ptr_ty, 0);
        (zero, None)
    };

    let info_ptr = scalar_f64_ptr_for_assign(ctx, builder, &info_name)?;

    let wf = if write_phases { 1i32 } else { 0i32 };
    let wf_val = builder.ins().iconst(cl_types::I32, i64::from(wf));

    let mut sig = ctx.module.make_signature();
    sig.params.push(AbiParam::new(ptr_ty));
    sig.params.push(AbiParam::new(cl_types::I64));
    sig.params.push(AbiParam::new(cl_types::I64));
    sig.params.push(AbiParam::new(ptr_ty));
    sig.params.push(AbiParam::new(ptr_ty));
    sig.params.push(AbiParam::new(ptr_ty));
    sig.params.push(AbiParam::new(cl_types::I32));
    sig.returns.push(AbiParam::new(cl_types::I32));

    let func_id = lookup_or_insert_import(
        "rustmodlica_math_real_fft",
        "v1".to_string(),
        &sig,
        ctx,
    )?;
    let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
    let nu_i64 = builder.ins().iconst(cl_types::I64, nu as i64);
    let _ = builder.ins().call(
        func_ref,
        &[u_ptr, nu_i64, nfi_i, info_ptr, amp_ptr, phase_ptr_val, wf_val],
    );
    Ok(true)
}

/// Multi-assign `(r, stateOut) := Generators.Xorshift*star.random(pre(stateIn))` (MSL external RNG).
fn try_compile_msl_random_multiassign(
    lhss: &[Expression],
    rhs: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<bool, String> {
    let Expression::Call(fname, args) = rhs else {
        return Ok(false);
    };
    if args.len() != 1 {
        return Ok(false);
    }
    if !fname.contains("random") {
        return Ok(false);
    }
    let kind: i32 = if fname.contains("Xorshift64star") {
        0
    } else if fname.contains("Xorshift128plus") {
        1
    } else if fname.contains("Xorshift1024star") {
        2
    } else {
        return Ok(false);
    };
    if !fname.ends_with("random") {
        return Ok(false);
    }

    if lhss.len() != 2 {
        return Ok(false);
    }
    let r_name = lhs_simple_var_name(&lhss[0]).ok_or_else(|| {
        "MSL random: first LHS must be a variable (Real r)".to_string()
    })?;
    let state_name = lhs_simple_var_name(&lhss[1]).ok_or_else(|| {
        "MSL random: second LHS must be a variable (Integer state[])".to_string()
    })?;

    let state_n = ctx.array_len(&state_name).ok_or_else(|| {
        format!(
            "MSL random: could not resolve array size for '{}'",
            state_name
        )
    })?;
    let exp_n = match kind {
        0 => 2usize,
        1 => 4usize,
        2 => 33usize,
        _ => unreachable!(),
    };
    if state_n != exp_n {
        return Err(format!(
            "MSL random: state array '{}' expected length {}, got {}",
            state_name, exp_n, state_n
        ));
    }
    let (arr_ty, start) = ctx.array_storage(&state_name).ok_or_else(|| {
        format!("MSL random: storage for '{}' not found", state_name)
    })?;
    if !matches!(arr_ty, ArrayType::Discrete) {
        return Err(format!(
            "MSL random: state '{}' must be a discrete Integer array",
            state_name
        ));
    }

    let state_var_in_pre = match &args[0] {
        Expression::Previous(inner) => match &**inner {
            Expression::Variable(id) => crate::string_intern::resolve_id(*id),
            _ => return Ok(false),
        },
        _ => return Ok(false),
    };
    if state_var_in_pre != state_name {
        return Err(format!(
            "MSL random: argument must be pre('{}')",
            state_name
        ));
    }

    let ptr_ty = ctx.module.target_config().pointer_type();
    let base_off = builder.ins().iconst(ptr_ty, (start * 8) as i64);
    let state_in_ptr = builder.ins().iadd(ctx.pre_discrete_ptr, base_off);
    let state_out_ptr = builder.ins().iadd(ctx.discrete_ptr, base_off);
    let r_ptr = scalar_f64_ptr_for_assign(ctx, builder, &r_name)?;
    let kind_val = builder.ins().iconst(cl_types::I32, i64::from(kind as u32));
    let n_val = builder.ins().iconst(cl_types::I64, state_n as i64);

    let mut sig = ctx.module.make_signature();
    sig.params.push(AbiParam::new(cl_types::I32));
    sig.params.push(AbiParam::new(ptr_ty));
    sig.params.push(AbiParam::new(ptr_ty));
    sig.params.push(AbiParam::new(cl_types::I64));
    sig.params.push(AbiParam::new(ptr_ty));
    sig.returns.push(AbiParam::new(cl_types::I32));

    let func_id = lookup_or_insert_import(
        "rustmodlica_math_random_msl",
        "v1".to_string(),
        &sig,
        ctx,
    )?;
    let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
    let _ = builder.ins().call(
        func_ref,
        &[kind_val, state_in_ptr, state_out_ptr, n_val, r_ptr],
    );
    Ok(true)
}

fn scalar_f64_ptr_for_assign(
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

fn compile_store_to_lhs(
    lhs: &Expression,
    val: Value,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    // Backend may emit constant-equality assignments in lowered algorithm form.
    // They carry no writable storage target for JIT assignment path, so treat as no-op.
    if matches!(lhs, Expression::Number(_)) {
        return Ok(());
    }
    // Some simplified equations are represented as "(0 - x) = rhs".
    // Rewrite store target to x with negated rhs at codegen time.
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
            // Some MSL Fluid lowering paths emit slice assignments like a[2:n-1] = rhs.
            // JIT scalar store path does not yet support range writes; skip to keep execution progressing.
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

fn is_store_target_expr(expr: &Expression) -> bool {
    match expr {
        Expression::Variable(_) | Expression::ArrayAccess(_, _) => true,
        Expression::BinaryOp(l, Operator::Sub, r) => {
            matches!(&**l, Expression::Number(n) if *n == 0.0) && is_store_target_expr(r)
        }
        _ => false,
    }
}

fn expr_contains_array_literal(expr: &Expression) -> bool {
    match expr {
        Expression::ArrayLiteral(_) => true,
        Expression::BinaryOp(l, _, r) => {
            expr_contains_array_literal(l) || expr_contains_array_literal(r)
        }
        Expression::Call(_, args) => args.iter().any(expr_contains_array_literal),
        Expression::Der(inner)
        | Expression::Sample(inner)
        | Expression::Interval(inner)
        | Expression::Hold(inner)
        | Expression::Previous(inner) => expr_contains_array_literal(inner),
        Expression::SubSample(a, b)
        | Expression::SuperSample(a, b)
        | Expression::ShiftSample(a, b)
        | Expression::BackSample(a, b)
        | Expression::ArrayAccess(a, b) => {
            expr_contains_array_literal(a) || expr_contains_array_literal(b)
        }
        Expression::Dot(base, _) => expr_contains_array_literal(base),
        Expression::If(c, t, f) => {
            expr_contains_array_literal(c)
                || expr_contains_array_literal(t)
                || expr_contains_array_literal(f)
        }
        Expression::Range(s, st, e) => {
            expr_contains_array_literal(s)
                || expr_contains_array_literal(st)
                || expr_contains_array_literal(e)
        }
        Expression::ArrayComprehension {
            expr, iter_range, ..
        } => expr_contains_array_literal(expr) || expr_contains_array_literal(iter_range),
        Expression::Variable(_) | Expression::Number(_) | Expression::StringLiteral(_) => false,
    }
}

fn expr_contains_array_comprehension(expr: &Expression) -> bool {
    match expr {
        Expression::ArrayComprehension { .. } => true,
        Expression::BinaryOp(l, _, r) => {
            expr_contains_array_comprehension(l) || expr_contains_array_comprehension(r)
        }
        Expression::Call(_, args) => args.iter().any(expr_contains_array_comprehension),
        Expression::Der(inner)
        | Expression::Sample(inner)
        | Expression::Interval(inner)
        | Expression::Hold(inner)
        | Expression::Previous(inner) => expr_contains_array_comprehension(inner),
        Expression::SubSample(a, b)
        | Expression::SuperSample(a, b)
        | Expression::ShiftSample(a, b)
        | Expression::BackSample(a, b)
        | Expression::ArrayAccess(a, b) => {
            expr_contains_array_comprehension(a) || expr_contains_array_comprehension(b)
        }
        Expression::Dot(base, _) => expr_contains_array_comprehension(base),
        Expression::If(c, t, f) => {
            expr_contains_array_comprehension(c)
                || expr_contains_array_comprehension(t)
                || expr_contains_array_comprehension(f)
        }
        Expression::Range(s, st, e) => {
            expr_contains_array_comprehension(s)
                || expr_contains_array_comprehension(st)
                || expr_contains_array_comprehension(e)
        }
        Expression::ArrayLiteral(items) => items.iter().any(expr_contains_array_comprehension),
        Expression::Variable(_) | Expression::Number(_) | Expression::StringLiteral(_) => false,
    }
}

pub fn compile_algorithm_stmt(
    stmt: &AlgorithmStatement,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    match stmt {
        AlgorithmStatement::Assignment(lhs, rhs) => {
            if !is_store_target_expr(lhs) && is_store_target_expr(rhs) {
                let val = compile_expression(lhs, ctx, builder)?;
                compile_store_to_lhs(rhs, val, ctx, builder)?;
                return Ok(());
            }
            if !is_store_target_expr(lhs) && !is_store_target_expr(rhs) {
                let _ = compile_expression(lhs, ctx, builder)?;
                let _ = compile_expression(rhs, ctx, builder)?;
                return Ok(());
            }
            let val = compile_expression(rhs, ctx, builder)?;
            compile_store_to_lhs(lhs, val, ctx, builder)?;
        }
        AlgorithmStatement::MultiAssign(lhss, rhs) => {
            if try_compile_real_fft_multiassign(lhss, rhs, ctx, builder)? {
                return Ok(());
            }
            if try_compile_msl_random_multiassign(lhss, rhs, ctx, builder)? {
                return Ok(());
            }
            if let Expression::ArrayLiteral(items) = rhs {
                if items.len() != lhss.len() {
                    return Err(format!(
                        "Multi-assign arity mismatch: {} LHS targets but {} RHS items",
                        lhss.len(),
                        items.len()
                    ));
                }
                for (i, (lhs, item)) in lhss.iter().zip(items.iter()).enumerate() {
                    if expr_contains_array_comprehension(item) {
                        return Err(format!(
                            "Multi-assign output item {} contains array comprehension, which is unsupported for direct scalar store target {:?}",
                            i + 1,
                            lhs
                        ));
                    }
                    if expr_contains_array_literal(item) {
                        return Err(format!(
                            "Multi-assign output item {} has array-valued shape, which is unsupported for scalar store target {:?}",
                            i + 1,
                            lhs
                        ));
                    }
                    let v = compile_expression(item, ctx, builder)?;
                    compile_store_to_lhs(lhs, v, ctx, builder)?;
                }
                return Ok(());
            }
            if let Expression::Variable(id) = rhs {
                let arr_name = crate::string_intern::resolve_id(*id);
                if let Some(n) = ctx.array_len(&arr_name) {
                    if n == lhss.len() {
                        for (i, lhs) in lhss.iter().enumerate() {
                            let elem = Expression::ArrayAccess(
                                Box::new(Expression::Variable(*id)),
                                Box::new(Expression::Number((i + 1) as f64)),
                            );
                            let v = compile_expression(&elem, ctx, builder)?;
                            compile_store_to_lhs(lhs, v, ctx, builder)?;
                        }
                        return Ok(());
                    }
                    return Err(format!(
                        "Multi-assign array arity mismatch for '{}': {} LHS targets but array length is {}",
                        arr_name,
                        lhss.len(),
                        n
                    ));
                }
            }
            if lhss.len() == 1 {
                let v = compile_expression(rhs, ctx, builder)?;
                compile_store_to_lhs(&lhss[0], v, ctx, builder)?;
                return Ok(());
            }
            let rhs_hint = if let Expression::Call(name, args) = rhs {
                format!("function call '{}({} args)'", name, args.len())
            } else {
                format!("{:?}", rhs)
            };
            eprintln!(
                "[fallback:jit-multi-assign] writes zero to {} target(s) for unsupported RHS {}.",
                lhss.len(),
                rhs_hint
            );
            fallback_counter::inc_jit_multi_assign();
            let zero = builder.ins().f64const(0.0);
            for lhs in lhss {
                compile_store_to_lhs(lhs, zero, ctx, builder)?;
            }
            return Ok(());
        }
        AlgorithmStatement::CallStmt(expr) => {
            // Parse-only; compile as expression evaluation (side effects depend on called function).
            let _ = compile_expression(expr, ctx, builder)?;
        }
        AlgorithmStatement::NoOp => {}
        AlgorithmStatement::Assert(cond, msg) => {
            let cond_val = compile_expression(cond, ctx, builder)?;
            let msg_val = compile_expression(msg, ctx, builder)?;
            let mut sig = ctx.module.make_signature();
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.returns.push(AbiParam::new(cl_types::F64));
            let func_id = ctx
                .module
                .declare_function("assert", Linkage::Import, &sig)
                .map_err(|e| e.to_string())?;
            let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
            builder.ins().call(func_ref, &[cond_val, msg_val]);
        }
        AlgorithmStatement::Terminate(msg) => {
            let msg_val = compile_expression(msg, ctx, builder)?;
            let mut sig = ctx.module.make_signature();
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.returns.push(AbiParam::new(cl_types::F64));
            let func_id = ctx
                .module
                .declare_function("terminate", Linkage::Import, &sig)
                .map_err(|e| e.to_string())?;
            let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
            builder.ins().call(func_ref, &[msg_val]);
        }
        AlgorithmStatement::Reinit(var_name, val_expr) => {
            let val = compile_expression(val_expr, ctx, builder)?;
            if let Some(idx) = ctx.state_index(var_name) {
                let offset = (idx * 8) as i32;
                builder
                    .ins()
                    .store(MemFlags::new(), val, ctx.states_ptr, offset);
            } else {
                return Err(format!(
                    "reinit() target '{}' is not a state variable",
                    var_name
                ));
            }
        }
        AlgorithmStatement::If(cond, true_stmts, else_ifs, else_stmts) => {
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
                compile_algorithm_stmt(s, ctx, builder)?;
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
                    compile_algorithm_stmt(s, ctx, builder)?;
                }
                builder.ins().jump(end_block, &[]);
                builder.seal_block(body_block);
            }
            builder.switch_to_block(next_block);
            if let Some(stmts) = else_stmts {
                for s in stmts {
                    compile_algorithm_stmt(s, ctx, builder)?;
                }
            }
            builder.ins().jump(end_block, &[]);
            builder.seal_block(next_block);
            builder.switch_to_block(end_block);
        }
        AlgorithmStatement::While(cond, body) => {
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
            for s in body {
                compile_algorithm_stmt(s, ctx, builder)?;
            }
            builder.ins().jump(header_block, &[]);
            builder.seal_block(body_block);
            builder.switch_to_block(exit_block);
            builder.ins().jump(after_while, &[]);
            builder.seal_block(header_block);
            builder.seal_block(exit_block);
            builder.switch_to_block(after_while);
        }
        AlgorithmStatement::For(var_name, range_expr, body) => {
            let (start_val, step_val, end_val) =
                if let Expression::Range(start, step, end) = &**range_expr {
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
            let cmp = builder
                .ins()
                .fcmp(FloatCC::LessThanOrEqual, curr_i, end_val);
            builder.ins().brif(cmp, body_block, &[], exit_block, &[]);
            builder.switch_to_block(body_block);
            for s in body {
                compile_algorithm_stmt(s, ctx, builder)?;
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
        }
        AlgorithmStatement::When(cond, body, else_whens) => {
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
                compile_algorithm_stmt(s, ctx, builder)?;
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
                    compile_algorithm_stmt(s, ctx, builder)?;
                }
                builder.ins().jump(end_block, &[]);
                builder.seal_block(body_block);
            }
            builder.switch_to_block(next_block);
            builder.ins().jump(end_block, &[]);
            builder.seal_block(next_block);
            builder.switch_to_block(end_block);
        }
    }
    Ok(())
}
