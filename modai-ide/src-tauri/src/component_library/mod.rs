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

include!("part1.rs");
include!("part2.rs");
