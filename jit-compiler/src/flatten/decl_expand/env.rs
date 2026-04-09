pub(super) fn perf_trace_enabled() -> bool {
    std::env::var("RUSTMODLICA_PERF_TRACE")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

pub(super) fn flatten_decl_parallel_enabled() -> bool {
    std::env::var("RUSTMODLICA_FLATTEN_DECL_PARALLEL")
        .ok()
        .map(|v| matches!(v.trim(), "1" | "true" | "TRUE" | "on" | "ON"))
        .unwrap_or(false)
}

pub(super) fn flatten_decl_parallel_min_items() -> usize {
    std::env::var("RUSTMODLICA_FLATTEN_DECL_PARALLEL_MIN_ITEMS")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(256)
}
