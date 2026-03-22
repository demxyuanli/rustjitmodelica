fn add_component_library_by_id(
    project_dir: Option<&Path>,
    scope: &str,
    id: &str,
    kind: &str,
    source_path: &str,
    display_name: &str,
    source_type: Option<String>,
    source_url: Option<String>,
    source_ref: Option<String>,
) -> Result<ComponentLibraryRecord, String> {
    let path_buf = PathBuf::from(source_path);
    if kind == KIND_FOLDER && !path_buf.is_dir() {
        return Err("Library folder does not exist".to_string());
    }
    let normalized_path = normalize_existing_path(&path_buf)?;
    let mut entries = load_scope_entries(project_dir, scope)?;
    if let Some(existing) = entries.iter_mut().find(|e| e.id == id) {
        existing.display_name = display_name.to_string();
        existing.enabled = true;
        existing.source_path = normalized_path.clone();
        existing.source_type = source_type.clone();
        existing.source_url = source_url.clone();
        existing.source_ref = source_ref.clone();
        let result = existing.clone();
        save_scope_entries(project_dir, scope, &entries)?;
        return Ok(to_record(scope, result));
    }
    let entry = ComponentLibraryConfigEntry {
        id: id.to_string(),
        kind: kind.to_string(),
        source_path: normalized_path,
        display_name: display_name.to_string(),
        enabled: true,
        priority: 0,
        source_type,
        source_url,
        source_ref,
    };
    entries.push(entry.clone());
    save_scope_entries(project_dir, scope, &entries)?;
    Ok(to_record(scope, entry))
}

pub fn installed_libraries_root() -> Result<PathBuf, String> {
    crate::app_data::installed_libraries_root()
}

/// After cloning a repo, return the directory that should be used as the library root for the
/// loader (i.e. the path under which "Modelica/package.mo" or similar can be found).
/// If the clone root already contains Modelica/package.mo, use it; otherwise if there is exactly
/// one subdirectory that contains Modelica/package.mo, use that (handles wrapped repos).
fn effective_library_root_after_clone(clone_root: &Path) -> PathBuf {
    let modelica_package = PathBuf::from("Modelica").join("package.mo");
    if clone_root.join(&modelica_package).is_file() {
        return clone_root.to_path_buf();
    }
    let Ok(entries) = fs::read_dir(clone_root) else {
        return clone_root.to_path_buf();
    };
    let mut candidate: Option<PathBuf> = None;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path.join(&modelica_package).is_file() {
            if candidate.is_some() {
                return clone_root.to_path_buf();
            }
            candidate = Some(path);
        }
    }
    candidate.unwrap_or_else(|| clone_root.to_path_buf())
}

pub fn install_library_from_git(
    project_dir: Option<&Path>,
    scope: &str,
    url: &str,
    ref_name: Option<&str>,
    display_name: Option<String>,
) -> Result<ComponentLibraryRecord, String> {
    let url = url.trim();
    if url.is_empty() {
        return Err("Git URL is required".to_string());
    }
    let ref_name = ref_name.map(str::trim).filter(|s| !s.is_empty());
    let id = stable_id(SOURCE_TYPE_GIT, &format!("{}\0{}", url, ref_name.unwrap_or("")));
    let root = installed_libraries_root()?;
    let target = root.join(&id);
    if target.is_dir() {
        let git_dir = target.join(".git");
        if git_dir.is_dir() {
            let effective_root = effective_library_root_after_clone(&target);
            let path_str = normalize_existing_path(&effective_root)?;
            let name = display_name
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| derive_display_name(&effective_root));
            return add_component_library_by_id(
                project_dir,
                scope,
                &id,
                KIND_FOLDER,
                &path_str,
                &name,
                Some(SOURCE_TYPE_GIT.to_string()),
                Some(url.to_string()),
                ref_name.map(String::from),
            );
        }
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let status = if let Some(r) = ref_name {
        Command::new("git")
            .args(["clone", "--depth", "1", "--branch", r, url])
            .arg(&target)
            .status()
    } else {
        Command::new("git")
            .args(["clone", "--depth", "1", url])
            .arg(&target)
            .status()
    };
    let status = status.map_err(|e| format!("Failed to run git: {}", e))?;
    if !status.success() {
        if target.is_dir() {
            let _ = fs::remove_dir_all(&target);
        }
        return Err(format!("Git clone failed with status: {}", status));
    }
    let effective_root = effective_library_root_after_clone(&target);
    let path_str = normalize_existing_path(&effective_root)?;
    let name = display_name
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| derive_display_name(&effective_root));
    add_component_library_by_id(
        project_dir,
        scope,
        &id,
        KIND_FOLDER,
        &path_str,
        &name,
        Some(SOURCE_TYPE_GIT.to_string()),
        Some(url.to_string()),
        ref_name.map(String::from),
    )
}

