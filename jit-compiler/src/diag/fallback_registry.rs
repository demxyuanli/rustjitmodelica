use std::env;

const FALLBACK_ENV_VARS: &[&str] = &[
    "RUSTMODLICA_BLT_MAX_EQ_FOR_SORT",
    "RUSTMODLICA_BLT_SHARE_EDGE_MAX_N",
    "RUSTMODLICA_STRICT_NEWTON",
    "RUSTMODLICA_JIT_DOT_FALLBACK_ZERO",
    "RUSTMODLICA_NEWTON_SPARSE_POLICY",
    "RUSTMODLICA_SYNC_WARN",
    "RUSTMODLICA_NEWTON_DUAL_VALIDATE",
    "RUSTMODLICA_JIT_BUILTIN_TRACE",
    "RUSTMODLICA_JIT_VAR_FALLBACK_TRACE",
];

pub fn print_fallback_config() {
    eprintln!("[fallback:config] begin");
    for key in FALLBACK_ENV_VARS {
        let value = env::var(key).unwrap_or_else(|_| "<unset>".to_string());
        eprintln!("[fallback:config] {}={}", key, value);
    }
    eprintln!("[fallback:config] end");
}
