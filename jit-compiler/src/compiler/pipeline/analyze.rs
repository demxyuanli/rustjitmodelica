use std::collections::HashSet;
use std::time::Instant;

use crate::analysis::{sort_algebraic_equations, AnalysisOptions};
use crate::ast::{Equation, Expression};
use crate::compiler::{jacobian, CompilerOptions};
use crate::flatten::FlattenedModel;

use super::normalize_eq::{
    build_diff_equations, ensure_derivative_outputs, normalize_equations,
};
use super::trace::log_stage_timing;
use super::types::{AnalysisStage, VariableLayout};

include!("analyze_body.rs");
