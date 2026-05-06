static ICON_CACHE: once_cell::sync::Lazy<std::sync::Mutex<HashMap<String, Option<IconDiagramAnnotation>>>> =
    once_cell::sync::Lazy::new(|| std::sync::Mutex::new(HashMap::new()));

fn try_load_type_icon(
    type_name: &str,
    project_dir: Option<&str>,
) -> Option<IconDiagramAnnotation> {
    if let Ok(cache) = ICON_CACHE.lock() {
        if let Some(cached) = cache.get(type_name) {
            return cached.clone();
        }
    }

    let result = try_load_type_icon_uncached(type_name, project_dir);

    if let Ok(mut cache) = ICON_CACHE.lock() {
        cache.insert(type_name.to_string(), result.clone());
    }
    result
}

fn try_load_type_icon_uncached(
    type_name: &str,
    project_dir: Option<&str>,
) -> Option<IconDiagramAnnotation> {
    let pdir = project_dir?;
    let base = Path::new(pdir);
    let candidates = vec![
        format!("{}.mo", type_name),
        format!("{}.mo", type_name.replace('.', "/")),
    ];
    for c in &candidates {
        let path = base.join(c);
        if path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(item) = parser::parse(&content) {
                    if let ClassItem::Model(m) = item {
                        if let Some(ref ann_str) = m.annotation {
                            if let Some(ad) = annotation::parse_annotation(ann_str) {
                                if ad.icon.is_some() {
                                    return ad.icon;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn determine_connector_kind(type_name: &str, is_input: bool, is_output: bool) -> Option<String> {
    let lower = type_name.to_lowercase();
    if lower.contains("expandable") {
        return Some("expandable".to_string());
    }
    if lower.contains("flange") || lower.contains("rotational") || lower.contains("translational") {
        return Some("mechanical".to_string());
    }
    if lower.contains("pin") || lower.contains("positivepin") || lower.contains("negativepin") {
        return Some("electrical".to_string());
    }
    if lower.contains("heatport") || lower.contains("thermal") {
        return Some("thermal".to_string());
    }
    if lower.contains("fluidport") || lower.contains("fluid") {
        return Some("fluid".to_string());
    }
    if is_input || lower.contains("realinput") || lower.contains("integerinput") || lower.contains("booleaninput") {
        return Some("signal_input".to_string());
    }
    if is_output || lower.contains("realoutput") || lower.contains("integeroutput") || lower.contains("booleanoutput") {
        return Some("signal_output".to_string());
    }
    None
}

fn extract_diagram_from_model(m: &Model, project_dir: Option<&str>) -> DiagramModel {
    let class_annotation = m
        .annotation
        .as_ref()
        .and_then(|s| annotation::parse_annotation(s));

    let components: Vec<ComponentInstance> = m
        .declarations
        .iter()
        .filter(|d| !d.is_parameter)
        .map(|d| {
            let decl_ann = d
                .annotation
                .as_ref()
                .and_then(|s| annotation::parse_annotation(s));

            let placement = decl_ann.as_ref().and_then(|a| a.placement.clone());

            let (rotation, origin) = placement
                .as_ref()
                .and_then(|p| p.transformation.as_ref())
                .map(|t| (t.rotation, t.origin.clone()))
                .unwrap_or((None, None));

            let icon = try_load_type_icon(&d.type_name, project_dir);

            let params = extract_params(d);

            let connector_kind =
                determine_connector_kind(&d.type_name, d.is_input, d.is_output);

            ComponentInstance {
                name: d.name.clone(),
                type_name: d.type_name.clone(),
                placement,
                icon,
                rotation,
                origin,
                params,
                connector_kind,
                is_input: d.is_input,
                is_output: d.is_output,
                replaceable: d.replaceable,
                constrainedby_type: d.constrainedby_type.clone(),
            }
        })
        .collect();

    let mut connections = Vec::new();
    for eq in &m.equations {
        if let Equation::Connect(a, b) = eq {
            if let (Some(from), Some(to)) = (expr_to_connector_path(a), expr_to_connector_path(b))
            {
                connections.push(Connection {
                    from,
                    to,
                    line: None,
                });
            }
        }
    }

    let layout = parse_layout_from_annotation(m.annotation.as_ref());

    let diagram_annotation = class_annotation.as_ref().and_then(|a| a.diagram.clone());
    let icon_annotation = class_annotation.as_ref().and_then(|a| a.icon.clone());

    DiagramModel {
        model_name: m.name.clone(),
        components,
        connections,
        layout,
        diagram_annotation,
        icon_annotation,
    }
}

/// Parses source and returns diagram data for the top-level model.
/// If project_dir and relative_path are set, internal diagram state is loaded from .modai/diagram-state.json.
///
/// `persistent_hint`: `None` — read state synchronously inside this function; `Some(inner)` — state
/// was already read on the async runtime (`inner == None` means no saved state, same as a miss on disk).
fn get_diagram_data_from_source_impl(
    source: &str,
    project_dir: Option<&str>,
    relative_path: Option<&str>,
    persistent_hint: Option<Option<DiagramPersistentState>>,
) -> Result<DiagramModel, String> {
    let item = parser::parse(source).map_err(|e| e.to_string())?;
    let m = match item {
        ClassItem::Model(model) => model,
        ClassItem::Function(_) => return Err("File defines a function, not a model".to_string()),
    };
    let mut diagram = extract_diagram_from_model(&m, project_dir);
    if let (Some(pdir), Some(rpath)) = (project_dir, relative_path) {
        validate_relative_path(rpath).map_err(|e| e.to_string())?;
        tracing::info!(target: "modai_diagram", "diagram data for {}", rpath);
        let state_opt = match persistent_hint {
            None => load_persistent_state(pdir, rpath),
            Some(pre) => pre,
        };
        if let Some(state) = state_opt {
            if !state.layout.is_empty() {
                diagram.layout = Some(state.layout.into_iter().collect());
            }
            if let Some(annotation) = state.diagram_annotation {
                diagram.diagram_annotation = Some(annotation);
            }
            if let Some(annotation) = state.icon_annotation {
                diagram.icon_annotation = Some(annotation);
            }
            for connection in &mut diagram.connections {
                let key = connection_key(&connection.from, &connection.to);
                if let Some(line) = state.connection_lines.get(&key) {
                    connection.line = Some(line.clone());
                }
            }
        } else {
            if let Err(e) = save_persistent_state(
                pdir,
                rpath,
                &DiagramPersistentState {
                    layout: diagram
                        .layout
                        .clone()
                        .unwrap_or_default()
                        .into_iter()
                        .collect::<BTreeMap<_, _>>(),
                    connection_lines: diagram
                        .connections
                        .iter()
                        .filter_map(|connection| {
                            connection.line.as_ref().map(|line| {
                                (connection_key(&connection.from, &connection.to), line.clone())
                            })
                        })
                        .collect::<BTreeMap<_, _>>(),
                    diagram_annotation: diagram.diagram_annotation.clone(),
                    icon_annotation: diagram.icon_annotation.clone(),
                },
            ) {
                tracing::warn!(
                    target: "modai_diagram",
                    "failed to save initial diagram persistent state: {}",
                    e
                );
            }
        }
    }
    Ok(diagram)
}

pub fn get_diagram_data_from_source(
    source: &str,
    project_dir: Option<&str>,
    relative_path: Option<&str>,
) -> Result<DiagramModel, String> {
    get_diagram_data_from_source_impl(source, project_dir, relative_path, None)
}

/// Loads persistent diagram JSON asynchronously, then parses source on the blocking pool.
pub async fn load_and_build_graphical_document_from_source(
    source: String,
    project_dir: Option<String>,
    relative_path: Option<String>,
) -> Result<GraphicalDocumentModel, String> {
    if let (Some(_), Some(r)) = (&project_dir, &relative_path) {
        validate_relative_path(r).map_err(|e| e.to_string())?;
    }
    let persistent_hint: Option<Option<DiagramPersistentState>> =
        match (&project_dir, &relative_path) {
            (Some(p), Some(r)) => Some(load_persistent_state_async(p, r).await),
            _ => None,
        };
    tokio::time::timeout(
        Duration::from_secs(30),
        async move {
            tokio::task::spawn_blocking(move || {
                get_diagram_data_from_source_impl(
                    &source,
                    project_dir.as_deref(),
                    relative_path.as_deref(),
                    persistent_hint,
                )
                .map(GraphicalDocumentModel::from_diagram_model)
            })
            .await
            .map_err(|e| format!("join error: {e}"))?
        },
    )
    .await
    .map_err(|_| "diagram load timed out after 30s".to_string())?
}

/// Reads file and returns diagram data.
pub fn get_diagram_data(project_dir: &str, relative_path: &str) -> Result<DiagramModel, String> {
    validate_relative_path(relative_path).map_err(|e| e.to_string())?;
    let path = std::path::Path::new(project_dir).join(relative_path);
    let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    get_diagram_data_from_source(&source, Some(project_dir), Some(relative_path))
}

/// Synchronous build (blocking I/O + parse). Prefer `load_and_build_graphical_document_from_source` from async commands.
#[allow(dead_code)]
pub fn get_graphical_document_from_source(
    source: &str,
    project_dir: Option<&str>,
    relative_path: Option<&str>,
) -> Result<GraphicalDocumentModel, String> {
    get_diagram_data_from_source_impl(source, project_dir, relative_path, None)
        .map(GraphicalDocumentModel::from_diagram_model)
}

pub fn get_graphical_document(
    project_dir: &str,
    relative_path: &str,
) -> Result<GraphicalDocumentModel, String> {
    get_diagram_data(project_dir, relative_path).map(GraphicalDocumentModel::from_diagram_model)
}

fn declaration_from_component(c: &ComponentInstance) -> Declaration {
    let mut decl = Declaration {
        type_name: c.type_name.clone(),
        name: c.name.clone(),
        replaceable: c.replaceable,
        constrainedby_type: c.constrainedby_type.clone(),
        is_parameter: false,
        is_flow: false,
        is_stream: false,
        is_discrete: false,
        is_input: c.is_input,
        is_output: c.is_output,
        is_inner: false,
        is_outer: false,
        is_public: false,
        is_protected: false,
        start_value: None,
        array_size: None,
        modifications: vec![],
        is_rest: false,
        annotation: None,
        condition: None,
    };
    apply_component_to_declaration(&mut decl, c);
    decl
}

fn parse_param_expression(value: &str) -> Option<Expression> {
    parser::parse_expression_from_str(value).ok()
}

fn apply_component_to_declaration(decl: &mut Declaration, component: &ComponentInstance) {
    decl.type_name = component.type_name.clone();
    decl.is_input = component.is_input;
    decl.is_output = component.is_output;
    decl.replaceable = component.replaceable;
    decl.constrainedby_type = component.constrainedby_type.clone();
    decl.annotation = upsert_annotation_item(
        decl.annotation.as_ref(),
        "Placement(",
        component.placement.as_ref().map(format_placement),
    );

    let incoming = component.params.clone().unwrap_or_default();
    let mut incoming_mods = HashMap::new();
    for param in &incoming {
        if param.name.trim().is_empty() {
            continue;
        }
        incoming_mods.insert(param.name.clone(), param.value.clone());
    }
    if let Some(start) = incoming_mods.remove("start") {
        decl.start_value = parse_param_expression(&start).or_else(|| decl.start_value.clone());
    }
    for modification in &mut decl.modifications {
        if let Some(new_value) = incoming_mods.remove(&modification.name) {
            modification.value = parse_param_expression(&new_value).or(modification.value.clone());
        }
    }
    let mut remaining_mods: Vec<(String, String)> = incoming_mods.into_iter().collect();
    remaining_mods.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    for (name, value) in remaining_mods {
        if let Some(expr) = parse_param_expression(&value) {
            decl.modifications.push(rustmodlica::ast::Modification {
                name,
                value: Some(expr),
                each: false,
                redeclare: false,
                redeclare_type: None,
                is_inner: false,
                is_outer: false,
                is_public: false,
                is_protected: false,
                is_operator_function: false,
            });
        }
    }
}

fn merge_declarations(
    existing: &[Declaration],
    components: &[ComponentInstance],
) -> Vec<Declaration> {
    let mut component_map = HashMap::new();
    for component in components {
        component_map.insert(component.name.clone(), component.clone());
    }
    let mut used = HashSet::new();
    let mut merged = Vec::new();

    for decl in existing {
        if decl.is_parameter {
            merged.push(decl.clone());
            continue;
        }
        if let Some(component) = component_map.get(&decl.name) {
            let mut updated = decl.clone();
            apply_component_to_declaration(&mut updated, component);
            merged.push(updated);
            used.insert(component.name.clone());
        }
    }

    for component in components {
        if !used.contains(&component.name) {
            merged.push(declaration_from_component(component));
        }
    }

    merged
}

fn merge_connections(existing: &[Equation], connections: &[Connection]) -> Vec<Equation> {
    let mut remaining = HashMap::new();
    for connection in connections {
        remaining.insert(connection_key(&connection.from, &connection.to), connection.clone());
    }

    let mut merged = Vec::new();
    for eq in existing {
        if let Equation::Connect(a, b) = eq {
            if let (Some(from), Some(to)) = (expr_to_connector_path(a), expr_to_connector_path(b)) {
                let key = connection_key(&from, &to);
                if remaining.remove(&key).is_some() {
                    merged.push(eq.clone());
                }
                continue;
            }
        }
        merged.push(eq.clone());
    }

    for connection in connections {
        let key = connection_key(&connection.from, &connection.to);
        if remaining.contains_key(&key) {
            merged.push(Equation::Connect(
                connector_path_to_expr(&connection.from),
                connector_path_to_expr(&connection.to),
            ));
        }
    }

    merged
}

/// Builds new .mo source from original source and new diagram components/connections.
/// Layout is stored in .modai/diagram-layout.json when project_dir and relative_path are set; never written into .mo.
pub fn apply_diagram_edits(
    source: &str,
    components: &[ComponentInstance],
    connections: &[Connection],
    layout: Option<&HashMap<String, LayoutPoint>>,
    diagram_annotation: Option<&IconDiagramAnnotation>,
    icon_annotation: Option<&IconDiagramAnnotation>,
    project_dir: Option<&str>,
    relative_path: Option<&str>,
) -> Result<ApplyDiagramEditsOutput, String> {
    let item = parser::parse(source).map_err(|e| e.to_string())?;
    let mut m = match item {
        ClassItem::Model(model) => model,
        ClassItem::Function(_) => return Err("File defines a function, not a model".to_string()),
    };

    m.declarations = merge_declarations(&m.declarations, components);
    m.equations = merge_connections(&m.equations, connections);

    let mut persistent_warning: Option<String> = None;
    if let (Some(pdir), Some(rpath)) = (project_dir, relative_path) {
        validate_relative_path(rpath).map_err(|e| e.to_string())?;
        tracing::info!(target: "modai_diagram", "apply_diagram_edits for {}", rpath);
        let state = DiagramPersistentState {
            layout: layout
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect::<BTreeMap<_, _>>(),
            connection_lines: connections
                .iter()
                .filter_map(|connection| {
                    connection
                        .line
                        .as_ref()
                        .map(|line| (connection_key(&connection.from, &connection.to), line.clone()))
                })
                .collect::<BTreeMap<_, _>>(),
            diagram_annotation: diagram_annotation.cloned(),
            icon_annotation: icon_annotation.cloned(),
        };
        if let Err(e) = save_persistent_state(pdir, rpath, &state) {
            tracing::warn!(
                target: "modai_diagram",
                "failed to save diagram persistent state after edits: {}",
                e
            );
            persistent_warning = Some(format!(
                "Failed to save .modai diagram state (layout persists in editor only until fixed): {}",
                e
            ));
        }
    }
    let inner_base = m
        .annotation
        .as_ref()
        .and_then(|ann| annotation_inner(ann))
        .map(|s| strip_diagram_layout_from_annotation_inner(&s))
        .unwrap_or_default();

    let mut items: Vec<String> = split_top_level_items(&inner_base)
        .into_iter()
        .filter(|item| {
            let t = item.trim();
            if icon_annotation.is_some() && t.starts_with("Icon(") {
                return false;
            }
            if diagram_annotation.is_some() && t.starts_with("Diagram(") {
                return false;
            }
            true
        })
        .collect();

    if let Some(icon) = icon_annotation {
        items.push(format_icon_diagram_record("Icon", icon));
    }
    if let Some(diagram) = diagram_annotation {
        items.push(format_icon_diagram_record("Diagram", diagram));
    }

    m.annotation = if items.is_empty() {
        None
    } else {
        Some(format!("annotation({});", items.join(", ")))
    };

    Ok(ApplyDiagramEditsOutput {
        new_source: unparse::model_to_mo(&m),
        warning: persistent_warning,
    })
}

pub fn apply_graphical_document_edits(
    source: &str,
    document: &GraphicalDocumentModel,
    project_dir: Option<&str>,
    relative_path: Option<&str>,
) -> Result<ApplyDiagramEditsOutput, String> {
    let diagram = document.clone().into_diagram_model();
    apply_diagram_edits(
        source,
        &diagram.components,
        &diagram.connections,
        diagram.layout.as_ref(),
        diagram.diagram_annotation.as_ref(),
        diagram.icon_annotation.as_ref(),
        project_dir,
        relative_path,
    )
}
