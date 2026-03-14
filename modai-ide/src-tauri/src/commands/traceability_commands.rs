use crate::traceability;

use super::common::jit_compiler_root;

#[tauri::command]
pub fn load_traceability_config() -> Result<traceability::TraceabilityConfig, String> {
    traceability::load_config(&jit_compiler_root()?)
}

#[tauri::command]
pub fn save_traceability_config(
    config: traceability::TraceabilityConfig,
) -> Result<(), String> {
    traceability::save_config(&jit_compiler_root()?, &config)
}

#[tauri::command]
pub fn get_traceability_matrix() -> Result<traceability::TraceabilityMatrix, String> {
    traceability::get_traceability_matrix(&jit_compiler_root()?)
}

#[tauri::command]
pub fn traceability_impact_analysis(
    changed_files: Vec<String>,
) -> Result<traceability::ImpactAnalysisResult, String> {
    traceability::impact_analysis(&jit_compiler_root()?, &changed_files)
}

#[tauri::command]
pub fn traceability_coverage_analysis() -> Result<traceability::CoverageAnalysisResult, String> {
    traceability::coverage_analysis(&jit_compiler_root()?)
}

#[tauri::command]
pub fn update_traceability_link(
    link_type: String,
    source: String,
    target: String,
    add: bool,
) -> Result<(), String> {
    traceability::update_traceability_link(&jit_compiler_root()?, &link_type, &source, &target, add)
}

#[tauri::command]
pub fn traceability_sync_check() -> Result<traceability::SyncCheckResult, String> {
    traceability::sync_check(&jit_compiler_root()?)
}

#[tauri::command]
pub fn traceability_validate() -> Result<traceability::ValidationResult, String> {
    traceability::validate_config(&jit_compiler_root()?)
}

#[tauri::command]
pub fn traceability_apply_sync(request: traceability::ApplySyncRequest) -> Result<(), String> {
    traceability::apply_sync(&jit_compiler_root()?, &request)
}

#[tauri::command]
pub fn traceability_git_impact() -> Result<traceability::GitImpactResult, String> {
    traceability::git_changed_impact(&jit_compiler_root()?)
}
