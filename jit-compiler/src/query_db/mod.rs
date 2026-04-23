use crate::analysis::ProvenanceIndex;
use crate::ast::{ClassItem, Model};
use crate::cache::cache_key::{CacheKeyV2, CacheStage, CompileFlagsKey};
use crate::cache::cache_scope::{classify_model_scope_with_heuristics, CacheScope};
use crate::cache::closure_hash;
use crate::loader_compat::{early_compat, EarlyCompat};
use crate::parser;
use crate::flatten::{cache_shm, cache_sqlite};
use crate::flatten::inheritance_cache_v1::{InheritanceCacheV1, INHERITANCE_CACHE_SCHEMA_V1};
use crate::flatten::flat_cache_v1::DepHashEntry;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::{OnceLock, RwLock};
use xxhash_rust::xxh64::Xxh64;

mod inheritance;
mod cache_v1;
mod dep_graph;
mod decl_expand;
mod eq_expand;
mod flat_model;
mod provenance_q;
pub(crate) mod constrainedby;
mod perf;
pub mod salsa_session;

pub use perf::{reset as perf_reset, snapshot as perf_snapshot};
pub use perf::record_add as perf_record_add;
pub use perf::record_us as perf_record_us;
pub use dep_graph::{affected_models, impact_radius, reverse_dep_snapshot, ReverseDepEntry};
pub(crate) use perf::{record_cache_event, CacheEvent};
pub(super) use dep_graph::{dep_record_deps, dep_record_file, DepScope};

use cache_v1::{ParseCacheV1, ModelAstCacheV1, PARSE_CACHE_SCHEMA_V1, MODEL_AST_CACHE_SCHEMA_V1};

#[derive(Clone, Debug)]
enum QueryCachePolicy {
    Disabled,
    Enabled { namespace: String },
}

fn query_cache_policy() -> QueryCachePolicy {
    let disabled = std::env::var("RUSTMODLICA_QUERY_CACHE")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "0" || t.eq_ignore_ascii_case("false") || t.eq_ignore_ascii_case("no")
        })
        .unwrap_or(false);
    if disabled {
        return QueryCachePolicy::Disabled;
    }
    let ns = std::env::var("RUSTMODLICA_QUERY_CACHE_NAMESPACE")
        .ok()
        .map(|s| s.trim().to_string())
        .unwrap_or_default();
    QueryCachePolicy::Enabled { namespace: ns }
}

fn runtime_boundary_epoch() -> String {
    std::env::var("RUSTMODLICA_RUNTIME_BOUNDARY_EPOCH")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "1".to_string())
}

pub(super) fn key_with_policy(qualified_key: String) -> (bool, String) {
    let keyed = format!("epoch={}:{}", runtime_boundary_epoch(), qualified_key);
    match query_cache_policy() {
        QueryCachePolicy::Disabled => (false, keyed),
        QueryCachePolicy::Enabled { namespace } => {
            if namespace.is_empty() {
                (true, keyed)
            } else {
                (true, format!("{}:{}", namespace, keyed))
            }
        }
    }
}

pub(super) fn sqlite_get_with_scope_chain(
    primary: CacheScope,
    key: &str,
    kind: &str,
) -> Option<Vec<u8>> {
    for cfg in cache_sqlite::sqlite_read_try_configs(primary) {
        if let Ok(Some(bytes)) = cache_sqlite::sqlite_get(&cfg.path, key, kind) {
            return Some(bytes);
        }
    }
    None
}

fn flags_for_query_stage(db: &dyn QueryDb, model_name: &str) -> CompileFlagsKey {
    let target_platform = if crate::cache::msl_pack::context::is_active()
        && model_name.starts_with("Modelica.")
    {
        "msl-pack".to_string()
    } else {
        format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
    };
    CompileFlagsKey {
        validation_mode: db.validation_mode().as_ref().clone(),
        compile_stop: db.compile_stop().as_ref().clone(),
        coarse_constrainedby_only: db.coarse_constrainedby_only(),
        array_size_policy: 0,
        warnings_level: String::new(),
        target_platform,
    }
}

/// Qualified storage key for [`flat_model::flattened_model_q`] (FlatModelQ stage). Shared with
/// [`salsa_session`] so process-local DB reuse matches on-disk / SHM cache keys.
pub(super) fn flat_model_q_cache_key(
    db: &dyn QueryDb,
    model_name: String,
) -> (bool, String, CacheScope) {
    let libs = db.library_paths();
    let coarse = db.coarse_constrainedby_only();
    let st = db.source_text(model_name.clone());
    let root_hash = semantic_hash_text(st.text.as_str());
    let scope = scope_from_path(st.path.as_str(), model_name.as_str());
    let mut flags = flags_for_query_stage(db, model_name.as_str());
    flags.coarse_constrainedby_only = coarse;
    let key_v2 = CacheKeyV2::builder(CacheStage::FlatModelQ, scope.clone(), model_name.as_str())
        .libs_from_path_bufs(libs.as_slice())
        .root_content_hash(root_hash)
        .compile_flags(flags)
        .build();
    let (cache_enabled, key) = key_with_policy(key_v2.to_qualified_key());
    (cache_enabled, key, scope)
}

