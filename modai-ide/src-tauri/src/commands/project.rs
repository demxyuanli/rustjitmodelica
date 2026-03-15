use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

use rustmodlica::annotation;
use rustmodlica::ast::ClassItem;
use rustmodlica::parser;

use crate::{component_library, diagram};

use super::jit::parse_modelica_deps;

#[tauri::command]
pub fn open_project_dir() -> Option<String> {
    rfd::FileDialog::new()
        .pick_folder()
        .and_then(|p| p.to_str().map(String::from))
}

#[tauri::command]
pub fn pick_component_library_folder() -> Option<String> {
    rfd::FileDialog::new()
        .pick_folder()
        .and_then(|path| path.to_str().map(String::from))
}

#[tauri::command]
pub fn pick_component_library_files() -> Vec<String> {
    rfd::FileDialog::new()
        .add_filter("Modelica", &["mo"])
        .pick_files()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|path| path.to_str().map(String::from))
        .collect()
}

#[tauri::command]
pub fn list_component_libraries(
    project_dir: Option<String>,
) -> Result<Vec<component_library::ComponentLibraryRecord>, String> {
    component_library::list_component_libraries(project_dir.as_deref().map(Path::new))
}

#[tauri::command]
pub fn add_component_library(
    project_dir: Option<String>,
    scope: String,
    kind: String,
    source_path: String,
    display_name: Option<String>,
) -> Result<component_library::ComponentLibraryRecord, String> {
    component_library::add_component_library(
        project_dir.as_deref().map(Path::new),
        &scope,
        &kind,
        &source_path,
        display_name,
    )
}

#[tauri::command]
pub fn remove_component_library(
    project_dir: Option<String>,
    scope: String,
    library_id: String,
) -> Result<(), String> {
    component_library::remove_component_library(
        project_dir.as_deref().map(Path::new),
        &scope,
        &library_id,
    )
}

#[tauri::command]
pub fn set_component_library_enabled(
    project_dir: Option<String>,
    scope: String,
    library_id: String,
    enabled: bool,
) -> Result<component_library::ComponentLibraryRecord, String> {
    component_library::set_component_library_enabled(
        project_dir.as_deref().map(Path::new),
        &scope,
        &library_id,
        enabled,
    )
}

fn list_mo_files_impl(dir: &Path) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    for e in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let p = e.path();
        if p.is_dir() {
            let sub = list_mo_files_impl(&p)?;
            for s in sub {
                out.push(format!(
                    "{}/{}",
                    p.file_name().and_then(|n| n.to_str()).unwrap_or(""),
                    s
                ));
            }
        } else if p.extension().is_some_and(|e| e == "mo") {
            out.push(
                p.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string(),
            );
        }
    }
    Ok(out)
}

#[tauri::command]
pub fn list_mo_files(project_dir: String) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let dir = Path::new(&project_dir);
    if !dir.is_dir() {
        return Ok(out);
    }
    for e in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let p = e.path();
        if p.is_dir() {
            let sub = list_mo_files_impl(&p)?;
            let prefix = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            for s in sub {
                out.push(format!("{}/{}", prefix, s));
            }
        } else if p.extension().is_some_and(|e| e == "mo") {
            out.push(
                p.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string(),
            );
        }
    }
    out.sort();
    Ok(out)
}

#[tauri::command]
pub fn read_project_file(project_dir: String, relative_path: String) -> Result<String, String> {
    let path = Path::new(&project_dir).join(&relative_path);
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    let dir_canonical = Path::new(&project_dir)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !canonical.starts_with(&dir_canonical) {
        return Err("Path is outside project directory".to_string());
    }
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn write_project_file(
    project_dir: String,
    relative_path: String,
    content: String,
) -> Result<(), String> {
    let project_canonical = Path::new(&project_dir)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let full = project_canonical.join(&relative_path);
    if let Some(parent) = full.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let parent_canonical = parent.canonicalize().map_err(|e| e.to_string())?;
        if !parent_canonical.starts_with(&project_canonical) {
            return Err("Path is outside project directory".to_string());
        }
    }
    fs::write(&full, content).map_err(|e| e.to_string())
}

