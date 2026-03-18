use rustmodlica::ast::ClassItem;
use rustmodlica::parser;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::UNIX_EPOCH;

use crate::component_library_index::{self, ComponentRow};
use crate::profiler::ScopedTimer;

const GLOBAL_CONFIG_FILENAME: &str = "component-libraries.json";
const PROJECT_CONFIG_DIR: &str = ".modai";
const PROJECT_CONFIG_FILENAME: &str = "component-libraries.json";

pub const SCOPE_SYSTEM: &str = "system";
pub const SCOPE_GLOBAL: &str = "global";
pub const SCOPE_PROJECT: &str = "project";
pub const KIND_FOLDER: &str = "folder";
pub const KIND_FILE: &str = "file";

pub const SOURCE_TYPE_LOCAL: &str = "local";
pub const SOURCE_TYPE_GIT: &str = "git";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentLibraryConfigEntry {
    pub id: String,
    pub kind: String,
    pub source_path: String,
    pub display_name: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub priority: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentLibraryRecord {
    pub id: String,
    pub scope: String,
    pub kind: String,
    pub display_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    pub enabled: bool,
    pub priority: i32,
    pub built_in: bool,
    pub component_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySuggestion {
    pub display_name: String,
    pub url: String,
    pub ref_name: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedComponentLibrary {
    pub record: ComponentLibraryRecord,
    pub absolute_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct DiscoveredComponentType {
    pub name: String,
    pub qualified_name: String,
    pub path: Option<String>,
    pub source: String,
    pub kind: String,
    pub library_id: String,
    pub library_name: String,
    pub library_scope: String,
    pub summary: Option<String>,
    pub usage_help: Option<String>,
    pub example_titles: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedComponentType {
    pub qualified_name: String,
    pub absolute_path: PathBuf,
    pub relative_path: Option<String>,
    pub source: String,
    pub library_id: String,
    pub library_name: String,
    pub library_scope: String,
    pub library_kind: String,
    pub library_absolute_path: PathBuf,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentLibraryExampleMetadata {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub model_path: Option<String>,
    #[serde(default)]
    pub usage: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentLibraryTypeMetadata {
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub usage_help: Option<String>,
    #[serde(default)]
    pub parameter_docs: HashMap<String, String>,
    #[serde(default)]
    pub connector_docs: HashMap<String, String>,
    #[serde(default)]
    pub examples: Vec<ComponentLibraryExampleMetadata>,
}

#[derive(Debug, Clone)]
pub struct ResolvedComponentMetadata {
    pub summary: Option<String>,
    pub description: Option<String>,
    pub usage_help: Option<String>,
    pub metadata_source: String,
    pub parameter_docs: HashMap<String, String>,
    pub connector_docs: HashMap<String, String>,
    pub examples: Vec<ComponentLibraryExampleMetadata>,
}

#[derive(Debug, Clone, Default)]
pub struct QueryComponentTypesOptions {
    pub library_id: Option<String>,
    pub scope: Option<String>,
    pub enabled_only: bool,
    pub query: Option<String>,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone)]
pub struct QueryComponentTypesResult {
    pub items: Vec<DiscoveredComponentType>,
    pub total: usize,
    pub has_more: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ComponentLibraryMetadataFile {
    #[serde(default)]
    components: HashMap<String, ComponentLibraryTypeMetadata>,
}

fn default_enabled() -> bool {
    true
}

fn scope_rank(scope: &str) -> i32 {
    match scope {
        SCOPE_PROJECT => 300,
        SCOPE_GLOBAL => 200,
        _ => 100,
    }
}

fn stable_id(kind: &str, source_path: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(kind.as_bytes());
    hasher.update([0]);
    hasher.update(source_path.as_bytes());
    let digest = hasher.finalize();
    digest[..12]
        .iter()
        .map(|value| format!("{value:02x}"))
        .collect::<String>()
}

/// Strip Windows long-path prefix so stored paths work with PathBuf::from() and .exists().
fn strip_long_path_prefix(s: &str) -> &str {
    let s = s.trim_start_matches('\\');
    let s = s.trim_start_matches('/');
    if let Some(rest) = s.strip_prefix("?\\") {
        return rest;
    }
    if let Some(rest) = s.strip_prefix("?/") {
        return rest;
    }
    s
}

fn normalize_existing_path(path: &Path) -> Result<String, String> {
    let canonical = fs::canonicalize(path).map_err(|e| e.to_string())?;
    let s = canonical.to_string_lossy().replace('\\', "/");
    let s = strip_long_path_prefix(&s).to_string();
    Ok(s)
}

fn derive_display_name(path: &Path) -> String {
    path.file_stem()
        .or_else(|| path.file_name())
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Library")
        .to_string()
}

fn normalize_text(value: &str) -> Option<String> {
    let compact = value
        .replace("\r\n", "\n")
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n");
    let cleaned = compact
        .split('\n')
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned)
    }
}

fn first_line(value: &str) -> Option<String> {
    value.lines().find(|line| !line.trim().is_empty()).map(|line| line.trim().to_string())
}

fn unescape_modelica_string(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn strip_html_tags(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut in_tag = false;
    for ch in value.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

fn extract_named_string_argument(raw: &str, section_name: &str, field_name: &str) -> Option<String> {
    let section = raw.find(section_name)?;
    let field = raw[section..].find(field_name)?;
    let mut idx = section + field + field_name.len();
    let bytes = raw.as_bytes();
    while idx < raw.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    if idx >= raw.len() || bytes[idx] != b'=' {
        return None;
    }
    idx += 1;
    while idx < raw.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    if idx >= raw.len() || bytes[idx] != b'"' {
        return None;
    }
    idx += 1;
    let mut escaped = false;
    let mut value = String::new();
    for ch in raw[idx..].chars() {
        if escaped {
            value.push('\\');
            value.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '"' {
            return normalize_text(&strip_html_tags(&unescape_modelica_string(&value)));
        }
        value.push(ch);
    }
    None
}

fn extract_leading_comment_block(content: &str) -> Option<String> {
    let mut block = Vec::new();
    let mut in_multiline = false;
    for raw_line in content.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            if !block.is_empty() && !in_multiline {
                break;
            }
            continue;
        }
        if in_multiline {
            if let Some(end_idx) = line.find("*/") {
                block.push(line[..end_idx].trim().trim_start_matches('*').trim().to_string());
                in_multiline = false;
            } else {
                block.push(line.trim_start_matches('*').trim().to_string());
            }
            continue;
        }
        if let Some(comment) = line.strip_prefix("//") {
            block.push(comment.trim().to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("/*") {
            if let Some(end_idx) = rest.find("*/") {
                block.push(rest[..end_idx].trim().trim_start_matches('*').trim().to_string());
                break;
            }
            block.push(rest.trim().trim_start_matches('*').trim().to_string());
            in_multiline = true;
            continue;
        }
        break;
    }
    normalize_text(&block.join("\n"))
}

fn metadata_file_candidates(library: &ResolvedComponentLibrary) -> Vec<PathBuf> {
    if library.record.kind == KIND_FOLDER {
        vec![library.absolute_path.join(".modai-library.json")]
    } else {
        let mut candidates = Vec::new();
        if let Some(parent) = library.absolute_path.parent() {
            candidates.push(parent.join(".modai-library.json"));
        }
        let mut file_sidecar = library.absolute_path.clone();
        file_sidecar.set_extension("modai-library.json");
        candidates.push(file_sidecar);
        candidates
    }
}

fn load_library_metadata_file(library: &ResolvedComponentLibrary) -> Option<(ComponentLibraryMetadataFile, String)> {
    for path in metadata_file_candidates(library) {
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(parsed) = serde_json::from_str::<ComponentLibraryMetadataFile>(&content) else {
            continue;
        };
        return Some((parsed, path.to_string_lossy().replace('\\', "/")));
    }
    None
}

fn load_component_metadata(
    library: &ResolvedComponentLibrary,
    qualified_name: &str,
) -> Option<(ComponentLibraryTypeMetadata, String)> {
    let (metadata_file, source_path) = load_library_metadata_file(library)?;
    metadata_file
        .components
        .get(qualified_name)
        .cloned()
        .map(|entry| (entry, source_path))
}

fn extract_annotation_documentation(item: &ClassItem) -> Option<String> {
    match item {
        ClassItem::Model(model) => model
            .annotation
            .as_deref()
            .and_then(|raw| extract_named_string_argument(raw, "Documentation", "info")),
        ClassItem::Function(_) => None,
    }
}

fn auto_documentation(content: &str, item: &ClassItem) -> (Option<String>, Option<String>, Option<String>) {
    let description = extract_annotation_documentation(item).or_else(|| extract_leading_comment_block(content));
    let summary = description.as_deref().and_then(first_line);
    let usage_help = description.clone();
    (summary, description, usage_help)
}

pub fn load_resolved_component_metadata(
    resolved: &ResolvedComponentType,
    item: &ClassItem,
    content: &str,
) -> ResolvedComponentMetadata {
    let library = ResolvedComponentLibrary {
        record: ComponentLibraryRecord {
            id: resolved.library_id.clone(),
            scope: resolved.library_scope.clone(),
            kind: resolved.library_kind.clone(),
            display_name: resolved.library_name.clone(),
            source_path: Some(resolved.library_absolute_path.to_string_lossy().replace('\\', "/")),
            enabled: true,
            priority: 0,
            built_in: false,
            component_count: 0,
            source_url: None,
            source_ref: None,
        },
        absolute_path: resolved.library_absolute_path.clone(),
    };
    let (auto_summary, auto_description, auto_usage_help) = auto_documentation(content, item);
    if let Some((manual, metadata_path)) = load_component_metadata(&library, &resolved.qualified_name) {
        let summary = manual.summary.or(auto_summary);
        let description = manual.description.or(auto_description);
        let usage_help = manual.usage_help.or(auto_usage_help);
        let source_kind = if summary.is_some() || description.is_some() || usage_help.is_some() {
            "auto+sidecar"
        } else {
            "sidecar"
        };
        return ResolvedComponentMetadata {
            summary,
            description,
            usage_help,
            metadata_source: format!("{source_kind}:{metadata_path}"),
            parameter_docs: manual.parameter_docs,
            connector_docs: manual.connector_docs,
            examples: manual.examples,
        };
    }
    ResolvedComponentMetadata {
        summary: auto_summary,
        description: auto_description,
        usage_help: auto_usage_help,
        metadata_source: "auto".to_string(),
        parameter_docs: HashMap::new(),
        connector_docs: HashMap::new(),
        examples: Vec::new(),
    }
}

fn global_config_path() -> Result<PathBuf, String> {
    Ok(crate::app_data::app_data_root()?.join(GLOBAL_CONFIG_FILENAME))
}

fn project_config_path(project_dir: &Path) -> PathBuf {
    project_dir.join(PROJECT_CONFIG_DIR).join(PROJECT_CONFIG_FILENAME)
}

fn load_config_entries(path: &Path) -> Result<Vec<ComponentLibraryConfigEntry>, String> {
    if !path.is_file() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&content).map_err(|e| e.to_string())
}

fn save_config_entries(path: &Path, entries: &[ComponentLibraryConfigEntry]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = serde_json::to_string_pretty(entries).map_err(|e| e.to_string())?;
    fs::write(path, content).map_err(|e| e.to_string())
}

fn load_scope_entries(project_dir: Option<&Path>, scope: &str) -> Result<Vec<ComponentLibraryConfigEntry>, String> {
    match scope {
        SCOPE_GLOBAL => load_config_entries(&global_config_path()?),
        SCOPE_PROJECT => {
            let Some(project_root) = project_dir else {
                return Err("Project directory is required for project-scoped libraries".to_string());
            };
            load_config_entries(&project_config_path(project_root))
        }
        _ => Err(format!("Unsupported component library scope: {}", scope)),
    }
}

fn save_scope_entries(
    project_dir: Option<&Path>,
    scope: &str,
    entries: &[ComponentLibraryConfigEntry],
) -> Result<(), String> {
    match scope {
        SCOPE_GLOBAL => save_config_entries(&global_config_path()?, entries),
        SCOPE_PROJECT => {
            let Some(project_root) = project_dir else {
                return Err("Project directory is required for project-scoped libraries".to_string());
            };
            save_config_entries(&project_config_path(project_root), entries)
        }
        _ => Err(format!("Unsupported component library scope: {}", scope)),
    }
}

pub fn repo_library_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../jit-compiler")
        .to_path_buf()
}

fn builtin_library_records() -> Vec<ComponentLibraryRecord> {
    vec![
        ComponentLibraryRecord {
            id: "system-standard".to_string(),
            scope: SCOPE_SYSTEM.to_string(),
            kind: KIND_FOLDER.to_string(),
            display_name: "StandardLib".to_string(),
            source_path: Some(repo_library_root().join("StandardLib").to_string_lossy().replace('\\', "/")),
            enabled: true,
            priority: 0,
            built_in: true,
            component_count: 0,
            source_url: None,
            source_ref: None,
        },
        ComponentLibraryRecord {
            id: "system-test".to_string(),
            scope: SCOPE_SYSTEM.to_string(),
            kind: KIND_FOLDER.to_string(),
            display_name: "TestLib".to_string(),
            source_path: Some(repo_library_root().join("TestLib").to_string_lossy().replace('\\', "/")),
            enabled: true,
            priority: 0,
            built_in: true,
            component_count: 0,
            source_url: None,
            source_ref: None,
        },
    ]
}

fn to_record(scope: &str, entry: ComponentLibraryConfigEntry) -> ComponentLibraryRecord {
    ComponentLibraryRecord {
        id: entry.id,
        scope: scope.to_string(),
        kind: entry.kind,
        display_name: entry.display_name,
        source_path: Some(entry.source_path),
        enabled: entry.enabled,
        priority: entry.priority,
        built_in: false,
        component_count: 0,
        source_url: entry.source_url,
        source_ref: entry.source_ref,
    }
}

fn sort_records(records: &mut [ComponentLibraryRecord]) {
    records.sort_by(|a, b| {
        scope_rank(&b.scope)
            .cmp(&scope_rank(&a.scope))
            .then(b.priority.cmp(&a.priority))
            .then(a.display_name.cmp(&b.display_name))
    });
}

fn list_component_library_records(project_dir: Option<&Path>) -> Result<Vec<ComponentLibraryRecord>, String> {
    let mut records = builtin_library_records();
    records.extend(
        load_scope_entries(project_dir, SCOPE_GLOBAL)?
            .into_iter()
            .map(|entry| to_record(SCOPE_GLOBAL, entry)),
    );
    if project_dir.is_some() {
        records.extend(
            load_scope_entries(project_dir, SCOPE_PROJECT)?
                .into_iter()
                .map(|entry| to_record(SCOPE_PROJECT, entry)),
        );
    }
    sort_records(&mut records);
    Ok(records)
}

fn path_from_config(stored: &str) -> PathBuf {
    let cleaned = strip_long_path_prefix(stored.trim());
    PathBuf::from(cleaned)
}

fn resolved_component_libraries_from_records(
    records: Vec<ComponentLibraryRecord>,
    enabled_only: bool,
) -> Vec<ResolvedComponentLibrary> {
    let mut libraries = Vec::new();
    for record in records
        .into_iter()
        .filter(|item| !enabled_only || item.enabled)
    {
        if let Some(source_path) = &record.source_path {
            let absolute_path = path_from_config(source_path);
            let exists = if record.kind == KIND_FOLDER {
                absolute_path.is_dir()
            } else {
                absolute_path.is_file()
            };
            if exists {
                libraries.push(ResolvedComponentLibrary {
                    record,
                    absolute_path,
                });
            }
        }
    }
    libraries.sort_by(|a, b| {
        scope_rank(&b.record.scope)
            .cmp(&scope_rank(&a.record.scope))
            .then(b.record.priority.cmp(&a.record.priority))
            .then(a.record.display_name.cmp(&b.record.display_name))
    });
    libraries
}

fn library_path_mtime(path: &Path) -> i64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn discovered_to_component_row(d: &DiscoveredComponentType) -> ComponentRow {
    ComponentRow {
        library_id: d.library_id.clone(),
        qualified_name: d.qualified_name.clone(),
        name: d.name.clone(),
        kind: d.kind.clone(),
        rel_path: d.path.clone(),
        summary: d.summary.clone(),
        usage_help: d.usage_help.clone(),
        example_titles: d.example_titles.clone(),
        library_name: d.library_name.clone(),
        library_scope: d.library_scope.clone(),
    }
}

fn populate_component_index(
    conn: &rusqlite::Connection,
    libraries: &[ResolvedComponentLibrary],
    discovered: &[DiscoveredComponentType],
) -> Result<(), String> {
    for lib in libraries {
        let mtime = library_path_mtime(&lib.absolute_path);
        let source_path = lib
            .record
            .source_path
            .as_deref()
            .unwrap_or("")
            .to_string();
        component_library_index::upsert_library_meta(
            conn,
            &lib.record.id,
            &source_path,
            &lib.record.display_name,
            &lib.record.scope,
            mtime,
        )?;
        let rows: Vec<ComponentRow> = discovered
            .iter()
            .filter(|d| d.library_id == lib.record.id)
            .map(discovered_to_component_row)
            .collect();
        component_library_index::replace_components(conn, &lib.record.id, &rows)?;
    }
    Ok(())
}

fn discover_instantiable_components_from_libraries(
    libraries: &[ResolvedComponentLibrary],
) -> Result<Vec<DiscoveredComponentType>, String> {
    let mut out = Vec::new();
    for library in libraries {
        if library.record.kind == KIND_FOLDER {
            let mut stack = vec![library.absolute_path.clone()];
            while let Some(current) = stack.pop() {
                for entry in fs::read_dir(&current).map_err(|e| e.to_string())? {
                    let entry = entry.map_err(|e| e.to_string())?;
                    let path = entry.path();
                    if path.is_dir() {
                        stack.push(path);
                    } else if path.extension().is_some_and(|value| value == "mo") {
                        let relative_path = path
                            .strip_prefix(&library.absolute_path)
                            .ok()
                            .and_then(|value| value.to_str().map(|item| item.replace('\\', "/")));
                        let hint = relative_path
                            .as_deref()
                            .map(rel_path_to_qualified_name)
                            .unwrap_or_else(|| file_hint_to_qualified_name(&path));
                        out.extend(scan_modelica_file(&path, hint, relative_path, library)?);
                    }
                }
            }
        } else if library.absolute_path.extension().is_some_and(|value| value == "mo") {
            let relative_path = library
                .absolute_path
                .file_name()
                .and_then(|value| value.to_str())
                .map(|value| value.to_string());
            out.extend(scan_modelica_file(
                &library.absolute_path,
                file_hint_to_qualified_name(&library.absolute_path),
                relative_path,
                library,
            )?);
        }
    }
    out.sort_by(|a, b| {
        scope_rank(&b.library_scope)
            .cmp(&scope_rank(&a.library_scope))
            .then(a.library_name.cmp(&b.library_name))
            .then(a.qualified_name.cmp(&b.qualified_name))
    });
    Ok(out)
}

pub fn list_component_libraries(
    project_dir: Option<&Path>,
    use_index: bool,
) -> Result<Vec<ComponentLibraryRecord>, String> {
    let _timer = ScopedTimer::new("component_library::list_component_libraries");
    let mut records = list_component_library_records(project_dir)?;
    let library_ids: Vec<String> = records.iter().map(|r| r.id.clone()).collect();
    let use_index = use_index
        && match component_library_index::open_connection() {
            Ok(conn) => library_ids
                .iter()
                .all(|id| component_library_index::get_library_mtime(&conn, id).ok().flatten().is_some()),
            Err(_) => false,
        };
    let counts: HashMap<String, usize> = if use_index {
        if let Ok(conn) = component_library_index::open_connection() {
            component_library_index::get_component_counts(&conn, &library_ids).unwrap_or_default()
        } else {
            HashMap::new()
        }
    } else {
        let libraries = resolved_component_libraries_from_records(records.clone(), false);
        let discovered = discover_instantiable_components_from_libraries(&libraries)?;
        let mut counts = HashMap::<String, usize>::new();
        for item in &discovered {
            *counts.entry(item.library_id.clone()).or_insert(0) += 1;
        }
        if let Ok(conn) = component_library_index::open_connection() {
            let _ = populate_component_index(&conn, &libraries, &discovered);
        }
        counts
    };
    for record in &mut records {
        record.component_count = counts.get(&record.id).copied().unwrap_or(0);
    }
    Ok(records)
}

pub fn add_component_library(
    project_dir: Option<&Path>,
    scope: &str,
    kind: &str,
    source_path: &str,
    display_name: Option<String>,
) -> Result<ComponentLibraryRecord, String> {
    if kind != KIND_FOLDER && kind != KIND_FILE {
        return Err(format!("Unsupported component library kind: {}", kind));
    }
    let source = PathBuf::from(source_path);
    if kind == KIND_FOLDER && !source.is_dir() {
        return Err("Selected component library folder does not exist".to_string());
    }
    if kind == KIND_FILE && !source.is_file() {
        return Err("Selected component library file does not exist".to_string());
    }
    let normalized_path = normalize_existing_path(&source)?;
    let source = PathBuf::from(&normalized_path);
    let id = stable_id(kind, &normalized_path);
    let mut entries = load_scope_entries(project_dir, scope)?;
    if let Some(existing) = entries.iter_mut().find(|entry| entry.id == id) {
        existing.display_name = display_name
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| derive_display_name(&source));
        existing.enabled = true;
        let result = existing.clone();
        save_scope_entries(project_dir, scope, &entries)?;
        return Ok(to_record(scope, result));
    }
    let entry = ComponentLibraryConfigEntry {
        id: id.clone(),
        kind: kind.to_string(),
        source_path: normalized_path.clone(),
        display_name: display_name
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| derive_display_name(&source)),
        enabled: true,
        priority: 0,
        source_type: None,
        source_url: None,
        source_ref: None,
    };
    entries.push(entry.clone());
    save_scope_entries(project_dir, scope, &entries)?;
    Ok(to_record(scope, entry))
}

pub fn remove_component_library(
    project_dir: Option<&Path>,
    scope: &str,
    library_id: &str,
) -> Result<(), String> {
    let mut entries = load_scope_entries(project_dir, scope)?;
    let original_len = entries.len();
    entries.retain(|entry| entry.id != library_id);
    if entries.len() == original_len {
        return Err(format!("Component library not found: {}", library_id));
    }
    save_scope_entries(project_dir, scope, &entries)
}

pub fn set_component_library_enabled(
    project_dir: Option<&Path>,
    scope: &str,
    library_id: &str,
    enabled: bool,
) -> Result<ComponentLibraryRecord, String> {
    let mut entries = load_scope_entries(project_dir, scope)?;
    let entry = entries
        .iter_mut()
        .find(|item| item.id == library_id)
        .ok_or_else(|| format!("Component library not found: {}", library_id))?;
    entry.enabled = enabled;
    let result = entry.clone();
    save_scope_entries(project_dir, scope, &entries)?;
    Ok(to_record(scope, result))
}

fn update_library_source_metadata(
    project_dir: Option<&Path>,
    scope: &str,
    library_id: &str,
    source_type: Option<String>,
    source_url: Option<String>,
    source_ref: Option<String>,
) -> Result<ComponentLibraryRecord, String> {
    let mut entries = load_scope_entries(project_dir, scope)?;
    let entry = entries
        .iter_mut()
        .find(|item| item.id == library_id)
        .ok_or_else(|| format!("Component library not found: {}", library_id))?;
    entry.source_type = source_type;
    entry.source_url = source_url;
    entry.source_ref = source_ref;
    let result = entry.clone();
    save_scope_entries(project_dir, scope, &entries)?;
    Ok(to_record(scope, result))
}

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
