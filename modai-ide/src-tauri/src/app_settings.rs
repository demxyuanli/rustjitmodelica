//! Application-wide settings persisted to app_data_root/settings.json.
//! Complements compiler.json, keyring, and localStorage preferences.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const SETTINGS_FILENAME: &str = "settings.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSettings {
    /// Where to store index: "project" (default, .modai-ide-data) or "central" (future)
    #[serde(default)]
    pub index_path_policy: String,
    /// Allow writing diagram layout, traceability etc. into project/repo
    #[serde(default = "default_true")]
    pub allow_project_writes: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcesSettings {
    /// Default library search paths (complement to add-library)
    #[serde(default)]
    pub library_search_paths: Vec<String>,
    /// Modelica package cache directory (optional)
    #[serde(default)]
    pub package_cache_dir: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentationSettings {
    /// Help/docs base URL or local path
    #[serde(default)]
    pub help_base_url: String,
    /// Show welcome/onboarding on first launch
    #[serde(default = "default_true")]
    pub show_welcome_on_first_launch: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionsSettings {
    /// Plugin directory path (for future plugin support)
    #[serde(default)]
    pub plugin_dir: String,
    /// Third-party library path (e.g. Modelica standard library)
    #[serde(default)]
    pub modelica_stdlib_path: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    #[serde(default)]
    pub storage: StorageSettings,
    #[serde(default)]
    pub resources: ResourcesSettings,
    #[serde(default)]
    pub documentation: DocumentationSettings,
    #[serde(default)]
    pub extensions: ExtensionsSettings,
}

fn settings_path() -> Result<PathBuf, String> {
    Ok(crate::app_data::app_data_root()?.join(SETTINGS_FILENAME))
}

pub fn load_settings() -> Result<AppSettings, String> {
    let path = settings_path()?;
    if !path.is_file() {
        return Ok(AppSettings::default());
    }
    let s = fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let settings: AppSettings = serde_json::from_str(&s).unwrap_or_default();
    Ok(settings)
}

pub fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = settings_path()?;
    let s = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    fs::write(&path, s).map_err(|e| e.to_string())?;
    Ok(())
}
