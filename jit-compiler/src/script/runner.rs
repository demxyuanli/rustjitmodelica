// See parse.rs for script line parsing.

use crate::compiler::Compiler;

#[derive(Debug, Clone)]
pub(crate) enum MosValue {
    Number(f64),
    String(String),
    Bool(bool),
    Array(Vec<MosValue>),
}

impl MosValue {
    pub(crate) fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Number(v) => Some(*v),
            _ => None,
        }
    }
    pub(crate) fn as_string(&self) -> Option<String> {
        match self {
            Self::String(v) => Some(v.clone()),
            _ => None,
        }
    }
    pub(crate) fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            Self::Number(v) => Some(*v != 0.0),
            _ => None,
        }
    }
}

pub struct ScriptRunner {
    pub compiler: Compiler,
    /// SCRIPT-5: multiple loaded models by name; current model key for setParameter/simulate etc.
    pub artifacts_map: std::collections::HashMap<String, crate::compiler::Artifacts>,
    pub current_model: Option<String>,
    pub(crate) mos_vars: std::collections::HashMap<String, MosValue>,
    pub(crate) last_error: String,
}
