use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

use rustmodlica::annotation;
use rustmodlica::ast::ClassItem;
use rustmodlica::parser;

use crate::{app_settings, component_library, diagram, profiler::ScopedTimer};


#[tauri::command]
pub fn open_project_dir() -> Option<String> {
    let _timer = ScopedTimer::new("open_project_dir");
    rfd::FileDialog::new()
        .pick_folder()
        .and_then(|p| p.to_str().map(String::from))
}

#[tauri::command]
pub fn reopen_project_dir(path: String) -> Result<String, String> {
    let _timer = ScopedTimer::new("reopen_project_dir");
    let p = Path::new(&path);
    if !p.exists() {
        return Err("Path does not exist".to_string());
    }
    if !p.is_dir() {
        return Err("Path is not a directory".to_string());
    }
    p.canonicalize()
        .map_err(|e| e.to_string())
        .and_then(|canonical| {
            canonical
                .to_str()
                .map(String::from)
                .ok_or_else(|| "Path is not valid UTF-8".to_string())
        })
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
    let _timer = ScopedTimer::new("list_component_libraries");
    let settings = app_settings::load_settings().unwrap_or_default();
    let use_index = settings.index_cache.component_library_index_enabled;
    component_library::list_component_libraries(
        project_dir.as_deref().map(Path::new),
        use_index,
    )
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

#[tauri::command]
pub fn install_third_party_library_from_git(
    project_dir: Option<String>,
    scope: String,
    url: String,
    ref_name: Option<String>,
    display_name: Option<String>,
) -> Result<component_library::ComponentLibraryRecord, String> {
    component_library::install_library_from_git(
        project_dir.as_deref().map(Path::new),
        &scope,
        &url,
        ref_name.as_deref(),
        display_name,
    )
}

#[tauri::command]
pub fn sync_third_party_library(
    project_dir: Option<String>,
    scope: String,
    library_id: String,
) -> Result<(), String> {
    component_library::sync_library(
        project_dir.as_deref().map(Path::new),
        &scope,
        &library_id,
    )
}

#[tauri::command]
pub fn sync_all_third_party_libraries(
    project_dir: Option<String>,
) -> Result<usize, String> {
    component_library::sync_all_managed_libraries(project_dir.as_deref().map(Path::new))
}

#[tauri::command]
pub fn suggest_library_for_missing_type(type_name: String) -> Option<component_library::LibrarySuggestion> {
    component_library::suggest_library_for_missing_type(&type_name)
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
    let _timer = ScopedTimer::new("list_mo_files");
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
    let _timer = ScopedTimer::new("read_project_file");
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
    let _timer = ScopedTimer::new("write_project_file");
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
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub line_content: String,
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
    let _timer = ScopedTimer::new("search_in_project");
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
pub async fn get_graphical_document_from_source(
    source: String,
    project_dir: Option<String>,
    relative_path: Option<String>,
) -> Result<diagram::GraphicalDocumentModel, String> {
    diagram::load_and_build_graphical_document_from_source(source, project_dir, relative_path).await
}

pub type ApplyDiagramEditsResult = diagram::ApplyDiagramEditsOutput;

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
    diagram::apply_diagram_edits(
        &source,
        &components,
        &connections,
        layout.as_ref(),
        diagram_annotation.as_ref(),
        icon_annotation.as_ref(),
        project_dir.as_deref(),
        relative_path.as_deref(),
    )
}

#[tauri::command]
pub async fn apply_graphical_document_edits(
    source: String,
    document: diagram::GraphicalDocumentModel,
    project_dir: Option<String>,
    relative_path: Option<String>,
) -> Result<ApplyDiagramEditsResult, String> {
    tokio::task::spawn_blocking(move || {
        diagram::apply_graphical_document_edits(
            &source,
            &document,
            project_dir.as_deref(),
            relative_path.as_deref(),
        )
    })
    .await
    .map_err(|e| format!("join error: {e}"))?
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
    /// Path of the resolved `.mo` file relative to `project_dir`, when the file lies under the project root.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_relative_path: Option<String>,
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


include!("project_tail.rs");
