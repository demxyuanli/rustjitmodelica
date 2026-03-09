// SQLite-backed persistent index for source code symbols, chunks, and dependencies.

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

fn index_db_path(project_dir: &str) -> Result<PathBuf, String> {
    let dir = Path::new(project_dir).join(".modai-ide-data");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("index.db"))
}

pub fn open_connection(project_dir: &str) -> Result<Connection, String> {
    let path = index_db_path(project_dir)?;
    let conn = Connection::open(&path).map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
        .map_err(|e| e.to_string())?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS indexed_files (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            path TEXT NOT NULL UNIQUE,
            content_hash TEXT NOT NULL,
            mtime_ms INTEGER NOT NULL,
            language TEXT NOT NULL,
            size_bytes INTEGER NOT NULL,
            indexed_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS symbols (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id INTEGER NOT NULL REFERENCES indexed_files(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            kind TEXT NOT NULL,
            line_start INTEGER NOT NULL,
            line_end INTEGER NOT NULL,
            parent_symbol_id INTEGER,
            signature TEXT,
            doc_comment TEXT,
            UNIQUE(file_id, name, kind, line_start)
        );

        CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
        CREATE INDEX IF NOT EXISTS idx_symbols_kind ON symbols(kind);
        CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_id);

        CREATE TABLE IF NOT EXISTS chunks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id INTEGER NOT NULL REFERENCES indexed_files(id) ON DELETE CASCADE,
            line_start INTEGER NOT NULL,
            line_end INTEGER NOT NULL,
            content TEXT NOT NULL,
            context_label TEXT,
            content_hash TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_chunks_file ON chunks(file_id);

        CREATE TABLE IF NOT EXISTS dependencies (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            from_file_id INTEGER NOT NULL REFERENCES indexed_files(id) ON DELETE CASCADE,
            to_name TEXT NOT NULL,
            kind TEXT NOT NULL,
            line INTEGER
        );

        CREATE INDEX IF NOT EXISTS idx_deps_to ON dependencies(to_name);
        CREATE INDEX IF NOT EXISTS idx_deps_from ON dependencies(from_file_id);",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedFile {
    pub id: i64,
    pub path: String,
    pub content_hash: String,
    pub mtime_ms: i64,
    pub language: String,
    pub size_bytes: i64,
    pub indexed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolInfo {
    pub id: i64,
    pub file_id: i64,
    pub name: String,
    pub kind: String,
    pub line_start: i64,
    pub line_end: i64,
    pub parent_symbol_id: Option<i64>,
    pub signature: Option<String>,
    pub doc_comment: Option<String>,
    #[serde(default)]
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkInfo {
    pub id: i64,
    pub file_id: i64,
    pub line_start: i64,
    pub line_end: i64,
    pub content: String,
    pub context_label: Option<String>,
    pub content_hash: String,
    #[serde(default)]
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyInfo {
    pub id: i64,
    pub from_file_id: i64,
    pub to_name: String,
    pub kind: String,
    pub line: Option<i64>,
    #[serde(default)]
    pub from_file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStats {
    pub file_count: i64,
    pub symbol_count: i64,
    pub chunk_count: i64,
    pub dependency_count: i64,
}

// ---------------------------------------------------------------------------
// File CRUD
// ---------------------------------------------------------------------------

pub fn upsert_file(
    conn: &Connection,
    path: &str,
    content_hash: &str,
    mtime_ms: i64,
    language: &str,
    size_bytes: i64,
) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO indexed_files (path, content_hash, mtime_ms, language, size_bytes, indexed_at)
         VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
         ON CONFLICT(path) DO UPDATE SET
            content_hash = excluded.content_hash,
            mtime_ms = excluded.mtime_ms,
            language = excluded.language,
            size_bytes = excluded.size_bytes,
            indexed_at = excluded.indexed_at",
        params![path, content_hash, mtime_ms, language, size_bytes],
    )
    .map_err(|e| e.to_string())?;

    let file_id: i64 = conn
        .query_row(
            "SELECT id FROM indexed_files WHERE path = ?1",
            params![path],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;
    Ok(file_id)
}

pub fn get_file_by_path(conn: &Connection, path: &str) -> Result<Option<IndexedFile>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, path, content_hash, mtime_ms, language, size_bytes, indexed_at
             FROM indexed_files WHERE path = ?1",
        )
        .map_err(|e| e.to_string())?;
    let mut rows = stmt
        .query_map(params![path], |row| {
            Ok(IndexedFile {
                id: row.get(0)?,
                path: row.get(1)?,
                content_hash: row.get(2)?,
                mtime_ms: row.get(3)?,
                language: row.get(4)?,
                size_bytes: row.get(5)?,
                indexed_at: row.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    match rows.next() {
        Some(Ok(f)) => Ok(Some(f)),
        Some(Err(e)) => Err(e.to_string()),
        None => Ok(None),
    }
}

pub fn delete_file(conn: &Connection, file_id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM symbols WHERE file_id = ?1", params![file_id])
        .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM chunks WHERE file_id = ?1", params![file_id])
        .map_err(|e| e.to_string())?;
    conn.execute(
        "DELETE FROM dependencies WHERE from_file_id = ?1",
        params![file_id],
    )
    .map_err(|e| e.to_string())?;
    conn.execute("DELETE FROM indexed_files WHERE id = ?1", params![file_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn delete_file_by_path(conn: &Connection, path: &str) -> Result<(), String> {
    if let Some(f) = get_file_by_path(conn, path)? {
        delete_file(conn, f.id)?;
    }
    Ok(())
}

pub fn list_indexed_paths(conn: &Connection) -> Result<Vec<(i64, String, String)>, String> {
    let mut stmt = conn
        .prepare("SELECT id, path, content_hash FROM indexed_files")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Symbol CRUD
// ---------------------------------------------------------------------------

pub fn clear_file_symbols(conn: &Connection, file_id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM symbols WHERE file_id = ?1", params![file_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn insert_symbol(
    conn: &Connection,
    file_id: i64,
    name: &str,
    kind: &str,
    line_start: i64,
    line_end: i64,
    parent_symbol_id: Option<i64>,
    signature: Option<&str>,
    doc_comment: Option<&str>,
) -> Result<i64, String> {
    conn.execute(
        "INSERT OR IGNORE INTO symbols (file_id, name, kind, line_start, line_end, parent_symbol_id, signature, doc_comment)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            file_id,
            name,
            kind,
            line_start,
            line_end,
            parent_symbol_id,
            signature,
            doc_comment
        ],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn get_file_symbols(conn: &Connection, file_id: i64) -> Result<Vec<SymbolInfo>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT s.id, s.file_id, s.name, s.kind, s.line_start, s.line_end,
                    s.parent_symbol_id, s.signature, s.doc_comment, f.path
             FROM symbols s JOIN indexed_files f ON s.file_id = f.id
             WHERE s.file_id = ?1 ORDER BY s.line_start ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![file_id], map_symbol_row)
        .map_err(|e| e.to_string())?;
    collect_rows(rows)
}

pub fn search_symbols(
    conn: &Connection,
    query: &str,
    kind: Option<&str>,
    limit: i64,
) -> Result<Vec<SymbolInfo>, String> {
    let pattern = format!("%{}%", query);
    let sql = if kind.is_some() {
        "SELECT s.id, s.file_id, s.name, s.kind, s.line_start, s.line_end,
                s.parent_symbol_id, s.signature, s.doc_comment, f.path
         FROM symbols s JOIN indexed_files f ON s.file_id = f.id
         WHERE s.name LIKE ?1 AND s.kind = ?2
         ORDER BY s.name ASC LIMIT ?3"
    } else {
        "SELECT s.id, s.file_id, s.name, s.kind, s.line_start, s.line_end,
                s.parent_symbol_id, s.signature, s.doc_comment, f.path
         FROM symbols s JOIN indexed_files f ON s.file_id = f.id
         WHERE s.name LIKE ?1
         ORDER BY s.name ASC LIMIT ?3"
    };

    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = if let Some(k) = kind {
        stmt.query_map(params![pattern, k, limit], map_symbol_row)
            .map_err(|e| e.to_string())?
    } else {
        stmt.query_map(params![pattern, "", limit], map_symbol_row)
            .map_err(|e| e.to_string())?
    };
    collect_rows(rows)
}

pub fn find_references(
    conn: &Connection,
    symbol_name: &str,
) -> Result<Vec<DependencyInfo>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT d.id, d.from_file_id, d.to_name, d.kind, d.line, f.path
             FROM dependencies d JOIN indexed_files f ON d.from_file_id = f.id
             WHERE d.to_name = ?1
             ORDER BY f.path ASC, d.line ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![symbol_name], |row| {
            Ok(DependencyInfo {
                id: row.get(0)?,
                from_file_id: row.get(1)?,
                to_name: row.get(2)?,
                kind: row.get(3)?,
                line: row.get(4)?,
                from_file_path: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    collect_rows(rows)
}

// ---------------------------------------------------------------------------
// Chunk CRUD
// ---------------------------------------------------------------------------

pub fn clear_file_chunks(conn: &Connection, file_id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM chunks WHERE file_id = ?1", params![file_id])
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn insert_chunk(
    conn: &Connection,
    file_id: i64,
    line_start: i64,
    line_end: i64,
    content: &str,
    context_label: Option<&str>,
    content_hash: &str,
) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO chunks (file_id, line_start, line_end, content, context_label, content_hash)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![file_id, line_start, line_end, content, context_label, content_hash],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn search_chunks(
    conn: &Connection,
    query: &str,
    max_chunks: i64,
) -> Result<Vec<ChunkInfo>, String> {
    let pattern = format!("%{}%", query);
    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.file_id, c.line_start, c.line_end, c.content,
                    c.context_label, c.content_hash, f.path
             FROM chunks c JOIN indexed_files f ON c.file_id = f.id
             WHERE c.content LIKE ?1 OR c.context_label LIKE ?1
             ORDER BY c.id ASC LIMIT ?2",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![pattern, max_chunks], map_chunk_row)
        .map_err(|e| e.to_string())?;
    collect_rows(rows)
}

// ---------------------------------------------------------------------------
// Dependency CRUD
// ---------------------------------------------------------------------------

pub fn clear_file_dependencies(conn: &Connection, file_id: i64) -> Result<(), String> {
    conn.execute(
        "DELETE FROM dependencies WHERE from_file_id = ?1",
        params![file_id],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn insert_dependency(
    conn: &Connection,
    from_file_id: i64,
    to_name: &str,
    kind: &str,
    line: Option<i64>,
) -> Result<i64, String> {
    conn.execute(
        "INSERT INTO dependencies (from_file_id, to_name, kind, line)
         VALUES (?1, ?2, ?3, ?4)",
        params![from_file_id, to_name, kind, line],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.last_insert_rowid())
}

pub fn get_file_dependencies(
    conn: &Connection,
    file_id: i64,
) -> Result<Vec<DependencyInfo>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT d.id, d.from_file_id, d.to_name, d.kind, d.line, f.path
             FROM dependencies d JOIN indexed_files f ON d.from_file_id = f.id
             WHERE d.from_file_id = ?1
             ORDER BY d.line ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![file_id], |row| {
            Ok(DependencyInfo {
                id: row.get(0)?,
                from_file_id: row.get(1)?,
                to_name: row.get(2)?,
                kind: row.get(3)?,
                line: row.get(4)?,
                from_file_path: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    collect_rows(rows)
}

// ---------------------------------------------------------------------------
// Stats
// ---------------------------------------------------------------------------

pub fn clear_all(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "DELETE FROM dependencies; DELETE FROM chunks; DELETE FROM symbols; DELETE FROM indexed_files;",
    )
    .map_err(|e| e.to_string())
}

pub fn get_stats(conn: &Connection) -> Result<IndexStats, String> {
    let file_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM indexed_files", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    let symbol_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    let chunk_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM chunks", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    let dependency_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM dependencies", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    Ok(IndexStats {
        file_count,
        symbol_count,
        chunk_count,
        dependency_count,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn map_symbol_row(row: &rusqlite::Row) -> rusqlite::Result<SymbolInfo> {
    Ok(SymbolInfo {
        id: row.get(0)?,
        file_id: row.get(1)?,
        name: row.get(2)?,
        kind: row.get(3)?,
        line_start: row.get(4)?,
        line_end: row.get(5)?,
        parent_symbol_id: row.get(6)?,
        signature: row.get(7)?,
        doc_comment: row.get(8)?,
        file_path: row.get(9)?,
    })
}

fn map_chunk_row(row: &rusqlite::Row) -> rusqlite::Result<ChunkInfo> {
    Ok(ChunkInfo {
        id: row.get(0)?,
        file_id: row.get(1)?,
        line_start: row.get(2)?,
        line_end: row.get(3)?,
        content: row.get(4)?,
        context_label: row.get(5)?,
        content_hash: row.get(6)?,
        file_path: row.get(7)?,
    })
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row) -> rusqlite::Result<T>>,
) -> Result<Vec<T>, String> {
    let mut out = Vec::new();
    for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
}
