use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub mod artifacts;
pub mod baseline;
pub mod correctness;
pub mod legacy;
pub mod runner;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CacheDirPolicy {
    /// Delete the directory if it exists, then recreate it.
    PurgeAndCreate,
    /// Create the directory if missing; keep existing contents.
    CreateIfMissing,
    /// Same as [`CacheDirPolicy::CreateIfMissing`]; documents multi-scenario pipelines that reuse one cache root.
    PreserveBetweenScenarios,
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
    /// Optional shared cache root for all scenarios.
    /// When set, every scenario resolves cache dir under this root instead of `out_dir/cache_<scenario>`.
    #[serde(default)]
    pub shared_cache_dir: Option<PathBuf>,
    /// Force-enable flatten full cache for every scenario (sets `RUSTMODLICA_FLATTEN_FULL_CACHE=1`).
    #[serde(default)]
    pub force_flatten_full_cache: bool,
    /// PoC: execute per-scenario batches via worker mode entrypoint.
    /// Current implementation keeps artifact parity with legacy one-case-one-process execution.
    #[serde(default)]
    pub worker_per_scenario: bool,
    /// Extra env vars applied to every rustmodlica child (merged after scenario env).
    #[serde(default)]
    pub child_env: EnvOverlay,
    /// Optional `RUSTMODLICA_STD_CACHE_ROOT` for all child processes.
    #[serde(default)]
    pub std_cache_root: Option<PathBuf>,
    /// Optional `RUSTMODLICA_USER_CACHE_ROOT` for all child processes.
    #[serde(default)]
    pub user_cache_root: Option<PathBuf>,
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

/// Default `--lib-path` list for `jit validate-perf`, matching `run_regression.ps1`:
/// include `jit-compiler` (when `Modelica/package.mo` exists) and always add `jit-compiler/TestLib`.
/// Using only `jit-compiler` as a single root breaks some intra-TestLib models (connector helpers).
/// Optional `jit-compiler/StandardLib` is appended when present for MSL-backed TestLib cases.
/// When `jit-compiler/ModelicaTest/JitStress` exists, return canonical stress model names for optional perf runs.
pub fn probe_optional_jitstress_models(repo_root: &Path) -> Vec<String> {
    let stress = repo_root
        .join("jit-compiler")
        .join("ModelicaTest")
        .join("JitStress");
    if !stress.is_dir() {
        return Vec::new();
    }
    vec![
        "ModelicaTest.JitStress.MslBroadCoverage".to_string(),
        "ModelicaTest.JitStress.ComplexJitRegression".to_string(),
        "ModelicaTest.JitStress.RobotElectricalControl".to_string(),
        "ModelicaTest.JitStress.SyncOmCompare".to_string(),
    ]
}

pub fn default_jit_validate_perf_lib_paths(repo_root: &Path) -> Vec<PathBuf> {
    let jc = repo_root.join("jit-compiler");
    if !jc.is_dir() {
        return vec![jc];
    }
    let mut paths: Vec<PathBuf> = Vec::new();
    if jc.join("Modelica").join("package.mo").is_file() {
        paths.push(jc.clone());
    }
    let testlib = jc.join("TestLib");
    if testlib.is_dir() {
        paths.push(testlib);
    }
    if paths.is_empty() {
        paths.push(jc.clone());
    } else if paths.len() == 1 && paths[0].ends_with("TestLib") {
        paths.insert(0, jc.clone());
    }
    let stdlib = jc.join("StandardLib");
    if stdlib.is_dir() && !paths.iter().any(|p| p == &stdlib) {
        paths.push(stdlib);
    }
    paths
}

pub fn ensure_cache_dir(path: &Path, policy: CacheDirPolicy) -> Result<()> {
    match policy {
        CacheDirPolicy::PurgeAndCreate => {
            let _ = std::fs::remove_dir_all(path);
            std::fs::create_dir_all(path)
                .with_context(|| format!("create cache dir {}", path.display()))?;
        }
        CacheDirPolicy::CreateIfMissing | CacheDirPolicy::PreserveBetweenScenarios => {
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

