use crate::ast::{Declaration, Expression, Model};
use crate::diag::SourceLocation;
use crate::loader::LoadError;
use crate::flatten::utils::{is_primitive, resolve_inner_class_alias, resolve_type_alias};
use crate::flatten::{apply_modification_to_model, ModifyContext};
use crate::flatten::{eval_const_expr_with_param_exprs, index_expression, FlattenError};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

mod array_dim;
mod env;
mod param_pass;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ExpandDeclMode {
    /// SuperFast: Only top-level declarations, no recursive sub-model loading
    SuperFast,
    /// DeclOnly: Expand declarations but skip sub-equations
    DeclOnly,
    /// DeclAndSubEq: Full expansion including sub-model equations
    DeclAndSubEq,
}

include!("flattener_impl.rs");
