use rusqlite::{params, Connection, OpenFlags, TransactionBehavior};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use crate::cache::cache_scope::CacheScope;

#[derive(Debug, Clone)]
pub struct SqliteCacheConfig {
    pub path: PathBuf,
}

fn parse_bool_env_default_true(name: &str) -> bool {
    match std::env::var(name) {
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

pub fn sqlite_config(cache_dir: Option<&Path>) -> Option<SqliteCacheConfig> {
    if !parse_bool_env_default_true("RUSTMODLICA_CACHE_SQLITE") {
        return None;
    }
    let dir = cache_dir?;
    let path = dir.join("rustmodlica-cache.sqlite");
    Some(SqliteCacheConfig {
        path,
    })
}

pub fn sqlite_config_for_scope(scope: CacheScope, cache_dir: Option<&Path>) -> Option<SqliteCacheConfig> {
    if !parse_bool_env_default_true("RUSTMODLICA_CACHE_SQLITE") {
        return None;
    }
    let dir = cache_dir?;
    let scoped = scope.resolve_dir(dir);
    let path = scoped.join(scope.sqlite_db_name());
    Some(SqliteCacheConfig { path })
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

fn table_exists(conn: &Connection, name: &str) -> Result<bool, rusqlite::Error> {
    let n: i64 = conn.query_row(
        "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name=?1",
        params![name],
        |r| r.get(0),
    )?;
    Ok(n > 0)
}

fn column_exists(conn: &Connection, table: &str, col: &str) -> Result<bool, rusqlite::Error> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({})", table))?;
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        let name: String = row.get(1)?;
        if name == col {
            return Ok(true);
        }
    }
    Ok(false)
}

fn migrate_cache_kind_stats_v1_to_v2(conn: &mut Connection) -> Result<(), rusqlite::Error> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute_batch(
        r#"
        ALTER TABLE cache_kind_stats RENAME TO cache_kind_stats_old;
        CREATE TABLE cache_kind_stats (
            scope TEXT NOT NULL,
            kind TEXT NOT NULL,
            get_count INTEGER NOT NULL,
            hit_count INTEGER NOT NULL,
            put_count INTEGER NOT NULL,
            bytes_put INTEGER NOT NULL,
            last_get_ms INTEGER NOT NULL,
            last_put_ms INTEGER NOT NULL,
            PRIMARY KEY (scope, kind)
        );
        INSERT INTO cache_kind_stats (scope, kind, get_count, hit_count, put_count, bytes_put, last_get_ms, last_put_ms)
        SELECT 'legacy', kind, get_count, hit_count, put_count, bytes_put, last_get_ms, last_put_ms
        FROM cache_kind_stats_old;
        DROP TABLE cache_kind_stats_old;
        "#,
    )?;
    tx.commit()?;
    Ok(())
}

