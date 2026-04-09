use crate::ast::{Equation, Expression};
use std::collections::HashMap;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BlockCausalityInfo {
    pub diff_index: u32,
    pub tearing_vars: Vec<String>,
    pub strongly_connected: bool,
    pub is_nonlinear: bool,
}

#[derive(Debug, Clone)]
pub struct SortAlgebraicResult {
    pub sorted_equations: Vec<Equation>,
    pub differential_index: u32,
    pub constraint_equation_count: usize,
    pub constant_conflict_count: usize,
    pub alias_map: HashMap<String, Expression>,
    pub index_reduction_rounds: u32,
    pub dummy_derivative_equation_count: usize,
    pub tearing_block_count: usize,
    pub tearing_residual_equation_count: usize,
    pub block_causality: Vec<BlockCausalityInfo>,
    pub blt_degrade_guard_triggered: bool,
    pub blt_degrade_guard_limit: Option<usize>,
    pub blt_degrade_guard_equation_count: Option<usize>,
}
