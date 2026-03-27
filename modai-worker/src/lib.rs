pub mod artifact;
pub mod discover;
pub mod planner;
pub mod runner;
pub mod workspace;

use chrono::Utc;
use modai_protocol::{
    PlanStrategy, RegressionPlanRequest, RegressionWorkspaceInfo,
    RegressionWorkspaceState, WorkspaceRunResult, WorkspaceStatus,
};
use std::path::Path;

pub fn create_workspace(
    repo_root: &Path,
    request: RegressionPlanRequest,
) -> Result<RegressionWorkspaceState, String> {
    let ws = workspace::init_workspace(repo_root, &request)?;
    let plan = planner::build_plan(repo_root, &request)?;
    let info = RegressionWorkspaceInfo {
        workspace_id: ws.workspace_id.clone(),
        workspace_path: ws.workspace_path.clone(),
        strategy: request.strategy.clone(),
        status: WorkspaceStatus::Planned,
        created_at: Utc::now().to_rfc3339(),
    };
    let state = RegressionWorkspaceState {
        info,
        plan,
        result: None,
        records: Vec::new(),
    };
    artifact::write_workspace_state(&ws.workspace_dir, &state)?;
    artifact::write_plan_cases(&ws.workspace_dir, &state.plan)?;
    Ok(state)
}

pub fn run_workspace(repo_root: &Path, workspace_id: &str) -> Result<RegressionWorkspaceState, String> {
    let ws = workspace::open_workspace(repo_root, workspace_id)?;
    let mut state = artifact::read_workspace_state(&ws.workspace_dir)?;
    state.info.status = WorkspaceStatus::Running;
    artifact::write_workspace_state(&ws.workspace_dir, &state)?;
    let run = runner::run_cases(repo_root, &state.plan, &ws.workspace_dir)?;
    state.records = run.records;
    state.result = Some(WorkspaceRunResult {
        total: run.total,
        passed: run.passed,
        failed: run.failed,
        duration_ms: run.duration_ms,
    });
    state.info.status = if run.failed == 0 {
        WorkspaceStatus::Completed
    } else {
        WorkspaceStatus::Failed
    };
    artifact::write_workspace_state(&ws.workspace_dir, &state)?;
    Ok(state)
}

pub fn get_workspace_state(repo_root: &Path, workspace_id: &str) -> Result<RegressionWorkspaceState, String> {
    let ws = workspace::open_workspace(repo_root, workspace_id)?;
    artifact::read_workspace_state(&ws.workspace_dir)
}

pub fn list_workspaces(repo_root: &Path) -> Result<Vec<RegressionWorkspaceInfo>, String> {
    workspace::list_workspace_infos(repo_root)
}

pub fn cancel_workspace(repo_root: &Path, workspace_id: &str) -> Result<RegressionWorkspaceState, String> {
    let ws = workspace::open_workspace(repo_root, workspace_id)?;
    let mut state = artifact::read_workspace_state(&ws.workspace_dir)?;
    state.info.status = WorkspaceStatus::Cancelled;
    artifact::write_workspace_state(&ws.workspace_dir, &state)?;
    Ok(state)
}

pub fn default_large_full_request() -> RegressionPlanRequest {
    RegressionPlanRequest {
        strategy: PlanStrategy::Relation,
        categories: Vec::new(),
        feature_ids: Vec::new(),
        changed_files: Vec::new(),
        include_indirect: true,
        max_cases: None,
        workspace_mode: modai_protocol::WorkspaceMode::Persistent,
        include_modelica_examples: true,
        include_modelica_test: true,
    }
}
