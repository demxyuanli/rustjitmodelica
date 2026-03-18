// Persistent SQLite index for component library types to avoid full disk scan on every list/query.
// Stored in app_data_root()/component-libraries.db.

use rusqlite::{params, Connection};
use std::collections::HashMap;

fn db_path() -> Result<std::path::PathBuf, String> {
    let root = crate::app_data::app_data_root()?;
    Ok(root.join("component-libraries.db"))
}

fn migrate(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS lib_meta (
            library_id TEXT PRIMARY KEY,
            source_path TEXT NOT NULL,
            display_name TEXT NOT NULL,
            scope TEXT NOT NULL,
            last_scanned_mtime INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS component_types (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            library_id TEXT NOT NULL,
            qualified_name TEXT NOT NULL,
            name TEXT NOT NULL,
            kind TEXT NOT NULL,
            rel_path TEXT,
            summary TEXT,
            usage_help TEXT,
            example_titles TEXT,
            library_name TEXT NOT NULL,
            library_scope TEXT NOT NULL,
            name_lower TEXT,
            qualified_name_lower TEXT,
            search_text TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_ct_library ON component_types(library_id);
        CREATE INDEX IF NOT EXISTS idx_ct_qualified ON component_types(qualified_name);
        CREATE INDEX IF NOT EXISTS idx_ct_name_lower ON component_types(name_lower);
        CREATE INDEX IF NOT EXISTS idx_ct_qname_lower ON component_types(qualified_name_lower);
        CREATE INDEX IF NOT EXISTS idx_ct_search ON component_types(search_text);",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn open_connection() -> Result<Connection, String> {
    let path = db_path()?;
    let conn = Connection::open(&path).map_err(|e| e.to_string())?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")
        .map_err(|e| e.to_string())?;
    migrate(&conn)?;
    Ok(conn)
}

pub struct ComponentRow {
    pub library_id: String,
    pub qualified_name: String,
    pub name: String,
    pub kind: String,
    pub rel_path: Option<String>,
    pub summary: Option<String>,
    pub usage_help: Option<String>,
    pub example_titles: Vec<String>,
    pub library_name: String,
    pub library_scope: String,
}

pub fn upsert_library_meta(
    conn: &Connection,
    library_id: &str,
    source_path: &str,
    display_name: &str,
    scope: &str,
    mtime: i64,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO lib_meta (library_id, source_path, display_name, scope, last_scanned_mtime)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(library_id) DO UPDATE SET
            source_path = excluded.source_path,
            display_name = excluded.display_name,
            scope = excluded.scope,
            last_scanned_mtime = excluded.last_scanned_mtime",
        params![library_id, source_path, display_name, scope, mtime],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

pub fn replace_components(
    conn: &Connection,
    library_id: &str,
    rows: &[ComponentRow],
) -> Result<(), String> {
    conn.execute("DELETE FROM component_types WHERE library_id = ?1", params![library_id])
        .map_err(|e| e.to_string())?;
    for r in rows {
        let ex_titles = serde_json::to_string(&r.example_titles).unwrap_or_else(|_| "[]".to_string());
        let name_lower = r.name.to_lowercase();
        let qname_lower = r.qualified_name.to_lowercase();
        let search_text = format!(
            "{} {} {} {}",
            name_lower,
            qname_lower,
            r.summary.as_deref().unwrap_or(""),
            r.usage_help.as_deref().unwrap_or("")
        );
        conn.execute(
            "INSERT INTO component_types (library_id, qualified_name, name, kind, rel_path, summary, usage_help, example_titles, library_name, library_scope, name_lower, qualified_name_lower, search_text)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                library_id,
                r.qualified_name,
                r.name,
                r.kind,
                r.rel_path,
                r.summary,
                r.usage_help,
                ex_titles,
                r.library_name,
                r.library_scope,
                name_lower,
                qname_lower,
                search_text,
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn row_to_component_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ComponentRow> {
    let ex_titles: String = row.get(7).unwrap_or_else(|_| "[]".to_string());
    let example_titles: Vec<String> = serde_json::from_str(&ex_titles).unwrap_or_default();
    Ok(ComponentRow {
        library_id: row.get(0)?,
        qualified_name: row.get(1)?,
        name: row.get(2)?,
        kind: row.get(3)?,
        rel_path: row.get(4)?,
        summary: row.get(5)?,
        usage_help: row.get(6)?,
        example_titles,
        library_name: row.get(8)?,
        library_scope: row.get(9)?,
    })
}

pub fn get_component_counts(
    conn: &Connection,
    library_ids: &[String],
) -> Result<HashMap<String, usize>, String> {
    let mut out = HashMap::new();
    for id in library_ids {
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM component_types WHERE library_id = ?1",
                params![id],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        out.insert(id.clone(), count as usize);
    }
    Ok(out)
}

fn count_with_filter(
    conn: &Connection,
    library_ids: &[String],
    query_lower: Option<&str>,
) -> Result<i64, String> {
    let placeholders = library_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let (extra_where, param_count) = if let Some(q) = query_lower {
        if q.is_empty() {
            (String::new(), 0)
        } else {
            (" AND (name_lower LIKE ? OR qualified_name_lower LIKE ? OR search_text LIKE ?)".to_string(), 3)
        }
    } else {
        (String::new(), 0)
    };
    let sql = format!(
        "SELECT COUNT(*) FROM component_types WHERE library_id IN ({}){}",
        placeholders, extra_where
    );
    let total: i64 = if param_count == 0 {
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let mut param_refs: Vec<&dyn rusqlite::ToSql> = Vec::new();
        for id in library_ids {
            param_refs.push(id);
        }
        stmt.query_row(rusqlite::params_from_iter(param_refs), |row| row.get(0))
            .map_err(|e| e.to_string())?
    } else {
        let like = format!("%{}%", query_lower.unwrap());
        let mut stmt = conn.prepare(&sql).map_err(|e| e.to_string())?;
        let mut param_refs: Vec<&dyn rusqlite::ToSql> = Vec::new();
        for id in library_ids {
            param_refs.push(id);
        }
        param_refs.push(&like);
        param_refs.push(&like);
        param_refs.push(&like);
        stmt.query_row(rusqlite::params_from_iter(param_refs), |row| row.get(0))
            .map_err(|e| e.to_string())?
    };
    Ok(total)
}

pub fn query_components(
    conn: &Connection,
    library_ids: &[String],
    query_lower: Option<&str>,
    offset: usize,
    limit: usize,
) -> Result<(Vec<ComponentRow>, usize), String> {
    if library_ids.is_empty() {
        return Ok((Vec::new(), 0));
    }
    let total = count_with_filter(conn, library_ids, query_lower)? as usize;

    let placeholders = library_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let (extra_where, use_like) = if let Some(q) = query_lower {
        if q.is_empty() {
            (String::new(), false)
        } else {
            (" AND (name_lower LIKE ? OR qualified_name_lower LIKE ? OR search_text LIKE ?)".to_string(), true)
        }
    } else {
        (String::new(), false)
    };
    let limit_i = limit as i64;
    let offset_i = offset as i64;
    let select_sql = format!(
        "SELECT library_id, qualified_name, name, kind, rel_path, summary, usage_help, example_titles, library_name, library_scope
         FROM component_types
         WHERE library_id IN ({}) {}
         ORDER BY library_scope DESC, library_name ASC, qualified_name ASC
         LIMIT {} OFFSET {}",
        placeholders, extra_where, limit_i, offset_i
    );
    let mut stmt = conn.prepare(&select_sql).map_err(|e| e.to_string())?;
    let out: Vec<ComponentRow> = if use_like {
        let like = format!("%{}%", query_lower.unwrap());
        let mut param_refs: Vec<&dyn rusqlite::ToSql> = library_ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        param_refs.push(&like);
        param_refs.push(&like);
        param_refs.push(&like);
        stmt.query_map(rusqlite::params_from_iter(param_refs), row_to_component_row)
            .map(|rows| rows.map(|r| r.map_err(|e: rusqlite::Error| e.to_string())).collect::<Result<Vec<_>, String>>())
            .map_err(|e| e.to_string())??
    } else {
        let param_refs: Vec<&dyn rusqlite::ToSql> = library_ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
        stmt.query_map(rusqlite::params_from_iter(param_refs), row_to_component_row)
            .map(|rows| rows.map(|r| r.map_err(|e: rusqlite::Error| e.to_string())).collect::<Result<Vec<_>, String>>())
            .map_err(|e| e.to_string())??
    };
    Ok((out, total))
}

pub fn get_library_mtime(conn: &Connection, library_id: &str) -> Result<Option<i64>, String> {
    let res = conn.query_row(
        "SELECT last_scanned_mtime FROM lib_meta WHERE library_id = ?1",
        params![library_id],
        |row| row.get::<_, i64>(0),
    );
    match res {
        Ok(m) => Ok(Some(m)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

pub fn has_any_components(conn: &Connection) -> Result<bool, String> {
    let count: i64 = conn
        .query_row("SELECT COUNT(*) FROM component_types", [], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    Ok(count > 0)
}

pub fn clear_all(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "DELETE FROM component_types;
         DELETE FROM lib_meta;",
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// Result row for AI context: qualified name and doc text (summary + usage_help).
pub struct ContextRow {
    pub qualified_name: String,
    pub summary: Option<String>,
    pub usage_help: Option<String>,
}

/// Search component types by query (name_lower, qualified_name_lower, search_text) for AI context.
/// Returns up to `limit` rows with qualified_name, summary, usage_help.
pub fn search_for_context(
    conn: &Connection,
    query: &str,
    limit: usize,
) -> Result<Vec<ContextRow>, String> {
    let limit_i = limit as i64;
    let q = query.trim().to_lowercase();
    let rows = if q.is_empty() {
        conn.prepare(
            "SELECT qualified_name, summary, usage_help FROM component_types
             ORDER BY library_scope DESC, library_name ASC, qualified_name ASC
             LIMIT ?1",
        )
        .map_err(|e| e.to_string())?
        .query_map(params![limit_i], |row| {
            Ok(ContextRow {
                qualified_name: row.get(0)?,
                summary: row.get(1)?,
                usage_help: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
    } else {
        let like = format!("%{}%", q);
        conn.prepare(
            "SELECT qualified_name, summary, usage_help FROM component_types
             WHERE name_lower LIKE ?1 OR qualified_name_lower LIKE ?1 OR search_text LIKE ?1
             ORDER BY library_scope DESC, library_name ASC, qualified_name ASC
             LIMIT ?2",
        )
        .map_err(|e| e.to_string())?
        .query_map(params![like, limit_i], |row| {
            Ok(ContextRow {
                qualified_name: row.get(0)?,
                summary: row.get(1)?,
                usage_help: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
    };
    Ok(rows)
}
