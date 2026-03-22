use crate::ast::{AlgorithmStatement, Declaration, Equation, Expression, StringInterner};
use std::collections::{HashMap, HashSet};

/// SYNC-2: One clock partition: variables that are updated on the same clock (e.g. same when sample(...) branch).
#[derive(Debug, Clone, Default)]
pub struct ClockPartition {
    /// Stable id for this partition (e.g. "default", or derived from clock condition).
    pub id: String,
    /// Variable names (flattened) in this partition.
    pub var_names: HashSet<String>,
}

pub struct FlattenedModel {
    pub declarations: Vec<Declaration>,
    pub equations: Vec<Equation>,
    pub algorithms: Vec<AlgorithmStatement>,
    pub initial_equations: Vec<Equation>,
    pub initial_algorithms: Vec<AlgorithmStatement>,
    pub connections: Vec<(String, String)>,
    /// F4-1: connect() inside when; (condition, (a_path, b_path))
    pub conditional_connections: Vec<(Expression, (String, String))>,
    pub instances: HashMap<String, String>, // full_path -> type_name
    pub array_sizes: HashMap<String, usize>, // full_path -> size
    /// SYNC-2: Union of all clocked variable names (for backward compat and quick lookup).
    #[allow(dead_code)]
    pub clocked_var_names: HashSet<String>,
    /// SYNC-2: Per-clock partitions; used by solver/jacobian for clocked state handling.
    pub clock_partitions: Vec<ClockPartition>,
    /// SYNC-6: Pairs of connector instance paths when both sides are clock connectors (infer same clock network).
    pub clock_signal_connections: Vec<(String, String)>,
    pub interner: StringInterner,
}
