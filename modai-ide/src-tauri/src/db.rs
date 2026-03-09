// SQLite storage for self-iteration history, test runs, and source snapshots.

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
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct TestRunRecord {
    pub id: i64,
    pub iteration_id: Option<i64>,
    pub case_name: String,
    pub status: String,
    pub exit_code: Option<i32>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub duration_ms: Option<i64>,
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

pub fn save_iteration_full(
    target: &str,
    diff: Option<&str>,
    success: bool,
    message: &str,
    branch_name: Option<&str>,
    parent_id: Option<i64>,
    affected_files: Option<&str>,
    test_results: Option<&str>,
    duration_ms: Option<i64>,
) -> Result<i64, String> {
    with_connection(|conn| {
        conn.execute(
            "INSERT INTO iterations (target, diff, success, message, created_at, branch_name, parent_iteration_id, affected_files, test_results, duration_ms)
             VALUES (?1, ?2, ?3, ?4, datetime('now'), ?5, ?6, ?7, ?8, ?9)",
            params![
                target,
                diff,
                success as i32,
                message,
                branch_name,
                parent_id,
                affected_files,
                test_results,
                duration_ms
            ],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    })
}

pub fn save_test_run(
    iteration_id: Option<i64>,
    case_name: &str,
    status: &str,
    exit_code: Option<i32>,
    stdout: Option<&str>,
    stderr: Option<&str>,
    duration_ms: Option<i64>,
) -> Result<i64, String> {
    with_connection(|conn| {
        conn.execute(
            "INSERT INTO test_runs (iteration_id, case_name, status, exit_code, stdout, stderr, duration_ms, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
            params![iteration_id, case_name, status, exit_code, stdout, stderr, duration_ms],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    })
}

pub fn save_source_snapshot(
    iteration_id: i64,
    file_path: &str,
    content: &str,
) -> Result<i64, String> {
    with_connection(|conn| {
        conn.execute(
            "INSERT INTO source_snapshots (iteration_id, file_path, content, created_at)
             VALUES (?1, ?2, ?3, datetime('now'))",
            params![iteration_id, file_path, content],
        )
        .map_err(|e| e.to_string())?;
        Ok(conn.last_insert_rowid())
    })
}

pub fn list_iteration_history(limit: i32) -> Result<Vec<IterationRecord>, String> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, target, diff, success, message, created_at,
                    branch_name, parent_iteration_id, affected_files, test_results, duration_ms
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

pub fn list_test_runs_for_iteration(iteration_id: i64) -> Result<Vec<TestRunRecord>, String> {
    with_connection(|conn| {
        let mut stmt = conn.prepare(
            "SELECT id, iteration_id, case_name, status, exit_code, stdout, stderr, duration_ms, created_at
             FROM test_runs WHERE iteration_id = ?1 ORDER BY id ASC",
        )
        .map_err(|e| e.to_string())?;
        let rows = stmt
            .query_map(params![iteration_id], |row| {
                Ok(TestRunRecord {
                    id: row.get(0)?,
                    iteration_id: row.get(1)?,
                    case_name: row.get(2)?,
                    status: row.get(3)?,
                    exit_code: row.get(4).ok(),
                    stdout: row.get(5).ok(),
                    stderr: row.get(6).ok(),
                    duration_ms: row.get(7).ok(),
                    created_at: row.get(8)?,
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

pub fn get_iteration_stats() -> Result<IterationStats, String> {
    with_connection(|conn| {
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM iterations", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        let passed: i64 = conn
            .query_row("SELECT COUNT(*) FROM iterations WHERE success = 1", [], |row| row.get(0))
            .map_err(|e| e.to_string())?;
        let failed = total - passed;
        let avg_duration: f64 = conn
            .query_row(
                "SELECT COALESCE(AVG(duration_ms), 0) FROM iterations WHERE duration_ms IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        Ok(IterationStats {
            total,
            passed,
            failed,
            avg_duration_ms: avg_duration,
        })
    })
}

#[derive(Debug, serde::Serialize)]
pub struct IterationStats {
    pub total: i64,
    pub passed: i64,
    pub failed: i64,
    pub avg_duration_ms: f64,
}
