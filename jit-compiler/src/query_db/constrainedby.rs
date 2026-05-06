use crate::ast::Model;
use crate::cache::cache_key::{CacheKeyV2, CacheStage};
use crate::flatten::flat_cache_v1::DepHashEntry;
use crate::flatten::ValidationMode;
use crate::flatten::FlattenError;
use crate::flatten::{cache_shm, cache_sqlite};
use crate::query_db::{flags_for_query_stage, scope_from_path, QueryDb};
use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

pub const CONSTRAINEDBY_CACHE_SCHEMA_V1: &str = "rustmodlica_constrainedby_cache_v1";

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ConstrainedByInput {
    pub scope_model_name: String,
    pub import_scope: String,
    pub msl_context: String,
    pub new_type_raw: String,
    pub constraint_raw: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConstrainedByCacheV1 {
    pub schema: String,
    pub key: String,
    pub out: bool,
    pub deps: Vec<DepHashEntry>,
}

#[derive(Clone, Debug)]
pub struct ConstrainedByResPtr(pub Arc<ConstrainedByResult>);

impl PartialEq for ConstrainedByResPtr {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ConstrainedByResPtr {}

#[derive(Debug, Clone)]
pub struct ConstrainedByResult {
    pub out: bool,
    pub err: Option<String>,
}

fn load_in_scope(
    db: &dyn QueryDb,
    scope_model: &Model,
    import_scope: &str,
    msl_context: &str,
    raw: &str,
) -> Result<Arc<Model>, FlattenError> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Ok(Arc::new(Model {
            name: String::new(),
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
        }));
    }
    let short = raw.rsplit('.').next().unwrap_or(raw);
    if let Some(idx) = scope_model.inner_class_index.get(short) {
        return Ok(Arc::new(scope_model.inner_classes[*idx].clone()));
    }
    let resolved =
        crate::flatten::Flattener::resolve_import_scoped_type(scope_model, raw, import_scope, msl_context);
    if resolved != raw {
        if let Some(idx) = scope_model.inner_class_index.get(resolved.as_str()) {
            return Ok(Arc::new(scope_model.inner_classes[*idx].clone()));
        }
    }
    Ok(db.inheritance_flattened(resolved).0)
}

fn load_ast_in_scope(
    db: &dyn QueryDb,
    scope_model: &Model,
    import_scope: &str,
    msl_context: &str,
    raw: &str,
) -> Option<Arc<Model>> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let short = raw.rsplit('.').next().unwrap_or(raw);
    if let Some(idx) = scope_model.inner_class_index.get(short) {
        return Some(Arc::new(scope_model.inner_classes[*idx].clone()));
    }
    let resolved =
        crate::flatten::Flattener::resolve_import_scoped_type(scope_model, raw, import_scope, msl_context);
    if resolved != raw {
        if let Some(idx) = scope_model.inner_class_index.get(resolved.as_str()) {
            return Some(Arc::new(scope_model.inner_classes[*idx].clone()));
        }
    }
    let ast = db.model_ast(resolved);
    Some(Arc::clone(&ast.model))
}

