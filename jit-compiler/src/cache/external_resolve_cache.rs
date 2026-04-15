//! Persistent cache for `collect_external_calls` resolution (Modelica name -> C symbol + Library hint).

use crate::cache::cache_scope::CacheScope;
use crate::cache::lib_epoch::DepClosureFingerprint;
use crate::flatten::cache_sqlite;
use crate::loader::ModelLoader;
use serde::{Deserialize, Serialize};
use std::cmp::max;
use std::path::Path;
use xxhash_rust::xxh64::Xxh64;

const SCHEMA: &str = "erV1";
const KIND: &str = "external_resolve_v1";

fn external_resolve_ttl_days() -> i64 {
    std::env::var("RUSTMODLICA_EXTERNAL_RESOLVE_TTL_DAYS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|&v| v >= 1)
        .unwrap_or(90)
}

fn external_resolve_lru_max() -> i64 {
    std::env::var("RUSTMODLICA_EXTERNAL_RESOLVE_LRU_MAX")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|&v| v >= 1)
        .unwrap_or(2048)
}

fn prune(cache_root: &Path) {
    let Some(cfg) = cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root)) else {
        return;
    };
    let ttl_days = external_resolve_ttl_days();
    let lru_max = external_resolve_lru_max();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
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
        "DELETE FROM cache_entries WHERE kind=?1 AND created_ms < ?2",
        rusqlite::params![KIND, min_keep_created_ms],
    );
    let count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM cache_entries WHERE kind=?1",
            rusqlite::params![KIND],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if count > lru_max {
        let over = max(0, count - lru_max);
        let _ = tx.execute(
            "DELETE FROM cache_entries WHERE rowid IN (
                SELECT rowid FROM cache_entries
                WHERE kind=?1
                ORDER BY last_hit_ms ASC, created_ms ASC
                LIMIT ?2
            )",
            rusqlite::params![KIND, over],
        );
    }
    let _ = tx.commit();
}

#[derive(Serialize, Deserialize, Clone)]
struct ExternalResolveRecord {
    modelica_name: String,
    c_name: String,
    lib_hint: Option<String>,
}

pub fn external_resolve_cache_enabled() -> bool {
    match std::env::var("RUSTMODLICA_EXTERNAL_RESOLVE_CACHE") {
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

fn fingerprint_dynamic_libs(paths: &[String]) -> String {
    let mut h = Xxh64::new(0);
    let mut sorted: Vec<&str> = paths.iter().map(|s| s.as_str()).collect();
    sorted.sort_unstable();
    for p in sorted {
        h.update(p.as_bytes());
        h.update(&[0]);
        if let Ok(meta) = std::fs::metadata(p) {
            if let Some(mtime) = meta.modified().ok() {
                let ns = mtime
                    .duration_since(std::time::SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos();
                h.update(&ns.to_le_bytes());
            }
            h.update(&meta.len().to_le_bytes());
        }
    }
    format!("{:016x}", h.digest())
}

/// `sites_sorted` must be lexicographically sorted (stable cache key).
pub fn compute_external_resolve_key(
    model_name: &str,
    loader: &ModelLoader,
    sites_sorted: &[String],
    external_libs: &[String],
) -> String {
    let mut h = Xxh64::new(0);
    h.update(model_name.as_bytes());
    h.update(&[0]);
    for s in sites_sorted {
        h.update(s.as_bytes());
        h.update(&[0]);
    }
    let mut lib_dirs = loader.library_paths.clone();
    lib_dirs.sort_by_key(|p| p.to_string_lossy().to_string());
    lib_dirs.dedup();
    let mut paths_for_fp = loader.loaded_source_paths();
    if paths_for_fp.is_empty() {
        paths_for_fp = lib_dirs.clone();
    } else {
        paths_for_fp.sort_by_key(|p| p.to_string_lossy().to_string());
        paths_for_fp.dedup();
    }
    let fp = DepClosureFingerprint::compute(&paths_for_fp, &lib_dirs);
    h.update(fp.combined_hash().as_bytes());
    h.update(&[0]);
    h.update(fingerprint_dynamic_libs(external_libs).as_bytes());
    format!(
        "ext_resolve_v1:{}:{:016x}",
        model_name.replace('.', "_"),
        h.digest()
    )
}

pub fn try_load(
    cache_root: &Path,
    key: &str,
) -> Option<Vec<(String, String, Option<String>)>> {
    if external_resolve_cache_enabled() {
        prune(cache_root);
    }
    let cfg = cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root))?;
    let bytes = cache_sqlite::sqlite_get(&cfg.path, key, KIND).ok()??;
    let recs: Vec<ExternalResolveRecord> = bincode::deserialize(&bytes).ok()?;
    Some(
        recs
            .into_iter()
            .map(|r| (r.modelica_name, r.c_name, r.lib_hint))
            .collect(),
    )
}

/// Diagnose why a cache lookup missed: key absent, deserialization error, or other.
pub fn diagnose_miss(cache_root: &Path, key: &str) -> String {
    let cfg = match cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root)) {
        Some(c) => c,
        None => return "no_sqlite_config".to_string(),
    };
    match cache_sqlite::sqlite_get(&cfg.path, key, KIND) {
        Ok(Some(bytes)) => {
            match bincode::deserialize::<Vec<ExternalResolveRecord>>(&bytes) {
                Ok(_) => "key_found_but_stale_or_fingerprint_mismatch".to_string(),
                Err(e) => format!("deserialize_error: {}", e),
            }
        }
        Ok(None) => "key_not_found".to_string(),
        Err(e) => format!("sqlite_error: {}", e),
    }
}

pub fn try_store(
    cache_root: &Path,
    key: &str,
    list: &[(String, String, Option<String>)],
) -> Result<(), String> {
    if external_resolve_cache_enabled() {
        prune(cache_root);
    }
    let cfg = cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root))
        .ok_or_else(|| "no sqlite config for project scope".to_string())?;
    let recs: Vec<ExternalResolveRecord> = list
        .iter()
        .map(|(a, b, c)| ExternalResolveRecord {
            modelica_name: a.clone(),
            c_name: b.clone(),
            lib_hint: c.clone(),
        })
        .collect();
    let bytes = bincode::serialize(&recs).map_err(|e| e.to_string())?;
    cache_sqlite::sqlite_put(
        &cfg.path,
        key,
        SCHEMA,
        KIND,
        &bytes,
        None,
    )
    .map_err(|e| e.to_string())
}
