//! Global disk cache budget: cross-kind byte-level eviction (L4-T03).
//!
//! Scans SQLite cache directories and JIT codegen file cache, computes total usage,
//! and evicts oldest entries when the budget is exceeded.

use std::path::{Path, PathBuf};

const DEFAULT_MAX_BYTES: u64 = 1_073_741_824; // 1 GB
const DEFAULT_TTL_DAYS: i64 = 30;

fn max_cache_bytes() -> u64 {
    std::env::var("RUSTMODLICA_CACHE_MAX_BYTES")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v >= 1_048_576) // min 1 MB
        .unwrap_or(DEFAULT_MAX_BYTES)
}

fn global_ttl_days() -> i64 {
    std::env::var("RUSTMODLICA_CACHE_TTL_DAYS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(DEFAULT_TTL_DAYS)
}

/// Summary of a single cache file for eviction ordering.
struct CacheFileEntry {
    path: PathBuf,
    size: u64,
    modified_epoch_ms: i64,
}

fn scan_jit_codegen_files(dir: &Path) -> Vec<CacheFileEntry> {
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for e in entries.flatten() {
        let p = e.path();
        let ext_ok = p
            .extension()
            .map(|ext| ext == "bin" || ext == "rawbin" || ext == "json")
            .unwrap_or(false);
        if !ext_ok {
            continue;
        }
        if let Ok(meta) = p.metadata() {
            let mtime_ms = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0);
            out.push(CacheFileEntry {
                path: p,
                size: meta.len(),
                modified_epoch_ms: mtime_ms,
            });
        }
    }
    out
}

fn scan_sqlite_db_files(cache_root: &Path) -> Vec<CacheFileEntry> {
    let mut out = Vec::new();
    let scopes = ["project", "std", "user"];
    for scope in &scopes {
        let dir = cache_root.join(scope);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if let Ok(meta) = p.metadata() {
                let mtime_ms = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as i64)
                    .unwrap_or(0);
                out.push(CacheFileEntry {
                    path: p,
                    size: meta.len(),
                    modified_epoch_ms: mtime_ms,
                });
            }
        }
    }
    out
}

/// Snapshot of global cache usage.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct GlobalBudgetSnapshot {
    pub total_bytes: u64,
    pub max_bytes: u64,
    pub jit_file_bytes: u64,
    pub sqlite_file_bytes: u64,
    pub files_scanned: u64,
    pub files_evicted: u64,
    pub bytes_evicted: u64,
}

/// Scan all cache directories, evict oldest files if over budget, return snapshot.
pub fn enforce_global_budget(
    flatten_cache_root: Option<&Path>,
    jit_codegen_root: Option<&Path>,
) -> GlobalBudgetSnapshot {
    let max_bytes = max_cache_bytes();
    let ttl_days = global_ttl_days();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let min_keep_ms = now_ms - ttl_days * 24 * 60 * 60 * 1000;

    let mut all_files: Vec<CacheFileEntry> = Vec::new();

    let mut jit_bytes = 0_u64;
    if let Some(jit_root) = jit_codegen_root {
        let jit_files = scan_jit_codegen_files(jit_root);
        for f in &jit_files {
            jit_bytes += f.size;
        }
        all_files.extend(jit_files);
    }

    let mut sqlite_bytes = 0_u64;
    if let Some(cache_root) = flatten_cache_root {
        let sqlite_files = scan_sqlite_db_files(cache_root);
        for f in &sqlite_files {
            sqlite_bytes += f.size;
        }
        all_files.extend(sqlite_files);
    }

    let total_before = jit_bytes + sqlite_bytes;
    let files_scanned = all_files.len() as u64;

    // Sort by modified time ascending (oldest first) for LRU eviction.
    all_files.sort_by_key(|f| f.modified_epoch_ms);

    let mut evicted_count = 0_u64;
    let mut evicted_bytes = 0_u64;
    let mut current_total = total_before;

    for entry in &all_files {
        if current_total <= max_bytes && entry.modified_epoch_ms >= min_keep_ms {
            break;
        }
        let should_evict = entry.modified_epoch_ms < min_keep_ms || current_total > max_bytes;
        if should_evict {
            // Do not evict SQLite main DB files (only evict .bin/.rawbin/.json JIT files
            // and SQLite WAL/SHM temp files by TTL).
            let is_sqlite_main = entry
                .path
                .extension()
                .map(|e| e == "sqlite")
                .unwrap_or(false);
            if is_sqlite_main {
                continue;
            }
            if std::fs::remove_file(&entry.path).is_ok() {
                evicted_count += 1;
                evicted_bytes += entry.size;
                current_total = current_total.saturating_sub(entry.size);
            }
        }
    }

    GlobalBudgetSnapshot {
        total_bytes: total_before,
        max_bytes,
        jit_file_bytes: jit_bytes,
        sqlite_file_bytes: sqlite_bytes,
        files_scanned,
        files_evicted: evicted_count,
        bytes_evicted: evicted_bytes,
    }
}
