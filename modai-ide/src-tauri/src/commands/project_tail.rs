fn expr_to_string(expr: &rustmodlica::ast::Expression) -> String {
    match expr {
        rustmodlica::ast::Expression::Number(n) => format!("{}", n),
        rustmodlica::ast::Expression::Variable(id) => rustmodlica::string_intern::resolve_id(*id),
        rustmodlica::ast::Expression::StringLiteral(s) => format!("\"{}\"", s),
        rustmodlica::ast::Expression::BinaryOp(l, op, r) => {
            let op_str = match op {
                rustmodlica::ast::Operator::Add => "+",
                rustmodlica::ast::Operator::Sub => "-",
                rustmodlica::ast::Operator::Mul => "*",
                rustmodlica::ast::Operator::Div => "/",
                rustmodlica::ast::Operator::Less => "<",
                rustmodlica::ast::Operator::Greater => ">",
                rustmodlica::ast::Operator::LessEq => "<=",
                rustmodlica::ast::Operator::GreaterEq => ">=",
                rustmodlica::ast::Operator::Equal => "==",
                rustmodlica::ast::Operator::NotEqual => "<>",
                rustmodlica::ast::Operator::And => "and",
                rustmodlica::ast::Operator::Or => "or",
            };
            format!("{} {} {}", expr_to_string(l), op_str, expr_to_string(r))
        }
        rustmodlica::ast::Expression::Der(inner) => format!("der({})", expr_to_string(inner)),
        rustmodlica::ast::Expression::Call(name, args) => {
            let args = args.iter().map(expr_to_string).collect::<Vec<_>>().join(", ");
            format!("{}({})", name, args)
        }
        _ => String::new(),
    }
}

#[tauri::command]
pub fn list_instantiable_classes(project_dir: Option<String>) -> Result<Vec<InstantiableClass>, String> {
    let out = component_library::discover_instantiable_components(project_dir.as_deref().map(Path::new))?
        .into_iter()
        .map(|item| InstantiableClass {
            name: item.name,
            qualified_name: item.qualified_name,
            path: item.path,
            source: item.source,
            kind: item.kind,
            library_id: item.library_id,
            library_name: item.library_name,
            library_scope: item.library_scope,
            summary: item.summary,
            usage_help: item.usage_help,
            example_titles: item.example_titles,
        })
        .collect::<Vec<_>>();
    Ok(out)
}

