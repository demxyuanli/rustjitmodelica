use tauri::Emitter;

use crate::{component_library_index, file_watcher, index_db, index_manager};

use super::common::jit_compiler_root;

#[tauri::command]
pub fn index_build(project_dir: String) -> Result<index_db::IndexStats, String> {
    index_manager::CodeIndex::new(&project_dir).build_index()
}

#[tauri::command]
pub fn index_update_file(project_dir: String, file_path: String) -> Result<(), String> {
    index_manager::CodeIndex::new(&project_dir).update_file(&file_path)
}

#[tauri::command]
pub fn index_search_symbols(
    project_dir: String,
    query: String,
    kind: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<index_db::SymbolInfo>, String> {
    index_manager::CodeIndex::new(&project_dir)
        .search_symbols(&query, kind.as_deref(), limit.unwrap_or(100))
}

#[tauri::command]
pub fn index_file_symbols(
    project_dir: String,
    file_path: String,
) -> Result<Vec<index_db::SymbolInfo>, String> {
    index_manager::CodeIndex::new(&project_dir).file_symbols(&file_path)
}

#[tauri::command]
pub fn index_find_references(
    project_dir: String,
    symbol_name: String,
) -> Result<Vec<index_db::DependencyInfo>, String> {
    index_manager::CodeIndex::new(&project_dir).find_references(&symbol_name)
}

#[tauri::command]
pub fn index_get_context(
    project_dir: String,
    query: String,
    max_chunks: Option<i64>,
) -> Result<Vec<index_db::ChunkInfo>, String> {
    index_manager::CodeIndex::new(&project_dir).get_context(&query, max_chunks.unwrap_or(10))
}

#[tauri::command]
pub fn index_get_dependencies(
    project_dir: String,
    file_path: String,
) -> Result<Vec<index_db::DependencyInfo>, String> {
    index_manager::CodeIndex::new(&project_dir).get_dependencies(&file_path)
}

#[tauri::command]
pub fn index_stats(project_dir: String) -> Result<index_db::IndexStats, String> {
    index_manager::CodeIndex::new(&project_dir).stats()
}

#[tauri::command]
pub fn index_start_watcher(
    app_handle: tauri::AppHandle,
    project_dir: String,
) -> Result<(), String> {
    file_watcher::start_watching(app_handle, project_dir)
}

#[tauri::command]
pub fn index_stop_watcher() -> Result<(), String> {
    file_watcher::stop_watching()
}

#[tauri::command]
pub fn index_refresh(
    app_handle: tauri::AppHandle,
    project_dir: String,
) -> Result<index_db::IndexStats, String> {
    index_manager::CodeIndex::new(&project_dir).build_index_with_progress(|done, total| {
        let _ = app_handle.emit(
            "index-progress",
            serde_json::json!({ "done": done, "total": total }),
        );
    })
}

#[tauri::command]
pub fn index_rebuild(
    app_handle: tauri::AppHandle,
    project_dir: String,
) -> Result<index_db::IndexStats, String> {
    index_manager::CodeIndex::new(&project_dir).rebuild_index_with_progress(|done, total| {
        let _ = app_handle.emit(
            "index-progress",
            serde_json::json!({ "done": done, "total": total }),
        );
    })
}

#[tauri::command]
pub fn index_refresh_repo(app_handle: tauri::AppHandle) -> Result<index_db::IndexStats, String> {
    let dir_str = jit_compiler_root()?.to_string_lossy().to_string();
    index_manager::CodeIndex::new(&dir_str).build_index_with_progress(|done, total| {
        let _ = app_handle.emit(
            "index-progress",
            serde_json::json!({ "done": done, "total": total }),
        );
    })
}

#[tauri::command]
pub fn index_rebuild_repo(app_handle: tauri::AppHandle) -> Result<index_db::IndexStats, String> {
    let dir_str = jit_compiler_root()?.to_string_lossy().to_string();
    index_manager::CodeIndex::new(&dir_str).rebuild_index_with_progress(|done, total| {
        let _ = app_handle.emit(
            "index-progress",
            serde_json::json!({ "done": done, "total": total }),
        );
    })
}

#[tauri::command]
pub fn index_repo_root() -> Result<String, String> {
    Ok(jit_compiler_root()?.to_string_lossy().replace('\\', "/"))
}

#[tauri::command]
pub fn index_build_repo() -> Result<index_db::IndexStats, String> {
    let dir_str = jit_compiler_root()?.to_string_lossy().to_string();
    index_manager::CodeIndex::new(&dir_str).build_index()
}

#[tauri::command]
pub fn index_repo_stats() -> Result<index_db::IndexStats, String> {
    let dir_str = jit_compiler_root()?.to_string_lossy().to_string();
    index_manager::CodeIndex::new(&dir_str).stats()
}

#[tauri::command]
pub fn index_repo_file_symbols(file_path: String) -> Result<Vec<index_db::SymbolInfo>, String> {
    let dir_str = jit_compiler_root()?.to_string_lossy().to_string();
    index_manager::CodeIndex::new(&dir_str).file_symbols(&file_path)
}

#[tauri::command]
pub fn index_repo_search_symbols(
    query: String,
    kind: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<index_db::SymbolInfo>, String> {
    let dir_str = jit_compiler_root()?.to_string_lossy().to_string();
    index_manager::CodeIndex::new(&dir_str)
        .search_symbols(&query, kind.as_deref(), limit.unwrap_or(100))
}

#[tauri::command]
pub fn index_repo_get_context(
    query: String,
    max_chunks: Option<i64>,
) -> Result<Vec<index_db::ChunkInfo>, String> {
    let dir_str = jit_compiler_root()?.to_string_lossy().to_string();
    index_manager::CodeIndex::new(&dir_str).get_context(&query, max_chunks.unwrap_or(10))
}

#[tauri::command]
pub fn index_list_included_files(
    project_dir: String,
    limit: Option<u32>,
) -> Result<IndexIncludedFiles, String> {
    let conn = index_db::open_connection(&project_dir)?;
    let list = index_db::list_indexed_paths(&conn)?;
    let total = list.len();
    let cap = limit.unwrap_or(500).min(2000) as usize;
    let paths: Vec<String> = list
        .into_iter()
        .take(cap)
        .map(|(_, p, _)| p)
        .collect();
    Ok(IndexIncludedFiles {
        total,
        paths,
    })
}

#[derive(serde::Serialize)]
pub struct IndexIncludedFiles {
    pub total: usize,
    pub paths: Vec<String>,
}

#[tauri::command]
pub fn index_component_library_get_context(
    query: String,
    max_chunks: Option<i64>,
) -> Result<Vec<index_db::ChunkInfo>, String> {
    let conn = component_library_index::open_connection()?;
    let limit = max_chunks.unwrap_or(10).max(0).min(20) as usize;
    let rows = component_library_index::search_for_context(&conn, &query, limit)?;
    let chunks: Vec<index_db::ChunkInfo> = rows
        .into_iter()
        .enumerate()
        .map(|(i, row)| {
            let content = [
                row.summary.as_deref().unwrap_or(""),
                row.usage_help.as_deref().unwrap_or(""),
            ]
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n");
            index_db::ChunkInfo {
                id: i as i64,
                file_id: 0,
                line_start: 0,
                line_end: 0,
                content: content.clone(),
                context_label: Some("component library".to_string()),
                content_hash: String::new(),
                file_path: row.qualified_name,
            }
        })
        .collect();
    Ok(chunks)
}
