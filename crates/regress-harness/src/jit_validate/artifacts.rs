use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use super::EnvOverlay;

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
    #[serde(default)]
    pub purge_scenario_caches: bool,
    #[serde(default)]
    pub shared_cache_dir: Option<String>,
    #[serde(default)]
    pub force_flatten_full_cache: bool,
    #[serde(default)]
    pub worker_per_scenario: bool,
    #[serde(default)]
    pub child_env: EnvOverlay,
    #[serde(default)]
    pub std_cache_root: Option<String>,
    #[serde(default)]
    pub user_cache_root: Option<String>,
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
    /// Track A (microseconds): `compile_perf.flatten_wall_us`.
    #[serde(default)]
    pub flatten_wall_us_min: Option<u64>,
    #[serde(default)]
    pub flatten_wall_us_max: Option<u64>,
    pub inline_wall_ms_min: Option<u64>,
    pub inline_wall_ms_max: Option<u64>,
    #[serde(default)]
    pub inline_wall_us_min: Option<u64>,
    #[serde(default)]
    pub inline_wall_us_max: Option<u64>,
    /// Track B: `compile_perf.codegen_wall_ms` (same window as `jit_ms`).
    #[serde(default)]
    pub codegen_wall_ms_min: Option<u64>,
    #[serde(default)]
    pub codegen_wall_ms_max: Option<u64>,
    #[serde(default)]
    pub codegen_wall_us_min: Option<u64>,
    #[serde(default)]
    pub codegen_wall_us_max: Option<u64>,
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
    pub stub_compile_ms_min: Option<u64>,
    #[serde(default)]
    pub stub_compile_ms_max: Option<u64>,
    #[serde(default)]
    pub stub_compile_us_min: Option<u64>,
    #[serde(default)]
    pub stub_compile_us_max: Option<u64>,
    #[serde(default)]
    pub clock_partition_scan_ms_min: Option<u64>,
    #[serde(default)]
    pub clock_partition_scan_ms_max: Option<u64>,
    #[serde(default)]
    pub clock_partition_scan_us_min: Option<u64>,
    #[serde(default)]
    pub clock_partition_scan_us_max: Option<u64>,
    #[serde(default)]
    pub parallel_candidate_share_pct_min: Option<f64>,
    #[serde(default)]
    pub parallel_candidate_share_pct_max: Option<f64>,
    #[serde(default)]
    pub cache_deserialize_ms_min: Option<u64>,
    #[serde(default)]
    pub cache_deserialize_ms_max: Option<u64>,
    /// Convenience: delta between run1 and best later run (if runs >= 2).
    pub run1_flatten_inline_ms: Option<u64>,
    pub best_after_run1_flatten_inline_ms: Option<u64>,
    pub run1_decl_expand_ms: Option<u64>,
    pub best_after_run1_decl_expand_ms: Option<u64>,
    #[serde(default)]
    pub cache_layer_stats: Option<BTreeMap<String, LayerStats>>,
    /// Hits per scope (L0/L1/L2) for JIT flatten full cache (`cache_stage_*:Lx:flat_full`).
    #[serde(default)]
    pub cache_flat_full_layer_hits: BTreeMap<String, u64>,
    #[serde(default)]
    pub cache_flat_full_layer_misses: BTreeMap<String, u64>,
    /// Hits per scope for array size merge hint cache (`cache_stage_*:Lx:array_sizes`).
    #[serde(default)]
    pub cache_array_sizes_layer_hits: BTreeMap<String, u64>,
    #[serde(default)]
    pub cache_array_sizes_layer_misses: BTreeMap<String, u64>,
    /// Aggregated `query_cache_counters` from `RUSTMODLICA_CACHE_STATS_JSON` (summed over runs).
    #[serde(default)]
    pub cache_query_counters: BTreeMap<String, u64>,
    /// Summed from `compile_perf.salsa_process_db_*` when `RUSTMODLICA_PERF_SALSA_STATS=1` on the compiler process.
    #[serde(default)]
    pub salsa_process_db_hits_sum: u64,
    #[serde(default)]
    pub salsa_process_db_misses_sum: u64,
    #[serde(default)]
    pub salsa_process_db_evictions_sum: u64,
    /// Best `flat_full` SQLite hit % for scope L0 across runs (from `compile_perf.cache_scope_stage_*`).
    #[serde(default)]
    pub std_flat_full_hit_rate_max: Option<f64>,
    #[serde(default)]
    pub user_flat_full_hit_rate_max: Option<f64>,
    #[serde(default)]
    pub l2_flat_full_hit_rate_max: Option<f64>,
    /// Best-run AOT native load proxy: 100 when `aot_native_load_status` indicates loaded, else 0.
    #[serde(default)]
    pub aot_hit_rate_max: Option<f64>,
    /// Min observed `flatten_wall_ms + codegen_wall_ms` from `compile_perf` (project-tier work proxy).
    #[serde(default)]
    pub project_rebuild_wall_ms_min: Option<u64>,
    #[serde(default)]
    pub cache_warm_ratio_max: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LayerStats {
    pub hits: u64,
    pub misses: u64,
    /// SQLite / cross-process cache rows written (from compile perf `cache_l*_writes`).
    #[serde(default)]
    pub writes: u64,
    pub invalidations: u64,
    pub recompute_reasons: Vec<String>,
    #[serde(default)]
    pub stage_hits: BTreeMap<String, u64>,
    #[serde(default)]
    pub stage_misses: BTreeMap<String, u64>,
    #[serde(default)]
    pub stage_invalidations: BTreeMap<String, u64>,
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

