use crate::{git, source_manager};

use super::common::jit_compiler_root;

#[tauri::command]
pub fn list_compiler_source_tree() -> Result<source_manager::SourceTreeEntry, String> {
    source_manager::list_source_tree(&jit_compiler_root()?)
}

#[tauri::command]
pub fn read_compiler_file(path: String) -> Result<String, String> {
    source_manager::read_file(&jit_compiler_root()?, &path)
}

#[tauri::command]
pub fn write_compiler_file(path: String, content: String) -> Result<(), String> {
    source_manager::write_file(&jit_compiler_root()?, &path, &content)
}

#[tauri::command]
pub fn compiler_file_git_log(
    path: String,
    limit: Option<u32>,
) -> Result<Vec<git::GitLogEntry>, String> {
    let root = jit_compiler_root()?;
    git::git_log_impl(&root, Some(&path), limit.unwrap_or(20))
}

#[tauri::command]
pub fn compiler_file_git_diff(path: String) -> Result<String, String> {
    let root = jit_compiler_root()?;
    git::git_diff_file_impl(&root, &path, None)
}

#[tauri::command]
pub fn create_iteration_branch(name: String) -> Result<String, String> {
    source_manager::create_iteration_branch(&jit_compiler_root()?, &name)
}

#[tauri::command]
pub fn list_iteration_branches() -> Result<Vec<String>, String> {
    source_manager::list_iteration_branches(&jit_compiler_root()?)
}

#[tauri::command]
pub fn switch_iteration_branch(name: String) -> Result<(), String> {
    source_manager::switch_branch(&jit_compiler_root()?, &name)
}

#[tauri::command]
pub fn merge_iteration_branch(name: String) -> Result<(), String> {
    source_manager::merge_branch(&jit_compiler_root()?, &name)
}
