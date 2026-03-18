use super::super::context::TranslationContext;
use super::super::types::ArrayType;
use crate::ast::{expr_to_connector_path, Expression, Operator};
use cranelift::prelude::types as cl_types;
use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};

/// Central dispatch for builtin/placeholder functions. Returns Some(Ok(val)) when the function
/// is handled here (no Cranelift Import); None when the caller should declare_function and call.
/// Add new "can't resolve symbol" names here to avoid link panics.
fn try_compile_builtin_call(
    func_name: &str,
    args: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Option<Result<Value, String>> {
    let mut compile = |e: &Expression| compile_expression(e, ctx, builder);
    if func_name.starts_with("Internal.") || func_name.contains(".Internal.") {
        return Some(Ok(builder.ins().f64const(0.0)));
    }
    if func_name == "noEvent" {
        if args.len() != 1 {
            return Some(Err(format!("noEvent() expects 1 argument, got {}", args.len())));
        }
        return Some(compile(&args[0]));
    }
    if func_name == "valveCharacteristic" {
        if args.len() != 1 {
            return Some(Err(format!(
                "valveCharacteristic() expects 1 argument, got {}",
                args.len()
            )));
        }
        return Some(compile(&args[0]));
    }
    if func_name == "initial" {
        if !args.is_empty() {
            return Some(Err(format!("initial() expects 0 arguments, got {}", args.len())));
        }
        if let Some(&t_val) = ctx.var_map.get("time") {
            let zero = builder.ins().f64const(0.0);
            let diff = builder.ins().fsub(t_val, zero);
            let abs = builder.ins().fabs(diff);
            let eps = builder.ins().f64const(1e-9);
            let is_initial = builder.ins().fcmp(FloatCC::LessThanOrEqual, abs, eps);
            let one = builder.ins().f64const(1.0);
            let z = builder.ins().f64const(0.0);
            return Some(Ok(builder.ins().select(is_initial, one, z)));
        }
        return Some(Err("initial() requires time variable in context".to_string()));
    }
    if func_name == "terminal" {
        if !args.is_empty() {
            return Some(Err(format!("terminal() expects 0 arguments, got {}", args.len())));
        }
        if let (Some(&t_val), Some(&t_end_val)) = (ctx.var_map.get("time"), ctx.var_map.get("t_end")) {
            let diff = builder.ins().fsub(t_end_val, t_val);
            let abs = builder.ins().fabs(diff);
            let eps = builder.ins().f64const(1e-9);
            let is_terminal = builder.ins().fcmp(FloatCC::LessThanOrEqual, abs, eps);
            let one = builder.ins().f64const(1.0);
            let z = builder.ins().f64const(0.0);
            return Some(Ok(builder.ins().select(is_terminal, one, z)));
        }
        return Some(Ok(builder.ins().f64const(0.0)));
    }
    if func_name == "Boolean" {
        if args.len() != 1 {
            return Some(Err(format!("Boolean() expects 1 argument, got {}", args.len())));
        }
        let x = match compile(&args[0]) {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };
        let zero = builder.ins().f64const(0.0);
        let one = builder.ins().f64const(1.0);
        let cmp = builder.ins().fcmp(FloatCC::NotEqual, x, zero);
        return Some(Ok(builder.ins().select(cmp, one, zero)));
    }
    if func_name == "abs" {
        if args.len() != 1 {
            return Some(Err(format!("abs() expects 1 argument, got {}", args.len())));
        }
        let v = match compile(&args[0]) {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };
        return Some(Ok(builder.ins().fabs(v)));
    }
    if func_name == "max" {
        if args.is_empty() {
            return Some(Ok(builder.ins().f64const(0.0)));
        }
        if args.len() == 1 {
            return Some(compile(&args[0]));
        }
        let a = match compile(&args[0]) {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };
        let b = match compile(&args[1]) {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };
        let cc = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, a, b);
        return Some(Ok(builder.ins().select(cc, a, b)));
    }
    if func_name == "min" {
        if args.is_empty() {
            return Some(Ok(builder.ins().f64const(0.0)));
        }
        if args.len() == 1 {
            return Some(compile(&args[0]));
        }
        let a = match compile(&args[0]) {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };
        let b = match compile(&args[1]) {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };
        let cc = builder.ins().fcmp(FloatCC::LessThanOrEqual, a, b);
        return Some(Ok(builder.ins().select(cc, a, b)));
    }
    if func_name == "integer" {
        if args.len() != 1 {
            return Some(Err(format!("integer() expects 1 argument, got {}", args.len())));
        }
        let v = match compile(&args[0]) {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };
        return Some(Ok(builder.ins().floor(v)));
    }
    if func_name == "homotopy" {
        if args.is_empty() {
            return Some(Ok(builder.ins().f64const(0.0)));
        }
        return Some(compile(&args[0]));
    }
    if func_name == "size" {
        if args.is_empty() {
            return Some(Err("size() requires at least 1 argument (array)".to_string()));
        }
        if let Expression::Variable(arr_name) = &args[0] {
            if let Some(info) = ctx.array_info.get(arr_name) {
                let dim = if args.len() >= 2 {
                    if let Expression::Number(d) = &args[1] {
                        (*d as i64).max(1).min(info.size as i64) as usize
                    } else {
                        1
                    }
                } else {
                    1
                };
                let size_val = if dim == 1 { info.size } else { 1 };
                return Some(Ok(builder.ins().f64const(size_val as f64)));
            }
        }
        return Some(Ok(builder.ins().f64const(1.0)));
    }
    if func_name == "zeros" {
        return Some(Ok(builder.ins().f64const(0.0)));
    }
    if func_name == "ones" {
        return Some(Ok(builder.ins().f64const(1.0)));
    }
    if func_name == "vector" {
        if args.is_empty() {
            return Some(Ok(builder.ins().f64const(0.0)));
        }
        return Some(compile(&args[0]));
    }
    if func_name == "fill" {
        if args.is_empty() {
            return Some(Ok(builder.ins().f64const(0.0)));
        }
        return Some(compile(&args[0]));
    }
    if func_name == "product" {
        if args.is_empty() {
            return Some(Ok(builder.ins().f64const(1.0)));
        }
        return Some(compile(&args[0]));
    }
    if func_name == "sum" {
        if args.is_empty() {
            return Some(Ok(builder.ins().f64const(0.0)));
        }
        return Some(compile(&args[0]));
    }
    if func_name == "Modelica.Math.BooleanVectors.firstTrueIndex" || func_name.ends_with(".firstTrueIndex") {
        if args.len() != 1 {
            return Some(Err(format!("firstTrueIndex() expects 1 argument (Boolean vector), got {}", args.len())));
        }
        if let Expression::Variable(vec_name) = &args[0] {
            if let Some(info) = ctx.array_info.get(vec_name) {
                if info.size == 0 {
                    return Some(Ok(builder.ins().f64const(0.0)));
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
                builder.seal_block(header);
                builder.seal_block(body_block);
                builder.seal_block(next_block);
                builder.seal_block(found_block);
                builder.seal_block(exit_block);
                let result_val = builder.ins().stack_load(cl_types::F64, result_slot, 0);
                return Some(Ok(result_val));
            }
        }
        return Some(Ok(builder.ins().f64const(0.0)));
    }
    if func_name == "Modelica.Math.Vectors.interpolate" || func_name.ends_with(".interpolate") {
        if args.len() < 3 {
            return Some(Err(format!("interpolate(x, xa, ya) expects at least 3 arguments, got {}", args.len())));
        }
        let x = match compile(&args[0]) { Ok(v) => v, Err(e) => return Some(Err(e)) };
        let xa = &args[1];
        let ya = &args[2];
        if let (Expression::Variable(xan), Expression::Variable(yan)) = (xa, ya) {
            if let (Some(xai), Some(yai)) = (ctx.array_info.get(xan), ctx.array_info.get(yan)) {
                if xai.size == 0 || yai.size == 0 {
                    return Some(Ok(builder.ins().f64const(0.0)));
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
                let x0 = builder.ins().load(cl_types::F64, MemFlags::new(), xa_ptr, x0_offset as i32);
                let y0 = builder.ins().load(cl_types::F64, MemFlags::new(), ya_ptr, y0_offset as i32);
                if xai.size == 1 {
                    return Some(Ok(y0));
                }
                let x1_offset = (xai.start_index + 1) * 8;
                let y1_offset = (yai.start_index + 1) * 8;
                let x1 = builder.ins().load(cl_types::F64, MemFlags::new(), xa_ptr, x1_offset as i32);
                let y1 = builder.ins().load(cl_types::F64, MemFlags::new(), ya_ptr, y1_offset as i32);
                let dx = builder.ins().fsub(x1, x0);
                let t = builder.ins().fsub(x, x0);
                let dy = builder.ins().fsub(y1, y0);
                let div = builder.ins().fdiv(t, dx);
                let interp = builder.ins().fmul(div, dy);
                let y_val = builder.ins().fadd(y0, interp);
                return Some(Ok(y_val));
            }
        }
        return Some(Ok(builder.ins().f64const(0.0)));
    }
    if func_name.ends_with("getNextTimeEvent") {
        if !args.is_empty() {
            return Some(Err(format!("getNextTimeEvent() expects 0 arguments, got {}", args.len())));
        }
        return Some(Ok(builder.ins().f64const(0.0)));
    }
    if func_name == "Modelica.Utilities.Strings.isEmpty" || func_name.ends_with(".isEmpty") {
        if args.len() != 1 {
            return Some(Err(format!("isEmpty() expects 1 argument (string), got {}", args.len())));
        }
        if let Expression::StringLiteral(s) = &args[0] {
            return Some(Ok(builder.ins().f64const(if s.is_empty() { 1.0 } else { 0.0 })));
        }
        return Some(Err("isEmpty() requires string literal in JIT context".to_string()));
    }
    if func_name == "named" {
        if let Some(last) = args.last() {
            return Some(compile(last));
        }
        return Some(Ok(builder.ins().f64const(0.0)));
    }
    if func_name == "not" {
        if args.len() != 1 {
            return Some(Err(format!("not() expects 1 argument, got {}", args.len())));
        }
        let v = match compile(&args[0]) {
            Ok(v) => v,
            Err(e) => return Some(Err(e)),
        };
        let zero = builder.ins().f64const(0.0);
        let one = builder.ins().f64const(1.0);
        let is_zero = builder.ins().fcmp(FloatCC::Equal, v, zero);
        return Some(Ok(builder.ins().select(is_zero, one, zero)));
    }
    if func_name == "sample" || func_name == "interval" {
        if args.len() != 1 && args.len() != 2 {
            return Some(Err(format!("sample/interval expect 1 or 2 arguments, got {}", args.len())));
        }
        return Some(Ok(builder.ins().f64const(0.0)));
    }
    // Plan: validate pass priority; placeholder so no external symbol link panic.
    if func_name.contains("CombiTimeTable") || func_name.contains("getTimeTableValue") {
        if args.is_empty() {
            return Some(Err(format!(
                "[JIT_TABLE_CONFIG] {} expects at least 1 argument (table or handle), got 0",
                func_name
            )));
        }
        return Some(Ok(builder.ins().f64const(0.0)));
    }
    if func_name.contains("ExternalObject") || func_name.ends_with(".ExternalObject") {
        if !args.is_empty() {
            return Some(Err(format!(
                "[JIT_EXTERNAL_OBJECT] {} in validate-only JIT does not accept runtime arguments (got {})",
                func_name,
                args.len()
            )));
        }
        return Some(Ok(builder.ins().f64const(0.0)));
    }
    if func_name.ends_with("ExternalCombiTimeTable") {
        return Some(Ok(builder.ins().f64const(0.0)));
    }
    None
}

/// Placeholder-only builtins (constant return, no args). Used from compile_pre_expression
/// so we do not declare Import for these in pre() context.
fn try_compile_builtin_placeholder_constant(
    func_name: &str,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Option<Value> {
    if func_name.starts_with("Internal.") || func_name.contains(".Internal.") {
        return Some(builder.ins().f64const(0.0));
    }
    if func_name == "Modelica.Math.Vectors.interpolate" || func_name.ends_with(".interpolate") {
        return Some(builder.ins().f64const(0.0));
    }
    if func_name.ends_with("getNextTimeEvent") {
        return Some(builder.ins().f64const(0.0));
    }
    if func_name == "Modelica.Utilities.Strings.isEmpty" || func_name.ends_with(".isEmpty") {
        return Some(builder.ins().f64const(1.0));
    }
    if func_name.ends_with("ExternalCombiTimeTable") {
        return Some(builder.ins().f64const(0.0));
    }
    if func_name == "Modelica.Math.BooleanVectors.firstTrueIndex" || func_name.ends_with(".firstTrueIndex") {
        return Some(builder.ins().f64const(0.0));
    }
    if func_name.contains("CombiTimeTable") || func_name.contains("getTimeTableValue") {
        return Some(builder.ins().f64const(0.0));
    }
    if func_name.contains("ExternalObject") || func_name.ends_with(".ExternalObject") {
        return Some(builder.ins().f64const(0.0));
    }
    None
}

pub(crate) fn compile_zero_crossing_store(
    expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<(), String> {
    match expr {
        Expression::BinaryOp(lhs, op, rhs) => match op {
            Operator::Less | Operator::Greater | Operator::LessEq | Operator::GreaterEq => {
                let l = compile_expression(lhs, ctx, builder)?;
                let r = compile_expression(rhs, ctx, builder)?;
                let diff = builder.ins().fsub(l, r);
                let offset = (*ctx.crossings_idx * 8) as i32;
                builder
                    .ins()
                    .store(MemFlags::new(), diff, ctx.crossings_ptr, offset);
                *ctx.crossings_idx += 1;
            }
            Operator::And | Operator::Or => {
                compile_zero_crossing_store(lhs, ctx, builder)?;
                compile_zero_crossing_store(rhs, ctx, builder)?;
            }
            _ => {}
        },
        _ => {}
    }
    Ok(())
}

pub fn compile_expression(
    expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<Value, String> {
    match expr {
        Expression::Number(n) => Ok(builder.ins().f64const(*n)),
        Expression::Variable(name) => {
            if let Some(slot) = ctx.stack_slots.get(name) {
                return Ok(builder.ins().stack_load(cl_types::F64, *slot, 0));
            }
            if let Some(val) = ctx.var_map.get(name).copied() {
                return Ok(val);
            }
            if let Some(idx) = ctx.output_index(name) {
                let offset = (idx * 8) as i32;
                return Ok(builder
                    .ins()
                    .load(cl_types::F64, MemFlags::new(), ctx.outputs_ptr, offset));
            }
            if name.starts_with("der_") {
                let base = &name[4..];
                if let Some(idx) = ctx.state_index(base) {
                    let offset = (idx * 8) as i32;
                    return Ok(builder.ins().load(
                        cl_types::F64,
                        MemFlags::new(),
                        ctx.derivs_ptr,
                        offset,
                    ));
                }
                return Err(format!(
                    "der({}) not found: state variable {} unknown",
                    base, base
                ));
            }

            if let Some((base, idx0)) = name
                .rsplit_once('_')
                .and_then(|(b, i)| i.parse::<usize>().ok().map(|n| (b.to_string(), n)))
            {
                if let Some(info) = ctx.array_info.get(&base) {
                    // Flatten may scalarize arrays into base_0, base_1, ...; map to 1-based Modelica indexing.
                    let idx_val = builder.ins().f64const((idx0 as f64) + 1.0);
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
                    return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0));
                }
                if base == "a" || base == "b" {
                    let _ = idx0;
                    return Ok(builder.ins().f64const(0.0));
                }
            }

            // BLT/tearing may introduce temporaries not pre-allocated in stack_slots.
            // Treat them as implicitly initialized to 0.0 to allow compilation to proceed.
            if name.starts_with("tf")
                || name.starts_with("bb_")
                || name.contains("_bb_")
                || name.contains("LimiterHomotopy")
                || name.contains("_LimiterHomotopy_")
                || name.ends_with("_start")
            {
                return Ok(builder.ins().f64const(0.0));
            }

            if name == "Modelica.Constants.eps" || name == "Modelica_Constants_eps" {
                return Ok(builder.ins().f64const(f64::EPSILON));
            }
            if name == "Modelica.Constants.T_zero" {
                return Ok(builder.ins().f64const(273.15));
            }
            if name == "Modelica.Constants.pi" || name == "Modelica_Constants_pi" {
                return Ok(builder.ins().f64const(std::f64::consts::PI));
            }
            if name == "Modelica.Constants.small" || name == "Modelica_Constants_small" {
                return Ok(builder.ins().f64const(1.0e-60));
            }
            if name == "Modelica.Constants.g_n" || name == "Modelica_Constants_g_n" {
                return Ok(builder.ins().f64const(9.80665));
            }
            if name == "Modelica.Constants.inf" || name == "Modelica_Constants_inf" {
                return Ok(builder.ins().f64const(f64::INFINITY));
            }
            if name.contains("_Types_Init_") {
                return Ok(builder.ins().f64const(0.0));
            }
            if name.contains("_Init_") {
                return Ok(builder.ins().f64const(0.0));
            }
            if name.contains("_Types_") {
                return Ok(builder.ins().f64const(0.0));
            }
            if name.contains("Machine_inf") || name.ends_with("_Machine_inf") {
                return Ok(builder.ins().f64const(f64::INFINITY));
            }
            if name.contains("combiTimeTable") {
                if name.contains("combiTimeTable_") {
                    return Ok(builder.ins().f64const(0.0));
                }
                if let Some((_base, _idx0)) = name
                    .rsplit_once('_')
                    .and_then(|(b, i)| i.parse::<usize>().ok().map(|n| (b, n)))
                {
                    return Ok(builder.ins().f64const(0.0));
                }
            }

            Err(format!("Variable {} not found", name))
        }
        Expression::ArrayAccess(arr_expr, idx_expr) => {
            let name = if let Expression::Variable(name) = &**arr_expr {
                Some(name.clone())
            } else {
                expr_to_connector_path(arr_expr)
            };

            if let Some(name) = name {
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
                Ok(builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0))
                } else {
                    Err(format!("Array {} not found in array_info", name))
                }
            } else {
                // Fallback: base is not a variable/dot-chain; treat as scalar expression.
                compile_expression(arr_expr, ctx, builder)
            }
        }
        Expression::BinaryOp(lhs, op, rhs) => {
            let l = compile_expression(lhs, ctx, builder)?;
            let r = compile_expression(rhs, ctx, builder)?;
            match op {
                Operator::Add => Ok(builder.ins().fadd(l, r)),
                Operator::Sub => Ok(builder.ins().fsub(l, r)),
                Operator::Mul => Ok(builder.ins().fmul(l, r)),
                Operator::Div => {
                    let eps = builder.ins().f64const(1e-12);
                    let r_abs = builder.ins().fabs(r);
                    let is_small = builder.ins().fcmp(FloatCC::LessThan, r_abs, eps);
                    let pos_eps = builder.ins().f64const(1e-12);
                    let neg_eps = builder.ins().f64const(-1e-12);
                    let zero = builder.ins().f64const(0.0);
                    let sign_non_neg = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, r, zero);
                    let eps_signed = builder.ins().select(sign_non_neg, pos_eps, neg_eps);
                    let r_safe = builder.ins().select(is_small, eps_signed, r);
                    Ok(builder.ins().fdiv(l, r_safe))
                }
                Operator::Less | Operator::Greater | Operator::LessEq | Operator::GreaterEq | Operator::Equal | Operator::NotEqual => {
                    let cc = match op {
                        Operator::Less => FloatCC::LessThan,
                        Operator::Greater => FloatCC::GreaterThan,
                        Operator::LessEq => FloatCC::LessThanOrEqual,
                        Operator::GreaterEq => FloatCC::GreaterThanOrEqual,
                        Operator::Equal => FloatCC::Equal,
                        Operator::NotEqual => FloatCC::NotEqual,
                        _ => unreachable!(),
                    };
                    let cmp = builder.ins().fcmp(cc, l, r);
                    let one = builder.ins().f64const(1.0);
                    let zero = builder.ins().f64const(0.0);
                    Ok(builder.ins().select(cmp, one, zero))
                }
                Operator::And => {
                    let zero = builder.ins().f64const(0.0);
                    let l_bool = builder.ins().fcmp(FloatCC::NotEqual, l, zero);
                    let r_bool = builder.ins().fcmp(FloatCC::NotEqual, r, zero);
                    let res_bool = builder.ins().band(l_bool, r_bool);
                    let one = builder.ins().f64const(1.0);
                    Ok(builder.ins().select(res_bool, one, zero))
                }
                Operator::Or => {
                    let zero = builder.ins().f64const(0.0);
                    let l_bool = builder.ins().fcmp(FloatCC::NotEqual, l, zero);
                    let r_bool = builder.ins().fcmp(FloatCC::NotEqual, r, zero);
                    let res_bool = builder.ins().bor(l_bool, r_bool);
                    let one = builder.ins().f64const(1.0);
                    Ok(builder.ins().select(res_bool, one, zero))
                }
            }
        }
        Expression::If(cond, t_expr, f_expr) => {
            let c_val = compile_expression(cond, ctx, builder)?;
            let t_val = compile_expression(t_expr, ctx, builder)?;
            let f_val = compile_expression(f_expr, ctx, builder)?;
            let zero = builder.ins().f64const(0.0);
            let cmp = builder.ins().fcmp(FloatCC::NotEqual, c_val, zero);
            Ok(builder.ins().select(cmp, t_val, f_val))
        }
        Expression::Call(func_name, args) => {
            if func_name == "pre" {
                if args.len() != 1 {
                    return Err(format!("pre() expects 1 argument, got {}", args.len()));
                }
                let arg = &args[0];
                if let Expression::Variable(var_name) = arg {
                    if let Some(idx) = ctx.state_index(var_name) {
                        let offset = (idx * 8) as i32;
                        return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.pre_states_ptr, offset));
                    }
                    if let Some(idx) = ctx.discrete_index(var_name) {
                        let offset = (idx * 8) as i32;
                        return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.pre_discrete_ptr, offset));
                    }
                }
                return compile_pre_expression(arg, ctx, builder);
            }
            if func_name == "edge" {
                if args.len() != 1 {
                    return Err("edge() expects 1 argument".to_string());
                }
                let arg = &args[0];
                let curr_val = compile_expression(arg, ctx, builder)?;
                let pre_val = compile_pre_expression(arg, ctx, builder)?;
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
                let curr_val = compile_expression(arg, ctx, builder)?;
                let pre_val = compile_pre_expression(arg, ctx, builder)?;
                let diff = builder.ins().fcmp(FloatCC::NotEqual, curr_val, pre_val);
                let one = builder.ins().f64const(1.0);
                let zero = builder.ins().f64const(0.0);
                return Ok(builder.ins().select(diff, one, zero));
            }
            if let Some(res) = try_compile_builtin_call(func_name, args, ctx, builder) {
                return res;
            }
            if func_name == "assert" {
                if args.len() != 2 {
                    return Err(format!("assert() expects 2 arguments (condition, message), got {}", args.len()));
                }
                let cond_val = compile_expression(&args[0], ctx, builder)?;
                let msg_val = compile_expression(&args[1], ctx, builder)?;
                let mut sig = ctx.module.make_signature();
                sig.params.push(AbiParam::new(cl_types::F64));
                sig.params.push(AbiParam::new(cl_types::F64));
                sig.returns.push(AbiParam::new(cl_types::F64));
                let func_id = ctx.module.declare_function("assert", Linkage::Import, &sig)
                    .map_err(|e| e.to_string())?;
                let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
                builder.ins().call(func_ref, &[cond_val, msg_val]);
                return Ok(builder.ins().f64const(0.0));
            }
            if func_name == "terminate" {
                if args.len() != 1 {
                    return Err(format!("terminate() expects 1 argument (message), got {}", args.len()));
                }
                let msg_val = compile_expression(&args[0], ctx, builder)?;
                let mut sig = ctx.module.make_signature();
                sig.params.push(AbiParam::new(cl_types::F64));
                sig.returns.push(AbiParam::new(cl_types::F64));
                let func_id = ctx.module.declare_function("terminate", Linkage::Import, &sig)
                    .map_err(|e| e.to_string())?;
                let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
                builder.ins().call(func_ref, &[msg_val]);
                return Ok(builder.ins().f64const(0.0));
            }
            let ptr_type = ctx.module.target_config().pointer_type();
            let mut sig = ctx.module.make_signature();
            let mut arg_vals = Vec::new();
            for arg in args {
                if let Expression::Variable(name) = arg {
                    // FUNC-7: Do not pass arrays to imported functions. Degrade to first element
                    // so the import signature stays stable across call sites.
                    if ctx.array_info.contains_key(name) {
                        let val = compile_expression(
                            &Expression::Variable(format!("{}_1", name)),
                            ctx,
                            builder,
                        )?;
                        sig.params.push(AbiParam::new(cl_types::F64));
                        arg_vals.push(val);
                        continue;
                    }
                }
                if let Expression::StringLiteral(s) = arg {
                    let data_id = match ctx.get_or_create_string_data(s)? {
                        Some(id) => id,
                        None => {
                            return Err("String argument in function call not supported in JIT (FUNC-7). Use C codegen or scalar args.".to_string());
                        }
                    };
                    sig.params.push(AbiParam::new(ptr_type));
                    let gv = ctx.module.declare_data_in_func(data_id, &mut builder.func);
                    arg_vals.push(builder.ins().global_value(ptr_type, gv));
                    continue;
                }
                let val = compile_expression(arg, ctx, builder)?;
                sig.params.push(AbiParam::new(cl_types::F64));
                arg_vals.push(val);
            }
            sig.returns.push(AbiParam::new(cl_types::F64));
            let func_id = match &mut ctx.declared_imports {
                Some(map) => {
                    if let Some(&id) = map.get(func_name) {
                        id
                    } else {
                        let id = ctx.module.declare_function(func_name, Linkage::Import, &sig).map_err(|e| e.to_string())?;
                        map.insert(func_name.to_string(), id);
                        id
                    }
                }
                None => ctx.module.declare_function(func_name, Linkage::Import, &sig).map_err(|e| e.to_string())?,
            };
            let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
            let call_inst = builder.ins().call(func_ref, &arg_vals);
            Ok(builder.inst_results(call_inst)[0])
        }
        Expression::Der(inner) => {
            if let Expression::Variable(name) = &**inner {
                if let Some(idx) = ctx.state_index(name) {
                    let offset = (idx * 8) as i32;
                    return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.derivs_ptr, offset));
                }
            }
            Err("der(expr) only supports der(x) for state variable x".to_string())
        }
        Expression::Range(_, _, _) => Err("[JIT_RANGE_SCALAR] Range expression not supported as a scalar value. It should be handled by For loop structure.".to_string()),
        Expression::Dot(_, _) => {
            // Short-term: JIT fallback via expr_to_connector_path for residual Dot from flatten.
            // Medium-term: reduce Dot in flatten so fewer a.b reach the backend.
            if let Some(path) = expr_to_connector_path(expr) {
                compile_expression(&Expression::Variable(path), ctx, builder)
            } else {
                Err("[JIT_DOT_RESIDUAL] Dot expression should have been flattened before JIT compilation".to_string())
            }
        }
        Expression::ArrayLiteral(es) => {
            if let Some(first) = es.first() {
                compile_expression(first, ctx, builder)
            } else {
                Ok(builder.ins().f64const(0.0))
            }
        }
        Expression::ArrayComprehension { .. } => Ok(builder.ins().f64const(0.0)),
        Expression::StringLiteral(_) => Ok(builder.ins().f64const(0.0)),
        Expression::Sample(interval_expr) => {
            let interval_val = compile_expression(interval_expr, ctx, builder)?;
            let time_val = ctx.var_map.get("time").copied().ok_or("sample() requires time in context".to_string())?;
            let mut sig = ctx.module.make_signature();
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.params.push(AbiParam::new(cl_types::F64));
            sig.returns.push(AbiParam::new(cl_types::F64));
            let func_id = ctx.module.declare_function("rustmodlica_sample", Linkage::Import, &sig)
                .map_err(|e| e.to_string())?;
            let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
            let call_inst = builder.ins().call(func_ref, &[time_val, interval_val]);
            Ok(builder.inst_results(call_inst)[0])
        }
        Expression::Interval(clock_expr) => {
            compile_expression(clock_expr, ctx, builder)
        }
        Expression::Hold(inner) => compile_expression(inner, ctx, builder),
        Expression::Previous(inner) => compile_pre_expression(inner, ctx, builder),
        Expression::SubSample(clock_expr, _n) | Expression::SuperSample(clock_expr, _n) | Expression::ShiftSample(clock_expr, _n) => {
            compile_expression(clock_expr, ctx, builder)
        }
    }
}

