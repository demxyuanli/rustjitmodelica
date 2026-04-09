use crate::flatten::flat_cache_v1::DepHashEntry;
use crate::jit::codegen_cache::CodegenCacheKey;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompiledArtifactBundle {
    pub schema_version: u32,
    pub model_name: String,
    pub codegen_key: CodegenCacheKey,
    pub deps: Vec<DepHashEntry>,
    pub libs_fingerprint: String,
    pub compile_flags_hash: String,
    pub when_count: usize,
    pub crossings_count: usize,
    pub state_vars: Vec<String>,
    pub discrete_vars: Vec<String>,
    pub param_vars: Vec<String>,
    pub output_vars: Vec<String>,
    pub state_var_index: std::collections::HashMap<String, usize>,
    pub output_start_vals: Vec<f64>,
    pub params: Vec<f64>,
    pub t_end: f64,
    pub dt: f64,
    pub atol: f64,
    pub rtol: f64,
    pub differential_index: u32,
    pub ida_component_id: Vec<f64>,
    pub solver: String,
    pub output_interval: f64,
    pub result_file: Option<String>,
    pub artifact_kind: String,
    #[serde(default)]
    pub clock_partitions_json: String,
    #[serde(default)]
    pub clock_partition_schedule_json: String,
}