#[tauri::command]
pub fn query_component_library_types(
    project_dir: Option<String>,
    library_id: Option<String>,
    scope: Option<String>,
    enabled_only: Option<bool>,
    query: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<ComponentLibraryTypeQueryResult, String> {
    let settings = app_settings::load_settings().unwrap_or_default();
    let use_index = settings.index_cache.component_library_index_enabled;
    let result = component_library::query_component_types(
        project_dir.as_deref().map(Path::new),
        component_library::QueryComponentTypesOptions {
            library_id,
            scope,
            enabled_only: enabled_only.unwrap_or(true),
            query,
            offset: offset.unwrap_or(0),
            limit: limit.unwrap_or(100),
        },
        use_index,
    )?;
    Ok(ComponentLibraryTypeQueryResult {
        items: result
            .items
            .into_iter()
            .map(|item| InstantiableClass {
                name: item.name,
                qualified_name: item.qualified_name,
                path: item.path,
                source: item.source,
                kind: item.kind,
                library_id: item.library_id,
                library_name: item.library_name,
                library_scope: item.library_scope,
                summary: item.summary,
                usage_help: item.usage_help,
                example_titles: item.example_titles,
            })
            .collect(),
        total: result.total,
        has_more: result.has_more,
    })
}

#[tauri::command]
pub fn read_component_type_source(
    project_dir: Option<String>,
    type_name: String,
    library_id: Option<String>,
) -> Result<ComponentTypeSource, String> {
    let resolved = component_library::resolve_component_type(
        project_dir.as_deref().map(Path::new),
        &type_name,
        library_id.as_deref(),
    )?
        .ok_or_else(|| format!("Component type not found: {}", type_name))?;
    let content = fs::read_to_string(&resolved.absolute_path).map_err(|e| e.to_string())?;

    Ok(ComponentTypeSource {
        qualified_name: resolved.qualified_name,
        source: resolved.source,
        path: resolved.relative_path,
        library_id: resolved.library_id,
        library_name: resolved.library_name,
        library_scope: resolved.library_scope,
        content,
    })
}

#[tauri::command]
pub fn get_component_type_details(
    project_dir: Option<String>,
    type_name: String,
    library_id: Option<String>,
) -> Result<ComponentTypeInfo, String> {
    let resolved = component_library::resolve_component_type(
        project_dir.as_deref().map(Path::new),
        &type_name,
        library_id.as_deref(),
    )?
        .ok_or_else(|| format!("Component type not found: {}", type_name))?;
    let content = fs::read_to_string(&resolved.absolute_path).map_err(|e| e.to_string())?;
    let item = parser::parse(&content).map_err(|e| e.to_string())?;
    let metadata = component_library::load_resolved_component_metadata(&resolved, &item, &content);
    let build_parameter = |declaration: &rustmodlica::ast::Declaration| {
        let dialog = declaration
            .annotation
            .as_ref()
            .and_then(|raw| annotation::parse_dialog(raw));
        ComponentTypeParameter {
            name: declaration.name.clone(),
            type_name: declaration.type_name.clone(),
            default_value: declaration
                .start_value
                .as_ref()
                .map(expr_to_string)
                .filter(|value| !value.is_empty()),
            dialog: dialog.clone(),
            description: metadata.parameter_docs.get(&declaration.name).cloned(),
            group: dialog.as_ref().and_then(|value| value.group.clone()),
            tab: dialog.as_ref().and_then(|value| value.tab.clone()),
            replaceable: declaration.replaceable,
        }
    };
    let build_connector = |declaration: &rustmodlica::ast::Declaration| {
        let direction = if declaration.is_input {
            "input"
        } else if declaration.is_output {
            "output"
        } else {
            "flow"
        };
        ComponentConnectorInfo {
            name: declaration.name.clone(),
            type_name: declaration.type_name.clone(),
            direction: direction.to_string(),
            description: metadata.connector_docs.get(&declaration.name).cloned(),
            replaceable: declaration.replaceable,
        }
    };
    let (parameters, connectors, extends_names) = match &item {
        ClassItem::Function(function) => (
            function
                .declarations
                .iter()
                .filter(|declaration| declaration.is_parameter)
                .map(build_parameter)
                .collect(),
            function
                .declarations
                .iter()
                .filter(|declaration| declaration.is_input || declaration.is_output || declaration.is_flow)
                .map(build_connector)
                .collect(),
            function
                .extends
                .iter()
                .map(|extend| extend.model_name.clone())
                .collect(),
        ),
        ClassItem::Model(model) => (
            model
                .declarations
                .iter()
                .filter(|declaration| declaration.is_parameter)
                .map(build_parameter)
                .collect(),
            model
                .declarations
                .iter()
                .filter(|declaration| declaration.is_input || declaration.is_output || declaration.is_flow)
                .map(build_connector)
                .collect(),
            model
                .extends
                .iter()
                .map(|extend| extend.model_name.clone())
                .collect(),
        ),
    };
    Ok(ComponentTypeInfo {
        name: type_name,
        qualified_name: resolved.qualified_name.clone(),
        kind: component_library::class_item_kind(&item),
        path: Some(resolved.absolute_path.to_string_lossy().replace('\\', "/")),
        library_id: Some(resolved.library_id),
        library_name: Some(resolved.library_name),
        library_scope: Some(resolved.library_scope),
        summary: metadata.summary,
        description: metadata.description,
        usage_help: metadata.usage_help,
        metadata_source: metadata.metadata_source,
        extends_names,
        connectors,
        examples: metadata
            .examples
            .into_iter()
            .map(|example| ComponentExampleInfo {
                title: example.title,
                description: example.description,
                model_path: example.model_path,
                usage: example.usage,
            })
            .collect(),
        parameters,
    })
}

fn split_connector_path(path: &str) -> (String, Option<String>) {
    if let Some((node, port)) = path.split_once('.') {
        (node.to_string(), Some(port.to_string()))
    } else {
        (path.to_string(), None)
    }
}

#[tauri::command]
pub fn get_component_type_relation_graph(
    project_dir: Option<String>,
    type_name: String,
    library_id: Option<String>,
) -> Result<ComponentTypeRelationGraph, String> {
    let resolved = component_library::resolve_component_type(
        project_dir.as_deref().map(Path::new),
        &type_name,
        library_id.as_deref(),
    )?
    .ok_or_else(|| format!("Component type not found: {}", type_name))?;
    let content = fs::read_to_string(&resolved.absolute_path).map_err(|e| e.to_string())?;
    match diagram::get_diagram_data_from_source(&content, None, None) {
        Ok(model) => {
            let layout = model.layout.clone();
            let nodes = model
                .components
                .into_iter()
                .enumerate()
                .map(|(index, component)| {
                    let layout_pos = layout
                        .as_ref()
                        .and_then(|layout| layout.get(&component.name))
                        .map(|position| (position.x, position.y));
                    ComponentTypeRelationNode {
                        id: component.name.clone(),
                        label: component.name,
                        kind: "component".to_string(),
                        type_name: component.type_name,
                        x: layout_pos.map(|value| value.0).or(Some((index % 4) as f64 * 220.0)),
                        y: layout_pos.map(|value| value.1).or(Some((index / 4) as f64 * 140.0)),
                        is_input: component.is_input,
                        is_output: component.is_output,
                    }
                })
                .collect::<Vec<_>>();
            let edges = model
                .connections
                .into_iter()
                .enumerate()
                .map(|(index, connection)| {
                    let (source, source_port) = split_connector_path(&connection.from);
                    let (target, target_port) = split_connector_path(&connection.to);
                    ComponentTypeRelationEdge {
                        id: format!("edge-{}", index),
                        source,
                        target,
                        source_port,
                        target_port,
                    }
                })
                .collect::<Vec<_>>();
            Ok(ComponentTypeRelationGraph {
                model_name: model.model_name,
                nodes,
                edges,
                unsupported_reason: None,
            })
        }
        Err(error) => Ok(ComponentTypeRelationGraph {
            model_name: resolved.qualified_name,
            nodes: Vec::new(),
            edges: Vec::new(),
            unsupported_reason: Some(error),
        }),
    }
}

fn list_mo_tree_impl(
    dir: &Path,
    project_dir: &Path,
    prefix: &str,
) -> Result<Vec<MoTreeEntry>, String> {
    let mut entries = Vec::new();
    if !dir.is_dir() {
        return Ok(entries);
    }
    let mut read_dir: Vec<_> = fs::read_dir(dir).map_err(|e| e.to_string())?.collect();
    read_dir.sort_by(|a, b| {
        let a = a.as_ref().map(|e| e.path()).unwrap_or_default();
        let b = b.as_ref().map(|e| e.path()).unwrap_or_default();
        match (a.is_dir(), b.is_dir()) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });
    for e in read_dir {
        let e = e.map_err(|e| e.to_string())?;
        let p = e.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if p.is_dir() {
            let sub = list_mo_tree_impl(&p, project_dir, &format!("{}{}/", prefix, name))?;
            if !sub.is_empty() {
                entries.push(MoTreeEntry {
                    name,
                    path: None,
                    children: Some(sub),
                    class_name: None,
                    extends: None,
                });
            }
        } else if p.extension().is_some_and(|e| e == "mo") {
            let rel = format!("{}{}", prefix, name);
            entries.push(MoTreeEntry {
                name: name.clone(),
                path: Some(rel),
                children: None,
                class_name: None,
                extends: None,
            });
        }
    }
    Ok(entries)
}

#[tauri::command]
pub fn list_mo_tree(project_dir: String) -> Result<MoTreeEntry, String> {
    let dir = Path::new(&project_dir);
    if !dir.is_dir() {
        return Ok(MoTreeEntry {
            name: String::new(),
            path: None,
            children: Some(Vec::new()),
            class_name: None,
            extends: None,
        });
    }
    Ok(MoTreeEntry {
        name: String::new(),
        path: None,
        children: Some(list_mo_tree_impl(dir, dir, "")?),
        class_name: None,
        extends: None,
    })
}

#[tauri::command]
pub fn extract_equations_from_source(
    source: String,
) -> Result<diagram::ModelEquationsAndVars, String> {
    diagram::extract_equations_from_source(&source)
}

#[tauri::command]
pub fn apply_equation_edits(
    source: String,
    variables: Vec<diagram::VariableDecl>,
    equations: Vec<diagram::EquationEntry>,
) -> Result<serde_json::Value, String> {
    let new_source = diagram::apply_equation_edits(&source, &variables, &equations)?;
    Ok(serde_json::json!({ "newSource": new_source }))
}
