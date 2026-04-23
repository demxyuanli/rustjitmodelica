//! Selective cache invalidation (SQLite rows and optional JIT files).

use crate::cache::cache_scope::CacheScope;
use crate::cache::ir_epoch::CacheStage;
use crate::flatten::cache_sqlite;
use rusqlite::params;
use std::path::Path;

/// Hard: delete all rows in the given scope DB (same as `sqlite_invalidate_scope`).
pub fn hard_invalidate_scope_db(path: &std::path::Path) -> Result<(), String> {
    cache_sqlite::sqlite_invalidate_scope(path).map_err(|e| e.to_string())
}

/// Soft (stage): delete cache_entries whose key matches `<scopePrefix>:<stageTag>:%`.
pub fn soft_invalidate_stage(cache_root: &Path, stage: CacheStage) -> Result<u64, String> {
    let tag = stage.tag();
    let mut total = 0_u64;
    for scope in [CacheScope::Project, CacheScope::UserExt, CacheScope::GlobalStd] {
        let Some(cfg) = cache_sqlite::sqlite_config_for_scope(scope.clone(), Some(cache_root)) else {
            continue;
        };
        if !cfg.path.is_file() {
            continue;
        }
        let conn = cache_sqlite::conn_for_prune(&cfg.path).map_err(|e| e.to_string())?;
        let pattern = format!("{}:{}:%", scope.prefix(), tag);
        let n = conn
            .execute(
                "DELETE FROM cache_entries WHERE key LIKE ?1",
                params![pattern],
            )
            .map_err(|e| e.to_string())?;
        cache_sqlite::sqlite_connection_pool_evict(&cfg.path);
        total += n as u64;
    }
    Ok(total)
}

/// Soft (stage): apply to all disk cache roots and the given scopes.
/// This is the API used by the compilation pipeline (CompileFlagsChanged).
pub fn soft_invalidate_stage_all_roots(
    stage: CacheStage,
    scopes: &[CacheScope],
) -> Result<u64, String> {
    let tag = stage.tag();
    let mut total = 0_u64;
    for root in crate::flatten::flatten_cache::all_disk_cache_roots() {
        for scope in scopes {
            let Some(cfg) = cache_sqlite::sqlite_config_for_scope(scope.clone(), Some(root.as_path()))
            else {
                continue;
            };
            if !cfg.path.is_file() {
                continue;
            }
            let conn = cache_sqlite::conn_for_prune(&cfg.path).map_err(|e| e.to_string())?;
            let pattern = format!("{}:{}:%", scope.prefix(), tag);
            let n = conn
                .execute(
                    "DELETE FROM cache_entries WHERE key LIKE ?1",
                    params![pattern],
                )
                .map(|n| n as u64)
                .unwrap_or(0);
            cache_sqlite::sqlite_connection_pool_evict(&cfg.path);
            total += n;
        }
    }
    Ok(total)
}

/// Delete artifact and sim-bundle rows for a model (keys embed `model_name` with `_` for dots).
pub fn invalidate_model_sqlite(_cache_root: &Path, model_name: &str) -> Result<u64, String> {
    let safe = model_name.replace('.', "_");
    let art = format!("artifact_v1:{}:%", safe);
    let sim = format!("sim_bundle_v1_{}_%", safe);
    let mut total = 0_u64;
    let scopes = [
        CacheScope::Project,
        CacheScope::UserExt,
        CacheScope::GlobalStd,
    ];
    for root in crate::flatten::flatten_cache::all_disk_cache_roots() {
        for &scope in &scopes {
            let Some(cfg) = cache_sqlite::sqlite_config_for_scope(scope, Some(root.as_path()))
            else {
                continue;
            };
            if !cfg.path.is_file() {
                continue;
            }
            let conn = cache_sqlite::conn_for_prune(&cfg.path).map_err(|e| e.to_string())?;
            let n1: u64 = conn
                .execute("DELETE FROM cache_entries WHERE key LIKE ?1", rusqlite::params![art])
                .map(|n| n as u64)
                .unwrap_or(0);
            let n2: u64 = conn
                .execute("DELETE FROM cache_entries WHERE key LIKE ?1", rusqlite::params![sim])
                .map(|n| n as u64)
                .unwrap_or(0);
            cache_sqlite::sqlite_connection_pool_evict(&cfg.path);
            total += n1 + n2;
        }
    }
    Ok(total)
}
