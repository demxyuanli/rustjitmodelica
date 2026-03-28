//! Data-driven JIT unknown-variable scalar fallbacks.
//! Implemented by `jit_policy` (`default_jit_policy.json` + `RUSTMODLICA_JIT_POLICY_JSON` + legacy `RUSTMODLICA_JIT_VAR_POLICY_JSON`).

/// Returns scalar value and trace tag for `jit_var_fallback_trace` (skip trace when empty).
pub fn lookup_var_fallback(name: &str) -> Option<(f64, String)> {
    crate::jit::jit_policy::lookup_variable_fallback(name)
}
