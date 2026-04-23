
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
        .compile_flags(flags_for_query_stage(db, model_name.as_str()))
        .build();
    let (cache_enabled, key) = key_with_policy(key_v2.to_qualified_key());

    if cache_enabled {
        if let Some(bytes) = cache_shm::shm_get(key.as_str()) {
            if let Ok(cache) = bincode::deserialize::<ParseCacheV1>(&bytes) {
                if cache.schema == PARSE_CACHE_SCHEMA_V1
                    && cache.key == key
                    && cache_deps_match_for_stage(&scope, "parse", model_name.as_str(), &cache.deps)
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
            if let Some(bytes) = sqlite_get_with_scope_chain(
                scope,
                key.as_str(),
                "parse_v1",
            ) {
                if let Ok(cache) = bincode::deserialize::<ParseCacheV1>(&bytes) {
                    if cache.schema == PARSE_CACHE_SCHEMA_V1
                        && cache.key == key
                        && cache_deps_match_for_stage(&scope, "parse", model_name.as_str(), &cache.deps)
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

    let items = match parser::parse_all(st.text.as_str()) {
        Ok(v) => v,
        Err(_e) => Vec::new(),
    };
    let deps = deps_scope.end();
    perf::record_cache_event(scope.prefix(), "parse", perf::CacheEvent::Miss);
    if cache_enabled {
        if let Some(cfg) = cache_sqlite::sqlite_write_config_for_scope(scope) {
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
        .compile_flags(flags_for_query_stage(db, model_name.as_str()))
        .build();
    let (cache_enabled, key) = key_with_policy(key_v2.to_qualified_key());

    if cache_enabled {
        if let Some(bytes) = cache_shm::shm_get(key.as_str()) {
            if let Ok(cache) = bincode::deserialize::<ModelAstCacheV1>(&bytes) {
                if cache.schema == MODEL_AST_CACHE_SCHEMA_V1
                    && cache.key == key
                    && cache_deps_match_for_stage(&scope, "model_ast", model_name.as_str(), &cache.deps)
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
            if let Some(bytes) = sqlite_get_with_scope_chain(
                scope,
                key.as_str(),
                "model_ast_v1",
            ) {
                if let Ok(cache) = bincode::deserialize::<ModelAstCacheV1>(&bytes) {
                    if cache.schema == MODEL_AST_CACHE_SCHEMA_V1
                        && cache.key == key
                        && cache_deps_match_for_stage(&scope, "model_ast", model_name.as_str(), &cache.deps)
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
        if let Some(cfg) = cache_sqlite::sqlite_write_config_for_scope(scope) {
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

pub(super) fn cache_deps_match_for_stage(
    scope: &CacheScope,
    stage: &str,
    model_name: &str,
    deps: &[DepHashEntry],
) -> bool {
    if crate::cache::msl_pack::context::relax_query_deps_for_stage(stage, model_name) {
        return true;
    }
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
    let mut flags = flags_for_query_stage(db, model_name.as_str());
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
                    && cache_deps_match_for_stage(&scope, "inheritance", model_name.as_str(), &cache.deps)
                {
                    perf::record_cache_event(scope.prefix(), "inheritance", perf::CacheEvent::Hit);
                    dep_record_deps(&cache.deps);
                    perf::record_us("inheritance_us", wall.elapsed().as_micros() as u64);
                    return ModelPtr(cache.into_model_arc());
                }
            }
        }
            if let Some(bytes) = sqlite_get_with_scope_chain(
                scope,
                key.as_str(),
                "inheritance_v1",
            ) {
                if let Ok(cache) = bincode::deserialize::<InheritanceCacheV1>(&bytes) {
                    if cache.schema == INHERITANCE_CACHE_SCHEMA_V1
                        && cache.key == key
                        && cache_deps_match_for_stage(&scope, "inheritance", model_name.as_str(), &cache.deps)
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
        if let Some(cfg) = cache_sqlite::sqlite_write_config_for_scope(scope) {
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
    let mut flags = flags_for_query_stage(db, model_name.as_str());
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
                    && cache_deps_match_for_stage(&scope, "decl_expand", model_name.as_str(), &cache.deps)
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
            if let Some(bytes) = sqlite_get_with_scope_chain(
                scope,
                key.as_str(),
                "decl_expand_v1",
            ) {
                if let Ok(cache) = bincode::deserialize::<decl_expand::DeclExpandCacheV1>(&bytes) {
                    if cache.schema == decl_expand::DECL_EXPAND_CACHE_SCHEMA_V1
                        && cache.key == key
                        && cache_deps_match_for_stage(&scope, "decl_expand", model_name.as_str(), &cache.deps)
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

    // Compute using legacy decl expansion over a pre-inherited root model.
    let root = db.inheritance_flattened(model_name.clone()).0;
    let mut flattener = crate::flatten::Flattener::new();
    flattener.coarse_constrainedby_only = coarse;
    flattener.validation_mode =
        crate::flatten::ValidationMode::parse(db.validation_mode().as_str());
    flattener.compile_stop_label = db.compile_stop().as_ref().clone();
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
        if let Some(cfg) = cache_sqlite::sqlite_write_config_for_scope(scope) {
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

    perf::record_us("decl_expand_us", wall.elapsed().as_micros() as u64);
    DeclExpandResPtr(Arc::new(decl_expand::DeclExpandResult {
        out: Some(out),
        err: None,
    }))
}

fn eq_expanded(db: &dyn QueryDb, model_name: String) -> EqExpandResPtr {
    eq_expand::eq_expanded(db, model_name)
}

pub fn eq_parallel_guard_update_candidate_share(model_name: &str, pct: f64) {
    eq_expand::eq_parallel_guard_update_candidate_share(model_name, pct);
}

fn flattened_model_q(db: &dyn QueryDb, model_name: String) -> FlatModelResPtr {
    flat_model::flattened_model_q(db, model_name)
}

fn provenance_index_q(db: &dyn QueryDb, model_name: String) -> ProvenanceIndexResPtr {
    provenance_q::provenance_index_q(db, model_name)
}

fn constrainedby_holds_extends_q(
    db: &dyn QueryDb,
    input: constrainedby::ConstrainedByInput,
) -> constrainedby::ConstrainedByResPtr {
    constrainedby::constrainedby_holds_extends_q(db, input)
}

