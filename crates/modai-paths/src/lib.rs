use chrono::Utc;
use std::path::{Path, PathBuf};

pub struct RegressionWorkspacePaths {
    pub root: PathBuf,
}

impl RegressionWorkspacePaths {
    pub fn new(repo_root: &Path) -> Self {
        Self {
            root: repo_root.join("build").join("regression-workspace"),
        }
    }

    pub fn by_category_root(&self) -> PathBuf {
        self.root.join("by-category")
    }

    pub fn by_feature_root(&self) -> PathBuf {
        self.root.join("by-feature")
    }

    pub fn by_relation_root(&self) -> PathBuf {
        self.root.join("by-relation")
    }

    pub fn workspace_dir(&self, bucket: &str, key: &str, workspace_id: &str) -> PathBuf {
        self.root.join(bucket).join(key).join(workspace_id)
    }
}

pub fn normalize_to_unix(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

pub fn new_workspace_id() -> String {
    format!("run_{}", Utc::now().format("%Y%m%d_%H%M%S_%3f"))
}
