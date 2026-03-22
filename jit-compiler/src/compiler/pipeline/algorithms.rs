use std::collections::HashSet;
use std::time::Instant;

use crate::analysis::normalize_der;
use crate::ast::{AlgorithmStatement, Equation, Expression};
use crate::compiler::equation_convert;
use crate::flatten::FlattenedModel;

use super::normalize_eq::normalize_simple_equation;
use super::trace::log_stage_timing;

include!("algorithms_body.rs");
