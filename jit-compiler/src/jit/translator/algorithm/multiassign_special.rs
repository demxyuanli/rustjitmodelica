use super::store_lhs::scalar_f64_ptr_for_assign;
use super::super::expr::compile_expression;
use super::super::expr::helpers::lookup_or_insert_import;
use crate::ast::Expression;
use crate::jit::context::TranslationContext;
use crate::jit::types::ArrayType;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::Module;

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

pub(super) fn try_compile_real_fft_multiassign(
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

pub(super) fn try_compile_msl_random_multiassign(
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
    let kind: i32 = if let Some(k) = crate::jit::jit_policy::algorithm_random_kind(fname) {
        k
    } else if fname.contains("Xorshift64star") {
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
