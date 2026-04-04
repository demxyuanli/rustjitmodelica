//! Library epoch mechanism for cache invalidation.
//!
//! Each library directory (e.g., Modelica/, StandardLib/) has an associated epoch
//! that changes when any .mo file in the library is modified. This provides
//! fast cache invalidation without scanning all files on every cache lookup.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::SystemTime;

/// Epoch file name within each library directory.
const LIB_EPOCH_FILE: &str = ".rustmodlica_lib_epoch";

/// Global cache of library epochs (path -> (epoch, mtime_check)).
static LIB_EPOCH_CACHE: OnceLock<RwLock<HashMap<PathBuf, LibEpochEntry>>> = OnceLock::new();

#[derive(Debug, Clone)]
struct LibEpochEntry {
    epoch: u64,
    computed_at: SystemTime,
    dir_mtime: SystemTime,
    #[allow(dead_code)]
    file_count: usize,
}

fn epoch_cache() -> &'static RwLock<HashMap<PathBuf, LibEpochEntry>> {
    LIB_EPOCH_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Compute a library's epoch based on its directory contents.
/// The epoch is a hash of all .mo file mtimes + sizes.
pub fn compute_lib_epoch(lib_dir: &Path) -> Option<u64> {
    use xxhash_rust::xxh64::Xxh64;

    let dir_meta = std::fs::metadata(lib_dir).ok()?;
    let dir_mtime = dir_meta.modified().ok()?;

    // Check cache first
    if let Ok(cache) = epoch_cache().read() {
        if let Some(entry) = cache.get(lib_dir) {
            // If directory mtime hasn't changed and we checked recently, use cached epoch
            if entry.dir_mtime == dir_mtime {
                let now = SystemTime::now();
                if let Ok(elapsed) = now.duration_since(entry.computed_at) {
                    // Cache for up to 30 seconds
                    if elapsed.as_secs() < 30 {
                        return Some(entry.epoch);
                    }
                }
            }
        }
    }

    // Recompute epoch by scanning directory
    let mut h = Xxh64::new(0);
    let mut file_count = 0usize;

    fn scan_dir(dir: &Path, h: &mut Xxh64, count: &mut usize) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                scan_dir(&path, h, count)?;
            } else if path.extension().and_then(|s| s.to_str()) == Some("mo") {
                let meta = entry.metadata()?;
                if let Some(mtime) = meta.modified().ok() {
                    let ns = mtime
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos();
                    h.update(&ns.to_le_bytes());
                }
                h.update(&meta.len().to_le_bytes());
                *count += 1;
            }
        }
        Ok(())
    }

    scan_dir(lib_dir, &mut h, &mut file_count).ok()?;
    let epoch = h.digest();

    // Update cache
    if let Ok(mut cache) = epoch_cache().write() {
        cache.insert(
            lib_dir.to_path_buf(),
            LibEpochEntry {
                epoch,
                computed_at: SystemTime::now(),
                dir_mtime,
                file_count,
            },
        );
    }

    Some(epoch)
}

/// Get the combined epoch for multiple library directories.
/// Returns a hash that changes if any library changes.
pub fn compute_libs_epoch(libs: &[PathBuf]) -> String {
    use xxhash_rust::xxh64::Xxh64;

    let mut h = Xxh64::new(0);
    let mut epochs: Vec<(&PathBuf, u64)> = Vec::new();

    for lib in libs {
        if let Some(epoch) = compute_lib_epoch(lib) {
            epochs.push((lib, epoch));
        }
    }

    // Sort for stability
    epochs.sort_by_key(|(p, _)| p.to_string_lossy().to_string());

    for (path, epoch) in &epochs {
        h.update(path.to_string_lossy().as_bytes());
        h.update(&epoch.to_le_bytes());
    }

    format!("{:016x}", h.digest())
}

/// Write an epoch file to a library directory (call after modifying library files).
pub fn touch_lib_epoch(lib_dir: &Path) -> std::io::Result<()> {
    let epoch_path = lib_dir.join(LIB_EPOCH_FILE);
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::fs::write(&epoch_path, format!("{}\n", now))?;

    // Invalidate cache entry
    if let Ok(mut cache) = epoch_cache().write() {
        cache.remove(lib_dir);
    }

    Ok(())
}

/// Dependency closure fingerprint for cache key enhancement.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DepClosureFingerprint {
    /// Combined epoch of all library directories.
    pub libs_epoch: String,
    /// Hash of all directly loaded file paths + their content hashes.
    pub deps_hash: String,
    /// Number of dependencies (for debugging).
    pub deps_count: usize,
}

impl DepClosureFingerprint {
    /// Compute fingerprint from loaded source paths and library directories.
    pub fn compute(loaded_paths: &[PathBuf], libs: &[PathBuf]) -> Self {
        use xxhash_rust::xxh64::Xxh64;

        let libs_epoch = compute_libs_epoch(libs);

        let mut h = Xxh64::new(0);
        let mut sorted_paths: Vec<&PathBuf> = loaded_paths.iter().collect();
        sorted_paths.sort_by_key(|p| p.to_string_lossy().to_string());

        for path in &sorted_paths {
            h.update(path.to_string_lossy().as_bytes());
            // Include file mtime + size for extra safety
            if let Ok(meta) = std::fs::metadata(path) {
                if let Some(mtime) = meta.modified().ok() {
                    let ns = mtime
                        .duration_since(SystemTime::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos();
                    h.update(&ns.to_le_bytes());
                }
                h.update(&meta.len().to_le_bytes());
            }
        }

        Self {
            libs_epoch,
            deps_hash: format!("{:016x}", h.digest()),
            deps_count: loaded_paths.len(),
        }
    }

    /// Combine into a single hash for embedding in cache key.
    pub fn combined_hash(&self) -> String {
        use xxhash_rust::xxh64::Xxh64;

        let mut h = Xxh64::new(0);
        h.update(self.libs_epoch.as_bytes());
        h.update(self.deps_hash.as_bytes());
        h.update(&self.deps_count.to_le_bytes());
        format!("{:016x}", h.digest())
    }
}

/// Check if the current fingerprint matches a previously recorded one.
pub fn fingerprint_matches(cached: &DepClosureFingerprint, loaded_paths: &[PathBuf], libs: &[PathBuf]) -> bool {
    let current = DepClosureFingerprint::compute(loaded_paths, libs);
    current.libs_epoch == cached.libs_epoch && current.deps_hash == cached.deps_hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_libs_epoch_stability() {
        // Same input should produce same epoch
        let libs: Vec<PathBuf> = vec![
            PathBuf::from("Modelica"),
            PathBuf::from("StandardLib"),
        ];

        // Note: These directories may not exist, so epochs will be 0
        let e1 = compute_libs_epoch(&libs);
        let e2 = compute_libs_epoch(&libs);
        assert_eq!(e1, e2);
    }

    #[test]
    fn test_fingerprint_combined_hash() {
        let fp1 = DepClosureFingerprint {
            libs_epoch: "abc123".to_string(),
            deps_hash: "def456".to_string(),
            deps_count: 5,
        };
        let fp2 = DepClosureFingerprint {
            libs_epoch: "abc123".to_string(),
            deps_hash: "def456".to_string(),
            deps_count: 5,
        };
        assert_eq!(fp1.combined_hash(), fp2.combined_hash());

        let fp3 = DepClosureFingerprint {
            libs_epoch: "abc123".to_string(),
            deps_hash: "def457".to_string(), // Different
            deps_count: 5,
        };
        assert_ne!(fp1.combined_hash(), fp3.combined_hash());
    }
}