pub fn sync_library(
    project_dir: Option<&Path>,
    scope: &str,
    library_id: &str,
) -> Result<(), String> {
    let entries = load_scope_entries(project_dir, scope)?;
    let entry = entries
        .iter()
        .find(|e| e.id == library_id)
        .ok_or_else(|| format!("Component library not found: {}", library_id))?;
    if entry.source_type.as_deref() != Some(SOURCE_TYPE_GIT) || entry.source_url.is_none() {
        return Err("Library is not a Git-sourced library".to_string());
    }
    let path = PathBuf::from(&entry.source_path);
    if !path.is_dir() {
        return Err("Library directory does not exist".to_string());
    }
    let ref_name = entry.source_ref.as_deref().unwrap_or("HEAD");
    let fetch = Command::new("git")
        .current_dir(&path)
        .args(["fetch", "origin", ref_name])
        .status()
        .map_err(|e| format!("Git fetch failed: {}", e))?;
    if !fetch.success() {
        return Err("Git fetch failed".to_string());
    }
    let checkout = Command::new("git")
        .current_dir(&path)
        .args(["checkout", ref_name])
        .status()
        .map_err(|e| format!("Git checkout failed: {}", e))?;
    if !checkout.success() {
        return Err("Git checkout failed".to_string());
    }
    Ok(())
}

