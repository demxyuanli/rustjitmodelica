//! Opt-in stderr tracing for Windows COFF relocation and exec buffers.
//!
//! Enable with `RUSTMODLICA_COFF_RELOC_TRACE=1` (summary) or `=2` (per-section relocation counts).
//! Used to triage warm-cache `STATUS_ACCESS_VIOLATION` (0xC0000005) around import slots / RX maps.

use std::sync::OnceLock;

pub(crate) fn trace_level() -> u8 {
    static LV: OnceLock<u8> = OnceLock::new();
    *LV.get_or_init(|| {
        std::env::var("RUSTMODLICA_COFF_RELOC_TRACE")
            .ok()
            .and_then(|s| match s.trim() {
                "2" => Some(2u8),
                "1" => Some(1u8),
                t if t.eq_ignore_ascii_case("true")
                    || t.eq_ignore_ascii_case("yes")
                    || t.eq_ignore_ascii_case("on") =>
                {
                    Some(1u8)
                }
                _ => None,
            })
            .unwrap_or(0)
    })
}

#[inline]
pub(crate) fn trace_basic() -> bool {
    trace_level() >= 1
}

#[inline]
pub(crate) fn trace_sections() -> bool {
    trace_level() >= 2
}

pub(crate) fn trace_line(args: std::fmt::Arguments<'_>) {
    eprintln!("[coff-reloc] {}", args);
}
