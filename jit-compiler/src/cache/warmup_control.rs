//! Cross-thread coordination: foreground compiles vs background warmup.

use std::sync::atomic::{AtomicU64, Ordering};

static COMPILE_EPOCH: AtomicU64 = AtomicU64::new(0);

/// Call at the start of every `Compiler::compile` so warmup can detect concurrent work.
pub fn bump_compile_epoch() -> u64 {
    COMPILE_EPOCH.fetch_add(1, Ordering::Relaxed) + 1
}

/// Snapshot the compile epoch (e.g. when a warmup thread starts after its delay).
pub fn compile_epoch_snapshot() -> u64 {
    COMPILE_EPOCH.load(Ordering::Relaxed)
}

/// Returns true if any compile started after `baseline` (exclusive of equal).
pub fn compile_epoch_changed_since(baseline: u64) -> bool {
    COMPILE_EPOCH.load(Ordering::Relaxed) != baseline
}