pub(super) fn scope_from_path(path: &str, model_name: &str) -> CacheScope {
    let p = if path.is_empty() {
        None
    } else {
        Some(Path::new(path))
    };
    classify_model_scope_with_heuristics(p, Some(model_name))
}

#[salsa::query_group(QueryStorage)]
pub trait QueryDb: salsa::Database {
    #[salsa::input]
    fn library_paths(&self) -> Arc<Vec<PathBuf>>;

    #[salsa::input]
    fn coarse_constrainedby_only(&self) -> bool;

    #[salsa::input]
    fn compile_stop(&self) -> Arc<String>;

    #[salsa::input]
    fn validation_mode(&self) -> Arc<String>;

    fn source_text(&self, model_name: String) -> Arc<SourceText>;
    fn parsed_items(&self, model_name: String) -> Arc<ParsedItems>;
    fn model_ast(&self, model_name: String) -> Arc<ModelAst>;
    fn inheritance_flattened(&self, model_name: String) -> ModelPtr;
    fn decl_expanded(&self, model_name: String) -> DeclExpandResPtr;
    fn eq_expanded(&self, model_name: String) -> EqExpandResPtr;
    fn flattened_model_q(&self, model_name: String) -> FlatModelResPtr;
    /// Provenance built from the same flat as `flattened_model_q` (Salsa pipeline; not post-`inline` compile).
    fn provenance_index_q(&self, model_name: String) -> ProvenanceIndexResPtr;
    fn constrainedby_holds_extends_q(&self, input: constrainedby::ConstrainedByInput) -> constrainedby::ConstrainedByResPtr;
}

#[salsa::database(QueryStorage)]
#[derive(Default)]
pub struct Database {
    storage: salsa::Storage<Self>,
}

impl salsa::Database for Database {}

#[derive(Clone, Debug)]
pub struct SourceText {
    pub model_name: String,
    pub text: Arc<String>,
    pub path: Arc<String>,
}

impl PartialEq for SourceText {
    fn eq(&self, other: &Self) -> bool {
        self.model_name == other.model_name
            && Arc::ptr_eq(&self.text, &other.text)
            && Arc::ptr_eq(&self.path, &other.path)
    }
}

impl Eq for SourceText {}

#[derive(Clone, Debug)]
pub struct ParsedItems {
    pub model_name: String,
    pub items: Arc<Vec<ClassItem>>,
    pub path: Arc<String>,
}

impl PartialEq for ParsedItems {
    fn eq(&self, other: &Self) -> bool {
        self.model_name == other.model_name
            && Arc::ptr_eq(&self.items, &other.items)
            && Arc::ptr_eq(&self.path, &other.path)
    }
}

impl Eq for ParsedItems {}

#[derive(Clone, Debug)]
pub struct ModelAst {
    pub model_name: String,
    pub model: Arc<Model>,
    pub path: Arc<String>,
}

impl PartialEq for ModelAst {
    fn eq(&self, other: &Self) -> bool {
        self.model_name == other.model_name
            && Arc::ptr_eq(&self.model, &other.model)
            && Arc::ptr_eq(&self.path, &other.path)
    }
}

impl Eq for ModelAst {}

#[derive(Clone, Debug)]
pub struct ModelPtr(pub Arc<Model>);

impl PartialEq for ModelPtr {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ModelPtr {}

#[derive(Clone, Debug)]
pub struct DeclExpandResPtr(pub Arc<decl_expand::DeclExpandResult>);

impl PartialEq for DeclExpandResPtr {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for DeclExpandResPtr {}

#[derive(Clone, Debug)]
pub struct EqExpandResPtr(pub Arc<eq_expand::EqExpandResult>);

impl PartialEq for EqExpandResPtr {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for EqExpandResPtr {}

#[derive(Clone, Debug)]
pub struct FlatModelResPtr(pub Arc<flat_model::FlatModelResult>);

impl PartialEq for FlatModelResPtr {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for FlatModelResPtr {}

/// Output of [`QueryDb::provenance_index_q`].
#[derive(Clone, Debug)]
pub struct ProvenanceQResult {
    pub index: Option<Arc<ProvenanceIndex>>,
    pub err: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ProvenanceIndexResPtr(pub Arc<ProvenanceQResult>);

impl PartialEq for ProvenanceIndexResPtr {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ProvenanceIndexResPtr {}

include!("mod_queries_tail.rs");