pub fn sync_all_managed_libraries(project_dir: Option<&Path>) -> Result<usize, String> {
    let mut count = 0usize;
    for scope in [SCOPE_GLOBAL, SCOPE_PROJECT] {
        let entries = if scope == SCOPE_PROJECT {
            project_dir
                .map(|p| load_scope_entries(Some(p), scope))
                .unwrap_or(Ok(Vec::new()))
        } else {
            load_scope_entries(project_dir, scope)
        };
        let entries = entries?;
        for entry in &entries {
            if entry.source_type.as_deref() == Some(SOURCE_TYPE_GIT) && entry.source_url.is_some() {
                if sync_library(project_dir, scope, &entry.id).is_ok() {
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}

const KNOWN_LIBRARIES: &[(&str, &str, &str, &str)] = &[
    (
        "Modelica.",
        "Modelica Standard Library",
        "https://github.com/modelica/ModelicaStandardLibrary",
        "v4.0.0",
    ),
];

pub fn suggest_library_for_missing_type(type_name: &str) -> Option<LibrarySuggestion> {
    let type_name = type_name.trim();
    for (pattern, display_name, url, ref_name) in KNOWN_LIBRARIES {
        if type_name.starts_with(*pattern) {
            return Some(LibrarySuggestion {
                display_name: (*display_name).to_string(),
                url: (*url).to_string(),
                ref_name: (*ref_name).to_string(),
            });
        }
    }
    None
}

pub fn resolved_component_libraries(project_dir: Option<&Path>) -> Result<Vec<ResolvedComponentLibrary>, String> {
    Ok(resolved_component_libraries_from_records(
        list_component_library_records(project_dir)?,
        true,
    ))
}

pub fn compiler_loader_paths(project_dir: Option<&Path>) -> Result<Vec<PathBuf>, String> {
    let records = list_component_library_records(project_dir)?;
    let libraries = resolved_component_libraries_from_records(records, true);
    let mut paths = Vec::new();
    if let Some(project_root) = project_dir {
        paths.push(project_root.to_path_buf());
    }
    for library in &libraries {
        if !library.record.built_in {
            if library.record.kind == KIND_FOLDER {
                paths.push(library.absolute_path.clone());
            } else if let Some(parent) = library.absolute_path.parent() {
                paths.push(parent.to_path_buf());
            }
        }
    }
    for library in &libraries {
        if library.record.built_in {
            if library.record.kind == KIND_FOLDER {
                paths.push(library.absolute_path.clone());
            } else if let Some(parent) = library.absolute_path.parent() {
                paths.push(parent.to_path_buf());
            }
        }
    }
    let repo_root = repo_library_root();
    paths.push(repo_root);
    let mut deduped = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for path in paths {
        let key = path.to_string_lossy().replace('\\', "/");
        let key = strip_long_path_prefix(&key).to_string();
        if seen.insert(key) {
            deduped.push(path);
        }
    }
    Ok(deduped)
}

pub fn rel_path_to_qualified_name(rel_path: &str) -> String {
    rel_path
        .replace('\\', "/")
        .strip_suffix(".mo")
        .unwrap_or(rel_path)
        .replace('/', ".")
}

fn file_hint_to_qualified_name(path: &Path) -> String {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(rel_path_to_qualified_name)
        .unwrap_or_else(|| "Component".to_string())
}

fn class_kind(item: &ClassItem) -> String {
    match item {
        ClassItem::Function(_) => "function".to_string(),
        ClassItem::Model(model) => {
            if model.is_connector {
                "connector".to_string()
            } else if model.is_block {
                "block".to_string()
            } else if model.is_record {
                "record".to_string()
            } else {
                "model".to_string()
            }
        }
    }
}

fn collect_instantiable_from_model(
    model: &rustmodlica::ast::Model,
    qualified_name: String,
    path: &Path,
    relative_path: Option<&String>,
    library: &ResolvedComponentLibrary,
    content: &str,
    out: &mut Vec<DiscoveredComponentType>,
) {
    if model.is_connector || model.is_function {
        return;
    }
    let resolved = ResolvedComponentType {
        qualified_name: qualified_name.clone(),
        absolute_path: path.to_path_buf(),
        relative_path: relative_path.cloned(),
        source: library.record.scope.clone(),
        library_id: library.record.id.clone(),
        library_name: library.record.display_name.clone(),
        library_scope: library.record.scope.clone(),
        library_kind: library.record.kind.clone(),
        library_absolute_path: library.absolute_path.clone(),
    };
    let metadata = load_resolved_component_metadata(
        &resolved,
        &ClassItem::Model(model.clone()),
        content,
    );
    out.push(DiscoveredComponentType {
        name: model.name.clone(),
        qualified_name: qualified_name.clone(),
        path: relative_path.cloned(),
        source: library.record.scope.clone(),
        kind: if model.is_block {
            "block".to_string()
        } else {
            "model".to_string()
        },
        library_id: library.record.id.clone(),
        library_name: library.record.display_name.clone(),
        library_scope: library.record.scope.clone(),
        summary: metadata.summary,
        usage_help: metadata.usage_help,
        example_titles: metadata.examples.into_iter().map(|e| e.title).collect(),
    });
    for inner in &model.inner_classes {
        let inner_qualified = format!("{}.{}", qualified_name, inner.name);
        collect_instantiable_from_model(
            inner,
            inner_qualified,
            path,
            relative_path,
            library,
            content,
            out,
        );
    }
}

fn scan_modelica_file(
    path: &Path,
    qualified_name_hint: String,
    relative_path: Option<String>,
    library: &ResolvedComponentLibrary,
) -> Result<Vec<DiscoveredComponentType>, String> {
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let item = match parser::parse(&content) {
        Ok(value) => value,
        Err(_) => return Ok(Vec::new()),
    };
    let mut out = Vec::new();
    if let ClassItem::Model(model) = item {
        let top_qualified = if library.record.kind == KIND_FILE {
            model.name.clone()
        } else {
            qualified_name_hint
        };
        collect_instantiable_from_model(
            &model,
            top_qualified,
            path,
            relative_path.as_ref(),
            library,
            &content,
            &mut out,
        );
    }
    Ok(out)
}

pub fn discover_instantiable_components(project_dir: Option<&Path>) -> Result<Vec<DiscoveredComponentType>, String> {
    let _timer = ScopedTimer::new("component_library::discover_instantiable_components");
    discover_instantiable_components_from_libraries(&resolved_component_libraries(project_dir)?)
}

fn component_row_to_discovered(row: &ComponentRow) -> DiscoveredComponentType {
    DiscoveredComponentType {
        name: row.name.clone(),
        qualified_name: row.qualified_name.clone(),
        path: row.rel_path.clone(),
        source: row.library_scope.clone(),
        kind: row.kind.clone(),
        library_id: row.library_id.clone(),
        library_name: row.library_name.clone(),
        library_scope: row.library_scope.clone(),
        summary: row.summary.clone(),
        usage_help: row.usage_help.clone(),
        example_titles: row.example_titles.clone(),
    }
}

pub fn query_component_types(
    project_dir: Option<&Path>,
    options: QueryComponentTypesOptions,
    use_index: bool,
) -> Result<QueryComponentTypesResult, String> {
    let _timer = ScopedTimer::new("component_library::query_component_types");
    let query_lower = options
        .query
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase());
    let mut records = list_component_library_records(project_dir)?;
    if let Some(library_id) = options.library_id.as_deref() {
        records.retain(|r| r.id == library_id);
    }
    if let Some(scope) = options.scope.as_deref() {
        records.retain(|r| r.scope == scope);
    }
    if options.enabled_only {
        records.retain(|r| r.enabled);
    }
    let libraries = resolved_component_libraries_from_records(records.clone(), false);
    let library_ids: Vec<String> = libraries.iter().map(|l| l.record.id.clone()).collect();
    if library_ids.is_empty() {
        return Ok(QueryComponentTypesResult {
            items: vec![],
            total: 0,
            has_more: false,
        });
    }
    let use_index = use_index
        && match component_library_index::open_connection() {
            Ok(conn) => library_ids
                .iter()
                .all(|id| component_library_index::get_library_mtime(&conn, id).ok().flatten().is_some()),
            Err(_) => false,
        };
    if use_index {
        if let Ok(conn) = component_library_index::open_connection() {
            let offset = options.offset;
            let limit = options.limit.max(1);
            let q = query_lower.as_deref();
            if let Ok((rows, total)) =
                component_library_index::query_components(&conn, &library_ids, q, offset, limit)
            {
                let items: Vec<DiscoveredComponentType> =
                    rows.iter().map(component_row_to_discovered).collect();
                let has_more = offset + items.len() < total;
                return Ok(QueryComponentTypesResult {
                    items,
                    total,
                    has_more,
                });
            }
        }
    }
    let discovered = discover_instantiable_components_from_libraries(&libraries)?;
    if let Ok(conn) = component_library_index::open_connection() {
        let _ = populate_component_index(&conn, &libraries, &discovered);
    }
    let mut items = discovered;
    if let Some(term) = query_lower.as_deref() {
        items.retain(|item| {
            item.name.to_lowercase().contains(term)
                || item.qualified_name.to_lowercase().contains(term)
                || item.library_name.to_lowercase().contains(term)
                || item
                    .path
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(term)
                || item
                    .summary
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(term)
                || item
                    .usage_help
                    .as_deref()
                    .unwrap_or_default()
                    .to_lowercase()
                    .contains(term)
                || item
                    .example_titles
                    .iter()
                    .any(|title| title.to_lowercase().contains(term))
        });
    }
    let total = items.len();
    let offset = options.offset.min(total);
    let limit = options.limit.max(1);
    let page_items = items.into_iter().skip(offset).take(limit).collect::<Vec<_>>();
    let has_more = offset + page_items.len() < total;
    Ok(QueryComponentTypesResult {
        items: page_items,
        total,
        has_more,
    })
}

fn folder_candidates(type_name: &str) -> Vec<String> {
    let normalized = type_name.replace('\\', "/");
    let stem = normalized.strip_suffix(".mo").unwrap_or(&normalized);
    let slash_path = stem.replace('.', "/");
    vec![
        normalized.clone(),
        format!("{}.mo", stem),
        slash_path.clone(),
        format!("{}.mo", slash_path),
    ]
}

fn file_matches(path: &Path, type_name: &str) -> bool {
    let normalized = type_name.replace('\\', "/");
    let stem = normalized.strip_suffix(".mo").unwrap_or(&normalized);
    let file_name = path.file_name().and_then(|value| value.to_str()).unwrap_or("");
    let file_stem = path.file_stem().and_then(|value| value.to_str()).unwrap_or("");
    stem == file_stem
        || normalized == file_name
        || normalized.ends_with(&format!("/{}", file_name))
        || normalized.ends_with(&format!(".{}", file_stem))
}

pub fn resolve_component_type(
    project_dir: Option<&Path>,
    type_name: &str,
    preferred_library_id: Option<&str>,
) -> Result<Option<ResolvedComponentType>, String> {
    let mut libraries = resolved_component_libraries(project_dir)?;
    if let Some(library_id) = preferred_library_id {
        libraries.sort_by_key(|library| if library.record.id == library_id { 0 } else { 1 });
    }
    for library in libraries {
        if library.record.kind == KIND_FOLDER {
            for candidate in folder_candidates(type_name) {
                let absolute_path = library.absolute_path.join(&candidate);
                if absolute_path.is_file() {
                    let relative_path = absolute_path
                        .strip_prefix(&library.absolute_path)
                        .ok()
                        .and_then(|value| value.to_str().map(|item| item.replace('\\', "/")));
                    let qualified_name = relative_path
                        .as_deref()
                        .map(rel_path_to_qualified_name)
                        .unwrap_or_else(|| type_name.to_string());
                    return Ok(Some(ResolvedComponentType {
                        qualified_name,
                        absolute_path,
                        relative_path,
                        source: library.record.scope.clone(),
                        library_id: library.record.id.clone(),
                        library_name: library.record.display_name.clone(),
                        library_scope: library.record.scope.clone(),
                        library_kind: library.record.kind.clone(),
                        library_absolute_path: library.absolute_path.clone(),
                    }));
                }
            }
        } else if file_matches(&library.absolute_path, type_name) {
            let relative_path = library
                .absolute_path
                .file_name()
                .and_then(|value| value.to_str())
                .map(|value| value.to_string());
            return Ok(Some(ResolvedComponentType {
                qualified_name: type_name.to_string(),
                absolute_path: library.absolute_path.clone(),
                relative_path,
                source: library.record.scope.clone(),
                library_id: library.record.id.clone(),
                library_name: library.record.display_name.clone(),
                library_scope: library.record.scope.clone(),
                library_kind: library.record.kind.clone(),
                library_absolute_path: library.absolute_path.clone(),
            }));
        }
    }
    Ok(None)
}

pub fn class_item_kind(item: &ClassItem) -> String {
    class_kind(item)
}
