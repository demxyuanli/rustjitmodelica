use crate::ast::{AlgorithmStatement, Declaration, Equation, Expression};
use crate::flatten::structures::{ClockPartition, FlattenedModel};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const FLAT_CACHE_SCHEMA_V1: &str = "rustmodlica_flat_cache_v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepHashEntry {
    pub path: String,
    pub content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatCacheV1 {
    pub schema: String,
    pub key: String,
    pub model_name: String,
    pub declarations: Vec<Declaration>,
    pub equations: Vec<Equation>,
    pub algorithms: Vec<AlgorithmStatement>,
    pub initial_equations: Vec<Equation>,
    pub initial_algorithms: Vec<AlgorithmStatement>,
    pub connections: Vec<(String, String)>,
    pub conditional_connections: Vec<(Expression, (String, String))>,
    pub instances: HashMap<String, String>,
    pub array_sizes: HashMap<String, usize>,
    pub clocked_var_names: std::collections::HashSet<String>,
    pub clock_partitions: Vec<ClockPartition>,
    pub clock_signal_connections: Vec<(String, String)>,
    pub stream_peer_map: HashMap<String, String>,
    pub deps: Vec<DepHashEntry>,
}

impl FlatCacheV1 {
    pub fn from_flat_model(key: String, model_name: &str, flat: &FlattenedModel, deps: Vec<DepHashEntry>) -> Self {
        Self {
            schema: FLAT_CACHE_SCHEMA_V1.to_string(),
            key,
            model_name: model_name.to_string(),
            declarations: flat.declarations.clone(),
            equations: flat.equations.clone(),
            algorithms: flat.algorithms.clone(),
            initial_equations: flat.initial_equations.clone(),
            initial_algorithms: flat.initial_algorithms.clone(),
            connections: flat.connections.clone(),
            conditional_connections: flat.conditional_connections.clone(),
            instances: flat.instances.clone(),
            array_sizes: flat.array_sizes.clone(),
            clocked_var_names: flat.clocked_var_names.clone(),
            clock_partitions: flat.clock_partitions.clone(),
            clock_signal_connections: flat.clock_signal_connections.clone(),
            stream_peer_map: flat.stream_peer_map.clone(),
            deps,
        }
    }

    pub fn into_flat_model(self) -> FlattenedModel {
        FlattenedModel {
            declarations: self.declarations,
            equations: self.equations,
            algorithms: self.algorithms,
            initial_equations: self.initial_equations,
            initial_algorithms: self.initial_algorithms,
            connections: self.connections,
            conditional_connections: self.conditional_connections,
            instances: self.instances,
            array_sizes: self.array_sizes,
            clocked_var_names: self.clocked_var_names,
            clock_partitions: self.clock_partitions,
            clock_signal_connections: self.clock_signal_connections,
            stream_peer_map: self.stream_peer_map,
            interner: crate::string_intern::StringInterner::new(),
            inst_records: Vec::new(),
            path_to_inst: HashMap::new(),
        }
    }
}

