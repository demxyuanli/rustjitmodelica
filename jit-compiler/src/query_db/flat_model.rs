use crate::flatten::flat_cache_v1::DepHashEntry;
use crate::flatten::{FlattenedModel, ValidationMode};
use crate::query_db::{cache_deps_match_for_stage, QueryDb};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const FLAT_MODEL_Q_SCHEMA_V1: &str = "rustmodlica_flat_model_q_v1";

#[derive(Debug, Clone)]
pub struct FlatModelResult {
    pub flat: Option<Arc<FlattenedModel>>,
    pub err: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlatModelQCacheV1 {
    pub schema: String,
    pub key: String,
    pub model_name: String,
    pub flat: crate::flatten::flat_cache_v1::FlatCacheV1,
    pub deps: Vec<DepHashEntry>,
}

pub fn flattened_model_q(db: &dyn QueryDb, model_name: String) -> super::FlatModelResPtr {
    super::salsa_session::clear_last_flat_model_q_deps();
    let deps_scope = super::DepScope::begin();
    let libs = db.library_paths();
    let coarse = db.coarse_constrainedby_only();
    let compile_stop = db.compile_stop();
    let (cache_enabled, key, scope) = super::flat_model_q_cache_key(db, model_name.clone());

    if cache_enabled {
        if let Some(bytes) = crate::flatten::cache_shm::shm_get(key.as_str()) {
            if let Ok(cache) = bincode::deserialize::<FlatModelQCacheV1>(&bytes) {
                if cache.schema == FLAT_MODEL_Q_SCHEMA_V1
                    && cache.key == key
                    && cache_deps_match_for_stage(
                        &scope,
                        "flat_model_q",
                        model_name.as_str(),
                        &cache.deps,
                    )
                {
                    crate::query_db::perf::record_cache_event(
                        scope.prefix(),
                        "flat_full",
                        crate::query_db::perf::CacheEvent::Hit,
                    );
                    super::dep_record_deps(&cache.deps);
                    super::salsa_session::record_last_flat_model_q_deps(cache.deps.clone());
                    let flat = Arc::new(cache.flat.into_flat_model());
                    return super::FlatModelResPtr(Arc::new(FlatModelResult {
                        flat: Some(flat),
                        err: None,
                    }));
                }
            }
        }
            if let Some(bytes) = super::sqlite_get_with_scope_chain(
                scope,
                key.as_str(),
                "flat_model_q_v1",
            ) {
                if let Ok(cache) = bincode::deserialize::<FlatModelQCacheV1>(&bytes) {
                    if cache.schema == FLAT_MODEL_Q_SCHEMA_V1
                        && cache.key == key
                        && cache_deps_match_for_stage(
                        &scope,
                        "flat_model_q",
                        model_name.as_str(),
                        &cache.deps,
                    )
                    {
                        crate::query_db::perf::record_cache_event(
                            scope.prefix(),
                            "flat_full",
                            crate::query_db::perf::CacheEvent::Hit,
                        );
                        let _ = crate::flatten::cache_shm::shm_put(key.as_str(), &bytes);
                        super::dep_record_deps(&cache.deps);
                        super::salsa_session::record_last_flat_model_q_deps(cache.deps.clone());
                        let flat = Arc::new(cache.flat.into_flat_model());
                        return super::FlatModelResPtr(Arc::new(FlatModelResult {
                            flat: Some(flat),
                            err: None,
                        }));
                    }
                }
            }
    }

    // Compose: use eq_expanded output as the full flat model payload for now.
    // Prefer moving out of the Arc result to avoid cloning large vectors.
    let eq_res_arc = Arc::clone(&db.eq_expanded(model_name.clone()).0);
    let mut eq_out_owned: Option<crate::query_db::eq_expand::EqExpandOut> = None;
    let mut eq_shared: Option<Arc<crate::query_db::eq_expand::EqExpandResult>> = None;
    match Arc::try_unwrap(eq_res_arc) {
        Ok(mut v) => {
            if let Some(e) = v.err.take() {
                return super::FlatModelResPtr(Arc::new(FlatModelResult {
                    flat: None,
                    err: Some(e),
                }));
            }
            eq_out_owned = v.out.take();
        }
        Err(shared) => {
            if let Some(e) = &shared.err {
                return super::FlatModelResPtr(Arc::new(FlatModelResult {
                    flat: None,
                    err: Some(e.clone()),
                }));
            }
            eq_shared = Some(shared);
        }
    }
    let eq_ref = eq_shared.as_ref().and_then(|s| s.out.as_ref());
    let eq = if let Some(v) = eq_out_owned.as_ref() {
        v
    } else if let Some(v) = eq_ref {
        v
    } else {
        return super::FlatModelResPtr(Arc::new(FlatModelResult {
            flat: None,
            err: Some("eq_expanded returned empty result".to_string()),
        }));
    };
    let mut flat = if let Some(eq) = eq_out_owned {
        FlattenedModel {
            declarations: Vec::new(),
            equations: eq.equations,
            algorithms: eq.algorithms,
            initial_equations: eq.initial_equations,
            initial_algorithms: eq.initial_algorithms,
            connections: eq.connections,
            conditional_connections: eq.conditional_connections,
            instances: std::collections::HashMap::new(),
            array_sizes: std::collections::HashMap::new(),
            clocked_var_names: std::collections::HashSet::new(),
            clock_partitions: Vec::new(),
            clock_signal_connections: Vec::new(),
            stream_peer_map: std::collections::HashMap::new(),
            stream_connection_set: std::collections::HashMap::new(),
            stream_flow_map: std::collections::HashMap::new(),
            expandable_instances: std::collections::HashSet::new(),
            interner: crate::string_intern::StringInterner::new(),
            inst_records: Vec::new(),
            path_to_inst: std::collections::HashMap::new(),
        }
    } else {
        FlattenedModel {
            declarations: Vec::new(),
            equations: eq.equations.clone(),
            algorithms: eq.algorithms.clone(),
            initial_equations: eq.initial_equations.clone(),
            initial_algorithms: eq.initial_algorithms.clone(),
            connections: eq.connections.clone(),
            conditional_connections: eq.conditional_connections.clone(),
            instances: std::collections::HashMap::new(),
            array_sizes: std::collections::HashMap::new(),
            clocked_var_names: std::collections::HashSet::new(),
            clock_partitions: Vec::new(),
            clock_signal_connections: Vec::new(),
            stream_peer_map: std::collections::HashMap::new(),
            stream_connection_set: std::collections::HashMap::new(),
            stream_flow_map: std::collections::HashMap::new(),
            expandable_instances: std::collections::HashSet::new(),
            interner: crate::string_intern::StringInterner::new(),
            inst_records: Vec::new(),
            path_to_inst: std::collections::HashMap::new(),
        }
    };
    // Fill from decl_expanded. Prefer moving out of the Arc result to avoid clones.
    let decl_res_arc = Arc::clone(&db.decl_expanded(model_name.clone()).0);
    let mut decl_out_owned: Option<crate::query_db::decl_expand::DeclExpandOut> = None;
    let mut decl_shared: Option<Arc<crate::query_db::decl_expand::DeclExpandResult>> = None;
    match Arc::try_unwrap(decl_res_arc) {
        Ok(mut v) => {
            if let Some(e) = v.err.take() {
                return super::FlatModelResPtr(Arc::new(FlatModelResult {
                    flat: None,
                    err: Some(e),
                }));
            }
            decl_out_owned = v.out.take();
        }
        Err(shared) => {
            if let Some(e) = &shared.err {
                return super::FlatModelResPtr(Arc::new(FlatModelResult {
                    flat: None,
                    err: Some(e.clone()),
                }));
            }
            decl_shared = Some(shared);
        }
    }
    let decl_out_ref = decl_shared.as_ref().and_then(|s| s.out.as_ref());
    if let Some(decl) = decl_out_owned {
        flat.declarations = decl.declarations;
        flat.instances = decl.instances;
        flat.array_sizes = decl.array_sizes;
        flat.inst_records = decl.inst_records;
        flat.path_to_inst = decl.path_to_inst;
    } else if let Some(decl) = decl_out_ref {
        flat.declarations = decl.declarations.clone();
        flat.instances = decl.instances.clone();
        flat.array_sizes = decl.array_sizes.clone();
        flat.inst_records = decl.inst_records.clone();
        flat.path_to_inst = decl.path_to_inst.clone();
    } else {
        return super::FlatModelResPtr(Arc::new(FlatModelResult {
            flat: None,
            err: Some("decl_expanded returned empty result".to_string()),
        }));
    }

    // Connections + clock inference are analysis-relevant but can be skipped if caller stops at flatten tier.
    let mut loaded_paths: Vec<std::path::PathBuf> = Vec::new();
    if compile_stop.as_str() != "flatten" {
        let mut flattener = crate::flatten::Flattener::new();
        flattener.coarse_constrainedby_only = coarse;
        flattener.validation_mode = ValidationMode::parse(db.validation_mode().as_str());
        flattener.warnings_level = "all".to_string();
        for p in libs.iter() {
            flattener.loader.add_path(p.clone());
        }
        let root_path = model_name.replace('/', ".");
        let t_conn = std::time::Instant::now();
        if let Err(e) = crate::flatten::connections::resolve_connections(
            &mut flat,
            Some(root_path.as_str()),
            &flattener.loader,
        ) {
            return super::FlatModelResPtr(Arc::new(FlatModelResult {
                flat: None,
                err: Some(format!("{}", e)),
            }));
        }
        crate::query_db::perf::record_us(
            "resolve_connections_us",
            t_conn.elapsed().as_micros() as u64,
        );
        let t_clock = std::time::Instant::now();
        flattener.infer_clocked_variables_preinherited(&mut flat);
        crate::query_db::perf::record_us("clock_infer_us", t_clock.elapsed().as_micros() as u64);
        loaded_paths = flattener.loader.loaded_source_paths();
    }

    for p in loaded_paths {
        if let Some(h) = super::semantic_hash_file_cached(p.as_path()) {
            let p_s = p.display().to_string();
            super::dep_record_file(p_s.as_str(), h.as_str());
        }
    }

    // Persist using existing FlatCacheV1 container for compatibility.
    let deps = deps_scope.end();
    crate::query_db::perf::record_cache_event(
        scope.prefix(),
        "flat_full",
        crate::query_db::perf::CacheEvent::Miss,
    );
    let cache_flat =
        crate::flatten::flat_cache_v1::FlatCacheV1::from_flat_model(key.clone(), model_name.as_str(), &flat, deps.clone());
    let cache = FlatModelQCacheV1 {
        schema: FLAT_MODEL_Q_SCHEMA_V1.to_string(),
        key: key.clone(),
        model_name: model_name.clone(),
        flat: cache_flat,
        deps: deps.clone(),
    };
    if let Ok(bytes) = bincode::serialize(&cache) {
        let _ = crate::flatten::cache_shm::shm_put(key.as_str(), &bytes);
        if let Some(cfg) =
                crate::flatten::cache_sqlite::sqlite_write_config_for_scope(scope)
            {
                let deps_json = serde_json::to_string(&deps).ok();
                crate::query_db::perf::record_cache_event(
                    scope.prefix(),
                    "flat_full",
                    crate::query_db::perf::CacheEvent::Write,
                );
                let _ = crate::flatten::cache_sqlite::sqlite_put(
                    &cfg.path,
                    key.as_str(),
                    FLAT_MODEL_Q_SCHEMA_V1,
                    "flat_model_q_v1",
                    &bytes,
                    deps_json.as_deref(),
                );
        }
    }
    super::salsa_session::record_last_flat_model_q_deps(deps.clone());
    let flat_arc = Arc::new(flat);
    super::FlatModelResPtr(Arc::new(FlatModelResult {
        flat: Some(flat_arc),
        err: None,
    }))
}

