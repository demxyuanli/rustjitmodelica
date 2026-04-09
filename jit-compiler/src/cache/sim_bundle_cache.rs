//! L1-style SQLite index: maps flatten identity + layout to a codegen-cache key so `jit.compile` can be skipped
//! when native code is already on disk (`jit-codegen-cache`).

use crate::cache::cache_scope::CacheScope;
use crate::flatten::cache_sqlite;
use crate::jit::codegen_cache::CodegenCacheKey;
use serde::{Deserialize, Serialize};
use std::cmp::max;
use std::path::Path;
use xxhash_rust::xxh64::Xxh64;

const SCHEMA_VER: u32 = 1;

#[derive(Serialize, Deserialize)]
pub struct CompiledSimBundle {
    pub schema_ver: u32,
    pub codegen_key: CodegenCacheKey,
    pub when_count: usize,
    pub crossings_count: usize,
    pub var_layout_fingerprint: String,
}

pub fn sim_bundle_cache_enabled() -> bool {
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

pub fn var_layout_fingerprint(
    state_vars: &[String],
    discrete_vars: &[String],
    param_vars: &[String],
    output_vars: &[String],
) -> String {
    let mut h = Xxh64::new(0);
    let mut push_sorted = |v: &[String]| {
        let mut s: Vec<&str> = v.iter().map(String::as_str).collect();
        s.sort_unstable();
        for n in s {
            h.update(n.as_bytes());
            h.update(&[0]);
        }
    };
    push_sorted(state_vars);
    push_sorted(discrete_vars);
    push_sorted(param_vars);
    push_sorted(output_vars);
    format!("{:016x}", h.digest())
}

pub fn storage_key(flat_full_cache_key: &str, model_name: &str) -> String {
    let mut h = Xxh64::new(0);
    h.update(flat_full_cache_key.as_bytes());
    format!(
        "sim_bundle_v1_{}_{:016x}",
        model_name.replace('.', "_"),
        h.digest()
    )
}

pub fn try_load(cache_root: &Path, key: &str) -> Option<CompiledSimBundle> {
    prune(cache_root);
    let cfg = cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root))?;
    let bytes = cache_sqlite::sqlite_get(&cfg.path, key, "sim_bundle_v1").ok()??;
    let b: CompiledSimBundle = bincode::deserialize(&bytes).ok()?;
    if b.schema_ver != SCHEMA_VER {
        return None;
    }
    Some(b)
}

pub fn try_store(cache_root: &Path, key: &str, bundle: &CompiledSimBundle) -> Result<(), String> {
    prune(cache_root);
    let cfg = cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root))
        .ok_or_else(|| "no sqlite project cache".to_string())?;
    let bytes = bincode::serialize(bundle).map_err(|e| e.to_string())?;
    cache_sqlite::sqlite_put(
        &cfg.path,
        key,
        "sbV1",
        "sim_bundle_v1",
        &bytes,
        None,
    )
    .map_err(|e| e.to_string())
}

fn sim_bundle_ttl_days() -> i64 {
    std::env::var("RUSTMODLICA_SIM_BUNDLE_TTL_DAYS")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(30)
}

fn sim_bundle_lru_max() -> i64 {
    std::env::var("RUSTMODLICA_SIM_BUNDLE_LRU_MAX")
        .ok()
        .and_then(|v| v.trim().parse::<i64>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(256)
}

fn prune(cache_root: &Path) {
    let Some(cfg) = cache_sqlite::sqlite_config_for_scope(CacheScope::Project, Some(cache_root)) else {
        return;
    };
    let ttl_days = sim_bundle_ttl_days();
    let lru_max = sim_bundle_lru_max();
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
        "DELETE FROM cache_entries WHERE kind='sim_bundle_v1' AND created_ms < ?1",
        rusqlite::params![min_keep_created_ms],
    );
    let count: i64 = tx
        .query_row(
            "SELECT COUNT(*) FROM cache_entries WHERE kind='sim_bundle_v1'",
            [],
            |r| r.get(0),
        )
        .unwrap_or(0);
    if count > lru_max {
        let over = max(0, count - lru_max);
        let _ = tx.execute(
            "DELETE FROM cache_entries WHERE rowid IN (
                SELECT rowid FROM cache_entries
                WHERE kind='sim_bundle_v1'
                ORDER BY last_hit_ms ASC, created_ms ASC
                LIMIT ?1
            )",
            rusqlite::params![over],
        );
    }
    let _ = tx.commit();
}
