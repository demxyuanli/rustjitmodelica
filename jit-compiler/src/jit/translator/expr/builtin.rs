use crate::ast::Expression;
use cranelift::prelude::*;

use crate::jit::context::TranslationContext;
use crate::jit::translator::expr::helpers::jit_builtin_fallback_warn_once;

pub(super) fn try_compile_builtin_call(
    func_name: &str,
    args: &[Expression],
    ctx: &mut TranslationContext,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
    compile_rec: fn(
        &Expression,
        &mut TranslationContext,
        &mut cranelift::frontend::FunctionBuilder<'_>,
    ) -> Result<Value, String>,
) -> Option<Result<Value, String>> {
    // Prefer JSON builtin handlers even when the name is also listed as an external
    // Modelica function (e.g. Strings.substring, ComplexMath.conj). Without a linked
    // shared library, validate would otherwise fail at finalize with "not linked".
    // Note: empty-arg calls like Clock() are legitimate — do not count them as fallback
    // before a named handler has a chance to claim the call.
    if let Some(hid) = crate::jit::jit_policy::match_function_builtin_rule(func_name) {
        return Some(super::builtin_policy_dispatch::dispatch_named_builtin_policy(
            hid.as_str(),
            func_name,
            args,
            ctx,
            builder,
            compile_rec,
        ));
    }
    if args.is_empty() {
        jit_builtin_fallback_warn_once(func_name, "empty-args");
    }
    // Unmatched external Modelica functions keep the generic import path in compile.rs / pre.rs.
    if ctx
        .external_modelica_names
        .map(|s| s.contains(func_name))
        .unwrap_or(false)
    {
        return None;
    }
    // J14: under strict function_builtin, do not silently absorb unknown helpers.
    if crate::jit::jit_policy::strict_function_builtin() {
        return None;
    }
    // Generic namespace helper fallback: package-qualified helper calls are often not linked as
    // standalone symbols in validate mode. Degrade to passthrough placeholder.
    if let Some(head) = func_name.split('.').next() {
        if !head.is_empty() {
            let c = head.chars().next().unwrap_or('\0');
            if c.is_ascii_uppercase() {
                if args.is_empty() {
                    jit_builtin_fallback_warn_once(func_name, "namespace-helper-empty-args");
                    return Some(Ok(builder.ins().f64const(0.0)));
                }
                return Some(compile_rec(&args[0], ctx, builder));
            }
        }
    }
    if !func_name.contains('.') {
        let c = func_name.chars().next().unwrap_or('\0');
        if c.is_ascii_uppercase() {
            if args.is_empty() {
                jit_builtin_fallback_warn_once(func_name, "capitalized-helper-empty-args");
                return Some(Ok(builder.ins().f64const(0.0)));
            }
            return Some(compile_rec(&args[0], ctx, builder));
        }
    }
    None
}

/// Placeholder-only builtins (constant return, no args). Used from compile_pre_expression
/// so we do not declare Import for these in pre() context.
pub(super) fn try_compile_builtin_placeholder_constant(
    func_name: &str,
    builder: &mut cranelift::frontend::FunctionBuilder<'_>,
) -> Option<Value> {
    if func_name.starts_with("Internal.") || func_name.contains(".Internal.") {
        jit_builtin_fallback_warn_once(func_name, "pre-placeholder-constant");
        return Some(builder.ins().f64const(0.0));
    }
    if func_name == "Modelica.Math.Vectors.interpolate" || func_name.ends_with(".interpolate") {
        jit_builtin_fallback_warn_once(func_name, "pre-placeholder-interpolate");
        return Some(builder.ins().f64const(0.0));
    }
    if func_name == "real"
        || func_name.ends_with(".real")
        || func_name == "conj"
        || func_name.ends_with(".conj")
        || func_name == "imag"
        || func_name.ends_with(".imag")
    {
        jit_builtin_fallback_warn_once(func_name, "pre-placeholder-complex-scalar");
        return Some(builder.ins().f64const(0.0));
    }
    // position() is handled by move_position via JSON rules / call.rs — not a zero placeholder.
    if func_name.ends_with("getNextTimeEvent") {
        jit_builtin_fallback_warn_once(func_name, "pre-placeholder-time-event");
        return Some(builder.ins().f64const(0.0));
    }
    if func_name == "Modelica.Utilities.Strings.isEmpty" || func_name.ends_with(".isEmpty") {
        jit_builtin_fallback_warn_once(func_name, "pre-placeholder-isEmpty");
        return Some(builder.ins().f64const(1.0));
    }
    if func_name.ends_with("ExternalCombiTimeTable")
        || func_name.contains("ExternalCombiTable1D")
        || func_name.contains("ExternalCombiTable2D")
    {
        // pre() constructor placeholders: strict mode must fail closed.
        if crate::jit::translator::expr::helpers::jit_strict_placeholders_enabled() {
            return None;
        }
        jit_builtin_fallback_warn_once(func_name, "pre-placeholder-combi-table");
        return Some(builder.ins().f64const(0.0));
    }
    if func_name == "Modelica.Math.BooleanVectors.firstTrueIndex" || func_name.ends_with(".firstTrueIndex") {
        jit_builtin_fallback_warn_once(func_name, "pre-placeholder-firstTrueIndex");
        return Some(builder.ins().f64const(0.0));
    }
    if func_name == "firstTick" || func_name.ends_with(".firstTick") {
        jit_builtin_fallback_warn_once(func_name, "pre-placeholder-firstTick");
        return Some(builder.ins().f64const(0.0));
    }
    if func_name.contains("CombiTimeTable") || func_name.contains("getTimeTableValue") {
        if crate::jit::translator::expr::helpers::jit_strict_placeholders_enabled() {
            return None;
        }
        jit_builtin_fallback_warn_once(func_name, "pre-placeholder-table");
        return Some(builder.ins().f64const(0.0));
    }
    if func_name.ends_with("getTable1DValue")
        || func_name.ends_with("getTable1DValueNoDer")
        || func_name.ends_with("getTable1DValueNoDer2")
        || func_name.ends_with("getTable2DValue")
        || func_name.ends_with("getTable2DValueNoDer")
    {
        if crate::jit::translator::expr::helpers::jit_strict_placeholders_enabled() {
            return None;
        }
        jit_builtin_fallback_warn_once(func_name, "pre-placeholder-table1d");
        return Some(builder.ins().f64const(0.0));
    }
    if func_name.contains("ExternalObject") || func_name.ends_with(".ExternalObject") {
        jit_builtin_fallback_warn_once(func_name, "pre-placeholder-external-object");
        return Some(builder.ins().f64const(0.0));
    }
    None
}
