use crate::analysis::ProvenanceIndex;
use crate::ast::{ClassItem, Model};
use crate::cache::cache_key::{CacheKeyV2, CacheStage, CompileFlagsKey};
use crate::cache::cache_scope::{classify_model_scope, CacheScope};
use crate::cache::closure_hash;
use crate::loader_compat::{early_compat, EarlyCompat};
use crate::parser;
use crate::flatten::{cache_shm, cache_sqlite, flatten_cache};
use crate::flatten::inheritance_cache_v1::{InheritanceCacheV1, INHERITANCE_CACHE_SCHEMA_V1};
use crate::flatten::flat_cache_v1::DepHashEntry;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::{OnceLock, RwLock};
use xxhash_rust::xxh64::Xxh64;

mod inheritance;
mod cache_v1;
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
pub(crate) use perf::{record_cache_event, CacheEvent};

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
    dir: &Path,
    primary: CacheScope,
    key: &str,
    kind: &str,
) -> Option<Vec<u8>> {
    for scope in crate::cache::cache_scope::sqlite_scope_lookup_chain(primary) {
        let Some(cfg) = cache_sqlite::sqlite_config_for_scope(scope, Some(dir)) else {
            continue;
        };
        if let Ok(Some(bytes)) = cache_sqlite::sqlite_get(&cfg.path, key, kind) {
            return Some(bytes);
        }
    }
    None
}

fn flags_for_query_stage(db: &dyn QueryDb) -> CompileFlagsKey {
    CompileFlagsKey {
        validation_mode: db.validation_mode().as_ref().clone(),
        compile_stop: db.compile_stop().as_ref().clone(),
        coarse_constrainedby_only: db.coarse_constrainedby_only(),
        array_size_policy: 0,
        warnings_level: String::new(),
        target_platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
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
    let mut flags = flags_for_query_stage(db);
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
    let by_path = if path.is_empty() {
        CacheScope::Project
    } else {
        classify_model_scope(Path::new(path))
    };
    if !matches!(by_path, CacheScope::Project) {
        return by_path;
    }
    if model_name.starts_with("Modelica.") {
        return CacheScope::GlobalStd;
    }
    if model_name.starts_with("ModelicaTest.") {
        return CacheScope::UserExt;
    }
    CacheScope::Project
}

#[derive(Default, Debug)]
struct DepCollector {
    files: HashMap<String, String>,
    models: HashSet<String>,
}

thread_local! {
    static DEP_STACK: std::cell::RefCell<Vec<DepCollector>> = std::cell::RefCell::new(Vec::new());
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ReverseDepEntry {
    pub file: String,
    pub content_hash: String,
    pub models: Vec<String>,
}

#[derive(Default)]
struct ReverseDepStore {
    file_to_models: HashMap<String, HashSet<String>>,
    file_hashes: HashMap<String, String>,
}

fn global_reverse_dep_store() -> &'static RwLock<ReverseDepStore> {
    static STORE: OnceLock<RwLock<ReverseDepStore>> = OnceLock::new();
    STORE.get_or_init(|| RwLock::new(ReverseDepStore::default()))
}

pub(super) fn dep_record_file(path: &str, semantic_hash: &str) {
    DEP_STACK.with(|s| {
        let mut st = s.borrow_mut();
        for c in st.iter_mut() {
            c.files
                .entry(path.to_string())
                .or_insert_with(|| semantic_hash.to_string());
            if let Ok(mut store) = global_reverse_dep_store().write() {
                let entry = store
                    .file_to_models
                    .entry(path.to_string())
                    .or_insert_with(HashSet::new);
                for model in &c.models {
                    entry.insert(model.clone());
                }
                store
                    .file_hashes
                    .insert(path.to_string(), semantic_hash.to_string());
            }
        }
    });
}

pub(super) fn dep_record_deps(deps: &[DepHashEntry]) {
    for d in deps {
        dep_record_file(d.path.as_str(), d.content_hash.as_str());
    }
}

pub(super) struct DepScope {
    active: bool,
}

impl DepScope {
    pub(super) fn begin() -> Self {
        DEP_STACK.with(|s| s.borrow_mut().push(DepCollector::default()));
        Self { active: true }
    }

    pub(super) fn begin_for_model(model_name: &str) -> Self {
        DEP_STACK.with(|s| {
            let mut st = s.borrow_mut();
            let mut c = DepCollector::default();
            c.models.insert(model_name.to_string());
            st.push(c);
        });
        Self { active: true }
    }

    pub(super) fn end(mut self) -> Vec<DepHashEntry> {
        self.active = false;
        let mut v = DEP_STACK.with(|s| s.borrow_mut().pop()).unwrap_or_default();
        let mut out: Vec<DepHashEntry> = v
            .files
            .drain()
            .map(|(path, content_hash)| DepHashEntry { path, content_hash })
            .collect();
        out.sort_by(|a, b| a.path.cmp(&b.path));
        out
    }
}

/// Model names associated with the given paths via the reverse dependency store (file path string
/// keys match `Path::display()`). Populated during query/flatten work in **this process**; use for
/// incremental re-validation scope in long-lived hosts (IDE, worker). Not persisted across restarts.
pub fn affected_models(changed_files: &[PathBuf]) -> Vec<String> {
    let mut out: HashSet<String> = HashSet::new();
    if let Ok(store) = global_reverse_dep_store().read() {
        for p in changed_files {
            let key = p.display().to_string();
            if let Some(models) = store.file_to_models.get(&key) {
                out.extend(models.iter().cloned());
            }
        }
    }
    let mut v: Vec<String> = out.into_iter().collect();
    v.sort();
    v
}

pub fn reverse_dep_snapshot() -> Vec<ReverseDepEntry> {
    if let Ok(store) = global_reverse_dep_store().read() {
        let mut out: Vec<ReverseDepEntry> = store
            .file_to_models
            .iter()
            .map(|(file, models)| {
                let mut models_v: Vec<String> = models.iter().cloned().collect();
                models_v.sort();
                ReverseDepEntry {
                    file: file.clone(),
                    content_hash: store.file_hashes.get(file).cloned().unwrap_or_default(),
                    models: models_v,
                }
            })
            .collect();
        out.sort_by(|a, b| a.file.cmp(&b.file));
        return out;
    }
    Vec::new()
}

impl Drop for DepScope {
    fn drop(&mut self) {
        if self.active {
            DEP_STACK.with(|s| {
                let _ = s.borrow_mut().pop();
            });
        }
    }
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
