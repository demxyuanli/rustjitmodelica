//! Unified JIT named rules: variable/pre fallbacks, Dot path zeros, hysteresis tables, function-builtin routing, algorithm RNG kind.
//! Default: `default_jit_policy.json`. Overlay: `RUSTMODLICA_JIT_POLICY_JSON`. Legacy var-only overlay: `RUSTMODLICA_JIT_VAR_POLICY_JSON`.

use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

#[derive(Debug, Deserialize, Clone)]
pub struct ScalarFallbackRule {
    pub op: String,
    pub pattern: String,
    pub value: String,
    #[serde(default)]
    pub trace: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
struct HysteresisConfig {
    func_suffix: String,
    members: HashMap<String, f64>,
}

#[derive(Debug, Deserialize, Clone, Default)]
struct JitPolicyFile {
    #[serde(default)]
    schema_version: u32,
    /// When set, skip writing `RUSTMODLICA_JIT_CODEGEN_CACHE` entries if `alg + diff` equation count exceeds this (in-process JIT still runs).
    #[serde(default)]
    codegen_disk_max_equations: Option<usize>,
    #[serde(default)]
    variable_fallbacks: Vec<ScalarFallbackRule>,
    #[serde(default)]
    pre_variable_fallbacks: Vec<ScalarFallbackRule>,
    #[serde(default = "default_true")]
    pre_chain_variable_fallbacks: bool,
    #[serde(default = "default_true")]
    pre_generic_underscore_fallback: bool,
    #[serde(default)]
    dot_prefix_zero_all_contains: Vec<Vec<String>>,
    #[serde(default)]
    dot_path_zero_any_contains: Vec<String>,
    #[serde(default)]
    hysteresis: HysteresisConfig,
    #[serde(default)]
    algorithm_random_kind_rules: Vec<AlgorithmRandomKindRule>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Deserialize, Clone)]
pub struct AlgorithmRandomKindRule {
    pub op: String,
    pub pattern: String,
    pub kind: i32,
}

impl Default for HysteresisConfig {
    fn default() -> Self {
        Self {
            func_suffix: String::new(),
            members: HashMap::new(),
        }
    }
}

#[derive(Clone)]
pub struct LoadedJitPolicy {
    pub codegen_disk_max_equations: Option<usize>,
    pub variable_fallbacks: Vec<ScalarFallbackRule>,
    pub pre_variable_fallbacks: Vec<ScalarFallbackRule>,
    pub pre_chain_variable_fallbacks: bool,
    pub pre_generic_underscore_fallback: bool,
    pub dot_prefix_zero_all_contains: Vec<Vec<String>>,
    pub dot_path_zero_any_contains: Vec<String>,
    hysteresis_suffix: String,
    hysteresis_members: HashMap<String, f64>,
    pub algorithm_random_kind_rules: Vec<AlgorithmRandomKindRule>,
}

fn rule_matches_name(name: &str, rule: &ScalarFallbackRule) -> bool {
    match rule.op.to_ascii_lowercase().as_str() {
        "starts_with" => name.starts_with(&rule.pattern),
        "ends_with" => name.ends_with(&rule.pattern),
        "contains" => name.contains(rule.pattern.as_str()),
        "equals" | "eq" => name == rule.pattern,
        _ => false,
    }
}

fn rule_matches_func(func_name: &str, op: &str, pattern: &str) -> bool {
    match op.to_ascii_lowercase().as_str() {
        "starts_with" => func_name.starts_with(pattern),
        "ends_with" => func_name.ends_with(pattern),
        "contains" => func_name.contains(pattern),
        "equals" | "eq" => func_name == pattern,
        _ => false,
    }
}

fn value_to_f64(s: &str) -> Option<f64> {
    match s.trim().to_ascii_lowercase().as_str() {
        "zero" | "0" => Some(0.0),
        "one" | "1" => Some(1.0),
        "inf" | "infinity" => Some(f64::INFINITY),
        _ => None,
    }
}

