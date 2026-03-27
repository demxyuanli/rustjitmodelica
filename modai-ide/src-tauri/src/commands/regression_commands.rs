use modai_protocol::{RegressionPlanRequest, RegressionWorkspaceInfo, RegressionWorkspaceState};

use super::common::repo_root;

#[tauri::command]
pub fn regression_create_workspace(
    request: RegressionPlanRequest,
) -> Result<RegressionWorkspaceState, String> {
    modai_worker::create_workspace(&repo_root()?, request)
}

#[tauri::command]
pub fn regression_run_workspace(workspace_id: String) -> Result<RegressionWorkspaceState, String> {
    modai_worker::run_workspace(&repo_root()?, &workspace_id)
}

#[tauri::command]
pub fn regression_get_workspace_state(workspace_id: String) -> Result<RegressionWorkspaceState, String> {
    modai_worker::get_workspace_state(&repo_root()?, &workspace_id)
}

#[tauri::command]
pub fn regression_list_workspaces() -> Result<Vec<RegressionWorkspaceInfo>, String> {
    modai_worker::list_workspaces(&repo_root()?)
}

#[tauri::command]
pub fn regression_cancel_workspace(workspace_id: String) -> Result<RegressionWorkspaceState, String> {
    modai_worker::cancel_workspace(&repo_root()?, &workspace_id)
}
