//! Configuration file format `version` == 1.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("unsupported config version: {0}")]
    UnsupportedVersion(u32),
    #[error("duplicate case id: {0}")]
    DuplicateCaseId(String),
    #[error("unknown tier extends: {0}")]
    UnknownTierExtends(String),
    #[error("IO: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Clone, Deserialize)]
pub struct HarnessConfig {
    pub version: u32,
    /// Optional list of additional config files to include (relative to this config file or absolute).
    /// Included configs are merged first; the current config then overlays defaults/execution/incremental/tiers,
    /// and appends its cases.
    #[serde(default)]
    pub includes: Vec<String>,
    #[serde(default)]
    pub defaults: Defaults,
    #[serde(default)]
    pub execution: ExecutionConfig,
    #[serde(default)]
    pub incremental: IncrementalConfig,
    #[serde(default)]
    pub tiers: HashMap<String, TierSpec>,
    pub cases: Vec<CaseDef>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct Defaults {
    /// Repository root (relative paths in config resolve against this). Default ".".
    #[serde(default = "default_dot")]
    pub repo_root: String,
    /// Path to rustmodlica executable (relative to repo_root or absolute).
    #[serde(default = "default_rustmodlica_exe")]
    pub rustmodlica_exe: String,
    /// Current directory for model runs (relative to repo_root), e.g. "jit-compiler".
    #[serde(default = "default_working_dir")]
    pub working_dir: String,
    #[serde(default = "default_cargo_exe")]
    pub cargo_exe: String,
    /// Extra args before `--` for `cargo run` (MOS mode), e.g. ["--release"].
    #[serde(default)]
    pub cargo_run_prefix: Vec<String>,
    /// Args for `cargo run` subcommand (placed after `run`, before `--`), e.g. ["--features", "sundials"].
    #[serde(default)]
    pub cargo_run_args: Vec<String>,
    /// Use `cargo run` instead of invoking `rustmodlica_exe` directly for `kind: model`.
    /// This enables Windows lock retry/fallback semantics similar to `run_regression.ps1`.
    #[serde(default)]
    pub cargo_run_models: bool,
    /// Primary cargo target dir for `cargo run` (e.g. "target_regression").
    #[serde(default)]
    pub cargo_target_dir_primary: Option<String>,
    /// Optional explicit fallback cargo target dir. If omitted, a run-scoped fallback is auto-generated.
    #[serde(default)]
    pub cargo_target_dir_fallback: Option<String>,
    /// Maximum attempts for `cargo run` when lock patterns are detected.
    #[serde(default = "default_cargo_run_max_attempts")]
    pub cargo_run_max_attempts: usize,
    #[serde(default = "default_solver")]
    pub solver: String,
    #[serde(default = "default_t_end")]
    pub t_end: f64,
    #[serde(default = "default_dt")]
    pub dt: f64,
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Default central regression data directory (relative to cwd). Overridden by CLI `--data-root` / `--out-dir`.
    #[serde(default)]
    pub regression_data_root: Option<String>,
}

fn default_dot() -> String {
    ".".to_string()
}

fn default_rustmodlica_exe() -> String {
    "auto".to_string()
}

fn default_working_dir() -> String {
    "jit-compiler".to_string()
}

fn default_cargo_exe() -> String {
    "cargo".to_string()
}

fn default_solver() -> String {
    "rk4".to_string()
}

fn default_t_end() -> f64 {
    10.0
}

fn default_dt() -> f64 {
    0.01
}

fn default_cargo_run_max_attempts() -> usize {
    3
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExecutionConfig {
    #[serde(default = "default_workers")]
    pub workers: usize,
    #[serde(default)]
    pub fail_fast: bool,
}

fn default_workers() -> usize {
    4
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum IncrementalStrategy {
    #[default]
    None,
    RerunFailed,
    SkipUnchanged,
    /// Only cases listed in `regress_manifest.json` (intersection with current config); full run within that scope.
    LastStructure,
    /// Same scope as `last_structure`, then only rerun cases that did not pass in the baseline report.
    LastStructureRerunFailed,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct IncrementalConfig {
    /// Path to previous report.json (relative to cwd or absolute).
    #[serde(default)]
    pub baseline_path: Option<String>,
    /// Path to `regress_manifest.json` from a prior run (defaults to `{data_root}/regress_manifest.json` when using `--data-root`).
    #[serde(default)]
    pub manifest_path: Option<String>,
    #[serde(default)]
    pub strategy: IncrementalStrategy,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct TierSpec {
    #[serde(default)]
    pub extends: Option<String>,
    #[serde(default)]
    pub include_tags: Vec<String>,
    #[serde(default)]
    pub case_ids: Vec<String>,
    #[serde(default)]
    pub include_globs: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CaseKind {
    Model,
    Mos,
    CustomCommand,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MosRunMode {
    /// `cargo run -- --script=...` from working_dir (default).
    CargoRunScript,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct ExpectDef {
    #[serde(default = "default_expect_kind")]
    pub kind: ExpectKind,
    #[serde(default)]
    pub code: Option<i32>,
}

fn default_expect_kind() -> ExpectKind {
    ExpectKind::ExitZero
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum ExpectKind {
    #[default]
    ExitZero,
    NonZero,
    ExitCode,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct OmcCompareDef {
    /// Path to reference CSV (relative to repo_root or absolute).
    pub reference_csv: String,
    #[serde(default = "default_max_diff")]
    pub max_abs_diff: f64,
}

fn default_max_diff() -> f64 {
    1e-9
}

#[derive(Debug, Clone, Deserialize)]
pub struct CaseDef {
    pub id: String,
    #[serde(default = "default_case_kind")]
    pub kind: CaseKind,
    /// Model path, .mos path relative to working_dir, or ignored for custom_command.
    pub target: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub tier: Option<String>,
    #[serde(default)]
    pub expect: ExpectDef,
    #[serde(default)]
    pub extra_rust_args: Vec<String>,
    #[serde(default)]
    pub timeout_ms: Option<u64>,
    #[serde(default)]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub omc_compare: Option<OmcCompareDef>,
    /// Per-case environment variables (merged on top of defaults.env).
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// For custom_command: program (searched on PATH).
    #[serde(default)]
    pub program: Option<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default = "default_mos_mode")]
    pub mos_mode: MosRunMode,
}

fn default_case_kind() -> CaseKind {
    CaseKind::Model
}

fn default_mos_mode() -> MosRunMode {
    MosRunMode::CargoRunScript
}

pub fn load_config(path: &Path) -> Result<HarnessConfig, ConfigError> {
    let mut stack = Vec::new();
    let cfg = load_config_recursive(path, &mut stack)?;
    validate_unique_ids(&cfg)?;
    Ok(cfg)
}

fn load_config_recursive(path: &Path, stack: &mut Vec<PathBuf>) -> Result<HarnessConfig, ConfigError> {
    let canonical_hint = path.to_path_buf();
    stack.push(canonical_hint.clone());

    let text = std::fs::read_to_string(path)?;
    let mut cfg: HarnessConfig = serde_json::from_str(&text)?;
    if cfg.version != 1 {
        return Err(ConfigError::UnsupportedVersion(cfg.version));
    }

    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));

    // Merge includes first, then overlay current cfg.
    if !cfg.includes.is_empty() {
        let mut merged = HarnessConfig {
            version: 1,
            includes: Vec::new(),
            defaults: Defaults::default(),
            execution: ExecutionConfig::default(),
            incremental: IncrementalConfig::default(),
            tiers: HashMap::new(),
            cases: Vec::new(),
        };

        for inc in cfg.includes.clone() {
            let inc_path = resolve_include_path(base_dir, &inc);
            let child = load_config_recursive(&inc_path, stack)?;

            // Merge child into merged. (child overlays earlier includes)
            merged.defaults = child.defaults;
            merged.execution = child.execution;
            merged.incremental = child.incremental;
            merged.tiers.extend(child.tiers);
            merged.cases.extend(child.cases);
        }

        // Overlay current config on top.
        merged.defaults = cfg.defaults;
        merged.execution = cfg.execution;
        merged.incremental = cfg.incremental;
        merged.tiers.extend(cfg.tiers);
        merged.cases.extend(cfg.cases);

        cfg = merged;
    }

    stack.pop();
    Ok(cfg)
}

fn resolve_include_path(base_dir: &Path, inc: &str) -> PathBuf {
    let p = Path::new(inc);
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        base_dir.join(p)
    }
}

fn validate_unique_ids(cfg: &HarnessConfig) -> Result<(), ConfigError> {
    let mut seen = std::collections::HashSet::new();
    for c in &cfg.cases {
        if !seen.insert(c.id.clone()) {
            return Err(ConfigError::DuplicateCaseId(c.id.clone()));
        }
    }
    Ok(())
}

impl HarnessConfig {
    /// Relative `repo_root` is resolved against the repository root when possible.
    pub fn resolve_repo_root(&self) -> PathBuf {
        let p = Path::new(&self.defaults.repo_root);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            discover_repo_root()
                .unwrap_or_else(|_| {
                    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                })
                .join(p)
        }
    }
}

fn discover_repo_root() -> anyhow::Result<PathBuf> {
    let mut dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|x| x.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    for _ in 0..24 {
        if dir.join(".git").is_dir() {
            return Ok(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    Ok(std::env::current_dir()?)
}
