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
pub(crate) mod constrainedby;
mod perf;

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

#[allow(dead_code)]
#[deprecated(note = "use key_with_policy with fully-qualified v2 key")]
pub(super) fn query_cache_key(raw_key: &str) -> (bool, String) {
    match query_cache_policy() {
        QueryCachePolicy::Disabled => (false, raw_key.to_string()),
        QueryCachePolicy::Enabled { namespace } => {
            if namespace.is_empty() {
                (true, raw_key.to_string())
            } else {
                (true, format!("{}:{}", namespace, raw_key))
            }
        }
    }
}

pub(super) fn key_with_policy(qualified_key: String) -> (bool, String) {
    match query_cache_policy() {
        QueryCachePolicy::Disabled => (false, qualified_key),
        QueryCachePolicy::Enabled { namespace } => {
            if namespace.is_empty() {
                (true, qualified_key)
            } else {
                (true, format!("{}:{}", namespace, qualified_key))
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

fn normalize_modelica_source_for_hash(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_block_comment = false;
    let mut in_line_comment = false;
    let mut in_string = false;
    while let Some(c) = chars.next() {
        if in_block_comment {
            if c == '*' && chars.peek() == Some(&'/') {
                let _ = chars.next();
                in_block_comment = false;
            }
            continue;
        }
        if in_line_comment {
            if c == '\n' {
                in_line_comment = false;
                out.push('\n');
            }
            continue;
        }
        if in_string {
            out.push(c);
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    // Escaped quote "" inside string.
                    out.push('"');
                    let _ = chars.next();
                } else {
                    in_string = false;
                }
            }
            continue;
        }
        if c == '"' {
            in_string = true;
            out.push(c);
            continue;
        }
        if c == '/' {
            if chars.peek() == Some(&'/') {
                let _ = chars.next();
                in_line_comment = true;
                continue;
            }
            if chars.peek() == Some(&'*') {
                let _ = chars.next();
                in_block_comment = true;
                continue;
            }
        }
        out.push(c);
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub(crate) fn semantic_hash_text(text: &str) -> String {
    let normalized = normalize_modelica_source_for_hash(text);
    let mut h = Xxh64::new(0);
    h.update(normalized.as_bytes());
    format!("{:016x}", h.digest())
}

fn candidate_paths_for_model(model_name: &str) -> Vec<PathBuf> {
    let rel = model_name.replace('.', "/");
    vec![
        PathBuf::from(format!("{}/package.mo", rel)),
        PathBuf::from(format!("{}.mo", rel)),
    ]
}

fn try_read_from_libs(libs: &[PathBuf], model_name: &str) -> Option<(String, String)> {
    let filenames = candidate_paths_for_model(model_name);
    for lib in libs {
        for f in &filenames {
            let full = lib.join(f);
            if full.is_file() {
                let text = std::fs::read_to_string(&full).ok()?;
                return Some((full.display().to_string(), text));
            }
        }
    }
    None
}

fn source_text(db: &dyn QueryDb, model_name: String) -> Arc<SourceText> {
    let libs = db.library_paths();
    let mut candidates: Vec<String> = vec![model_name.clone()];
    match early_compat(&model_name) {
        EarlyCompat::None => {}
        EarlyCompat::Hard(ts) | EarlyCompat::Soft(ts) => {
            candidates.extend(ts);
        }
    }
    for c in candidates {
        if let Some((path, text)) = try_read_from_libs(libs.as_slice(), &c) {
            let h = semantic_hash_text(text.as_str());
            dep_record_file(path.as_str(), h.as_str());
            return Arc::new(SourceText {
                model_name: model_name.clone(),
                text: Arc::new(text),
                path: Arc::new(path),
            });
        }
    }
    // Not found: store empty to keep query total, caller will handle as load error later.
    Arc::new(SourceText {
        model_name,
        text: Arc::new(String::new()),
        path: Arc::new(String::new()),
    })
}

fn parsed_items(db: &dyn QueryDb, model_name: String) -> Arc<ParsedItems> {
    let wall = std::time::Instant::now();
    let deps_scope = DepScope::begin_for_model(model_name.as_str());
    let st = db.source_text(model_name.clone());
    let libs = db.library_paths();
    let _coarse = db.coarse_constrainedby_only();

    let root_hash = semantic_hash_text(st.text.as_str());
    let scope = scope_from_path(st.path.as_str(), model_name.as_str());
    let key_v2 = CacheKeyV2::builder(CacheStage::Parse, scope.clone(), model_name.as_str())
        .libs_from_path_bufs(libs.as_slice())
        .root_content_hash(root_hash)
        .compile_flags(flags_for_query_stage(db))
        .build();
    let (cache_enabled, key) = key_with_policy(key_v2.to_qualified_key());

    if cache_enabled {
        if let Some(bytes) = cache_shm::shm_get(key.as_str()) {
            if let Ok(cache) = bincode::deserialize::<ParseCacheV1>(&bytes) {
                if cache.schema == PARSE_CACHE_SCHEMA_V1
                    && cache.key == key
                    && cache_deps_match_for_stage(&scope, "parse", &cache.deps)
                {
                    perf::record_cache_event(scope.prefix(), "parse", perf::CacheEvent::Hit);
                    dep_record_deps(&cache.deps);
                    perf::record_us("parse_us", wall.elapsed().as_micros() as u64);
                    return Arc::new(ParsedItems {
                        model_name,
                        items: Arc::new(cache.items),
                        path: Arc::new(cache.path),
                    });
                }
            }
        }
        if let Some(dir) = flatten_cache::flatten_cache_dir() {
            if let Some(bytes) = sqlite_get_with_scope_chain(
                dir.as_path(),
                scope.clone(),
                key.as_str(),
                "parse_v1",
            ) {
                if let Ok(cache) = bincode::deserialize::<ParseCacheV1>(&bytes) {
                    if cache.schema == PARSE_CACHE_SCHEMA_V1
                        && cache.key == key
                        && cache_deps_match_for_stage(&scope, "parse", &cache.deps)
                    {
                        perf::record_cache_event(scope.prefix(), "parse", perf::CacheEvent::Hit);
                        let _ = cache_shm::shm_put(key.as_str(), &bytes);
                        dep_record_deps(&cache.deps);
                        perf::record_us("parse_us", wall.elapsed().as_micros() as u64);
                        return Arc::new(ParsedItems {
                            model_name,
                            items: Arc::new(cache.items),
                            path: Arc::new(cache.path),
                        });
                    }
                }
            }
        }
    }

    let items = match parser::parse_all(st.text.as_str()) {
        Ok(v) => v,
        Err(_e) => Vec::new(),
    };
    let deps = deps_scope.end();
    perf::record_cache_event(scope.prefix(), "parse", perf::CacheEvent::Miss);
    if cache_enabled {
        if let Some(dir) = flatten_cache::flatten_cache_dir() {
            if let Some(cfg) = cache_sqlite::sqlite_config_for_scope(scope.clone(), Some(dir.as_path())) {
                let cache = ParseCacheV1 {
                    schema: PARSE_CACHE_SCHEMA_V1.to_string(),
                    key: key.clone(),
                    model_name: model_name.clone(),
                    path: st.path.to_string(),
                    items: items.clone(),
                    deps: deps.clone(),
                };
                if let Ok(bytes) = bincode::serialize(&cache) {
                    perf::record_cache_event(scope.prefix(), "parse", perf::CacheEvent::Write);
                    let _ = cache_shm::shm_put(key.as_str(), &bytes);
                    let deps_json = serde_json::to_string(&deps).ok();
                    let _ = cache_sqlite::sqlite_put(
                        &cfg.path,
                        key.as_str(),
                        PARSE_CACHE_SCHEMA_V1,
                        "parse_v1",
                        &bytes,
                        deps_json.as_deref(),
                    );
                }
            }
        }
    }

    perf::record_us("parse_us", wall.elapsed().as_micros() as u64);
    Arc::new(ParsedItems {
        model_name,
        items: Arc::new(items),
        path: Arc::clone(&st.path),
    })
}

fn model_ast(db: &dyn QueryDb, model_name: String) -> Arc<ModelAst> {
    let deps_scope = DepScope::begin_for_model(model_name.as_str());
    let libs = db.library_paths();
    let pi = db.parsed_items(model_name.clone());
    let items = pi.items.as_ref();
    let st = db.source_text(model_name.clone());
    let root_hash = semantic_hash_text(st.text.as_str());

    let scope = scope_from_path(st.path.as_str(), model_name.as_str());
    let key_v2 = CacheKeyV2::builder(CacheStage::ModelAst, scope.clone(), model_name.as_str())
        .libs_from_path_bufs(libs.as_slice())
        .root_content_hash(root_hash)
        .compile_flags(flags_for_query_stage(db))
        .build();
    let (cache_enabled, key) = key_with_policy(key_v2.to_qualified_key());

    if cache_enabled {
        if let Some(bytes) = cache_shm::shm_get(key.as_str()) {
            if let Ok(cache) = bincode::deserialize::<ModelAstCacheV1>(&bytes) {
                if cache.schema == MODEL_AST_CACHE_SCHEMA_V1
                    && cache.key == key
                    && cache_deps_match_for_stage(&scope, "model_ast", &cache.deps)
                {
                    perf::record_cache_event(scope.prefix(), "model_ast", perf::CacheEvent::Hit);
                    dep_record_deps(&cache.deps);
                    return Arc::new(ModelAst {
                        model_name,
                        model: Arc::new(cache.model),
                        path: Arc::new(cache.path),
                    });
                }
            }
        }
        if let Some(dir) = flatten_cache::flatten_cache_dir() {
            if let Some(bytes) = sqlite_get_with_scope_chain(
                dir.as_path(),
                scope.clone(),
                key.as_str(),
                "model_ast_v1",
            ) {
                if let Ok(cache) = bincode::deserialize::<ModelAstCacheV1>(&bytes) {
                    if cache.schema == MODEL_AST_CACHE_SCHEMA_V1
                        && cache.key == key
                        && cache_deps_match_for_stage(&scope, "model_ast", &cache.deps)
                    {
                        perf::record_cache_event(scope.prefix(), "model_ast", perf::CacheEvent::Hit);
                        let _ = cache_shm::shm_put(key.as_str(), &bytes);
                        dep_record_deps(&cache.deps);
                        return Arc::new(ModelAst {
                            model_name,
                            model: Arc::new(cache.model),
                            path: Arc::new(cache.path),
                        });
                    }
                }
            }
        }
    }

    let short_name = model_name.rsplit('.').next().unwrap_or(model_name.as_str());
    let mut selected_idx = 0usize;
    if let Some((idx, _)) = items.iter().enumerate().find(|(_, it)| match it {
        ClassItem::Model(m) => m.name == short_name,
        ClassItem::Function(f) => f.name == short_name,
    }) {
        selected_idx = idx;
    }
    let model = if items.is_empty() {
        Arc::new(Model {
            name: model_name.clone(),
            is_connector: false,
            is_function: false,
            is_operator_function: false,
            is_record: false,
            is_block: false,
            extends: vec![],
            declarations: vec![],
            equations: vec![],
            algorithms: vec![],
            initial_equations: vec![],
            initial_algorithms: vec![],
            annotation: None,
            inner_classes: vec![],
            inner_class_index: std::collections::HashMap::new(),
            is_operator_record: false,
            type_aliases: vec![],
            imports: vec![],
            external_info: None,
            redeclare_extends: Vec::new(),
        })
    } else {
        match &items[selected_idx] {
            ClassItem::Model(m) => Arc::new(m.clone()),
            ClassItem::Function(f) => Arc::new(crate::ast::Model::from(f.clone())),
        }
    };
    let deps = deps_scope.end();
    perf::record_cache_event(scope.prefix(), "model_ast", perf::CacheEvent::Miss);

    if cache_enabled {
        if let Some(dir) = flatten_cache::flatten_cache_dir() {
            if let Some(cfg) = cache_sqlite::sqlite_config_for_scope(scope.clone(), Some(dir.as_path())) {
                let cache = ModelAstCacheV1 {
                    schema: MODEL_AST_CACHE_SCHEMA_V1.to_string(),
                    key: key.clone(),
                    model_name: model_name.clone(),
                    path: pi.path.to_string(),
                    model: (*model).clone(),
                    deps: deps.clone(),
                };
                if let Ok(bytes) = bincode::serialize(&cache) {
                    perf::record_cache_event(scope.prefix(), "model_ast", perf::CacheEvent::Write);
                    let _ = cache_shm::shm_put(key.as_str(), &bytes);
                    let deps_json = serde_json::to_string(&deps).ok();
                    let _ = cache_sqlite::sqlite_put(
                        &cfg.path,
                        key.as_str(),
                        MODEL_AST_CACHE_SCHEMA_V1,
                        "model_ast_v1",
                        &bytes,
                        deps_json.as_deref(),
                    );
                }
            }
        }
    }

    Arc::new(ModelAst {
        model_name,
        model,
        path: Arc::clone(&pi.path),
    })
}

#[derive(Clone, Debug)]
struct SemanticFileHashEntry {
    modified_ms: u128,
    len: u64,
    hash: String,
}

pub(super) fn semantic_hash_file_cached(path: &Path) -> Option<String> {
    static CACHE: OnceLock<RwLock<std::collections::HashMap<String, SemanticFileHashEntry>>> =
        OnceLock::new();
    let cache = CACHE.get_or_init(|| RwLock::new(std::collections::HashMap::new()));
    let meta = std::fs::metadata(path).ok()?;
    let len = meta.len();
    let modified_ms = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let key = path.to_string_lossy().to_string();

    if let Ok(g) = cache.read() {
        if let Some(e) = g.get(&key) {
            if e.len == len && e.modified_ms == modified_ms {
                return Some(e.hash.clone());
            }
        }
    }

    let hash = closure_hash::unified_file_hash(path)?;
    if let Ok(mut g) = cache.write() {
        g.insert(
            key,
            SemanticFileHashEntry {
                modified_ms,
                len,
                hash: hash.clone(),
            },
        );
    }
    Some(hash)
}

pub(super) fn deps_match(deps: &[DepHashEntry]) -> bool {
    let t0 = std::time::Instant::now();
    let ok = closure_hash::deps_match(deps);
    if !ok {
        perf::record_cache_event("L2", "deps", perf::CacheEvent::DepsMismatch);
    }
    perf::record_us("qcache_deps_match_us", t0.elapsed().as_micros() as u64);
    ok
}

pub(super) fn cache_deps_match_for_stage(scope: &CacheScope, stage: &str, deps: &[DepHashEntry]) -> bool {
    let ok = deps_match(deps);
    if !ok {
        perf::record_cache_event(scope.prefix(), stage, perf::CacheEvent::Invalidate);
    }
    ok
}

fn inheritance_flattened(db: &dyn QueryDb, model_name: String) -> ModelPtr {
    let wall = std::time::Instant::now();
    let deps_scope = DepScope::begin_for_model(model_name.as_str());
    let libs = db.library_paths();
    let coarse = db.coarse_constrainedby_only();
    let st = db.source_text(model_name.clone());
    let root_hash = semantic_hash_text(st.text.as_str());
    let scope = scope_from_path(st.path.as_str(), model_name.as_str());
    let mut flags = flags_for_query_stage(db);
    flags.coarse_constrainedby_only = coarse;
    let key_v2 = CacheKeyV2::builder(CacheStage::Inheritance, scope.clone(), model_name.as_str())
        .libs_from_path_bufs(libs.as_slice())
        .root_content_hash(root_hash)
        .compile_flags(flags)
        .build();
    let (cache_enabled, key) = key_with_policy(key_v2.to_qualified_key());

    if cache_enabled {
        if let Some(bytes) = cache_shm::shm_get(key.as_str()) {
            if let Ok(cache) = bincode::deserialize::<InheritanceCacheV1>(&bytes) {
                if cache.schema == INHERITANCE_CACHE_SCHEMA_V1
                    && cache.key == key
                    && cache_deps_match_for_stage(&scope, "inheritance", &cache.deps)
                {
                    perf::record_cache_event(scope.prefix(), "inheritance", perf::CacheEvent::Hit);
                    dep_record_deps(&cache.deps);
                    perf::record_us("inheritance_us", wall.elapsed().as_micros() as u64);
                    return ModelPtr(cache.into_model_arc());
                }
            }
        }
        if let Some(dir) = flatten_cache::flatten_cache_dir() {
            if let Some(bytes) = sqlite_get_with_scope_chain(
                dir.as_path(),
                scope.clone(),
                key.as_str(),
                "inheritance_v1",
            ) {
                if let Ok(cache) = bincode::deserialize::<InheritanceCacheV1>(&bytes) {
                    if cache.schema == INHERITANCE_CACHE_SCHEMA_V1
                        && cache.key == key
                        && cache_deps_match_for_stage(&scope, "inheritance", &cache.deps)
                    {
                        perf::record_cache_event(scope.prefix(), "inheritance", perf::CacheEvent::Hit);
                        let _ = cache_shm::shm_put(key.as_str(), &bytes);
                        dep_record_deps(&cache.deps);
                        perf::record_us("inheritance_us", wall.elapsed().as_micros() as u64);
                        return ModelPtr(cache.into_model_arc());
                    }
                }
            }
        }
    }

    let root = Arc::clone(&db.model_ast(model_name.clone()).model);
    let mut out = Arc::clone(&root);
    let mut deps: Vec<DepHashEntry> = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    // Record root dep (collector already recorded it; keep seen set for inheritance recursion only).
    if !st.path.is_empty() {
        let _ = seen.insert(st.path.to_string());
    }
    let _ = inheritance::flatten_inheritance_pure(db, &mut out, model_name.as_str(), &mut deps, &mut seen);
    perf::record_us("inheritance_us", wall.elapsed().as_micros() as u64);
    let deps = deps_scope.end();
    perf::record_cache_event(scope.prefix(), "inheritance", perf::CacheEvent::Miss);
    if cache_enabled {
        if let Some(dir) = flatten_cache::flatten_cache_dir() {
            if let Some(cfg) = cache_sqlite::sqlite_config_for_scope(scope.clone(), Some(dir.as_path())) {
                let cache = InheritanceCacheV1::new(
                    key.clone(),
                    model_name.as_str(),
                    Arc::clone(&out),
                    deps.clone(),
                );
                if let Ok(bytes) = bincode::serialize(&cache) {
                    perf::record_cache_event(scope.prefix(), "inheritance", perf::CacheEvent::Write);
                    let _ = cache_shm::shm_put(key.as_str(), &bytes);
                    let deps_json = serde_json::to_string(&deps).ok();
                    let _ = cache_sqlite::sqlite_put(
                        &cfg.path,
                        key.as_str(),
                        INHERITANCE_CACHE_SCHEMA_V1,
                        "inheritance_v1",
                        &bytes,
                        deps_json.as_deref(),
                    );
                }
            }
        }
    }
    ModelPtr(out)
}

fn decl_expanded(db: &dyn QueryDb, model_name: String) -> DeclExpandResPtr {
    let wall = std::time::Instant::now();
    let deps_scope = DepScope::begin_for_model(model_name.as_str());
    let libs = db.library_paths();
    let coarse = db.coarse_constrainedby_only();
    let st = db.source_text(model_name.clone());
    let root_hash = semantic_hash_text(st.text.as_str());
    let scope = scope_from_path(st.path.as_str(), model_name.as_str());
    let mut flags = flags_for_query_stage(db);
    flags.coarse_constrainedby_only = coarse;
    let key_v2 = CacheKeyV2::builder(CacheStage::DeclExpand, scope.clone(), model_name.as_str())
        .libs_from_path_bufs(libs.as_slice())
        .root_content_hash(root_hash)
        .compile_flags(flags)
        .build();
    let (cache_enabled, key) = key_with_policy(key_v2.to_qualified_key());

    if cache_enabled {
        if let Some(bytes) = cache_shm::shm_get(key.as_str()) {
            if let Ok(cache) = bincode::deserialize::<decl_expand::DeclExpandCacheV1>(&bytes) {
                if cache.schema == decl_expand::DECL_EXPAND_CACHE_SCHEMA_V1
                    && cache.key == key
                    && cache_deps_match_for_stage(&scope, "decl_expand", &cache.deps)
                {
                    perf::record_cache_event(scope.prefix(), "decl_expand", perf::CacheEvent::Hit);
                    dep_record_deps(&cache.deps);
                    perf::record_us("decl_expand_us", wall.elapsed().as_micros() as u64);
                    return DeclExpandResPtr(Arc::new(decl_expand::DeclExpandResult {
                        out: cache.out,
                        err: cache.err,
                    }));
                }
            }
        }
        if let Some(dir) = flatten_cache::flatten_cache_dir() {
            if let Some(bytes) = sqlite_get_with_scope_chain(
                dir.as_path(),
                scope.clone(),
                key.as_str(),
                "decl_expand_v1",
            ) {
                if let Ok(cache) = bincode::deserialize::<decl_expand::DeclExpandCacheV1>(&bytes) {
                    if cache.schema == decl_expand::DECL_EXPAND_CACHE_SCHEMA_V1
                        && cache.key == key
                        && cache_deps_match_for_stage(&scope, "decl_expand", &cache.deps)
                    {
                        perf::record_cache_event(scope.prefix(), "decl_expand", perf::CacheEvent::Hit);
                        let _ = cache_shm::shm_put(key.as_str(), &bytes);
                        dep_record_deps(&cache.deps);
                        perf::record_us("decl_expand_us", wall.elapsed().as_micros() as u64);
                        return DeclExpandResPtr(Arc::new(decl_expand::DeclExpandResult {
                            out: cache.out,
                            err: cache.err,
                        }));
                    }
                }
            }
        }
    }

    // Compute using legacy decl expansion over a pre-inherited root model.
    let root = db.inheritance_flattened(model_name.clone()).0;
    let mut flattener = crate::flatten::Flattener::new();
    flattener.coarse_constrainedby_only = coarse;
    flattener.validation_mode = crate::flatten::ValidationMode::Full;
    flattener.array_size_policy = crate::flatten::ArraySizePolicy::default();
    flattener.warnings_level = "all".to_string();
    for p in libs.iter() {
        flattener.loader.add_path(p.clone());
    }
    flattener.loader.set_quiet(true);

    let flat = match flattener.decl_expand_preinherited(Arc::clone(&root), model_name.as_str()) {
        Ok(v) => v,
        Err(e) => {
            perf::record_us("decl_expand_us", wall.elapsed().as_micros() as u64);
            let res = decl_expand::DeclExpandResult {
                out: None,
                err: Some(format!("{}", e)),
            };
            return DeclExpandResPtr(Arc::new(res));
        }
    };

    // Record deps from any models loaded by the legacy flattener loader.
    for p in flattener.loader.loaded_source_paths() {
        if let Some(h) = semantic_hash_file_cached(p.as_path()) {
            let p_s = p.display().to_string();
            dep_record_file(p_s.as_str(), h.as_str());
        }
    }

    let out = decl_expand::DeclExpandOut {
        declarations: flat.declarations,
        // decl_expanded is decl-only; equations are expanded in eq_expanded.
        equations: Vec::new(),
        instances: flat.instances,
        array_sizes: flat.array_sizes,
        inst_records: flat.inst_records,
        path_to_inst: flat.path_to_inst,
    };

    let deps = deps_scope.end();
    perf::record_cache_event(scope.prefix(), "decl_expand", perf::CacheEvent::Miss);

    if cache_enabled {
        if let Some(dir) = flatten_cache::flatten_cache_dir() {
            if let Some(cfg) = cache_sqlite::sqlite_config_for_scope(scope.clone(), Some(dir.as_path())) {
                let cache = decl_expand::DeclExpandCacheV1 {
                    schema: decl_expand::DECL_EXPAND_CACHE_SCHEMA_V1.to_string(),
                    key: key.clone(),
                    model_name: model_name.clone(),
                    out: Some(out.clone()),
                    err: None,
                    deps: deps.clone(),
                };
                if let Ok(bytes) = bincode::serialize(&cache) {
                    perf::record_cache_event(scope.prefix(), "decl_expand", perf::CacheEvent::Write);
                    let _ = cache_shm::shm_put(key.as_str(), &bytes);
                    let deps_json = serde_json::to_string(&deps).ok();
                    let _ = cache_sqlite::sqlite_put(
                        &cfg.path,
                        key.as_str(),
                        decl_expand::DECL_EXPAND_CACHE_SCHEMA_V1,
                        "decl_expand_v1",
                        &bytes,
                        deps_json.as_deref(),
                    );
                }
            }
        }
    }

    perf::record_us("decl_expand_us", wall.elapsed().as_micros() as u64);
    DeclExpandResPtr(Arc::new(decl_expand::DeclExpandResult {
        out: Some(out),
        err: None,
    }))
}

fn eq_expanded(db: &dyn QueryDb, model_name: String) -> EqExpandResPtr {
    eq_expand::eq_expanded(db, model_name)
}

fn flattened_model_q(db: &dyn QueryDb, model_name: String) -> FlatModelResPtr {
    flat_model::flattened_model_q(db, model_name)
}

fn constrainedby_holds_extends_q(
    db: &dyn QueryDb,
    input: constrainedby::ConstrainedByInput,
) -> constrainedby::ConstrainedByResPtr {
    constrainedby::constrainedby_holds_extends_q(db, input)
}

