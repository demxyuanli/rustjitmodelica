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
