use rusqlite::{params, Connection, OpenFlags};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone)]
pub struct SqliteCacheConfig {
    pub path: PathBuf,
}

fn parse_bool_env(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

pub fn sqlite_config(cache_dir: Option<&Path>) -> Option<SqliteCacheConfig> {
    if !parse_bool_env("RUSTMODLICA_CACHE_SQLITE") {
        return None;
    }
    let dir = cache_dir?;
    let path = dir.join("rustmodlica-cache.sqlite");
    Some(SqliteCacheConfig {
        path,
    })
}

fn conn_for_path(path: &Path) -> Result<Connection, rusqlite::Error> {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE;
    let conn = Connection::open_with_flags(path, flags)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    Ok(conn)
}

fn init_schema(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS cache_entries (
            key TEXT PRIMARY KEY,
            schema TEXT NOT NULL,
            kind TEXT NOT NULL,
            blob BLOB NOT NULL,
            deps_json TEXT,
            created_ms INTEGER NOT NULL,
            last_hit_ms INTEGER NOT NULL,
            hit_count INTEGER NOT NULL
        );
        "#,
    )?;
    Ok(())
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn global_conn(path: &Path) -> Result<&'static Mutex<Connection>, rusqlite::Error> {
    static CONN: OnceLock<Mutex<Connection>> = OnceLock::new();
    if let Some(c) = CONN.get() {
        return Ok(c);
    }
    let conn = conn_for_path(path)?;
    init_schema(&conn)?;
    Ok(CONN.get_or_init(|| Mutex::new(conn)))
}

pub fn sqlite_get(path: &Path, key: &str) -> Result<Option<Vec<u8>>, rusqlite::Error> {
    let conn = global_conn(path)?;
    let guard = conn.lock().unwrap();
    let mut stmt = guard.prepare("SELECT blob FROM cache_entries WHERE key = ?1")?;
    let mut rows = stmt.query(params![key])?;
    if let Some(row) = rows.next()? {
        let blob: Vec<u8> = row.get(0)?;
        let now = now_ms();
        let _ = guard.execute(
            "UPDATE cache_entries SET last_hit_ms = ?2, hit_count = hit_count + 1 WHERE key = ?1",
            params![key, now],
        );
        Ok(Some(blob))
    } else {
        Ok(None)
    }
}

pub fn sqlite_put(
    path: &Path,
    key: &str,
    schema: &str,
    kind: &str,
    blob: &[u8],
    deps_json: Option<&str>,
) -> Result<(), rusqlite::Error> {
    let conn = global_conn(path)?;
    let guard = conn.lock().unwrap();
    let now = now_ms();
    guard.execute(
        r#"
        INSERT INTO cache_entries(key, schema, kind, blob, deps_json, created_ms, last_hit_ms, hit_count)
        VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?6, 0)
        ON CONFLICT(key) DO UPDATE SET
            schema=excluded.schema,
            kind=excluded.kind,
            blob=excluded.blob,
            deps_json=excluded.deps_json
        "#,
        params![key, schema, kind, blob, deps_json, now],
    )?;
    Ok(())
}