#[derive(Serialize)]
pub struct SearchMatch {
    file: String,
    line: u32,
    column: u32,
    line_content: String,
}

fn walk_dir_recursive(dir: &Path, results: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.')
                || name == "node_modules"
                || name == "target"
                || name == "build"
            {
                continue;
            }
        }
        if path.is_dir() {
            walk_dir_recursive(&path, results);
        } else if path.is_file() {
            results.push(path);
        }
    }
}

#[tauri::command]
pub fn search_in_project(
    project_dir: String,
    query: String,
    case_sensitive: bool,
    file_pattern: Option<String>,
    max_results: Option<usize>,
) -> Result<Vec<SearchMatch>, String> {
    let base = Path::new(&project_dir);
    if !base.is_dir() {
        return Err("Project directory does not exist".to_string());
    }
    if query.is_empty() {
        return Ok(vec![]);
    }

    let limit = max_results.unwrap_or(500);
    let query_lower = if case_sensitive {
        query.clone()
    } else {
        query.to_lowercase()
    };

    let ext_filter: Option<String> = file_pattern.and_then(|p| {
        let p = p.trim();
        if p.starts_with("*.") {
            Some(p[1..].to_string())
        } else if p.starts_with('.') {
            Some(p.to_string())
        } else {
            None
        }
    });

    let mut files = Vec::new();
    walk_dir_recursive(base, &mut files);

    let mut matches = Vec::new();
    for file_path in &files {
        if matches.len() >= limit {
            break;
        }
        if let Some(ref ext) = ext_filter {
            let file_ext = file_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| format!(".{}", e))
                .unwrap_or_default();
            if file_ext != *ext {
                continue;
            }
        }
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let relative = file_path
            .strip_prefix(base)
            .unwrap_or(file_path)
            .to_string_lossy()
            .replace('\\', "/");
        for (line_idx, line) in content.lines().enumerate() {
            if matches.len() >= limit {
                break;
            }
            let haystack = if case_sensitive {
                line.to_string()
            } else {
                line.to_lowercase()
            };
            let mut start = 0;
            while let Some(pos) = haystack[start..].find(&query_lower) {
                let col = start + pos;
                matches.push(SearchMatch {
                    file: relative.clone(),
                    line: (line_idx + 1) as u32,
                    column: (col + 1) as u32,
                    line_content: line.to_string(),
                });
                start = col + query_lower.len();
                if matches.len() >= limit {
                    break;
                }
            }
        }
    }
    Ok(matches)
}

#[tauri::command]
pub fn get_diagram_data(
    project_dir: String,
    relative_path: String,
) -> Result<diagram::DiagramModel, String> {
    diagram::get_diagram_data(&project_dir, &relative_path)
}

#[tauri::command]
pub fn get_diagram_data_from_source(
    source: String,
    project_dir: Option<String>,
    relative_path: Option<String>,
) -> Result<diagram::DiagramModel, String> {
    diagram::get_diagram_data_from_source(
        &source,
        project_dir.as_deref(),
        relative_path.as_deref(),
    )
}

#[tauri::command]
pub fn get_graphical_document(
    project_dir: String,
    relative_path: String,
) -> Result<diagram::GraphicalDocumentModel, String> {
    diagram::get_graphical_document(&project_dir, &relative_path)
}

#[tauri::command]
pub fn get_graphical_document_from_source(
    source: String,
    project_dir: Option<String>,
    relative_path: Option<String>,
) -> Result<diagram::GraphicalDocumentModel, String> {
    diagram::get_graphical_document_from_source(
        &source,
        project_dir.as_deref(),
        relative_path.as_deref(),
    )
}

