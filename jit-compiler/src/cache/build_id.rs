//! Compile-time / runtime fingerprint of the running `rustmodlica` binary.
//!
//! Used as the root of the SQLite cache parent-hash chain (see
//! [`crate::flatten::cache_sqlite::compute_parent_hash`]). Rewritten binaries produce a new
//! fingerprint even if `CARGO_PKG_VERSION` did not change, so stale flatten / IR rows written
//! by an older binary are naturally rejected on read (treated as miss, regenerated on next use).
//!
//! Strategy (first variant that succeeds wins; all are memoized via [`OnceLock`]):
//! 1. Hash the bytes of `std::env::current_exe()` with `xxh3_128`. Deterministic per
//!    compiled-and-linked binary; changes whenever codegen emits a different executable.
//! 2. Fall back to `CARGO_PKG_VERSION` (prefixed `v:`). Used when the exe is unreadable
//!    (sandboxed CI, exotic embedding hosts).
//!
//! The value is intentionally short (~20 hex chars) so it stays cheap to include in every
//! SQLite row: `INSERT ... parent_hash = ?` is dominated by WAL fsync cost, not the column
//! width, but short strings help diagnostic tooling (`sqlite3 .schema` dumps, etc.).

use std::sync::OnceLock;

use xxhash_rust::xxh3::Xxh3;

const MAX_FP_BYTES_HASHED: usize = 8 * 1024 * 1024;

/// Compiler package version (same constant used by [`crate::cache::ir_epoch::COMPILER_VERSION`]).
pub const COMPILER_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Process-wide binary fingerprint. Prefix encodes the source:
/// * `bin:<20 hex>` — `xxh3_128` of the current exe bytes (truncated to 20 hex chars).
/// * `v:<pkg-version>` — fallback when `current_exe()` is unreachable.
pub fn binary_build_id() -> &'static str {
    static FP: OnceLock<String> = OnceLock::new();
    FP.get_or_init(|| {
        if let Some(h) = hash_current_exe() {
            return format!("bin:{}", &h[..20.min(h.len())]);
        }
        format!("v:{}", COMPILER_VERSION)
    })
    .as_str()
}

fn hash_current_exe() -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let md = std::fs::metadata(&exe).ok()?;
    let len = md.len();
    // For very large binaries (e.g., >8 MiB debug build), hash a prefix window to bound
    // startup cost. The prefix still changes for every code regeneration because linkers
    // emit the .text section near the file start.
    let mut hasher = Xxh3::new();
    hasher.update(COMPILER_VERSION.as_bytes());
    hasher.update(&len.to_le_bytes());
    let data = if len <= MAX_FP_BYTES_HASHED as u64 {
        std::fs::read(&exe).ok()?
    } else {
        use std::io::Read;
        let mut f = std::fs::File::open(&exe).ok()?;
        let mut buf = vec![0u8; MAX_FP_BYTES_HASHED];
        f.read_exact(&mut buf).ok()?;
        buf
    };
    hasher.update(&data);
    Some(format!("{:032x}", hasher.digest128()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_build_id_is_nonempty_and_memoized() {
        let a = binary_build_id();
        assert!(!a.is_empty());
        assert!(a.starts_with("bin:") || a.starts_with("v:"));
        let b = binary_build_id();
        assert_eq!(a, b, "binary_build_id must be process-wide memoized");
    }
}
