// SQLite storage for self-iteration history.

use rusqlite::{Connection, params};

fn db_path() -> Result<std::path::PathBuf, String> {
    let dir = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join(".modai-ide-data");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("iterations.db"))
}

fn with_connection<T, F: FnOnce(&Connection) -> Result<T, String>>(f: F) -> Result<T, String> {
    let path = db_path()?;
    let conn = Connection::open(&path).map_err(|e| e.to_string())?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS iterations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            target TEXT NOT NULL,
            diff TEXT,
            success INTEGER NOT NULL,
            message TEXT NOT NULL,
            created_at TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| e.to_string())?;
    f(&conn)
}

#[derive(Debug, serde::Serialize)]
pub struct IterationRecord {
    pub id: i64,
    pub target: String,
    pub diff: Option<String>,
    pub success: bool,
    pub message: String,
    pub created_at: String,
}

pub fn save_iteration(
    target: &str,
    diff: Option<&str>,
    success: bool,
    message: &str,
) -> Result<i64, String> {
    with_connection(|conn| {
        conn.execute(
            "INSERT INTO iterations (target, diff, success, message, created_at) VALUES (?1, ?2, ?3, ?4, datetime('now'))",
            params![target, diff, success as i32, message],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    })
}

pub fn list_iteration_history(limit: i32) -> Result<Vec<IterationRecord>, String> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, target, diff, success, message, created_at FROM iterations ORDER BY id DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![limit], |row| {
                Ok(IterationRecord {
                    id: row.get(0)?,
                    target: row.get(1)?,
                    diff: row.get(2)?,
                    success: row.get::<_, i32>(3)? != 0,
                    message: row.get(4)?,
                    created_at: row.get(5)?,
                })
            })
            .map_err(|e| e.to_string())?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r.map_err(|e| e.to_string())?);
        }
        Ok(out)
    })
}
