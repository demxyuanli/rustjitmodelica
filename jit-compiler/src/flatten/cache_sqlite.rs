use rusqlite::{params, Connection, OpenFlags, TransactionBehavior};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use xxhash_rust::xxh3::Xxh3;

use crate::cache::build_id::binary_build_id;
use crate::cache::cache_scope::CacheScope;
use crate::cache::ir_epoch;

/// Parent-hash chain root prefix. Bump when the chain format itself changes (e.g., new
/// composition algorithm). Does not invalidate rows by itself; combined with
/// [`binary_build_id`] and the stage epoch into [`compute_parent_hash`].
pub const PARENT_HASH_CHAIN_VERSION: u8 = 1;

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

fn project_cache_root() -> Option<PathBuf> {
    super::flatten_cache::flatten_cache_dir()
}

fn std_tier_root() -> Option<PathBuf> {
    super::flatten_cache::std_cache_root()
}

fn user_tier_root() -> Option<PathBuf> {
    super::flatten_cache::user_cache_root()
}

fn roots_in_read_priority(primary: CacheScope) -> Vec<Option<PathBuf>> {
    let p = project_cache_root();
    let u = user_tier_root();
    let s = std_tier_root();
    match primary {
        CacheScope::Project => vec![p, u, s],
        CacheScope::UserExt => vec![u, p, s],
        CacheScope::GlobalStd => vec![s, u, p],
    }
}

/// Ordered SQLite files to try for a cache read for queries keyed with `primary` scope.
pub fn sqlite_read_try_configs(primary: CacheScope) -> Vec<SqliteCacheConfig> {
    if !parse_bool_env_default_true("RUSTMODLICA_CACHE_SQLITE") {
        return Vec::new();
    }
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for root_opt in roots_in_read_priority(primary) {
        let Some(root) = root_opt else {
            continue;
        };
        for scope in crate::cache::cache_scope::sqlite_scope_lookup_chain(primary) {
            if let Some(cfg) = sqlite_config_for_scope(scope, Some(root.as_path())) {
                if seen.insert(cfg.path.clone()) {
                    out.push(cfg);
                }
            }
        }
    }
    out
}

