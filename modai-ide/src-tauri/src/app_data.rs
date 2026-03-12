//! Stable application data directory (per user, independent of process CWD).
//! Used for iterations.db, compiler.json, and other app-level persisted data.
//! Index DB remains per-project at project_dir/.modai-ide-data/index.db.
//!
//! ## Data layer path conventions (backend)
//!
//! | Store               | Path convention              | Purpose / readers & writers           |
//! |----------------------|-----------------------------|----------------------------------------|
//! | iterations.db        | app_data_root/iterations.db | Self-iteration history, test_runs, source_snapshots (db.rs) |
//! | compiler.json        | app_data_root/compiler.json | Compiler exe path and args (compiler_config.rs) |
//! | index.db             | project_dir/.modai-ide-data/index.db | Code index: symbols, chunks, deps (index_db.rs, index_manager.rs) |
//! | jit_traceability.json| repo_root/jit_traceability.json | JIT traceability config, compiler-iteration only |
//! | diagram-layout.json  | project_dir/.modai/diagram-layout.json | Diagram layout per file |
//! | API key              | Keyring (system)            | AI API key (ai.rs)                     |
//!
//! ## Frontend localStorage keys
//!
//! | Key             | Meaning / usage        |
//! |-----------------|------------------------|
//! | modai-theme     | UI theme preference    |
//! | modai-ai-daily  | AI daily usage         |
//! | modai-ai-model  | Selected AI model name |

use std::fs;
use std::path::PathBuf;

/// Returns a stable app data root, e.g.:
/// - Windows: %LOCALAPPDATA%\modai-ide
/// - macOS: ~/Library/Application Support/modai-ide
/// - Linux: ~/.local/share/modai-ide
/// Creates the directory if it does not exist.
pub fn app_data_root() -> Result<PathBuf, String> {
    let dir = directories::ProjectDirs::from("", "", "modai-ide")
        .map(|d| d.data_local_dir().to_path_buf())
        .ok_or_else(|| "Could not resolve application data directory".to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}
