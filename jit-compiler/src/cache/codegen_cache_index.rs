//! SQLite index of JIT codegen disk files for faster global budget scans.

use rusqlite::{params, Connection, OpenFlags};
use std::path::{Path, PathBuf};

const DB_NAME: &str = "codegen_path_index.sqlite";

fn db_path(jit_codegen_root: &Path) -> PathBuf {
    jit_codegen_root.join(DB_NAME)
}

fn open(jit_codegen_root: &Path) -> Result<Connection, rusqlite::Error> {
    let p = db_path(jit_codegen_root);
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE;
    let c = Connection::open_with_flags(&p, flags)?;
    let ver: i64 = c
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .unwrap_or(0);
    if ver < 2 {
        let _ = c.execute_batch("DROP TABLE IF EXISTS codegen_files;");
        let _ = c.pragma_update(None, "user_version", 2);
    }
    c.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS codegen_files (
            stable_hash TEXT PRIMARY KEY,
            bin_path TEXT NOT NULL,
            json_path TEXT NOT NULL,
            bin_size INTEGER NOT NULL,
            json_size INTEGER NOT NULL,
            mtime_ms INTEGER NOT NULL
        );
        "#,
    )?;
    Ok(c)
}

/// Record or update index rows after writing `.bin` + `.json` for a stable hash.
pub fn record_codegen_pair(jit_codegen_root: &Path, stable_hash: &str) {
    let bin_path = jit_codegen_root.join(format!("{}.bin", stable_hash));
    let json_path = jit_codegen_root.join(format!("{}.json", stable_hash));
    let (bin_size, bin_mtime) = file_meta(&bin_path);
    let (json_size, json_mtime) = file_meta(&json_path);
    let mtime_ms = bin_mtime.max(json_mtime);
    if let Ok(conn) = open(jit_codegen_root) {
        let _ = conn.execute(
            r#"
            INSERT INTO codegen_files (stable_hash, bin_path, json_path, bin_size, json_size, mtime_ms)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(stable_hash) DO UPDATE SET
                bin_path=excluded.bin_path,
                json_path=excluded.json_path,
                bin_size=excluded.bin_size,
                json_size=excluded.json_size,
                mtime_ms=excluded.mtime_ms
            "#,
            params![
                stable_hash,
                bin_path.to_string_lossy().as_ref(),
                json_path.to_string_lossy().as_ref(),
                bin_size as i64,
                json_size as i64,
                mtime_ms,
            ],
        );
    }
}

fn file_meta(p: &Path) -> (u64, i64) {
    let Ok(md) = std::fs::metadata(p) else {
        return (0, 0);
    };
    let len = md.len();
    let mtime_ms = md
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    (len, mtime_ms)
}

/// List indexed files for budget enforcement (mtime ascending compatible with global_budget).
pub fn list_indexed_files(jit_codegen_root: &Path) -> Option<Vec<(PathBuf, u64, i64)>> {
    let conn = open(jit_codegen_root).ok()?;
    let mut stmt = conn
        .prepare(
            "SELECT bin_path, bin_size, mtime_ms FROM codegen_files UNION ALL SELECT json_path, json_size, mtime_ms FROM codegen_files",
        )
        .ok()?;
    let mut out = Vec::new();
    let mut rows = stmt.query([]).ok()?;
    while let Some(row) = rows.next().ok()? {
        let path: String = row.get(0).ok()?;
        let size: i64 = row.get(1).ok()?;
        let mtime: i64 = row.get(2).ok()?;
        out.push((PathBuf::from(path), size.max(0) as u64, mtime));
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}
