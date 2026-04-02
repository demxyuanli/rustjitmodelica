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

        CREATE TABLE IF NOT EXISTS cache_kind_stats (
            kind TEXT PRIMARY KEY,
            get_count INTEGER NOT NULL,
            hit_count INTEGER NOT NULL,
            put_count INTEGER NOT NULL,
            bytes_put INTEGER NOT NULL,
            last_get_ms INTEGER NOT NULL,
            last_put_ms INTEGER NOT NULL
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

pub fn sqlite_get(path: &Path, key: &str, kind_hint: &str) -> Result<Option<Vec<u8>>, rusqlite::Error> {
    let conn = global_conn(path)?;
    let guard = conn.lock().unwrap();
    let mut stmt = guard.prepare("SELECT blob, kind FROM cache_entries WHERE key = ?1")?;
    let mut rows = stmt.query(params![key])?;
    if let Some(row) = rows.next()? {
        let blob: Vec<u8> = row.get(0)?;
        let kind: String = row.get(1)?;
        let now = now_ms();
        let _ = guard.execute(
            "UPDATE cache_entries SET last_hit_ms = ?2, hit_count = hit_count + 1 WHERE key = ?1",
            params![key, now],
        );
        let _ = guard.execute(
            r#"
            INSERT INTO cache_kind_stats(kind, get_count, hit_count, put_count, bytes_put, last_get_ms, last_put_ms)
            VALUES(?1, 1, 1, 0, 0, ?2, 0)
            ON CONFLICT(kind) DO UPDATE SET
                get_count = get_count + 1,
                hit_count = hit_count + 1,
                last_get_ms = excluded.last_get_ms
            "#,
            params![kind, now],
        );
        Ok(Some(blob))
    } else {
        // Miss: still count the get against the caller's kind hint.
        let now = now_ms();
        let _ = guard.execute(
            r#"
            INSERT INTO cache_kind_stats(kind, get_count, hit_count, put_count, bytes_put, last_get_ms, last_put_ms)
            VALUES(?1, 1, 0, 0, 0, ?2, 0)
            ON CONFLICT(kind) DO UPDATE SET
                get_count = get_count + 1,
                last_get_ms = excluded.last_get_ms
            "#,
            params![kind_hint, now],
        );
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
    let _ = guard.execute(
        r#"
        INSERT INTO cache_kind_stats(kind, get_count, hit_count, put_count, bytes_put, last_get_ms, last_put_ms)
        VALUES(?1, 0, 0, 1, ?2, 0, ?3)
        ON CONFLICT(kind) DO UPDATE SET
            put_count = put_count + 1,
            bytes_put = bytes_put + excluded.bytes_put,
            last_put_ms = excluded.last_put_ms
        "#,
        params![kind, blob.len() as i64, now],
    );
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheKindStatRow {
    pub kind: String,
    pub get_count: i64,
    pub hit_count: i64,
    pub put_count: i64,
    pub bytes_put: i64,
    pub last_get_ms: i64,
    pub last_put_ms: i64,
}

pub fn sqlite_kind_stats(path: &Path) -> Result<Vec<CacheKindStatRow>, rusqlite::Error> {
    let conn = global_conn(path)?;
    let guard = conn.lock().unwrap();
    let mut stmt = guard.prepare(
        r#"
        SELECT kind, get_count, hit_count, put_count, bytes_put, last_get_ms, last_put_ms
        FROM cache_kind_stats
        ORDER BY kind ASC
        "#,
    )?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(CacheKindStatRow {
            kind: row.get(0)?,
            get_count: row.get(1)?,
            hit_count: row.get(2)?,
            put_count: row.get(3)?,
            bytes_put: row.get(4)?,
            last_get_ms: row.get(5)?,
            last_put_ms: row.get(6)?,
        });
    }
    Ok(out)
}

