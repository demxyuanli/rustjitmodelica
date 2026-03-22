use std::time::Instant;

pub(crate) fn stage_trace_enabled() -> bool {
    std::env::var("RUSTMODLICA_STAGE_TRACE")
        .ok()
        .map(|v| {
            let v = v.trim();
            v == "1" || v.eq_ignore_ascii_case("true") || v.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

pub(crate) fn log_stage_timing(stage_trace: bool, stage: &str, started_at: Instant) {
    if stage_trace {
        eprintln!("[stage][timing] {} {} ms", stage, started_at.elapsed().as_millis());
    }
}