fn legacy_var_strict() -> bool {
    std::env::var("RUSTMODLICA_JIT_VAR_STRICT")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

fn policy_strict_domains() -> HashSet<String> {
    std::env::var("RUSTMODLICA_JIT_POLICY_STRICT")
        .ok()
        .map(|v| {
            v.split(',')
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn strict_variable_like() -> bool {
    legacy_var_strict() || policy_strict_domains().contains("variable")
}

fn strict_pre() -> bool {
    legacy_var_strict() || policy_strict_domains().contains("pre")
}

fn strict_pre_generic_underscore() -> bool {
    policy_strict_domains().contains("pre_generic_underscore")
}

fn strict_dot() -> bool {
    policy_strict_domains().contains("dot")
}

fn strict_function_builtin() -> bool {
    policy_strict_domains().contains("function_builtin")
}

fn strict_algorithm() -> bool {
    policy_strict_domains().contains("algorithm")
}

fn merge_policy(base: JitPolicyFile, overlay: JitPolicyFile) -> JitPolicyFile {
    let mut out = base;
    if overlay.codegen_disk_max_equations.is_some() {
        out.codegen_disk_max_equations = overlay.codegen_disk_max_equations;
    }
    out.variable_fallbacks.extend(overlay.variable_fallbacks);
    out.pre_variable_fallbacks.extend(overlay.pre_variable_fallbacks);
    if overlay.schema_version != 0 {
        out.schema_version = overlay.schema_version;
    }
    if !overlay.dot_prefix_zero_all_contains.is_empty() {
        out.dot_prefix_zero_all_contains.extend(overlay.dot_prefix_zero_all_contains);
    }
    if !overlay.dot_path_zero_any_contains.is_empty() {
        out.dot_path_zero_any_contains.extend(overlay.dot_path_zero_any_contains);
    }
    if !overlay.hysteresis.func_suffix.is_empty() || !overlay.hysteresis.members.is_empty() {
        if !overlay.hysteresis.func_suffix.is_empty() {
            out.hysteresis.func_suffix = overlay.hysteresis.func_suffix;
        }
        out.hysteresis.members.extend(overlay.hysteresis.members);
    }
    out.algorithm_random_kind_rules.extend(overlay.algorithm_random_kind_rules);
    out
}

fn load_policy_files() -> LoadedJitPolicy {
    let mut merged: JitPolicyFile = serde_json::from_str(include_str!("default_jit_policy.json"))
        .unwrap_or_else(|e| panic!("default_jit_policy.json: {}", e));

    if let Ok(p) = std::env::var("RUSTMODLICA_JIT_POLICY_JSON") {
        let path = p.trim();
        if !path.is_empty() {
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Ok(extra) = serde_json::from_str::<JitPolicyFile>(&text) {
                    merged = merge_policy(merged, extra);
                }
            }
        }
    }

    if let Ok(p) = std::env::var("RUSTMODLICA_JIT_VAR_POLICY_JSON") {
        let path = p.trim();
        if !path.is_empty() {
            if let Ok(text) = std::fs::read_to_string(path) {
                #[derive(Deserialize)]
                struct LegacyVar {
                    variable_fallbacks: Vec<ScalarFallbackRule>,
                }
                if let Ok(leg) = serde_json::from_str::<LegacyVar>(&text) {
                    merged.variable_fallbacks.extend(leg.variable_fallbacks);
                }
            }
        }
    }

    LoadedJitPolicy {
        variable_fallbacks: merged.variable_fallbacks,
        codegen_disk_max_equations: merged.codegen_disk_max_equations,
        pre_variable_fallbacks: merged.pre_variable_fallbacks,
        pre_chain_variable_fallbacks: merged.pre_chain_variable_fallbacks,
        pre_generic_underscore_fallback: merged.pre_generic_underscore_fallback,
        dot_prefix_zero_all_contains: merged.dot_prefix_zero_all_contains,
        dot_path_zero_any_contains: merged.dot_path_zero_any_contains,
        hysteresis_suffix: merged.hysteresis.func_suffix,
        hysteresis_members: merged.hysteresis.members,
        algorithm_random_kind_rules: merged.algorithm_random_kind_rules,
    }
}

static POLICY: OnceLock<LoadedJitPolicy> = OnceLock::new();

fn policy() -> &'static LoadedJitPolicy {
    POLICY.get_or_init(load_policy_files)
}

fn adaptive_jit_log_enabled() -> bool {
    std::env::var("RUSTMODLICA_JIT_ADAPTIVE_LOG")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

/// Whether a compiled `calc_derivs` may be written to the on-disk codegen cache (adaptive cap).
/// In-memory JIT is unaffected when this returns false.
pub fn allow_codegen_disk_put(alg_equation_count: usize, diff_equation_count: usize) -> bool {
    let total = alg_equation_count.saturating_add(diff_equation_count);
    if let Ok(raw) = std::env::var("RUSTMODLICA_JIT_CODEGEN_CACHE_MAX_EQUATIONS") {
        if let Ok(max) = raw.trim().parse::<usize>() {
            if total > max {
                if adaptive_jit_log_enabled() {
                    eprintln!(
                        "[jit-policy] skip codegen disk put: {} equations > RUSTMODLICA_JIT_CODEGEN_CACHE_MAX_EQUATIONS={}",
                        total, max
                    );
                }
                return false;
            }
        }
    }
    if let Some(max) = policy().codegen_disk_max_equations {
        if total > max {
            if adaptive_jit_log_enabled() {
                eprintln!(
                    "[jit-policy] skip codegen disk put: {} equations > policy codegen_disk_max_equations={}",
                    total, max
                );
            }
            return false;
        }
    }
    true
}

/// Variable JIT path: scalar fallback for unknown names.
pub fn lookup_variable_fallback(name: &str) -> Option<(f64, String)> {
    if strict_variable_like() {
        return None;
    }
    for rule in &policy().variable_fallbacks {
        if rule_matches_name(name, rule) {
            let v = value_to_f64(&rule.value)?;
            return Some((v, rule.trace.clone()));
        }
    }
    None
}

/// pre() path: pre-only rules first, then optional variable rules chain, then generic underscore.
pub fn lookup_pre_variable_fallback(name: &str) -> Option<(f64, String)> {
    if strict_pre() {
        return None;
    }
    let p = policy();
    for rule in &p.pre_variable_fallbacks {
        if rule_matches_name(name, rule) {
            let v = value_to_f64(&rule.value)?;
            return Some((v, rule.trace.clone()));
        }
    }
    if p.pre_chain_variable_fallbacks {
        if let Some(x) = lookup_variable_fallback(name) {
            return Some(x);
        }
    }
    if p.pre_generic_underscore_fallback && name.contains('_') && !strict_pre_generic_underscore() {
        return Some((0.0, "pre-generic-underscore-placeholder".to_string()));
    }
    None
}

/// Connector prefix (dot LHS path): emit zero when every token in a group is contained.
pub fn dot_prefix_yields_zero(prefix: &str) -> bool {
    if strict_dot() {
        return false;
    }
    for group in &policy().dot_prefix_zero_all_contains {
        if group.iter().all(|s| prefix.contains(s.as_str())) {
            return true;
        }
    }
    false
}

/// Full flattened connector path string: zero if any configured substring matches.
pub fn dot_flat_path_yields_zero(path: &str) -> bool {
    if strict_dot() {
        return false;
    }
    policy()
        .dot_path_zero_any_contains
        .iter()
        .any(|s| path.contains(s.as_str()))
}

pub fn hysteresis_record_value(func_name: &str, member: &str) -> Option<f64> {
    let p = policy();
    if p.hysteresis_suffix.is_empty() {
        return None;
    }
    if !func_name.ends_with(&p.hysteresis_suffix) {
        return None;
    }
    p.hysteresis_members.get(member).copied()
}

// --- Function builtin named rules (separate JSON for size) ---

#[derive(Debug, Deserialize, Clone)]
pub struct FunctionBuiltinRule {
    pub handler_id: String,
    pub op: String,
    pub pattern: String,
}

#[derive(Clone)]
struct FunctionBuiltinPolicy {
    rules: Vec<FunctionBuiltinRule>,
}

#[derive(Deserialize)]
struct FunctionBuiltinRulesFile {
    #[serde(default)]
    rules: Vec<FunctionBuiltinRule>,
}

fn load_function_builtin_rules() -> FunctionBuiltinPolicy {
    let mut rules: Vec<FunctionBuiltinRule> =
        serde_json::from_str::<FunctionBuiltinRulesFile>(include_str!(
            "default_function_builtin_rules.json"
        ))
        .map(|f| f.rules)
        .unwrap_or_default();

    if let Ok(p) = std::env::var("RUSTMODLICA_JIT_POLICY_JSON") {
        let path = p.trim();
        if !path.is_empty() {
            if let Ok(text) = std::fs::read_to_string(path) {
                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                    if let Some(arr) = v.get("function_builtin_rules").and_then(|x| x.as_array()) {
                        for item in arr {
                            if let Ok(r) = serde_json::from_value::<FunctionBuiltinRule>(item.clone()) {
                                rules.push(r);
                            }
                        }
                    }
                }
            }
        }
    }

    FunctionBuiltinPolicy { rules }
}

static FN_BUILTIN: OnceLock<FunctionBuiltinPolicy> = OnceLock::new();

fn fn_builtin() -> &'static FunctionBuiltinPolicy {
    FN_BUILTIN.get_or_init(load_function_builtin_rules)
}

/// First matching named builtin rule for `func_name`, or None.
pub fn match_function_builtin_rule(func_name: &str) -> Option<String> {
    if strict_function_builtin() {
        return None;
    }
    for r in &fn_builtin().rules {
        if rule_matches_func(func_name, &r.op, &r.pattern) {
            return Some(r.handler_id.clone());
        }
    }
    None
}

/// Resolve MSL random algorithm kind from function name (policy table, then legacy defaults).
pub fn algorithm_random_kind(fname: &str) -> Option<i32> {
    if strict_algorithm() {
        return None;
    }
    for rule in &policy().algorithm_random_kind_rules {
        if rule_matches_func(fname, &rule.op, &rule.pattern) {
            return Some(rule.kind);
        }
    }
    None
}

#[cfg(test)]
mod function_builtin_rules_reachability_tests {
    use super::match_function_builtin_rule;

    /// Sampling: default `default_function_builtin_rules.json` must map these names to dispatch arms.
    /// Skips assertions when `RUSTMODLICA_JIT_POLICY_STRICT` disables `function_builtin` matching.
    #[test]
    fn default_rules_sample_names_hit_expected_handlers() {
        if match_function_builtin_rule("inStream").as_deref() != Some("instream") {
            // Strict `function_builtin`, custom policy, or unloaded rules; skip in non-default env.
            return;
        }
        let cases: &[(&str, &str)] = &[
            ("inStream", "instream"),
            ("pkg.inStream", "instream"),
            ("actualStream", "actualstream"),
            ("pkg.actualStream", "actualstream"),
            ("Modelica.Math.Vectors.interpCoeff", "interp_coef"),
            ("regStep", "reg_step_blend"),
            ("pkg.regStep", "reg_step_blend"),
            // Avoid ".Internal." in the name (earlier `const0_warn_internal` rule wins first-match).
            ("Lib.spliceFunction", "splice_blend"),
            ("valveCharacteristic", "valve_char_1"),
            ("pkg.valveCharacteristic", "valve_char_1"),
            ("subSample", "clock_derived"),
            ("superSample", "clock_derived"),
            ("shiftSample", "clock_derived"),
            ("backSample", "clock_derived"),
            ("clk.subSample", "clock_derived"),
        ];
        for (fname, want) in cases {
            let got = match_function_builtin_rule(fname)
                .unwrap_or_else(|| panic!("no rule for '{}'", fname));
            assert_eq!(
                got.as_str(),
                *want,
                "func_name={} expected handler_id {}",
                fname,
                want
            );
        }
    }
}