fn constrainedby_holds_extends_impl(
    db: &dyn QueryDb,
    input: &ConstrainedByInput,
) -> Result<bool, FlattenError> {
    let scope_model = db.inheritance_flattened(input.scope_model_name.clone()).0;
    let target = load_in_scope(
        db,
        scope_model.as_ref(),
        input.import_scope.as_str(),
        input.msl_context.as_str(),
        input.constraint_raw.as_str(),
    )?;
    let start_ast = load_ast_in_scope(
        db,
        scope_model.as_ref(),
        input.import_scope.as_str(),
        input.msl_context.as_str(),
        input.new_type_raw.as_str(),
    );
    let start = match start_ast {
        Some(m) => m,
        None => {
            return load_in_scope(
                db,
                scope_model.as_ref(),
                input.import_scope.as_str(),
                input.msl_context.as_str(),
                input.new_type_raw.as_str(),
            )
            .map(|m| m.name == target.name);
        }
    };
    if start.name == target.name {
        return Ok(true);
    }

    let mut q: VecDeque<Arc<Model>> = VecDeque::new();
    let mut seen: HashSet<String> = HashSet::new();
    q.push_back(start);
    while let Some(m) = q.pop_front() {
        if !seen.insert(m.name.clone()) {
            continue;
        }
        for ext in &m.extends {
            let resolved = crate::flatten::Flattener::resolve_import_scoped_type(
                m.as_ref(),
                ext.model_name.as_str(),
                input.import_scope.as_str(),
                input.msl_context.as_str(),
            );
            if resolved == target.name {
                return Ok(true);
            }
            let child_ast = db.model_ast(resolved);
            if child_ast.model.name == target.name {
                return Ok(true);
            }
            q.push_back(Arc::clone(&child_ast.model));
        }
    }
    Ok(false)
}

