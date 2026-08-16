use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::analysis::{collect_states_from_eq, extract_unknowns, extract_unknowns_from_algorithm, collect_vars_expr};
use crate::ast::{AlgorithmStatement, Declaration, Equation, Expression};
use crate::compiler::initial_conditions;
use crate::flatten::{eval_const_expr_with_params, FlattenedModel};
use crate::jit::{ArrayInfo, ArrayType};

use super::geometric_default::geometric_default_for_name;
use super::trace::log_stage_timing;
use super::types::VariableLayout;

include!("classify_demote.rs");

include!("classify_body.rs");
