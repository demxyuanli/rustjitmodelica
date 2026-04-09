use crate::ast::{AlgorithmStatement, Equation, Expression};
use crate::compiler::inline::is_builtin_function;
use std::collections::HashMap;
use std::sync::OnceLock;

use super::expressions::{eval_const_expr, expr_to_path, index_expression, prefix_expression};
use super::utils::{convert_eq_to_alg, get_function_outputs};
use super::ExpandTarget;

mod helpers;

include!("flattener_impl_early.rs");
include!("flattener_impl_late.rs");
