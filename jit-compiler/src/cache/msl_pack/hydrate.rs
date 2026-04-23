//! Locate a pre-baked pack on disk and merge its SQLite rows into the active L0 cache.

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use crate::cache::cache_scope::CacheScope;
use crate::flatten::cache_sqlite;

use super::context;
use super::manifest::{self, MslPackManifestV1, PACK_FORMAT_V1};
use super::tree_digest;

const ENV_PACK_DIRS: &str = "RUSTMODLICA_MSL_PACK_DIRS";

fn merged_packs() -> &'static Mutex<HashSet<String>> {
    static M: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    M.get_or_init(|| Mutex::new(HashSet::new()))
}

fn pack_session_key(msl_root: &Path, tree: &str) -> String {
    format!(
        "{}|{}",
        msl_root.display().to_string().replace('\\', "/"),
        tree
    )
}

fn list_pack_roots(base: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if base.join("manifest.json").is_file() {
        out.push(base.to_path_buf());
        return out;
    }
    let Ok(rd) = fs::read_dir(base) else {
        return out;
    };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() && p.join("manifest.json").is_file() {
            out.push(p);
        }
    }
    out
}

fn find_matching_pack(msl_root: &Path, tree: &str) -> Option<(PathBuf, MslPackManifestV1)> {
    let dirs = std::env::var(ENV_PACK_DIRS).ok()?;
    for root in dirs.split(';') {
        let t = root.trim();
        if t.is_empty() {
            continue;
        }
        let base = PathBuf::from(t);
        if !base.is_dir() {
            continue;
        }
        for cand in list_pack_roots(&base) {
            let mf = cand.join("manifest.json");
            let Ok(m) = manifest::read_manifest(&mf) else {
                continue;
            };
            if m.pack_format != PACK_FORMAT_V1 {
                continue;
            }
            if m.tree_digest != tree {
                continue;
            }
            let _ = msl_root;
            return Some((cand, m));
        }
    }
    None
}

/// Copy rows from pack `cache-std.sqlite` into the process L0 SQLite (current std tier).
fn merge_sqlite_pack(pack_sqlite: &Path) -> Result<usize, String> {
    let Some(cfg) = cache_sqlite::sqlite_write_config_for_scope(CacheScope::GlobalStd) else {
        return Err("sqlite L0 cache disabled or std tier root missing".to_string());
    };
    let dest = &cfg.path;
    let src_conn = cache_sqlite::conn_for_prune(pack_sqlite).map_err(|e| e.to_string())?;
    let rows: Vec<(String, String, String, Vec<u8>, Option<String>)> = {
        let mut stmt = src_conn
            .prepare(
                "SELECT key, schema, kind, blob, deps_json FROM cache_entries WHERE kind IN ('parse_v1', 'model_ast_v1', 'flat_cache_v1', 'flat_cache_v2')",
            )
            .map_err(|e| e.to_string())?;
        let mapped = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, Vec<u8>>(3)?,
                    r.get::<_, Option<String>>(4)?,
                ))
            })
            .map_err(|e| e.to_string())?;
        let mut v = Vec::new();
        for row in mapped {
            v.push(row.map_err(|e| e.to_string())?);
        }
        v
    };
    let mut n = 0usize;
    for (key, schema, kind, blob, deps_json) in rows {
        cache_sqlite::sqlite_put_atomic(
            dest,
            key.as_str(),
            schema.as_str(),
            kind.as_str(),
            blob.as_slice(),
            deps_json.as_deref(),
        )
        .map_err(|e| e.to_string())?;
        n += 1;
    }
    cache_sqlite::sqlite_connection_pool_evict(dest);
    Ok(n)
}

/// Called from [`crate::loader::ModelLoader::add_path`] when a directory looks like an MSL root.
pub fn on_msl_library_path_added(msl_root: &Path) -> Result<(), String> {
    if !msl_root.join("Modelica").join("package.mo").is_file() {
        return Ok(());
    }
    let tree = tree_digest::compute_msl_tree_digest(msl_root).map_err(|e| e.to_string())?;
    let key = pack_session_key(msl_root, &tree);
    {
        let g = merged_packs().lock().map_err(|e| e.to_string())?;
        if g.contains(&key) {
            return Ok(());
        }
    }
    let Some((pack_dir, man)) = find_matching_pack(msl_root, &tree) else {
        return Ok(());
    };
    let sqlite_path = pack_dir.join(&man.cache_std_sqlite.relative_path);
    if !sqlite_path.is_file() {
        return Err(format!(
            "pack sqlite missing: {}",
            sqlite_path.display()
        ));
    }
    manifest::verify_entry(&sqlite_path, &man.cache_std_sqlite).map_err(|e| e.to_string())?;
    let _n = merge_sqlite_pack(&sqlite_path)?;
    context::session_activate(man.msl_version.as_str(), tree.as_str());
    let mut g = merged_packs().lock().map_err(|e| e.to_string())?;
    g.insert(key);
    Ok(())
}
