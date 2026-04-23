//! Cross-process path -> content hash index (mtime + len fast path).
//! One `path_hash_index.sqlite` per cache root (project / user / std).

use crate::cache::cache_scope::{classify_model_scope, CacheScope};
use rusqlite::{params, Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const DB_NAME: &str = "path_hash_index.sqlite";

fn db_path(cache_root: &Path) -> PathBuf {
    cache_root.join(DB_NAME)
}

fn mtime_ms(t: SystemTime) -> i64 {
    t.duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn open(cache_root: &Path) -> Result<Connection, rusqlite::Error> {
    let _ = std::fs::create_dir_all(cache_root);
    let p = db_path(cache_root);
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE;
    let c = Connection::open_with_flags(&p, flags)?;
    let ver: i64 = c
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if ver < 1 {
        let _ = c.execute_batch("DROP TABLE IF EXISTS path_hashes;");
        let _ = c.pragma_update(None, "user_version", 1);
    }
    c.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS path_hashes (
            path TEXT PRIMARY KEY,
            mtime_ms INTEGER NOT NULL,
            len INTEGER NOT NULL,
            hash TEXT NOT NULL
        );
        "#,
    )?;
    Ok(c)
}

fn lookup_in_root(
    root: &Path,
    path_s: &str,
    m_ms: i64,
    len: u64,
) -> Option<String> {
    let conn = open(root).ok()?;
    let row: Result<(i64, i64, String), rusqlite::Error> = conn.query_row(
        "SELECT mtime_ms, len, hash FROM path_hashes WHERE path = ?1",
        params![path_s],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    );
    let (stored_ms, stored_len, h) = row.ok()?;
    if stored_ms == m_ms && stored_len == len as i64 {
        Some(h)
    } else {
        None
    }
}

fn path_hash_store_root(path: &Path) -> Option<PathBuf> {
    match classify_model_scope(path) {
        CacheScope::GlobalStd => {
            crate::flatten::std_cache_root().or_else(crate::flatten::flatten_cache_dir)
        }
        CacheScope::UserExt => {
            crate::flatten::user_cache_root().or_else(crate::flatten::flatten_cache_dir)
        }
        CacheScope::Project => crate::flatten::flatten_cache_dir(),
    }
}

/// Return cached hash if path mtime+len match index in any configured cache root.
pub fn lookup(
    _cache_root: Option<&Path>,
    path: &Path,
    modified: Option<SystemTime>,
    len: u64,
) -> Option<String> {
    let m_ms = modified.map(mtime_ms)?;
    let path_s = path.to_string_lossy().to_string();
    let mut roots = crate::flatten::flatten_cache::all_disk_cache_roots();
    if let Some(h) = _cache_root {
        let hb = h.to_path_buf();
        if !roots.iter().any(|r| r == &hb) {
            roots.insert(0, hb);
        }
    }
    for root in roots {
        if let Some(h) = lookup_in_root(root.as_path(), path_s.as_str(), m_ms, len) {
            return Some(h);
        }
    }
    None
}

pub fn store(_cache_root: Option<&Path>, path: &Path, modified: Option<SystemTime>, len: u64, hash: &str) {
    let Some(mt) = modified else {
        return;
    };
    let m_ms = mtime_ms(mt);
    let path_s = path.to_string_lossy().to_string();
    let Some(root) = path_hash_store_root(path)
        .or_else(|| _cache_root.map(|p| p.to_path_buf()))
        .or_else(crate::flatten::flatten_cache_dir)
    else {
        return;
    };
    if let Ok(conn) = open(root.as_path()) {
        let _ = conn.execute(
            r#"
            INSERT INTO path_hashes (path, mtime_ms, len, hash)
            VALUES (?1, ?2, ?3, ?4)
            ON CONFLICT(path) DO UPDATE SET
                mtime_ms=excluded.mtime_ms,
                len=excluded.len,
                hash=excluded.hash
            "#,
            params![path_s, m_ms, len as i64, hash],
        );
    }
}