/// Authoritative SQLite path for a write for this logical tier (std/user/project roots).
pub fn sqlite_write_config_for_scope(scope: CacheScope) -> Option<SqliteCacheConfig> {
    let root = match scope {
        CacheScope::GlobalStd => std_tier_root().or_else(project_cache_root)?,
        CacheScope::UserExt => user_tier_root().or_else(project_cache_root)?,
        CacheScope::Project => project_cache_root()?,
    };
    sqlite_config_for_scope(scope, Some(root.as_path()))
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
                parent_hash TEXT NOT NULL DEFAULT '',
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
    // parent_hash chain column: pre-existing rows get '' and are treated as missing chain
    // (reads compare against current `compute_parent_hash(...)` and skip on mismatch, so
    // legacy rows are silently invalidated the first time a new binary asks for them).
    if !column_exists(conn, "cache_entries", "parent_hash")? {
        conn.execute(
            "ALTER TABLE cache_entries ADD COLUMN parent_hash TEXT NOT NULL DEFAULT ''",
            [],
        )?;
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

/// Remove one pooled [`Connection`] for `path`. The next `global_conn(path)` call
/// re-opens and re-initializes the database. Use after selective DELETE operations
/// so subsequent reads through the pool see a fresh snapshot.
pub fn sqlite_connection_pool_evict(path: &Path) {
    let key = path.to_path_buf();
    if let Ok(mut g) = connection_pool().lock() {
        g.remove(&key);
    }
}

/// Create a short-lived WAL-mode connection suitable for prune / invalidation
/// operations. Uses WAL mode and initializes schema but does **not** enter the
/// connection pool, avoiding pool-mutex contention during long-running transactions.
pub fn conn_for_prune(path: &Path) -> Result<Connection, rusqlite::Error> {
    let mut conn = conn_for_path(path)?;
    init_schema(&mut conn, path)?;
    Ok(conn)
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

/// Compose the deterministic parent-hash for `(key, kind)`.
///
/// Feeds [`binary_build_id`], the per-stage epoch (if we can recognize the kind), and the
/// caller-provided identifiers through `xxh3_128`. Semantics:
/// * Rewriting the binary -> `binary_build_id` changes -> parent_hash changes ->
///   previously stored rows mismatch and read as `None` (treated as miss; the next write
///   overwrites them). This catches the "IR structure silently drifted across builds" bug
///   without requiring anyone to bump `stage_epochs.txt`.
/// * Bumping a stage epoch in [`crate::cache::ir_epoch::STAGE_EPOCHS`] also flips the
///   parent_hash, so the manual-bump path still works and composes with the auto path.
/// * Scoped to `(key, kind)`: two different rows in the same DB never collide.
pub fn compute_parent_hash(key: &str, kind: &str) -> String {
    let mut h = Xxh3::new();
    h.update(&[PARENT_HASH_CHAIN_VERSION]);
    h.update(b"\x00bid:");
    h.update(binary_build_id().as_bytes());
    h.update(b"\x00kind:");
    h.update(kind.as_bytes());
    h.update(b"\x00se:");
    if let Some(stage) = ir_epoch::CacheStage::from_tag(kind_to_stage_tag(kind)) {
        let epoch = ir_epoch::epoch_for_stage(stage);
        h.update(&epoch.to_le_bytes());
    } else {
        // Kinds that don't map to a known stage (e.g. `sim_bundle_v1`, `analysis_summary_v1`)
        // still pin to the binary build id, which is the main goal.
        h.update(b"no-stage");
    }
    h.update(b"\x00key:");
    h.update(key.as_bytes());
    format!("{:032x}", h.digest128())
}

/// Map a `kind` column value used across the codebase to the closest [`CacheStage`] tag.
/// Kinds without a stage mapping return a sentinel that never round-trips to a `CacheStage`.
fn kind_to_stage_tag(kind: &str) -> &'static str {
    match kind {
        "flat_cache_v1" | "flat_cache_v2" => "flat_full_v2",
        "array_sizes_v2" => "array_sizes_v3",
        "eq_expand_v2" => "eq_expand_v2",
        "constrainedby_v2" => "constrainedby_v2",
        "flat_model_q_v2" => "flat_model_q_v2",
        "inheritance_v2" => "inheritance_v2",
        "model_ast_v2" => "model_ast_v2",
        "parse_v2" => "parse_v2",
        "decl_expand_v2" => "decl_expand_v2",
        _ => "__no_stage__",
    }
}

pub fn sqlite_get(path: &Path, key: &str, kind_hint: &str) -> Result<Option<Vec<u8>>, rusqlite::Error> {
    let expected = compute_parent_hash(key, kind_hint);
    sqlite_get_checked(path, key, kind_hint, &expected)
}

/// Same as [`sqlite_get`], but `expected_parent` is caller-supplied. Pass `""` to disable
/// the parent-hash check (accept any row). Used by migration tooling and by tests that
/// want to inspect rows produced by another binary.
pub fn sqlite_get_checked(
    path: &Path,
    key: &str,
    kind_hint: &str,
    expected_parent: &str,
) -> Result<Option<Vec<u8>>, rusqlite::Error> {
    let conn = global_conn(path)?;
    let guard = conn.lock().unwrap();
    let scope = scope_tag_for_db_path(path);
    let mut stmt = guard.prepare(
        "SELECT blob, kind, parent_hash FROM cache_entries WHERE key = ?1",
    )?;
    let mut rows = stmt.query(params![key])?;
    if let Some(row) = rows.next()? {
        let blob: Vec<u8> = row.get(0)?;
        let kind: String = row.get(1)?;
        let stored_parent: String = row.get(2)?;
        let chain_ok = expected_parent.is_empty() || stored_parent == expected_parent;
        let now = now_ms();
        if !chain_ok {
            // Row is from a different binary / stage epoch: treat as miss. Do not bump
            // hit_count. Track via `cache_kind_stats` as both a get and an
            // invalidation-due-to-chain-mismatch (stored in `put_count` of a synthetic
            // `__chain_miss__` kind so the miss does not inflate the real kind's stats).
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
            let _ = guard.execute(
                r#"
                INSERT INTO cache_kind_stats(scope, kind, get_count, hit_count, put_count, bytes_put, last_get_ms, last_put_ms)
                VALUES(?1, ?2, 0, 0, 1, 0, 0, ?3)
                ON CONFLICT(scope, kind) DO UPDATE SET
                    put_count = put_count + 1,
                    last_put_ms = excluded.last_put_ms
                "#,
                params![scope, "__chain_miss__", now],
            );
            return Ok(None);
        }
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
    /// Optional explicit parent-hash to stamp. Empty/`None` => compute via
    /// [`compute_parent_hash`]. Callers should only override this for test harnesses;
    /// production writes should let the helper tie the row to the current binary.
    pub parent_hash: Option<&'a str>,
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
            parent_hash: None,
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
        let parent_hash_owned: String = match p.parent_hash {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => compute_parent_hash(p.key, p.kind),
        };
        tx.execute(
            r#"
            INSERT INTO cache_entries(key, scope, schema, kind, blob, deps_json, parent_hash, created_ms, last_hit_ms, hit_count)
            VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, 0)
            ON CONFLICT(key) DO UPDATE SET
                scope=excluded.scope,
                schema=excluded.schema,
                kind=excluded.kind,
                blob=excluded.blob,
                deps_json=excluded.deps_json,
                parent_hash=excluded.parent_hash,
                last_hit_ms=excluded.last_hit_ms
            "#,
            params![
                p.key,
                scope,
                p.schema,
                p.kind,
                p.blob,
                p.deps_json,
                parent_hash_owned,
                now
            ],
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
