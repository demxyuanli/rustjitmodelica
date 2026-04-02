use crate::ast::{ClassItem, Model};
use crate::flatten::flat_cache_v1::DepHashEntry;
use serde::{Deserialize, Serialize};

pub const PARSE_CACHE_SCHEMA_V1: &str = "rustmodlica_parse_cache_v1";
pub const MODEL_AST_CACHE_SCHEMA_V1: &str = "rustmodlica_model_ast_cache_v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParseCacheV1 {
    pub schema: String,
    pub key: String,
    pub model_name: String,
    pub path: String,
    pub items: Vec<ClassItem>,
    pub deps: Vec<DepHashEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelAstCacheV1 {
    pub schema: String,
    pub key: String,
    pub model_name: String,
    pub path: String,
    pub model: Model,
    pub deps: Vec<DepHashEntry>,
}

