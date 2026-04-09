use crate::ast::{flat_index_suffix_for_scalar_name, Expression};
use std::borrow::Cow;
use std::collections::HashMap;

use super::expressions::{eval_const_expr, expr_to_path};

mod cache;
pub use cache::SubstituteCache;

include!("flattener_impl_early.rs");
include!("flattener_impl_late.rs");
