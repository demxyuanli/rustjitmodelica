fn compile_real_fft_write_to_file_call(
    args: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<Value, String> {
    if args.len() < 4 {
        return Err(format!(
            "realFFTwriteToFile expects at least 4 arguments, got {}",
            args.len()
        ));
    }
    let t_val = super::compile_expression(&args[0], ctx, builder)?;
    let path_s = match &args[1] {
        Expression::StringLiteral(s) => s.clone(),
        _ => {
            return Err(
                "realFFTwriteToFile: fileName must be a string literal in JIT".to_string(),
            );
        }
    };
    let data_id = match ctx.get_or_create_string_data(&path_s)? {
        Some(id) => id,
        None => {
            return Err(
                "realFFTwriteToFile: string data not available (FUNC-7)".to_string(),
            );
        }
    };
    let ptr_ty = ctx.module.target_config().pointer_type();
    let path_ptr = {
        let gv = ctx.module.declare_data_in_func(data_id, &mut builder.func);
        builder.ins().global_value(ptr_ty, gv)
    };
    let f_max_val = super::compile_expression(&args[2], ctx, builder)?;
    let amp_name = match &args[3] {
        Expression::Variable(id) => crate::string_intern::resolve_id(*id),
        _ => {
            return Err(
                "realFFTwriteToFile: amplitudes must be an array variable".to_string(),
            );
        }
    };
    let n_amp = ctx.array_len(&amp_name).ok_or_else(|| {
        format!(
            "realFFTwriteToFile: unknown array length for '{}'",
            amp_name
        )
    })?;
    let (a_ty, a_start) = ctx.array_storage(&amp_name).ok_or_else(|| {
        format!(
            "realFFTwriteToFile: no storage for array '{}'",
            amp_name
        )
    })?;
    let amp_base = match a_ty {
        ArrayType::State => ctx.states_ptr,
        ArrayType::Discrete => ctx.discrete_ptr,
        ArrayType::Parameter => ctx.params_ptr,
        ArrayType::Output => ctx.outputs_ptr,
        ArrayType::Derivative => ctx.derivs_ptr,
    };
    let amp_off = builder
        .ins()
        .iconst(ptr_ty, (a_start * 8) as i64);
    let amp_ptr = builder.ins().iadd(amp_base, amp_off);

    let (phase_ptr_val, n_phase_val) = if args.len() >= 5 {
        let ph_name = match &args[4] {
            Expression::Variable(id) => crate::string_intern::resolve_id(*id),
            _ => {
                return Err(
                    "realFFTwriteToFile: phases must be an array variable".to_string(),
                );
            }
        };
        let n_ph = ctx.array_len(&ph_name).ok_or_else(|| {
            format!(
                "realFFTwriteToFile: unknown phase array length for '{}'",
                ph_name
            )
        })?;
        let (p_ty, p_start) = ctx.array_storage(&ph_name).ok_or_else(|| {
            format!(
                "realFFTwriteToFile: no storage for phase array '{}'",
                ph_name
            )
        })?;
        let p_base = match p_ty {
            ArrayType::State => ctx.states_ptr,
            ArrayType::Discrete => ctx.discrete_ptr,
            ArrayType::Parameter => ctx.params_ptr,
            ArrayType::Output => ctx.outputs_ptr,
            ArrayType::Derivative => ctx.derivs_ptr,
        };
        let p_off = builder
            .ins()
            .iconst(ptr_ty, (p_start * 8) as i64);
        let pp = builder.ins().iadd(p_base, p_off);
        let n_ph_i = builder.ins().iconst(cl_types::I64, n_ph as i64);
        (pp, n_ph_i)
    } else {
        let zero = builder.ins().iconst(ptr_ty, 0);
        let nz = builder.ins().iconst(cl_types::I64, 0);
        (zero, nz)
    };

    let n_amp_val = builder.ins().iconst(cl_types::I64, n_amp as i64);

    let mut sig = ctx.module.make_signature();
    sig.params.push(AbiParam::new(cl_types::F64));
    sig.params.push(AbiParam::new(ptr_ty));
    sig.params.push(AbiParam::new(cl_types::F64));
    sig.params.push(AbiParam::new(ptr_ty));
    sig.params.push(AbiParam::new(cl_types::I64));
    sig.params.push(AbiParam::new(ptr_ty));
    sig.params.push(AbiParam::new(cl_types::I64));
    sig.returns.push(AbiParam::new(cl_types::F64));

    let func_id = lookup_or_insert_import(
        "rustmodlica_real_fft_write_to_file",
        "v1".to_string(),
        &sig,
        ctx,
    )?;
    let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
    let call_inst = builder.ins().call(
        func_ref,
        &[
            t_val,
            path_ptr,
            f_max_val,
            amp_ptr,
            n_amp_val,
            phase_ptr_val,
            n_phase_val,
        ],
    );
    Ok(builder.inst_results(call_inst)[0])
}
