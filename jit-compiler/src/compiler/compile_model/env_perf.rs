pub(crate) fn perf_trace_enabled() -> bool {
    std::env::var("RUSTMODLICA_PERF_TRACE")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

pub(crate) fn jit_stub_parallel_enabled() -> bool {
    std::env::var("RUSTMODLICA_JIT_STUB_PARALLEL")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(false)
}

pub(crate) fn jit_partition_scan_parallel_enabled() -> bool {
    std::env::var("RUSTMODLICA_JIT_PARTITION_SCAN_PARALLEL")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(false)
}

pub(crate) fn env_flag(name: &str, default: bool) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(default)
}

pub(crate) fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .unwrap_or(default)
}