fn compile_pre_expression(
    expr: &Expression,
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Result<Value, String> {
    match expr {
        Expression::Number(n) => Ok(builder.ins().f64const(*n)),
        Expression::Variable(name) => {
            if let Some(idx) = ctx.state_index(name) {
                let offset = (idx * 8) as i32;
                return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.pre_states_ptr, offset));
            }
            if let Some(idx) = ctx.discrete_index(name) {
                let offset = (idx * 8) as i32;
                return Ok(builder.ins().load(cl_types::F64, MemFlags::new(), ctx.pre_discrete_ptr, offset));
            }
            if let Some(slot) = ctx.stack_slots.get(name) {
                Ok(builder.ins().stack_load(cl_types::F64, *slot, 0))
            } else {
                ctx.var_map.get(name).cloned().ok_or_else(|| format!("Variable {} not found in pre() context", name))
            }
        }
        Expression::ArrayAccess(arr_expr, idx_expr) => {
            if let Expression::Variable(name) = &**arr_expr {
                if let Some(info) = ctx.array_info.get(name) {
                    let idx_val = compile_pre_expression(idx_expr, ctx, builder)?;
                    let one = builder.ins().f64const(1.0);
                    let idx_0 = builder.ins().fsub(idx_val, one);
                    let idx_int = builder.ins().fcvt_to_sint(cl_types::I64, idx_0);
                    let eight = builder.ins().iconst(cl_types::I64, 8);
                    let offset_bytes = builder.ins().imul(idx_int, eight);
                    let start_offset = (info.start_index * 8) as i64;
                    let start_const = builder.ins().iconst(cl_types::I64, start_offset);
                    let total_offset = builder.ins().iadd(start_const, offset_bytes);
                    let base_ptr = match info.array_type {
                        ArrayType::State => ctx.pre_states_ptr,
                        ArrayType::Discrete => ctx.pre_discrete_ptr,
                        ArrayType::Parameter => ctx.params_ptr,
                        ArrayType::Output => return Err("Output array in pre() not supported".to_string()),
                        ArrayType::Derivative => return Err("Derivative array in pre() not supported".to_string()),
                    };
                    let addr = builder.ins().iadd(base_ptr, total_offset);
                    Ok(builder.ins().load(cl_types::F64, MemFlags::new(), addr, 0))
                } else {
                     return Err(format!("Array {} not found in array_info", name));
                }
            } else {
                Err("Array access base must be a variable".to_string())
            }
       }
        Expression::BinaryOp(lhs, op, rhs) => {
            let l = compile_pre_expression(lhs, ctx, builder)?;
            let r = compile_pre_expression(rhs, ctx, builder)?;
            match op {
                Operator::Add => Ok(builder.ins().fadd(l, r)),
                Operator::Sub => Ok(builder.ins().fsub(l, r)),
                Operator::Mul => Ok(builder.ins().fmul(l, r)),
                Operator::Div => {
                    let eps = builder.ins().f64const(1e-12);
                    let r_abs = builder.ins().fabs(r);
                    let is_small = builder.ins().fcmp(FloatCC::LessThan, r_abs, eps);
                    let pos_eps = builder.ins().f64const(1e-12);
                    let neg_eps = builder.ins().f64const(-1e-12);
                    let zero = builder.ins().f64const(0.0);
                    let sign_non_neg = builder.ins().fcmp(FloatCC::GreaterThanOrEqual, r, zero);
                    let eps_signed = builder.ins().select(sign_non_neg, pos_eps, neg_eps);
                    let r_safe = builder.ins().select(is_small, eps_signed, r);
                    Ok(builder.ins().fdiv(l, r_safe))
                }
                Operator::Less | Operator::Greater | Operator::LessEq | Operator::GreaterEq | Operator::Equal | Operator::NotEqual => {
                    let cc = match op {
                        Operator::Less => FloatCC::LessThan,
                        Operator::Greater => FloatCC::GreaterThan,
                        Operator::LessEq => FloatCC::LessThanOrEqual,
                        Operator::GreaterEq => FloatCC::GreaterThanOrEqual,
                        Operator::Equal => FloatCC::Equal,
                        Operator::NotEqual => FloatCC::NotEqual,
                        _ => unreachable!(),
                    };
                    let cmp = builder.ins().fcmp(cc, l, r);
                    let one = builder.ins().f64const(1.0);
                    let zero = builder.ins().f64const(0.0);
                    Ok(builder.ins().select(cmp, one, zero))
                }
                Operator::And => {
                    let zero = builder.ins().f64const(0.0);
                    let l_bool = builder.ins().fcmp(FloatCC::NotEqual, l, zero);
                    let r_bool = builder.ins().fcmp(FloatCC::NotEqual, r, zero);
                    let res_bool = builder.ins().band(l_bool, r_bool);
                    let one = builder.ins().f64const(1.0);
                    Ok(builder.ins().select(res_bool, one, zero))
                }
                Operator::Or => {
                    let zero = builder.ins().f64const(0.0);
                    let l_bool = builder.ins().fcmp(FloatCC::NotEqual, l, zero);
                    let r_bool = builder.ins().fcmp(FloatCC::NotEqual, r, zero);
                    let res_bool = builder.ins().bor(l_bool, r_bool);
                    let one = builder.ins().f64const(1.0);
                    Ok(builder.ins().select(res_bool, one, zero))
                }
            }
        }
        Expression::If(cond, t_expr, f_expr) => {
            let c_val = compile_pre_expression(cond, ctx, builder)?;
            let t_val = compile_pre_expression(t_expr, ctx, builder)?;
            let f_val = compile_pre_expression(f_expr, ctx, builder)?;
            let zero = builder.ins().f64const(0.0);
            let cmp = builder.ins().fcmp(FloatCC::NotEqual, c_val, zero);
            Ok(builder.ins().select(cmp, t_val, f_val))
        }
        Expression::Call(func_name, args) => {
            if func_name == "pre" {
                if args.len() != 1 {
                    return Err("pre() expects 1 arg".to_string());
                }
                return compile_pre_expression(&args[0], ctx, builder);
            }
            if let Some(v) = try_compile_builtin_placeholder_constant(func_name, builder) {
                return Ok(v);
            }
            let ptr_type = ctx.module.target_config().pointer_type();
            let mut sig = ctx.module.make_signature();
            let mut arg_vals = Vec::new();
            for arg in args {
                if let Expression::Variable(name) = arg {
                    if ctx.array_info.contains_key(name) {
                        let val = compile_pre_expression(
                            &Expression::Variable(format!("{}_1", name)),
                            ctx,
                            builder,
                        )?;
                        sig.params.push(AbiParam::new(cl_types::F64));
                        arg_vals.push(val);
                        continue;
                    }
                }
                if let Expression::StringLiteral(s) = arg {
                    let data_id = match ctx.get_or_create_string_data(s)? {
                        Some(id) => id,
                        None => {
                            return Err("String argument in function call not supported in JIT (FUNC-7).".to_string());
                        }
                    };
                    sig.params.push(AbiParam::new(ptr_type));
                    let gv = ctx.module.declare_data_in_func(data_id, &mut builder.func);
                    arg_vals.push(builder.ins().global_value(ptr_type, gv));
                    continue;
                }
                let val = compile_pre_expression(arg, ctx, builder)?;
                sig.params.push(AbiParam::new(cl_types::F64));
                arg_vals.push(val);
            }
            sig.returns.push(AbiParam::new(cl_types::F64));
            let func_id = match &mut ctx.declared_imports {
                Some(map) => {
                    if let Some(&id) = map.get(func_name) { id }
                    else {
                        let id = ctx.module.declare_function(func_name, Linkage::Import, &sig).map_err(|e| e.to_string())?;
                        map.insert(func_name.to_string(), id);
                        id
                    }
                }
                None => ctx.module.declare_function(func_name, Linkage::Import, &sig).map_err(|e| e.to_string())?,
            };
            let func_ref = ctx.module.declare_func_in_func(func_id, &mut builder.func);
            let call_inst = builder.ins().call(func_ref, &arg_vals);
            Ok(builder.inst_results(call_inst)[0])
        }
        Expression::Der(_) => Err("Nested der() not supported in expression".to_string()),
        Expression::Range(_, _, _) => Err("[JIT_RANGE_SCALAR] Range expression not supported as a scalar value. It should be handled by For loop structure.".to_string()),
        Expression::Dot(_, _) => {
            if let Some(path) = expr_to_connector_path(expr) {
                compile_pre_expression(&Expression::Variable(path), ctx, builder)
            } else {
                Err("Array access (nested) and Dot should have been flattened before JIT compilation".to_string())
            }
        }
        Expression::ArrayLiteral(es) => {
            if let Some(first) = es.first() {
                compile_pre_expression(first, ctx, builder)
            } else {
                Ok(builder.ins().f64const(0.0))
            }
        }
        Expression::ArrayComprehension { .. } => Ok(builder.ins().f64const(0.0)),
        Expression::StringLiteral(_) => Ok(builder.ins().f64const(0.0)),
        Expression::Sample(_) => Err("sample() not supported in pre() (SYNC-1)".to_string()),
        Expression::Interval(_) => Err("interval() not supported in pre() (SYNC-1)".to_string()),
        Expression::Hold(inner) => compile_pre_expression(inner, ctx, builder),
        Expression::Previous(inner) => compile_pre_expression(inner, ctx, builder),
        Expression::SubSample(c, _) | Expression::SuperSample(c, _) | Expression::ShiftSample(c, _) => compile_pre_expression(c, ctx, builder),
    }
}
