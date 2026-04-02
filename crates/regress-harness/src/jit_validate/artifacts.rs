use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunManifest {
    pub schema_version: u32,
    pub generated_at: String,
    pub repo_root: String,
    pub git: GitInfo,
    pub host: HostInfo,
    pub exe_path: String,
    pub lib_paths: Vec<String>,
    pub models: Vec<String>,
    pub validate_tier: String,
    pub validation_mode: String,
    pub trace: TraceFlags,
    pub scenarios: Vec<ScenarioResolved>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GitInfo {
    pub head: Option<String>,
    pub branch: Option<String>,
    pub dirty: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostInfo {
    pub os: Option<String>,
    pub arch: Option<String>,
    pub hostname: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TraceFlags {
    pub stage_trace: bool,
    pub perf_trace: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScenarioResolved {
    pub id: String,
    pub runs: usize,
    pub cache_dir: String,
    pub env_set: BTreeMap<String, String>,
    pub env_unset: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatePerfReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub out_dir: String,
    pub summary: Summary,
    pub cases: Vec<Case>,
    #[serde(default)]
    pub stats: PerfStats,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PerfStats {
    /// scenario -> model -> stats
    pub by_scenario: BTreeMap<String, BTreeMap<String, ModelPerfStats>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelPerfStats {
    pub runs: usize,
    pub duration_ms_min: Option<u64>,
    pub duration_ms_max: Option<u64>,
    /// Parsed from perf_json when present.
    pub flatten_inline_ms_min: Option<u64>,
    pub flatten_inline_ms_max: Option<u64>,
    pub flatten_wall_ms_min: Option<u64>,
    pub flatten_wall_ms_max: Option<u64>,
    pub inline_wall_ms_min: Option<u64>,
    pub inline_wall_ms_max: Option<u64>,
    pub decl_expand_ms_min: Option<u64>,
    pub decl_expand_ms_max: Option<u64>,
    pub eq_expand_ms_min: Option<u64>,
    pub eq_expand_ms_max: Option<u64>,
    #[serde(default)]
    pub inline_substitute_ms_min: Option<u64>,
    #[serde(default)]
    pub inline_substitute_ms_max: Option<u64>,
    #[serde(default)]
    pub inline_load_model_ms_min: Option<u64>,
    #[serde(default)]
    pub inline_load_model_ms_max: Option<u64>,
    #[serde(default)]
    pub cache_deserialize_ms_min: Option<u64>,
    #[serde(default)]
    pub cache_deserialize_ms_max: Option<u64>,
    /// Convenience: delta between run1 and best later run (if runs >= 2).
    pub run1_flatten_inline_ms: Option<u64>,
    pub best_after_run1_flatten_inline_ms: Option<u64>,
    pub run1_decl_expand_ms: Option<u64>,
    pub best_after_run1_decl_expand_ms: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Summary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    pub scenario: String,
    pub model: String,
    pub run_index: usize,
    pub success: bool,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub perf_json: Option<String>,
    pub cache_stats_json: Option<String>,
    #[serde(default)]
    pub dep_graph_json: Option<String>,
    pub stdout_path: Option<String>,
    pub stderr_path: Option<String>,
    pub repro: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub env_unset: Vec<String>,
    pub cache_dir: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CasePaths {
    pub perf_json: PathBuf,
    pub cache_stats_json: PathBuf,
    pub dep_graph_json: PathBuf,
    pub stdout_txt: PathBuf,
    pub stderr_txt: PathBuf,
}

