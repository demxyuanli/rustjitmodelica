use crate::git;

use super::common::project_dir_canonical;

#[tauri::command]
pub fn git_head_commit(project_dir: String) -> Result<String, String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_head_commit_impl(&dir)
}

#[tauri::command]
pub fn git_is_repo(project_dir: String) -> bool {
    let Ok(dir) = project_dir_canonical(&project_dir) else {
        return false;
    };
    git::git_is_repo_impl(&dir)
}

#[tauri::command]
pub fn git_init(project_dir: String) -> Result<(), String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_init_impl(&dir)
}

#[tauri::command]
pub fn git_status(project_dir: String) -> Result<git::GitStatus, String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_status_impl(&dir)
}

#[tauri::command]
pub fn git_diff_file(
    project_dir: String,
    relative_path: String,
    base: Option<String>,
) -> Result<String, String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_diff_file_impl(&dir, &relative_path, base.as_deref())
}

#[tauri::command]
pub fn git_diff_file_staged(project_dir: String, relative_path: String) -> Result<String, String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_diff_file_staged_impl(&dir, &relative_path)
}

#[tauri::command]
pub fn git_show_file(
    project_dir: String,
    revision: String,
    relative_path: String,
) -> Result<String, String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_show_file_impl(&dir, &revision, &relative_path)
}

#[tauri::command]
pub fn git_log(
    project_dir: String,
    relative_path: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<git::GitLogEntry>, String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_log_impl(&dir, relative_path.as_deref(), limit.unwrap_or(50))
}

#[tauri::command]
pub fn git_stage(project_dir: String, paths: Vec<String>) -> Result<(), String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_stage_impl(&dir, &paths)
}

#[tauri::command]
pub fn git_unstage(project_dir: String, paths: Vec<String>) -> Result<(), String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_unstage_impl(&dir, &paths)
}

#[tauri::command]
pub fn git_commit(project_dir: String, message: String) -> Result<(), String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_commit_impl(&dir, &message)
}

#[tauri::command]
pub fn git_commit_files(
    project_dir: String,
    hash: String,
) -> Result<Vec<git::GitCommitFile>, String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_commit_files_impl(&dir, &hash)
}

#[tauri::command]
pub fn git_log_graph(
    project_dir: String,
    limit: Option<u32>,
) -> Result<Vec<git::GitLogGraphEntry>, String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_log_graph_impl(&dir, limit.unwrap_or(50))
}
