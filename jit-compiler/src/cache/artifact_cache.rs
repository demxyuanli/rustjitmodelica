use crate::cache::artifact_bundle::CompiledArtifactBundle;
use crate::cache::cache_scope::CacheScope;
use crate::flatten::cache_sqlite;
use std::cmp::max;
use std::collections::VecDeque;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{OnceLock, RwLock};

const LRU_CAP: usize = 32;

struct LruEntry {
    key: String,
    bundle: CompiledArtifactBundle,
}

fn artifact_lru() -> &'static RwLock<VecDeque<LruEntry>> {
    static CACHE: OnceLock<RwLock<VecDeque<LruEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(VecDeque::with_capacity(LRU_CAP)))
}

fn lru_capacity() -> usize {
    std::env::var("RUSTMODLICA_ARTIFACT_LRU_SIZE")
        .ok()
        .and_then(|v| v.trim().parse::<usize>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(LRU_CAP)
}

pub fn artifact_cache_enabled() -> bool {
    match std::env::var("RUSTMODLICA_ARTIFACT_CACHE") {
        Ok(v) => {
            let t = v.trim();
            if t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("no") {
                return false;
            }
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes") || t.is_empty()
        }
        Err(_) => true,
    }
}

/// When true, artifact bundle writes are deferred until `Compiler::confirm_artifact_stored()`
/// is called (strict mode — only persist after validation success).
/// Default: true (strict). Set `RUSTMODLICA_ARTIFACT_DEFERRED_WRITE=0` for immediate writes.
pub fn artifact_deferred_write() -> bool {
    match std::env::var("RUSTMODLICA_ARTIFACT_DEFERRED_WRITE") {
        Ok(v) => {
            let t = v.trim();
            if t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("no") {
                return false;
            }
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes") || t.is_empty()
        }
        Err(_) => true,
    }
}

pub fn get(cache_root: &Path, key: &str) -> Option<CompiledArtifactBundle> {
    if !artifact_cache_enabled() {
        return None;
    }
    prune_sqlite(cache_root);
    // L1: process-internal LRU
    if let Ok(lock) = artifact_lru().read() {
        for entry in lock.iter().rev() {
            if entry.key == key {
                let mut clone = entry.bundle.clone();
                clone.artifact_kind = format!("{}_lru_hit", clone.artifact_kind);
                return Some(clone);
            }
        }
    }
    // L2: SQLite
    let cfg = cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root))?;
    let bytes = cache_sqlite::sqlite_get(&cfg.path, key, "compiled_artifact_v1").ok()??;
    let bundle: CompiledArtifactBundle = bincode::deserialize(&bytes).ok()?;
    // Promote to LRU
    promote(key, &bundle);
    Some(bundle)
}

pub fn put(cache_root: &Path, key: &str, bundle: &CompiledArtifactBundle) -> Result<(), String> {
    prune_sqlite(cache_root);
    let cfg = cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root))
        .ok_or_else(|| "sqlite config unavailable".to_string())?;
    let bytes = bincode::serialize(bundle).map_err(|e| e.to_string())?;
    cache_sqlite::sqlite_put(
        &cfg.path,
        key,
        "caV1",
        "compiled_artifact_v1",
        &bytes,
        None,
    )
    .map_err(|e| e.to_string())?;
    promote(key, bundle);
    Ok(())
}

fn promote(key: &str, bundle: &CompiledArtifactBundle) {
    let cap = lru_capacity();
    if let Ok(mut lock) = artifact_lru().write() {
        // Evict same key if present
        lock.retain(|e| e.key != key);
        lock.push_back(LruEntry {
            key: key.to_string(),
            bundle: bundle.clone(),
        });
        while lock.len() > cap {
            lock.pop_front();
        }
    }
}

fn artifact_ttl_days() -> i64 {
    std::env::var("RUSTMODLICA_ARTIFACT_TTL_DAYS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(30)
}

fn artifact_sqlite_row_cap() -> i64 {
    std::env::var("RUSTMODLICA_ARTIFACT_LRU_MAX")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(128)
}

static LAST_ARTIFACT_PRUNE_MS: AtomicU64 = AtomicU64::new(0);

fn prune_sqlite_throttle_ms() -> u64 {
    std::env::var("RUSTMODLICA_ARTIFACT_PRUNE_INTERVAL_MS")
        .ok()
        .and_then(|v| v.trim().parse::<u64>().ok())
        .filter(|v| *v >= 1_000)
        .unwrap_or(60_000)
}

fn prune_sqlite(cache_root: &Path) {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let last = LAST_ARTIFACT_PRUNE_MS.load(Ordering::Relaxed);
    if now_ms.saturating_sub(last) < prune_sqlite_throttle_ms() {
        return;
    }
    LAST_ARTIFACT_PRUNE_MS.store(now_ms, Ordering::Relaxed);

    let Some(cfg) = cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root)) else {
        return;
    };
    let ttl_days = artifact_ttl_days();
    let row_cap = artifact_sqlite_row_cap();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let min_keep_created_ms = now_ms - ttl_days * 24 * 60 * 60 * 1000;
    let mut conn = match rusqlite::Connection::open(&cfg.path) {
        Ok(c) => c,
        Err(_) => return,
    };
    let tx = match conn.transaction() {
        Ok(t) => t,
        Err(_) => return,
    };
    let _ = tx.execute(
        "DELETE FROM cache_entries WHERE kind='compiled_artifact_v1' AND created_ms < ?1",
        rusqlite::params![min_keep_created_ms],
    );
    let count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM cache_entries WHERE kind='compiled_artifact_v1'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if count > row_cap {
        let over = max(0, count - row_cap);
        let _ = tx.execute(
            "DELETE FROM cache_entries WHERE rowid IN (
                SELECT rowid FROM cache_entries
                WHERE kind='compiled_artifact_v1'
                ORDER BY last_hit_ms ASC, created_ms ASC
                LIMIT ?1
            )",
            rusqlite::params![over],
        );
    }
    let _ = tx.commit();
}
