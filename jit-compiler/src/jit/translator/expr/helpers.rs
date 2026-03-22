use crate::ast::Expression;
use cranelift::codegen::ir::Signature;
use cranelift_module::{FuncId, Linkage, Module};
use std::collections::HashMap;
use std::sync::OnceLock;

use crate::jit::context::TranslationContext;

/// EXT-3: ABI tag per argument so Import func_id matches (e.g. f vs s for const char*).
pub(super) fn import_call_abi_tag(args: &[Expression], ctx: &TranslationContext) -> String {
    args.iter()
        .map(|a| match a {
            Expression::StringLiteral(_) => 's',
            Expression::Variable(id) if ctx.array_info.contains_key(&crate::string_intern::resolve_id(*id)) => 'a',
            _ => 'f',
        })
        .collect()
}

pub(super) fn jit_import_debug_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("RUSTMODLICA_JIT_IMPORT_DEBUG")
            .ok()
            .map(|v| {
                let v = v.trim();
                v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(false)
    })
}

pub(super) fn abi_params_short(sig: &Signature) -> String {
    let mut out = String::new();
    out.push('(');
    for (i, p) in sig.params.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&format!("{}", p.value_type));
    }
    out.push(')');
    out.push_str("->");
    if sig.returns.is_empty() {
        out.push_str("()");
    } else {
        out.push('(');
        for (i, r) in sig.returns.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            out.push_str(&format!("{}", r.value_type));
        }
        out.push(')');
    }
    out
}

pub(super) fn jit_scalar_name_bound(ctx: &TranslationContext, name: &str) -> bool {
    if ctx.stack_slots.contains_key(name) {
        return true;
    }
    if ctx.var_map.contains_key(name) {
        return true;
    }
    if ctx.output_index(name).is_some() {
        return true;
    }
    if ctx.param_index(name).is_some() {
        return true;
    }
    if ctx.state_index(name).is_some() {
        return true;
    }
    if ctx.discrete_index(name).is_some() {
        return true;
    }
    if name.starts_with("der_") && ctx.state_index(&name[4..]).is_some() {
        return true;
    }
    if let Some((base, suffix)) = name.rsplit_once('_') {
        if suffix.parse::<usize>().is_ok() && ctx.array_storage(base).is_some() {
            return true;
        }
    }
    false
}

/// Flattened or dotted constant names visible as a single JIT variable id.
pub(super) fn modelica_constants_flat_variable(name: &str) -> Option<f64> {
    match name {
        "Modelica.Constants.eps" | "Modelica_Constants_eps" => Some(f64::EPSILON),
        "Modelica.Constants.T_zero" => Some(273.15),
        "Modelica.Constants.pi" | "Modelica_Constants_pi" => Some(std::f64::consts::PI),
        "Modelica.Constants.small" | "Modelica_Constants_small" => Some(1.0e-60),
        "Modelica.Constants.g_n" | "Modelica_Constants_g_n" => Some(9.80665),
        "Modelica.Constants.inf" | "Modelica_Constants_inf" => Some(f64::INFINITY),
        "pi" => Some(std::f64::consts::PI),
        "small" => Some(1.0e-60),
        _ => None,
    }
}

/// `inner.member` when `inner` resolves to Modelica.Constants (or imported `Constants`).
pub(super) fn modelica_constants_dot_member(prefix: &str, member: &str) -> Option<f64> {
    let is_constants_pkg = prefix == "Modelica.Constants"
        || prefix == "Constants"
        || prefix.ends_with(".Modelica.Constants");
    if !is_constants_pkg {
        return None;
    }
    match member {
        "pi" => Some(std::f64::consts::PI),
        "eps" => Some(f64::EPSILON),
        "small" => Some(1.0e-60),
        "Inf" | "inf" => Some(f64::INFINITY),
        "T_zero" => Some(273.15),
        "g_n" => Some(9.80665),
        "R_inf" => Some(f64::INFINITY),
        "maxInteger" => Some(i32::MAX as f64),
        _ => None,
    }
}

pub(super) fn pre_scalar_name_bound(ctx: &TranslationContext, name: &str) -> bool {
    if ctx.stack_slots.contains_key(name) {
        return true;
    }
    if ctx.var_map.contains_key(name) {
        return true;
    }
    if ctx.state_index(name).is_some() {
        return true;
    }
    if ctx.discrete_index(name).is_some() {
        return true;
    }
    if let Some((base, suffix)) = name.rsplit_once('_') {
        if suffix.parse::<usize>().is_ok() && ctx.array_storage(base).is_some() {
            return true;
        }
    }
    false
}

pub(super) fn jit_dot_trace_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("RUSTMODLICA_JIT_DOT_TRACE")
            .ok()
            .map(|v| {
                let v = v.trim();
                v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(false)
    })
}

pub(super) fn jit_dot_fallback_zero_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("RUSTMODLICA_JIT_DOT_FALLBACK_ZERO")
            .ok()
            .map(|v| {
                let v = v.trim();
                v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(false)
    })
}

/// Cache hit avoids allocating `func_name` when ABI tag matches a prior declaration.
pub(super) fn lookup_or_insert_import(
    func_name: &str,
    abi_tag: String,
    sig: &Signature,
    ctx: &mut TranslationContext,
) -> Result<FuncId, String> {
    match &mut ctx.declared_imports {
        Some(outer) => {
            if let Some(inner) = outer.get(func_name) {
                if let Some(&id) = inner.get(&abi_tag) {
                    return Ok(id);
                }
            }
            let id = ctx
                .module
                .declare_function(func_name, Linkage::Import, sig)
                .map_err(|e| e.to_string())?;
            match outer.get_mut(func_name) {
                Some(inner) => {
                    inner.insert(abi_tag, id);
                }
                None => {
                    let mut inner = HashMap::with_capacity(1);
                    inner.insert(abi_tag, id);
                    outer.insert(func_name.to_string(), inner);
                }
            }
            Ok(id)
        }
        None => ctx
            .module
            .declare_function(func_name, Linkage::Import, sig)
            .map_err(|e| e.to_string()),
    }
}
