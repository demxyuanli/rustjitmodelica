use crate::flatten::flat_cache_v1::DepHashEntry;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::flatten::structures::{InstId, InstPathRecord};
use crate::ast::{Declaration, Equation};

pub const DECL_EXPAND_CACHE_SCHEMA_V1: &str = "rustmodlica_decl_expand_cache_v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclExpandResult {
    pub out: Option<DeclExpandOut>,
    pub err: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclExpandOut {
    pub declarations: Vec<Declaration>,
    pub equations: Vec<Equation>,
    pub instances: HashMap<String, String>,
    pub array_sizes: HashMap<String, usize>,
    pub inst_records: Vec<InstPathRecord>,
    pub path_to_inst: HashMap<String, InstId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeclExpandCacheV1 {
    pub schema: String,
    pub key: String,
    pub model_name: String,
    pub out: Option<DeclExpandOut>,
    pub err: Option<String>,
    pub deps: Vec<DepHashEntry>,
}

// Intentionally no standalone helpers here yet; cache read/write lives in the query implementation.