fn ensure_cache_entries(conn: &mut Connection, db_path: &Path) -> Result<(), rusqlite::Error> {
    if !table_exists(conn, "cache_entries")? {
        conn.execute_batch(
            r#"
            CREATE TABLE cache_entries (
                key TEXT PRIMARY KEY,
                scope TEXT NOT NULL,
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
        return Ok(());
    }
    if !column_exists(conn, "cache_entries", "scope")? {
        conn.execute(
            "ALTER TABLE cache_entries ADD COLUMN scope TEXT NOT NULL DEFAULT 'legacy'",
            [],
        )?;
        let tag = scope_tag_for_db_path(db_path);
        conn.execute("UPDATE cache_entries SET scope = ?1", params![tag])?;
    }
    Ok(())
}

fn ensure_cache_kind_stats(conn: &mut Connection) -> Result<(), rusqlite::Error> {
    if !table_exists(conn, "cache_kind_stats")? {
        conn.execute_batch(
            r#"
            CREATE TABLE cache_kind_stats (
                scope TEXT NOT NULL,
                kind TEXT NOT NULL,
                get_count INTEGER NOT NULL,
                hit_count INTEGER NOT NULL,
                put_count INTEGER NOT NULL,
                bytes_put INTEGER NOT NULL,
                last_get_ms INTEGER NOT NULL,
                last_put_ms INTEGER NOT NULL,
                PRIMARY KEY (scope, kind)
            );
            "#,
        )?;
        return Ok(());
    }
    if !column_exists(conn, "cache_kind_stats", "scope")? {
        migrate_cache_kind_stats_v1_to_v2(conn)?;
    }
    Ok(())
}

fn init_schema(conn: &mut Connection, db_path: &Path) -> Result<(), rusqlite::Error> {
    ensure_cache_entries(conn, db_path)?;
    ensure_cache_kind_stats(conn)?;
    Ok(())
}

pub(crate) fn scope_tag_for_db_path(path: &Path) -> &'static str {
    match path.file_name().and_then(|n| n.to_str()) {
        Some("cache-std.sqlite") => "L0",
        Some("cache-user.sqlite") => "L1",
        Some("cache-project.sqlite") => "L2",
        Some("rustmodlica-cache.sqlite") => "legacy",
        Some(_) => "other",
        None => "legacy",
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn connection_pool() -> &'static Mutex<HashMap<PathBuf, Arc<Mutex<Connection>>>> {
    static CONNS: OnceLock<Mutex<HashMap<PathBuf, Arc<Mutex<Connection>>>>> = OnceLock::new();
    CONNS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Drop all pooled [`Connection`] handles (e.g. after deleting SQLite files on disk).
pub fn sqlite_connection_pool_clear() {
    if let Ok(mut g) = connection_pool().lock() {
        g.clear();
    }
}

fn global_conn(path: &Path) -> Result<Arc<Mutex<Connection>>, rusqlite::Error> {
    let key = path.to_path_buf();
    let map = connection_pool();
    let mut g = map.lock().unwrap();
    if let Some(c) = g.get(&key) {
        return Ok(Arc::clone(c));
    }
    let mut conn = conn_for_path(path)?;
    init_schema(&mut conn, path)?;
    let arc = Arc::new(Mutex::new(conn));
    g.insert(key, Arc::clone(&arc));
    Ok(arc)
}

pub fn sqlite_get(path: &Path, key: &str, kind_hint: &str) -> Result<Option<Vec<u8>>, rusqlite::Error> {
    let conn = global_conn(path)?;
    let guard = conn.lock().unwrap();
    let scope = scope_tag_for_db_path(path);
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
            INSERT INTO cache_kind_stats(scope, kind, get_count, hit_count, put_count, bytes_put, last_get_ms, last_put_ms)
            VALUES(?1, ?2, 1, 1, 0, 0, ?3, 0)
            ON CONFLICT(scope, kind) DO UPDATE SET
                get_count = get_count + 1,
                hit_count = hit_count + 1,
                last_get_ms = excluded.last_get_ms
            "#,
            params![scope, kind, now],
        );
        Ok(Some(blob))
    } else {
        let now = now_ms();
        let _ = guard.execute(
            r#"
            INSERT INTO cache_kind_stats(scope, kind, get_count, hit_count, put_count, bytes_put, last_get_ms, last_put_ms)
            VALUES(?1, ?2, 1, 0, 0, 0, ?3, 0)
            ON CONFLICT(scope, kind) DO UPDATE SET
                get_count = get_count + 1,
                last_get_ms = excluded.last_get_ms
            "#,
            params![scope, kind_hint, now],
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
    sqlite_put_atomic(path, key, schema, kind, blob, deps_json)
}

pub struct SqliteCachePut<'a> {
    pub key: &'a str,
    pub schema: &'a str,
    pub kind: &'a str,
    pub blob: &'a [u8],
    pub deps_json: Option<&'a str>,
}

pub fn sqlite_put_atomic(
    path: &Path,
    key: &str,
    schema: &str,
    kind: &str,
    blob: &[u8],
    deps_json: Option<&str>,
) -> Result<(), rusqlite::Error> {
    sqlite_put_batch(
        path,
        &[SqliteCachePut {
            key,
            schema,
            kind,
            blob,
            deps_json,
        }],
    )
}

/// Single `BEGIN IMMEDIATE` transaction for multiple cache rows (same DB file).
pub fn sqlite_put_batch(path: &Path, puts: &[SqliteCachePut<'_>]) -> Result<(), rusqlite::Error> {
    if puts.is_empty() {
        return Ok(());
    }
    let conn = global_conn(path)?;
    let mut guard = conn.lock().unwrap();
    let scope = scope_tag_for_db_path(path);
    let now = now_ms();
    let tx = guard.transaction_with_behavior(TransactionBehavior::Immediate)?;
    for p in puts {
        tx.execute(
            r#"
            INSERT INTO cache_entries(key, scope, schema, kind, blob, deps_json, created_ms, last_hit_ms, hit_count)
            VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, 0)
            ON CONFLICT(key) DO UPDATE SET
                scope=excluded.scope,
                schema=excluded.schema,
                kind=excluded.kind,
                blob=excluded.blob,
                deps_json=excluded.deps_json
            "#,
            params![p.key, scope, p.schema, p.kind, p.blob, p.deps_json, now],
        )?;
        let _ = tx.execute(
            r#"
            INSERT INTO cache_kind_stats(scope, kind, get_count, hit_count, put_count, bytes_put, last_get_ms, last_put_ms)
            VALUES(?1, ?2, 0, 0, 1, ?3, 0, ?4)
            ON CONFLICT(scope, kind) DO UPDATE SET
                put_count = put_count + 1,
                bytes_put = bytes_put + excluded.bytes_put,
                last_put_ms = excluded.last_put_ms
            "#,
            params![scope, p.kind, p.blob.len() as i64, now],
        );
    }
    tx.commit()?;
    Ok(())
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheKindStatRow {
    pub scope: String,
    pub kind: String,
    pub get_count: i64,
    pub hit_count: i64,
    pub put_count: i64,
    pub bytes_put: i64,
    pub last_get_ms: i64,
    pub last_put_ms: i64,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct CacheStatsLayerExport {
    pub tier: String,
    pub db_path: String,
    pub rows: Vec<CacheKindStatRow>,
}

/// Per-tier `cache_kind_stats` when the DB file already exists (does not create empty DBs).
pub fn export_sqlite_kind_stats_layers(cache_dir: &Path) -> Vec<CacheStatsLayerExport> {
    if !parse_bool_env_default_true("RUSTMODLICA_CACHE_SQLITE") {
        return Vec::new();
    }
    let mut out = Vec::new();
    for scope in [
        CacheScope::GlobalStd,
        CacheScope::UserExt,
        CacheScope::Project,
    ] {
        if let Some(cfg) = sqlite_config_for_scope(scope.clone(), Some(cache_dir)) {
            if cfg.path.is_file() {
                if let Ok(rows) = sqlite_kind_stats(&cfg.path) {
                    out.push(CacheStatsLayerExport {
                        tier: scope.prefix().to_string(),
                        db_path: cfg.path.display().to_string(),
                        rows,
                    });
                }
            }
        }
    }
    if let Some(cfg) = sqlite_config(Some(cache_dir)) {
        if cfg.path.is_file() {
            if let Ok(rows) = sqlite_kind_stats(&cfg.path) {
                out.push(CacheStatsLayerExport {
                    tier: "legacy".to_string(),
                    db_path: cfg.path.display().to_string(),
                    rows,
                });
            }
        }
    }
    out
}

pub fn sqlite_kind_stats(path: &Path) -> Result<Vec<CacheKindStatRow>, rusqlite::Error> {
    let conn = global_conn(path)?;
    let guard = conn.lock().unwrap();
    let mut stmt = guard.prepare(
        r#"
        SELECT scope, kind, get_count, hit_count, put_count, bytes_put, last_get_ms, last_put_ms
        FROM cache_kind_stats
        ORDER BY scope ASC, kind ASC
        "#,
    )?;
    let mut rows = stmt.query([])?;
    let mut out = Vec::new();
    while let Some(row) = rows.next()? {
        out.push(CacheKindStatRow {
            scope: row.get(0)?,
            kind: row.get(1)?,
            get_count: row.get(2)?,
            hit_count: row.get(3)?,
            put_count: row.get(4)?,
            bytes_put: row.get(5)?,
            last_get_ms: row.get(6)?,
            last_put_ms: row.get(7)?,
        });
    }
    Ok(out)
}

/// Wipes all rows in this SQLite file (one file per tier in layered mode).
pub fn sqlite_invalidate_scope(path: &Path) -> Result<(), rusqlite::Error> {
    let conn = global_conn(path)?;
    let mut guard = conn.lock().unwrap();
    let tx = guard.transaction_with_behavior(TransactionBehavior::Immediate)?;
    tx.execute("DELETE FROM cache_entries", [])?;
    let _ = tx.execute("DELETE FROM cache_kind_stats", []);
    tx.commit()?;
    Ok(())
}
