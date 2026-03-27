use chrono::Utc;
use modai_paths::{new_workspace_id, normalize_to_unix, RegressionWorkspacePaths};
use modai_protocol::{PlanStrategy, RegressionPlanRequest, RegressionWorkspaceInfo, WorkspaceStatus};
use std::fs;
use std::path::{Path, PathBuf};

pub struct WorkspaceHandle {
    pub workspace_id: String,
    pub workspace_path: String,
    pub workspace_dir: PathBuf,
}

fn strategy_bucket(strategy: &PlanStrategy) -> &'static str {
    match strategy {
        PlanStrategy::Category => "by-category",
        PlanStrategy::Feature => "by-feature",
        PlanStrategy::Relation => "by-relation",
    }
}

fn strategy_key(request: &RegressionPlanRequest) -> String {
    match request.strategy {
        PlanStrategy::Category => {
            if request.categories.is_empty() {
                "all".to_string()
            } else {
                request.categories.join("_")
            }
        }
        PlanStrategy::Feature => {
            if request.feature_ids.is_empty() {
                "all".to_string()
            } else {
                request.feature_ids.join("_")
            }
        }
        PlanStrategy::Relation => "git-impact".to_string(),
    }
}

pub fn init_workspace(repo_root: &Path, request: &RegressionPlanRequest) -> Result<WorkspaceHandle, String> {
    let layout = RegressionWorkspacePaths::new(repo_root);
    fs::create_dir_all(&layout.root).map_err(|e| e.to_string())?;
    let workspace_id = new_workspace_id();
    let bucket = strategy_bucket(&request.strategy);
    let key = strategy_key(request);
    let workspace_dir = layout.workspace_dir(bucket, &key, &workspace_id);
    fs::create_dir_all(workspace_dir.join("logs")).map_err(|e| e.to_string())?;
    Ok(WorkspaceHandle {
        workspace_id,
        workspace_path: normalize_to_unix(&workspace_dir),
        workspace_dir,
    })
}

pub fn open_workspace(repo_root: &Path, workspace_id: &str) -> Result<WorkspaceHandle, String> {
    let root = RegressionWorkspacePaths::new(repo_root).root;
    if !root.exists() {
        return Err("regression workspace root does not exist".to_string());
    }
    let mut stack = vec![root.clone()];
    while let Some(cur) = stack.pop() {
        for entry in fs::read_dir(&cur).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let p = entry.path();
            if p.is_dir() {
                if p.file_name().and_then(|n| n.to_str()) == Some(workspace_id) {
                    return Ok(WorkspaceHandle {
                        workspace_id: workspace_id.to_string(),
                        workspace_path: normalize_to_unix(&p),
                        workspace_dir: p,
                    });
                }
                stack.push(p);
            }
        }
    }
    Err(format!("workspace not found: {workspace_id}"))
}

pub fn list_workspace_infos(repo_root: &Path) -> Result<Vec<RegressionWorkspaceInfo>, String> {
    let root = RegressionWorkspacePaths::new(repo_root).root;
    if !root.exists() {
        return Ok(Vec::new());
    }
    let mut infos = Vec::new();
    let mut stack = vec![root.clone()];
    while let Some(cur) = stack.pop() {
        for entry in fs::read_dir(&cur).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let p = entry.path();
            if p.is_dir() {
                let state_file = p.join("workspace-state.json");
                if state_file.exists() {
                    let id = p
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default()
                        .to_string();
                    let strategy = if normalize_to_unix(&p).contains("by-category") {
                        PlanStrategy::Category
                    } else if normalize_to_unix(&p).contains("by-feature") {
                        PlanStrategy::Feature
                    } else {
                        PlanStrategy::Relation
                    };
                    infos.push(RegressionWorkspaceInfo {
                        workspace_id: id,
                        workspace_path: normalize_to_unix(&p),
                        strategy,
                        status: WorkspaceStatus::Planned,
                        created_at: Utc::now().to_rfc3339(),
                    });
                } else {
                    stack.push(p);
                }
            }
        }
    }
    infos.sort_by(|a, b| b.workspace_id.cmp(&a.workspace_id));
    Ok(infos)
}
