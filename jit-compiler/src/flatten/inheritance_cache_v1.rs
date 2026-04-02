use crate::ast::Model;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::flat_cache_v1::DepHashEntry;

pub const INHERITANCE_CACHE_SCHEMA_V1: &str = "rustmodlica_inheritance_cache_v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InheritanceCacheV1 {
    pub schema: String,
    pub key: String,
    pub model_name: String,
    pub model: Model,
    pub deps: Vec<DepHashEntry>,
}

impl InheritanceCacheV1 {
    pub fn new(key: String, model_name: &str, model: Arc<Model>, deps: Vec<DepHashEntry>) -> Self {
        Self {
            schema: INHERITANCE_CACHE_SCHEMA_V1.to_string(),
            key,
            model_name: model_name.to_string(),
            model: (*model).clone(),
            deps,
        }
    }

    pub fn into_model_arc(self) -> Arc<Model> {
        Arc::new(self.model)
    }
}

