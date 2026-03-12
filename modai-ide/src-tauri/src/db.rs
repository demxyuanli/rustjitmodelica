// SQLite storage for self-iteration history, test runs, and source snapshots.

use rusqlite::{Connection, params};

fn db_path() -> Result<std::path::PathBuf, String> {
    let dir = crate::app_data::app_data_root()?;
    Ok(dir.join("iterations.db"))
}

fn with_connection<T, F: FnOnce(&Connection) -> Result<T, String>>(f: F) -> Result<T, String> {
    let path = db_path()?;
    let conn = Connection::open(&path).map_err(|e| e.to_string())?;
    migrate(&conn)?;
    f(&conn)
}

fn migrate(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS iterations (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            target TEXT NOT NULL,
            diff TEXT,
            success INTEGER NOT NULL,
            message TEXT NOT NULL,
            created_at TEXT NOT NULL,
            branch_name TEXT,
            parent_iteration_id INTEGER,
            affected_files TEXT,
            test_results TEXT,
            duration_ms INTEGER
        );
        CREATE TABLE IF NOT EXISTS test_runs (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            iteration_id INTEGER,
            case_name TEXT NOT NULL,
            status TEXT NOT NULL,
            exit_code INTEGER,
            stdout TEXT,
            stderr TEXT,
            duration_ms INTEGER,
            created_at TEXT NOT NULL,
            FOREIGN KEY (iteration_id) REFERENCES iterations(id)
        );
        CREATE TABLE IF NOT EXISTS source_snapshots (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            iteration_id INTEGER NOT NULL,
            file_path TEXT NOT NULL,
            content TEXT NOT NULL,
            created_at TEXT NOT NULL,
            FOREIGN KEY (iteration_id) REFERENCES iterations(id)
        );",
    )
    .map_err(|e| e.to_string())?;

    let has_branch: bool = conn
        .prepare("PRAGMA table_info(iterations)")
        .and_then(|mut s| {
            let mut found = false;
            s.query_map([], |row| {
                let name: String = row.get(1)?;
                if name == "branch_name" {
                    found = true;
                }
                Ok(())
            })?
            .for_each(|_| {});
            Ok(found)
        })
        .unwrap_or(false);

    if !has_branch {
        let _ = conn.execute_batch(
            "ALTER TABLE iterations ADD COLUMN branch_name TEXT;
             ALTER TABLE iterations ADD COLUMN parent_iteration_id INTEGER;
             ALTER TABLE iterations ADD COLUMN affected_files TEXT;
             ALTER TABLE iterations ADD COLUMN test_results TEXT;
             ALTER TABLE iterations ADD COLUMN duration_ms INTEGER;",
        );
    }

    let has_git_commit: bool = conn
        .prepare("PRAGMA table_info(iterations)")
        .and_then(|mut s| {
            let mut found = false;
            s.query_map([], |row: &rusqlite::Row| {
                let name: String = row.get(1)?;
                if name == "git_commit" {
                    found = true;
                }
                Ok(())
            })?
            .for_each(|_| {});
            Ok(found)
        })
        .unwrap_or(false);

    if !has_git_commit {
        let _ = conn.execute("ALTER TABLE iterations ADD COLUMN git_commit TEXT", []);
    }

    Ok(())
}

#[derive(Debug, serde::Serialize)]
pub struct IterationRecord {
    pub id: i64,
    pub target: String,
    pub diff: Option<String>,
    pub success: bool,
    pub message: String,
    pub created_at: String,
    pub branch_name: Option<String>,
    pub parent_iteration_id: Option<i64>,
    pub affected_files: Option<String>,
    pub test_results: Option<String>,
    pub duration_ms: Option<i64>,
    pub git_commit: Option<String>,
}

pub fn save_iteration(
    target: &str,
    diff: Option<&str>,
    success: bool,
    message: &str,
    git_commit: Option<&str>,
) -> Result<i64, String> {
    with_connection(|conn| {
        conn.execute(
            "INSERT INTO iterations (target, diff, success, message, created_at, git_commit) VALUES (?1, ?2, ?3, ?4, datetime('now'), ?5)",
            params![target, diff, success as i32, message, git_commit],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    })
}

pub fn get_iteration_by_id(id: i64) -> Result<Option<IterationRecord>, String> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, target, diff, success, message, created_at,
                    branch_name, parent_iteration_id, affected_files, test_results, duration_ms, git_commit
             FROM iterations WHERE id = ?1",
        )
        .map_err(|e| e.to_string())?;
        let mut rows = stmt
            .query_map(params![id], |row| {
                Ok(IterationRecord {
                    id: row.get(0)?,
                    target: row.get(1)?,
                    diff: row.get(2)?,
                    success: row.get::<_, i32>(3)? != 0,
                    message: row.get(4)?,
                    created_at: row.get(5)?,
                    branch_name: row.get(6).ok(),
                    parent_iteration_id: row.get(7).ok(),
                    affected_files: row.get(8).ok(),
                    test_results: row.get(9).ok(),
                    duration_ms: row.get(10).ok(),
                    git_commit: row.get(11).ok(),
                })
            })
            .map_err(|e| e.to_string())?;
        Ok(rows.next().transpose().map_err(|e| e.to_string())?)
    })
}

pub fn list_iteration_history(limit: i32) -> Result<Vec<IterationRecord>, String> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, target, diff, success, message, created_at,
                    branch_name, parent_iteration_id, affected_files, test_results, duration_ms, git_commit
             FROM iterations ORDER BY id DESC LIMIT ?1",
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
                    branch_name: row.get(6).ok(),
                    parent_iteration_id: row.get(7).ok(),
                    affected_files: row.get(8).ok(),
                    test_results: row.get(9).ok(),
                    duration_ms: row.get(10).ok(),
                    git_commit: row.get(11).ok(),
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
