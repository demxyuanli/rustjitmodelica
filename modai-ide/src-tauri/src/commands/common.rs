use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JitValidateOptions {
    pub t_end: Option<f64>,
    pub dt: Option<f64>,
    pub atol: Option<f64>,
    pub rtol: Option<f64>,
    pub solver: Option<String>,
    pub output_interval: Option<f64>,
    pub coarse_constrainedby_only: Option<bool>,
    /// `full` | `parse` | `flatten` | `analyze` — validation stops after this tier (JIT only for `full`).
    pub validation_tier: Option<String>,
    /// When provenance is available after flatten, run `analyze_change_impact` for these flattened parameter names (analysis only).
    #[serde(default)]
    pub param_change_impact_probe: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolverContext {
    pub library_paths: Vec<String>,
    pub project_dir: Option<String>,
    pub coarse_constrainedby_only: bool,
}

pub fn repo_root() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "cannot determine repository root".to_string())
}

pub fn jit_compiler_root() -> Result<PathBuf, String> {
    Ok(repo_root()?.join("jit-compiler"))
}

pub fn project_dir_canonical(project_dir: &str) -> Result<PathBuf, String> {
    Path::new(project_dir)
        .canonicalize()
        .map_err(|e| e.to_string())
}