pub fn constrainedby_holds_extends_q(
    db: &dyn QueryDb,
    input: ConstrainedByInput,
) -> ConstrainedByResPtr {
    let wall = std::time::Instant::now();
    let deps_scope = super::DepScope::begin();

    let libs = db.library_paths();
    let coarse = db.coarse_constrainedby_only();

    // Resolve using scope model to stabilize cache key.
    let scope_ast = db.model_ast(input.scope_model_name.clone());
    let scope = scope_from_path(scope_ast.path.as_str(), input.scope_model_name.as_str());
    let resolved_new = crate::flatten::Flattener::resolve_import_scoped_type(
        scope_ast.model.as_ref(),
        input.new_type_raw.as_str(),
        input.import_scope.as_str(),
        input.msl_context.as_str(),
    );
    let resolved_constraint = crate::flatten::Flattener::resolve_import_scoped_type(
        scope_ast.model.as_ref(),
        input.constraint_raw.as_str(),
        input.import_scope.as_str(),
        input.msl_context.as_str(),
    );
    let mut flags = flags_for_query_stage(db, input.scope_model_name.as_str());
    flags.coarse_constrainedby_only = coarse;
    let key_v2 = CacheKeyV2::builder(
        CacheStage::ConstrainedBy,
        scope.clone(),
        input.scope_model_name.as_str(),
    )
    .libs_from_path_bufs(libs.as_slice())
    .root_content_hash(format!(
        "{}|{}|{}|{}|{}",
        input.import_scope,
        input.msl_context,
        resolved_new,
        resolved_constraint,
        input.scope_model_name
    ))
    .compile_flags(flags)
    .build();
    let (cache_enabled, key) = super::key_with_policy(key_v2.to_qualified_key());

    if cache_enabled {
        if let Some(bytes) = cache_shm::shm_get(key.as_str()) {
            if let Ok(cache) = bincode::deserialize::<ConstrainedByCacheV1>(&bytes) {
                if cache.schema == CONSTRAINEDBY_CACHE_SCHEMA_V1
                    && cache.key == key
                    && super::cache_deps_match_for_stage(
                        &scope,
                        "constrainedby",
                        input.scope_model_name.as_str(),
                        &cache.deps,
                    )
                {
                    crate::query_db::perf::record_cache_event(
                        scope.prefix(),
                        "constrainedby",
                        crate::query_db::perf::CacheEvent::Hit,
                    );
                    super::dep_record_deps(&cache.deps);
                    crate::query_db::perf::record_us(
                        "constrainedby_us",
                        wall.elapsed().as_micros() as u64,
                    );
                    return ConstrainedByResPtr(Arc::new(ConstrainedByResult { out: cache.out, err: None }));
                }
            }
        }
            if let Some(bytes) = super::sqlite_get_with_scope_chain(
                scope,
                key.as_str(),
                "constrainedby_v1",
            ) {
                if let Ok(cache) = bincode::deserialize::<ConstrainedByCacheV1>(&bytes) {
                    if cache.schema == CONSTRAINEDBY_CACHE_SCHEMA_V1
                        && cache.key == key
                        && super::cache_deps_match_for_stage(
                        &scope,
                        "constrainedby",
                        input.scope_model_name.as_str(),
                        &cache.deps,
                    )
                    {
                        crate::query_db::perf::record_cache_event(
                            scope.prefix(),
                            "constrainedby",
                            crate::query_db::perf::CacheEvent::Hit,
                        );
                        let _ = cache_shm::shm_put(key.as_str(), &bytes);
                        super::dep_record_deps(&cache.deps);
                        crate::query_db::perf::record_us(
                            "constrainedby_us",
                            wall.elapsed().as_micros() as u64,
                        );
                        return ConstrainedByResPtr(Arc::new(ConstrainedByResult {
                            out: cache.out,
                            err: None,
                        }));
                    }
                }
            }
    }

    let out = match constrainedby_holds_extends_impl(db, &input) {
        Ok(v) => v,
        Err(e) => {
            crate::query_db::perf::record_us(
                "constrainedby_us",
                wall.elapsed().as_micros() as u64,
            );
            return ConstrainedByResPtr(Arc::new(ConstrainedByResult {
                out: false,
                err: Some(format!("{}", e)),
            }));
        }
    };

    let deps = deps_scope.end();
    crate::query_db::perf::record_cache_event(
        scope.prefix(),
        "constrainedby",
        crate::query_db::perf::CacheEvent::Miss,
    );
    if cache_enabled {
        if let Some(cfg) = cache_sqlite::sqlite_write_config_for_scope(scope) {
                let cache = ConstrainedByCacheV1 {
                    schema: CONSTRAINEDBY_CACHE_SCHEMA_V1.to_string(),
                    key: key.clone(),
                    out,
                    deps: deps.clone(),
                };
                if let Ok(bytes) = bincode::serialize(&cache) {
                    crate::query_db::perf::record_cache_event(
                        scope.prefix(),
                        "constrainedby",
                        crate::query_db::perf::CacheEvent::Write,
                    );
                    let _ = cache_shm::shm_put(key.as_str(), &bytes);
                    let deps_json = serde_json::to_string(&deps).ok();
                    let _ = cache_sqlite::sqlite_put(
                        &cfg.path,
                        key.as_str(),
                        CONSTRAINEDBY_CACHE_SCHEMA_V1,
                        "constrainedby_v1",
                        &bytes,
                        deps_json.as_deref(),
                    );
                }
        }
    }

    crate::query_db::perf::record_us("constrainedby_us", wall.elapsed().as_micros() as u64);
    ConstrainedByResPtr(Arc::new(ConstrainedByResult { out, err: None }))
}

pub fn constrainedby_holds_extends_cached_with_loader(
    loader: &mut crate::loader::ModelLoader,
    scope_model_qualified: &str,
    import_scope: &str,
    msl_context: &str,
    new_type_raw: &str,
    constraint_raw: &str,
    validation_mode: ValidationMode,
    compile_stop_label: &str,
) -> Result<bool, FlattenError> {
    let mut db = crate::query_db::Database::default();
    db.set_library_paths(Arc::new(loader.library_paths.clone()));
    db.set_coarse_constrainedby_only(false);
    db.set_compile_stop(Arc::new(compile_stop_label.to_string()));
    db.set_validation_mode(Arc::new(format!("{validation_mode:?}")));
    let input = ConstrainedByInput {
        scope_model_name: scope_model_qualified.to_string(),
        import_scope: import_scope.to_string(),
        msl_context: msl_context.to_string(),
        new_type_raw: new_type_raw.to_string(),
        constraint_raw: constraint_raw.to_string(),
    };
    let res = db.constrainedby_holds_extends_q(input).0;
    if let Some(e) = &res.err {
        return Err(FlattenError::Load(crate::loader::LoadError::NotFound(format!(
            "constrainedby query failed: {}",
            e
        ))));
    }
    Ok(res.out)
}

