//! JIT env-driven options, in-memory cache key hashing, and disk cache key helpers.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::jit::codegen_cache;
use crate::jit::types::ArrayInfo;

pub fn jit_opt_level_from_env() -> String {
    let raw = std::env::var("RUSTMODLICA_CRANELIFT_OPT_LEVEL")
        .ok()
        .map(|s| s.trim().to_ascii_lowercase())
        .unwrap_or_else(|| "speed".to_string());
    match raw.as_str() {
        "none" | "speed" | "speed_and_size" => raw,
        "size" | "small" => "speed_and_size".to_string(),
        unknown => {
            eprintln!(
                "RUSTMODLICA_CRANELIFT_OPT_LEVEL='{}' is not recognized, using 'speed'.",
                unknown
            );
            "speed".to_string()
        }
    }
}

pub fn jit_cache_variant_from_env() -> String {
    let raw = std::env::var("RUSTMODLICA_JIT_CACHE_VARIANT")
        .ok()
        .map(|s| s.trim().to_ascii_lowercase())
        .unwrap_or_else(|| "speed".to_string());
    match raw.as_str() {
        "none" | "speed" | "speed_and_size" => raw,
        _ => "speed".to_string(),
    }
}

fn type_specialization_enabled() -> bool {
    std::env::var("RUSTMODLICA_JIT_TYPE_SPECIALIZATION")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(false)
}

fn incremental_recompile_enabled() -> bool {
    std::env::var("RUSTMODLICA_JIT_INCREMENTAL_RECOMPILE")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(false)
}

pub(crate) fn param_signature(params: &[f64]) -> String {
    if !incremental_recompile_enabled() {
        return "disabled".to_string();
    }
    if params.is_empty() {
        return "empty".to_string();
    }
    use xxhash_rust::xxh64::Xxh64;
    let mut h = Xxh64::new(0);
    for p in params {
        h.update(&p.to_bits().to_le_bytes());
    }
    format!("{:016x}", h.digest())
}

pub(crate) fn type_profile_hash(params: &[f64]) -> String {
    if !type_specialization_enabled() {
        return "disabled".to_string();
    }
    use xxhash_rust::xxh64::Xxh64;
    let mut h = Xxh64::new(0);
    for p in params {
        let is_integer_like = (p.fract().abs() < 1e-12) as u8;
        h.update(&[is_integer_like]);
    }
    format!("{:016x}", h.digest())
}

pub(crate) fn jit_verifier_dump_enabled() -> bool {
    static ENABLED: OnceLock<bool> = OnceLock::new();
    *ENABLED.get_or_init(|| {
        std::env::var("RUSTMODLICA_JIT_VERIFIER_DUMP")
            .ok()
            .map(|v| {
                let v = v.trim();
                v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
            })
            .unwrap_or(false)
    })
}

pub fn compute_jit_compile_cache_key(
    state_vars: &[String],
    discrete_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    array_info: &HashMap<String, ArrayInfo>,
    alg_equations: &[crate::ast::Equation],
    diff_equations: &[crate::ast::Equation],
    algorithms: &[crate::ast::AlgorithmStatement],
    clock_partition_schedule: &[crate::compiler::ClockPartitionScheduleEntry],
    param_values: &[f64],
    connector_connection_degree: Option<&std::collections::HashMap<String, usize>>,
) -> String {
    use xxhash_rust::xxh64::Xxh64;

    let mut h = Xxh64::new(0);

    h.update(jit_opt_level_from_env().as_bytes());
    h.update(jit_cache_variant_from_env().as_bytes());
    h.update(type_profile_hash(param_values).as_bytes());
    h.update(param_signature(param_values).as_bytes());

    if let Some(m) = connector_connection_degree {
        h.update(crate::jit::connector_degree::connector_degree_cache_digest(m).as_bytes());
    }

    let mut sorted: Vec<&String> = state_vars.iter().collect();
    sorted.sort();
    for v in &sorted {
        h.update(v.as_bytes());
    }

    let mut sorted: Vec<&String> = discrete_vars.iter().collect();
    sorted.sort();
    for v in &sorted {
        h.update(v.as_bytes());
    }

    let mut sorted: Vec<&String> = param_vars.iter().collect();
    sorted.sort();
    for v in &sorted {
        h.update(v.as_bytes());
    }

    let mut sorted: Vec<&String> = output_vars.iter().collect();
    sorted.sort();
    for v in &sorted {
        h.update(v.as_bytes());
    }

    let mut sorted_array: Vec<(&String, &ArrayInfo)> = array_info.iter().collect();
    sorted_array.sort_by_key(|(k, _)| *k);
    for (name, info) in sorted_array {
        h.update(name.as_bytes());
        h.update(&[info.array_type as u8]);
        h.update(&info.start_index.to_le_bytes());
        h.update(&info.size.to_le_bytes());
    }

    h.update(&alg_equations.len().to_le_bytes());
    h.update(&diff_equations.len().to_le_bytes());
    h.update(&algorithms.len().to_le_bytes());
    h.update(&clock_partition_schedule.len().to_le_bytes());

    format!("jit:{:016x}", h.digest())
}

/// Build the same [`codegen_cache::CodegenCacheKey`] used for `calc_derivs` disk cache I/O.
pub fn calc_derivs_codegen_cache_key(
    state_vars: &[String],
    discrete_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
    array_info: &HashMap<String, ArrayInfo>,
    param_values: &[f64],
    connector_connection_degree: Option<&HashMap<String, usize>>,
) -> codegen_cache::CodegenCacheKey {
    let opt_level = jit_opt_level_from_env();
    let cache_variant = jit_cache_variant_from_env();
    let type_hash = type_profile_hash(param_values);
    let param_sig = param_signature(param_values);
    let flat_hash = codegen_cache::flat_model_hash(
        "calc_derivs",
        state_vars,
        discrete_vars,
        param_vars,
        output_vars,
        array_info,
        &opt_level,
        &cache_variant,
        &type_hash,
        &param_sig,
        connector_connection_degree,
    );
    codegen_cache::CodegenCacheKey::new(
        "calc_derivs",
        &flat_hash,
        &opt_level,
        &cache_variant,
        &type_hash,
        &param_sig,
    )
}
