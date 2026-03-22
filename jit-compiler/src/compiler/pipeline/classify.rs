use std::collections::{HashMap, HashSet};
use std::time::Instant;

use crate::analysis::{collect_states_from_eq, extract_unknowns};
use crate::ast::{AlgorithmStatement, Equation, Expression};
use crate::compiler::initial_conditions;
use crate::flatten::{eval_const_expr_with_params, FlattenedModel};
use crate::jit::{ArrayInfo, ArrayType};

use super::geometric_default::geometric_default_for_name;
use super::trace::log_stage_timing;
use super::types::VariableLayout;

include!("classify_body.rs");