#[derive(Debug, Serialize)]
pub struct ApplyDiagramEditsResult {
    #[serde(rename = "newSource")]
    pub new_source: String,
}

#[tauri::command]
pub fn apply_diagram_edits(
    source: String,
    components: Vec<diagram::ComponentInstance>,
    connections: Vec<diagram::Connection>,
    layout: Option<std::collections::HashMap<String, diagram::LayoutPoint>>,
    diagram_annotation: Option<rustmodlica::annotation::IconDiagramAnnotation>,
    icon_annotation: Option<rustmodlica::annotation::IconDiagramAnnotation>,
    project_dir: Option<String>,
    relative_path: Option<String>,
) -> Result<ApplyDiagramEditsResult, String> {
    let new_source = diagram::apply_diagram_edits(
        &source,
        &components,
        &connections,
        layout.as_ref(),
        diagram_annotation.as_ref(),
        icon_annotation.as_ref(),
        project_dir.as_deref(),
        relative_path.as_deref(),
    )?;
    Ok(ApplyDiagramEditsResult { new_source })
}

#[tauri::command]
pub fn apply_graphical_document_edits(
    source: String,
    document: diagram::GraphicalDocumentModel,
    project_dir: Option<String>,
    relative_path: Option<String>,
) -> Result<ApplyDiagramEditsResult, String> {
    let new_source = diagram::apply_graphical_document_edits(
        &source,
        &document,
        project_dir.as_deref(),
        relative_path.as_deref(),
    )?;
    Ok(ApplyDiagramEditsResult { new_source })
}

#[derive(Debug, Clone, Serialize)]
pub struct MoTreeEntry {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<MoTreeEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstantiableClass {
    pub name: String,
    pub qualified_name: String,
    pub path: Option<String>,
    pub source: String,
    pub kind: String,
    pub library_id: String,
    pub library_name: String,
    pub library_scope: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_help: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub example_titles: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentTypeSource {
    pub qualified_name: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub library_id: String,
    pub library_name: String,
    pub library_scope: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentTypeParameter {
    pub name: String,
    pub type_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dialog: Option<annotation::DialogAnnotation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab: Option<String>,
    pub replaceable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentConnectorInfo {
    pub name: String,
    pub type_name: String,
    pub direction: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub replaceable: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentExampleInfo {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentTypeInfo {
    pub name: String,
    pub qualified_name: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage_help: Option<String>,
    pub metadata_source: String,
    pub extends_names: Vec<String>,
    pub connectors: Vec<ComponentConnectorInfo>,
    pub examples: Vec<ComponentExampleInfo>,
    pub parameters: Vec<ComponentTypeParameter>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentLibraryTypeQueryResult {
    pub items: Vec<InstantiableClass>,
    pub total: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentTypeRelationNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub type_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(default)]
    pub is_input: bool,
    #[serde(default)]
    pub is_output: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentTypeRelationEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_port: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_port: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentTypeRelationGraph {
    pub model_name: String,
    pub nodes: Vec<ComponentTypeRelationNode>,
    pub edges: Vec<ComponentTypeRelationEdge>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unsupported_reason: Option<String>,
}

fn expr_to_string(expr: &rustmodlica::ast::Expression) -> String {
    match expr {
        rustmodlica::ast::Expression::Number(n) => format!("{}", n),
        rustmodlica::ast::Expression::Variable(s) => s.clone(),
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
            let full = project_dir.join(&rel);
            let (class_name, extends) = fs::read_to_string(&full)
                .ok()
                .and_then(|c| parse_modelica_deps(&c))
                .unwrap_or((String::new(), Vec::new()));
            let (class_name, extends) = if class_name.is_empty() {
                (None, None)
            } else {
                (
                    Some(class_name),
                    if extends.is_empty() { None } else { Some(extends) },
                )
            };
            entries.push(MoTreeEntry {
                name: name.clone(),
                path: Some(rel),
                children: None,
                class_name,
                extends,
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
