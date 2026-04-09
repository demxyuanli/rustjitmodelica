use crate::ast::Expression;
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};

use crate::jit::context::TranslationContext;
use crate::jit::types::ArrayType;
use super::builtin::try_compile_builtin_call;
use super::helpers::{
    abi_params_short, import_call_abi_tag, jit_builtin_fallback_warn_once, jit_import_debug_enabled,
    jit_import_strict_enabled, jit_strict_placeholders_enabled, lookup_or_insert_import,
};
use std::sync::atomic::{AtomicU64, Ordering};

static INLINE_BUILTIN_HITS: AtomicU64 = AtomicU64::new(0);

pub fn take_inline_builtin_hits() -> u64 {
    INLINE_BUILTIN_HITS.swap(0, Ordering::Relaxed)
}

fn inline_builtins_enabled() -> bool {
    std::env::var("RUSTMODLICA_JIT_INLINE_BUILTINS")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(false)
}

fn builtin_inline_allowed(func_name: &str) -> bool {
    matches!(
        func_name,
        "sin" | "cos" | "sqrt" | "exp" | "log" | "abs" | "min" | "max"
    )
}

fn record_inline_builtin_hit() {
    INLINE_BUILTIN_HITS.fetch_add(1, Ordering::Relaxed);
}

fn name_matches(func_name: &str, plain: &str) -> bool {
    func_name == plain || func_name.ends_with(&format!(".{}", plain))
}

pub(super) fn compile_array_reduce(
    arr_name: &str,
    init: f64,
    is_product: bool,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<Value, String> {
    let Some((arr_ty, arr_start)) = ctx.array_storage(arr_name) else {
        return Ok(builder.ins().f64const(init));
    };
    let n = ctx.array_len(arr_name).unwrap_or(0);
    if n == 0 {
        return Ok(builder.ins().f64const(init));
    }
    let base_ptr = match arr_ty {
        ArrayType::State => ctx.states_ptr,
        ArrayType::Discrete => ctx.discrete_ptr,
        ArrayType::Parameter => ctx.params_ptr,
        ArrayType::Output => ctx.outputs_ptr,
        ArrayType::Derivative => ctx.derivs_ptr,
    };
    let ptr_ty = ctx.module.target_config().pointer_type();
    let sum_slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        8,
        8,
    ));
    let idx_slot = builder.create_sized_stack_slot(StackSlotData::new(
        StackSlotKind::ExplicitSlot,
        8,
        8,
    ));
    let init_val = builder.ins().f64const(init);
    builder.ins().stack_store(init_val, sum_slot, 0);
    let idx0 = builder.ins().iconst(cl_types::I64, 0);
    builder.ins().stack_store(idx0, idx_slot, 0);
    let header = builder.create_block();
    let body = builder.create_block();
    let done = builder.create_block();
    builder.ins().jump(header, &[]);
    builder.switch_to_block(header);
    let i = builder.ins().stack_load(cl_types::I64, idx_slot, 0);
    let n_val = builder.ins().iconst(cl_types::I64, n as i64);
    let cond = builder.ins().icmp(IntCC::UnsignedLessThan, i, n_val);
    builder.ins().brif(cond, body, &[], done, &[]);
    builder.switch_to_block(body);
    let start_val = builder.ins().iconst(cl_types::I64, arr_start as i64);
    let elem_idx = builder.ins().iadd(i, start_val);
    let elem_off = builder.ins().imul_imm(elem_idx, 8);
    let elem_off_ptr = if ptr_ty == cl_types::I64 {
        elem_off
    } else {
        builder.ins().ireduce(ptr_ty, elem_off)
    };
    let elem_ptr = builder.ins().iadd(base_ptr, elem_off_ptr);
    let elem = builder
        .ins()
        .load(cl_types::F64, MemFlags::new(), elem_ptr, 0);
    let acc = builder.ins().stack_load(cl_types::F64, sum_slot, 0);
    let next = if is_product {
        builder.ins().fmul(acc, elem)
    } else {
        builder.ins().fadd(acc, elem)
    };
    builder.ins().stack_store(next, sum_slot, 0);
    let i_next = builder.ins().iadd_imm(i, 1);
    builder.ins().stack_store(i_next, idx_slot, 0);
    builder.ins().jump(header, &[]);
    builder.seal_block(body);
    builder.switch_to_block(done);
    builder.seal_block(header);
    let out = builder.ins().stack_load(cl_types::F64, sum_slot, 0);
    Ok(out)
}

