use std::collections::HashMap;

use crate::ast::{Equation, Expression};
use crate::compiler::jacobian;
use crate::flatten::FlattenedModel;
use crate::jit::ArrayInfo;

pub(crate) type CompilerResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub(crate) struct FrontendStage {
    pub flat_model: FlattenedModel,
    pub total_equations: usize,
    pub total_declarations: usize,
}

pub(crate) struct VariableLayout {
    pub states: Vec<f64>,
    pub discrete_vals: Vec<f64>,
    pub params: Vec<f64>,
    pub state_vars: Vec<String>,
    pub discrete_vars: Vec<String>,
    pub param_vars: Vec<String>,
    pub output_vars: Vec<String>,
    pub output_start_vals: Vec<f64>,
    pub input_var_names: Vec<String>,
    pub state_var_index: HashMap<String, usize>,
    pub param_var_index: HashMap<String, usize>,
    pub output_var_index: HashMap<String, usize>,
    pub array_info: HashMap<String, ArrayInfo>,
}

pub(crate) struct AnalysisStage {
    pub alg_equations: Vec<Equation>,
    pub diff_equations: Vec<Equation>,
    pub differential_index: u32,
    pub constraint_equation_count: usize,
    pub constant_conflict_count: usize,
    pub numeric_ode_jacobian: bool,
    pub symbolic_ode_jacobian_matrix: Option<Vec<Vec<Expression>>>,
    pub ode_jacobian_sparse: Option<jacobian::SparseOdeJacobian>,
}
