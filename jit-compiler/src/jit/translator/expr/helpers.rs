use crate::ast::Expression;
use cranelift::codegen::ir::Signature;
use cranelift_module::{FuncId, Linkage, Module};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};

use crate::jit::context::TranslationContext;
use crate::diag::fallback_counter;

/// EXT-3: ABI tag per argument so Import func_id matches (e.g. f vs s for const char*, a for array).
/// Tags: 'f' = f64 scalar, 's' = string (const char*), 'a' = array (ptr + size dual param).
pub(super) fn import_call_abi_tag(args: &[Expression], ctx: &TranslationContext) -> String {
    let mut tag = String::new();
    for a in args {
        match a {
            Expression::ArrayLiteral(items) => {
                if items
                    .iter()
                    .all(|e| matches!(e, Expression::Number(_)))
                {
                    tag.push('a');
                } else {
                    tag.push('f');
                }
            }
            Expression::StringLiteral(_) => tag.push('s'),
            Expression::Variable(id) => {
                let name = crate::string_intern::resolve_id(*id);
                if ctx.array_info.contains_key(&name) {
                    // Array: dual param (ptr + size), tag as 'a'
                    tag.push('a');
                } else {
                    tag.push('f');
                }
            }
            _ => tag.push('f'),
        }
    }
    tag
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
        "Modelica.Constants.T_zero" | "Modelica_Constants_T_zero" => Some(273.15),
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

pub(crate) fn jit_strict_placeholders_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("RUSTMODLICA_JIT_STRICT_PLACEHOLDERS")
            .ok()
            .map(|v| {
                let v = v.trim();
                v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(false)
    })
}

pub(super) fn jit_import_strict_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("RUSTMODLICA_JIT_IMPORT_STRICT")
            .ok()
            .map(|v| {
                let v = v.trim();
                v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(false)
    })
}

fn env_flag(var_name: &str, default_enabled: bool) -> bool {
    std::env::var(var_name)
        .ok()
        .map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(default_enabled)
}

pub(super) fn jit_builtin_fallback_warn_once(func_name: &str, reason: &str) {
    if !env_flag("RUSTMODLICA_JIT_BUILTIN_TRACE", true) {
        return;
    }
    static WARNED: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    let key = format!("{}::{}", func_name, reason);
    let warned = WARNED.get_or_init(|| Mutex::new(HashSet::new()));
    if let Ok(mut set) = warned.lock() {
        if set.insert(key) {
            fallback_counter::inc_jit_builtin();
            eprintln!(
                "[fallback:jit-builtin] func={} reason={} -> using placeholder result",
                func_name, reason
            );
        }
    }
}

pub(super) fn jit_var_fallback_trace(name: &str, reason: &str) {
    jit_var_fallback_trace_val(name, reason, 0.0);
}

pub(super) fn jit_var_fallback_trace_val(name: &str, reason: &str, value: f64) {
    if !env_flag("RUSTMODLICA_JIT_VAR_FALLBACK_TRACE", false) {
        return;
    }
    fallback_counter::inc_jit_variable();
    eprintln!(
        "[fallback:jit-variable] name={} reason={} -> using {}",
        name, reason, value
    );
}

/// Cache hit avoids allocating `func_name` when ABI tag matches a prior declaration.
pub(crate) fn lookup_or_insert_import(
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