pub(super) fn compile_call(
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
    if name_matches(func_name, "noEvent") {
        if args.len() != 1 {
            return Err(format!("noEvent() expects 1 argument, got {}", args.len()));
        }
        let prev = ctx.suppress_zero_crossings;
        ctx.suppress_zero_crossings = true;
        let out = compile_rec(&args[0], ctx, builder);
        ctx.suppress_zero_crossings = prev;
        return out;
    }
    if name_matches(func_name, "smooth") {
        return match args.len() {
            1 => compile_rec(&args[0], ctx, builder),
            2 => compile_rec(&args[1], ctx, builder),
            _ => Err(format!("smooth() expects 1 or 2 arguments, got {}", args.len())),
        };
    }
    if name_matches(func_name, "sum") {
        if args.len() != 1 {
            return Err(format!("sum() expects 1 argument, got {}", args.len()));
        }
        if let Expression::Variable(id) = &args[0] {
            let arr_name = crate::string_intern::resolve_id(*id);
            return compile_array_reduce(&arr_name, 0.0, false, ctx, builder);
        }
        return compile_rec(&args[0], ctx, builder);
    }
    if name_matches(func_name, "product") {
        if args.len() != 1 {
            return Err(format!("product() expects 1 argument, got {}", args.len()));
        }
        if let Expression::Variable(id) = &args[0] {
            let arr_name = crate::string_intern::resolve_id(*id);
            return compile_array_reduce(&arr_name, 1.0, true, ctx, builder);
        }
        return compile_rec(&args[0], ctx, builder);
    }
    if name_matches(func_name, "delay") {
        if args.len() < 2 || args.len() > 3 {
            return Err(format!("delay() expects 2 or 3 arguments, got {}", args.len()));
        }
        let expr_val = compile_rec(&args[0], ctx, builder)?;
        let delay_time = compile_rec(&args[1], ctx, builder)?;
        let time_val = ctx
            .var_map
            .get("time")
            .copied()
            .unwrap_or_else(|| builder.ins().f64const(0.0));
        let id = ctx.delay_call_counter as i64;
        ctx.delay_call_counter += 1;
        let mut sig = ctx.module.make_signature();
        sig.params.push(AbiParam::new(cl_types::I64));
        sig.params.push(AbiParam::new(cl_types::F64));
        sig.params.push(AbiParam::new(cl_types::F64));
        sig.params.push(AbiParam::new(cl_types::F64));
        sig.returns.push(AbiParam::new(cl_types::F64));
        let func_id = ctx
            .module
            .declare_function(
                "rustmodlica_delay_lookup_record",
                Linkage::Import,
                &sig,
            )
            .map_err(|e| e.to_string())?;
        let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
        let id_val = builder.ins().iconst(cl_types::I64, id);
        let call_inst = builder.ins().call(
            func_ref,
            &[
                id_val,
                time_val,
                expr_val,
                delay_time,
            ],
        );
        return Ok(builder.inst_results(call_inst)[0]);
    }
    if name_matches(func_name, "cardinality") {
        if args.len() != 1 {
            return Err(format!(
                "cardinality() expects 1 argument, got {}",
                args.len()
            ));
        }
        if let Expression::Variable(id) = &args[0] {
            let path = crate::string_intern::resolve_id(*id);
            if let Some(&d) = ctx.connector_connection_degree.get(&path) {
                return Ok(builder.ins().f64const(d as f64));
            }
        }
        if jit_strict_placeholders_enabled() {
            return Err(format!(
                "cardinality: no flatten connection degree for '{}'",
                func_name
            ));
        }
        jit_builtin_fallback_warn_once(func_name, "cardinality-no-degree");
        return Ok(builder.ins().f64const(0.0));
    }
    if name_matches(func_name, "getInstanceName")
        || name_matches(func_name, "isRoot")
        || name_matches(func_name, "root")
        || name_matches(func_name, "rooted")
        || name_matches(func_name, "branch")
    {
        jit_builtin_fallback_warn_once(func_name, "graph-query-placeholder");
        return Ok(builder.ins().f64const(0.0));
    }
    if func_name == "size" {
        if args.is_empty() || args.len() > 2 {
            return Err(format!("size() expects 1 or 2 arguments, got {}", args.len()));
        }
        let dim = if args.len() == 1 {
            1_i64
        } else if let Expression::Number(n) = args[1] {
            n as i64
        } else {
            1_i64
        };
        match &args[0] {
            Expression::Variable(id) => {
                let name = crate::string_intern::resolve_id(*id);
                if let Some(size) = ctx.array_len(&name) {
                    let out = if dim <= 1 { size as f64 } else { 1.0 };
                    return Ok(builder.ins().f64const(out));
                }
            }
            Expression::Number(_) => {
                return Ok(builder.ins().f64const(1.0));
            }
            Expression::ArrayLiteral(items) => {
                let out = if dim <= 1 { items.len() as f64 } else { 1.0 };
                return Ok(builder.ins().f64const(out));
            }
            _ => {}
        }
        return Ok(builder.ins().f64const(1.0));
    }
    if func_name == "pre" {
        if args.len() != 1 {
            return Err(format!("pre() expects 1 argument, got {}", args.len()));
        }
        let arg = &args[0];
        if let Expression::Variable(id) = arg {
            let var_name = crate::string_intern::resolve_id(*id);
            if let Some(idx) = ctx.state_index(&var_name) {
                let offset = (idx * 8) as i32;
                return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.pre_states_ptr, offset));
            }
            if let Some(idx) = ctx.discrete_index(&var_name) {
                let offset = (idx * 8) as i32;
                return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.pre_discrete_ptr, offset));
            }
        }
        return super::pre::compile_pre_expression(arg, ctx, builder);
    }
    if func_name == "edge" {
        if args.len() != 1 {
            return Err("edge() expects 1 argument".to_string());
        }
        let arg = &args[0];
        let curr_val = compile_rec(arg, ctx, builder)?;
        let pre_val = super::pre::compile_pre_expression(arg, ctx, builder)?;
        let zero = builder.ins().f64const(0.0);
        let curr_bool = builder.ins().fcmp(FloatCC::NotEqual, curr_val, zero);
        let pre_zero = builder.ins().fcmp(FloatCC::Equal, pre_val, zero);
        let res_bool = builder.ins().band(curr_bool, pre_zero);
        let one = builder.ins().f64const(1.0);
        return Ok(builder.ins().select(res_bool, one, zero));
    }
    if func_name == "change" {
        if args.len() != 1 {
            return Err("change() expects 1 argument".to_string());
        }
        let arg = &args[0];
        let curr_val = compile_rec(arg, ctx, builder)?;
        let pre_val = super::pre::compile_pre_expression(arg, ctx, builder)?;
        let diff = builder.ins().fcmp(FloatCC::NotEqual, curr_val, pre_val);
        let one = builder.ins().f64const(1.0);
        let zero = builder.ins().f64const(0.0);
        return Ok(builder.ins().select(diff, one, zero));
    }
    if func_name.ends_with("realFFTwriteToFile") || func_name == "realFFTwriteToFile" {
        return compile_real_fft_write_to_file_call(args, ctx, builder);
    }
    // JSON / namespace builtin dispatch: always on (not gated by RUSTMODLICA_JIT_INLINE_BUILTINS).
    let json_builtin_rule = crate::jit::jit_policy::match_function_builtin_rule(func_name);
    if let Some(res) = try_compile_builtin_call(func_name, args, ctx, builder, compile_rec) {
        // Perf counter: only count whitelisted math names when resolution did not use a JSON rule
        // (avoids counting abs/max/min etc. that map to policy handlers).
        if inline_builtins_enabled()
            && builtin_inline_allowed(func_name)
            && json_builtin_rule.is_none()
        {
            record_inline_builtin_hit();
        }
        return res;
    }
    if func_name == "assert" {
        if args.len() != 2 {
            return Err(format!(
                "assert() expects 2 arguments (condition, message), got {}",
                args.len()
            ));
        }
        let cond_val = compile_rec(&args[0], ctx, builder)?;
        let msg_val = compile_rec(&args[1], ctx, builder)?;
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
        return Ok(builder.ins().f64const(0.0));
    }
    if func_name == "terminate" {
        if args.len() != 1 {
            return Err(format!(
                "terminate() expects 1 argument (message), got {}",
                args.len()
            ));
        }
        let msg_val = compile_rec(&args[0], ctx, builder)?;
        let mut sig = ctx.module.make_signature();
        sig.params.push(AbiParam::new(cl_types::F64));
        sig.returns.push(AbiParam::new(cl_types::F64));
        let func_id = ctx
            .module
            .declare_function("terminate", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?;
        let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
        builder.ins().call(func_ref, &[msg_val]);
        return Ok(builder.ins().f64const(0.0));
    }

    let is_external_modelica = ctx
        .external_modelica_names
        .map(|s| s.contains(func_name))
        .unwrap_or(false);
    if !is_external_modelica
        && !crate::jit::native::builtin_jit_symbol_names()
            .iter()
            .any(|&n| n == func_name)
    {
        if jit_import_strict_enabled() {
            return Err(format!(
                "JIT import strict: '{}' is not a linked builtin or external symbol",
                func_name
            ));
        }
        jit_builtin_fallback_warn_once(func_name, "unknown-import");
        if args.is_empty() {
            return Ok(builder.ins().f64const(0.0));
        }
        return compile_rec(&args[0], ctx, builder);
    }

    let ptr_type = ctx.module.target_config().pointer_type();
    let mut sig = ctx.module.make_signature();
    let mut arg_vals = Vec::new();
    for arg in args {
        if let Expression::Variable(id) = arg {
            let name = crate::string_intern::resolve_id(*id);
            if let Some(info) = ctx.array_info.get(&name) {
                let base_ptr = match info.array_type {
                    ArrayType::State => ctx.states_ptr,
                    ArrayType::Discrete => ctx.discrete_ptr,
                    ArrayType::Parameter => ctx.params_ptr,
                    ArrayType::Output => ctx.outputs_ptr,
                    ArrayType::Derivative => ctx.derivs_ptr,
                };
                let start_offset = (info.start_index * 8) as i64;
                let start_const = builder.ins().iconst(ptr_type, start_offset);
                let array_ptr = builder.ins().iadd(base_ptr, start_const);

                let size_val = builder.ins().f64const(info.size as f64);

                sig.params.push(AbiParam::new(ptr_type));
                arg_vals.push(array_ptr);
                sig.params.push(AbiParam::new(cl_types::F64));
                arg_vals.push(size_val);
                continue;
            }
        }
        if let Expression::ArrayLiteral(items) = arg {
            if items.iter().all(|it| matches!(it, Expression::Number(_))) {
                let mut elems: Vec<f64> = Vec::with_capacity(items.len());
                for it in items {
                    if let Expression::Number(n) = it {
                        elems.push(*n);
                    }
                }
                let data_id = match ctx.get_or_create_f64_array_literal_data(&elems)? {
                    Some(id) => id,
                    None => {
                        return Err(
                            "Array literal in external call requires JIT data context (EXT-3)."
                                .to_string(),
                        );
                    }
                };
                sig.params.push(AbiParam::new(ptr_type));
                let gv = ctx.module.declare_data_in_func(data_id, &mut builder.func);
                arg_vals.push(builder.ins().global_value(ptr_type, gv));
                sig.params.push(AbiParam::new(cl_types::F64));
                arg_vals.push(builder.ins().f64const(elems.len() as f64));
                continue;
            }

            let byte_len = (items.len() * 8) as u32;
            let tmp_slot = builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                byte_len,
                8,
            ));
            for (i, it) in items.iter().enumerate() {
                let item_val = compile_rec(it, ctx, builder)?;
                let off_i64 = (i as i64) * 8;
                let off = i32::try_from(off_i64).map_err(|_| {
                    format!(
                        "JIT external call '{}': array literal too large (EXT-3).",
                        func_name
                    )
                })?;
                builder.ins().stack_store(item_val, tmp_slot, off);
            }
            sig.params.push(AbiParam::new(ptr_type));
            arg_vals.push(builder.ins().stack_addr(ptr_type, tmp_slot, 0));
            sig.params.push(AbiParam::new(cl_types::F64));
            arg_vals.push(builder.ins().f64const(items.len() as f64));
            continue;
        }
        if let Expression::StringLiteral(s) = arg {
            let data_id = match ctx.get_or_create_string_data(s)? {
                Some(id) => id,
                None => {
                    return Err("String argument in function call requires string data context (FUNC-7). Ensure JIT compilation is configured with string literal support.".to_string());
                }
            };
            sig.params.push(AbiParam::new(ptr_type));
            let gv = ctx.module.declare_data_in_func(data_id, &mut builder.func);
            arg_vals.push(builder.ins().global_value(ptr_type, gv));
            continue;
        }
        let val = compile_rec(arg, ctx, builder)?;
        sig.params.push(AbiParam::new(cl_types::F64));
        arg_vals.push(val);
    }
    sig.returns.push(AbiParam::new(cl_types::F64));
    if jit_import_debug_enabled() {
        let mut array_args = Vec::new();
        for a in args {
            if let Expression::Variable(id) = a {
                let n = crate::string_intern::resolve_id(*id);
                if ctx.array_info.contains_key(&n) {
                    array_args.push(n);
                }
            }
        }
        eprintln!(
            "[jit-import] name={} sig={} array_args={}",
            func_name,
            abi_params_short(&sig),
            if array_args.is_empty() {
                "-".to_string()
            } else {
                array_args.join(",")
            }
        );
    }
    let abi_tag = import_call_abi_tag(args, ctx);
    let func_id = lookup_or_insert_import(func_name, abi_tag, &sig, ctx)?;
    let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
    let call_inst = builder.ins().call(func_ref, &arg_vals);
    Ok(builder.inst_results(call_inst)[0])
}

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
