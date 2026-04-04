use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub mod artifacts;
pub mod legacy;
pub mod runner;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheDirPolicy {
    /// Delete the directory if it exists, then recreate it.
    PurgeAndCreate,
    /// Create the directory if missing; keep existing contents.
    CreateIfMissing,
    /// Require the directory to exist; do not create.
    RequireExisting,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EnvOverlay {
    pub set: BTreeMap<String, String>,
    pub unset: Vec<String>,
}

impl EnvOverlay {
    pub fn apply_to_command(&self, cmd: &mut std::process::Command) {
        for k in &self.unset {
            cmd.env_remove(k);
        }
        for (k, v) in &self.set {
            cmd.env(k, v);
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scenario {
    pub id: String,
    pub runs: usize,
    pub cache_dir_policy: CacheDirPolicy,
    /// If set, overrides the cache dir for this scenario. Otherwise, runner chooses a subdir.
    pub cache_dir: Option<PathBuf>,
    pub env: EnvOverlay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateArgs {
    pub validate_tier: String,
    pub validation_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSpec {
    pub repo_root: PathBuf,
    pub exe_path: PathBuf,
    pub lib_paths: Vec<PathBuf>,
    pub out_dir: PathBuf,
    pub models: Vec<String>,
    pub validate: ValidateArgs,
    pub stage_trace: bool,
    pub perf_trace: bool,
    pub scenarios: Vec<Scenario>,
    /// Optional allow-list of scenario ids to execute.
    #[serde(default)]
    pub scenario_filter: Vec<String>,
    #[serde(default)]
    pub incremental: bool,
    /// Before running scenarios, delete every `out_dir/cache_*` directory (fresh on-disk tier state).
    #[serde(default)]
    pub purge_scenario_caches: bool,
}

pub fn normalize_model_list(models: &[String]) -> Vec<String> {
    let mut out: Vec<String> = models
        .iter()
        .map(|m| m.trim().to_string())
        .filter(|m| !m.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

pub fn ensure_cache_dir(path: &Path, policy: CacheDirPolicy) -> Result<()> {
    match policy {
        CacheDirPolicy::PurgeAndCreate => {
            let _ = std::fs::remove_dir_all(path);
            std::fs::create_dir_all(path)
                .with_context(|| format!("create cache dir {}", path.display()))?;
        }
        CacheDirPolicy::CreateIfMissing => {
            std::fs::create_dir_all(path)
                .with_context(|| format!("create cache dir {}", path.display()))?;
        }
        CacheDirPolicy::RequireExisting => {
            if !path.is_dir() {
                bail!("cache dir does not exist: {}", path.display());
            }
        }
    }
    Ok(())
}

