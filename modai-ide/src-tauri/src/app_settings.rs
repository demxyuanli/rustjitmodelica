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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexCacheSettings {
    /// Use SQLite index for component library list/query (faster reopen)
    #[serde(default = "index_cache_default_true")]
    pub component_library_index_enabled: bool,
    /// When opening JIT (compiler-iterate) workspace, refresh repo index in background
    #[serde(default = "index_cache_default_true")]
    pub repo_index_refresh_on_jit_load: bool,
    /// Minimum interval (ms) between git status refreshes in JIT workspace
    #[serde(default = "index_cache_default_git_throttle")]
    pub git_status_throttle_ms: u32,
}

fn index_cache_default_true() -> bool {
    true
}

fn index_cache_default_git_throttle() -> u32 {
    2000
}

impl Default for IndexCacheSettings {
    fn default() -> Self {
        Self {
            component_library_index_enabled: true,
            repo_index_refresh_on_jit_load: true,
            git_status_throttle_ms: 2000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexingSettings {
    /// Automatically index new folders (with fewer than max files)
    #[serde(default = "indexing_default_auto_new_folders")]
    pub index_auto_new_folders: bool,
    /// Max files in a new folder to allow auto-indexing
    #[serde(default = "indexing_default_max_files")]
    pub index_auto_new_folders_max_files: u32,
    /// Index repository for fast grep/search
    #[serde(default = "indexing_default_repo_for_grep")]
    pub index_repo_for_grep: bool,
}

fn indexing_default_auto_new_folders() -> bool {
    true
}

fn indexing_default_max_files() -> u32 {
    50_000
}

fn indexing_default_repo_for_grep() -> bool {
    true
}

impl Default for IndexingSettings {
    fn default() -> Self {
        Self {
            index_auto_new_folders: true,
            index_auto_new_folders_max_files: 50_000,
            index_repo_for_grep: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiRule {
    pub id: String,
    pub name: String,
    #[serde(default = "ai_scope_default")]
    pub scope: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSkill {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "ai_scope_default")]
    pub scope: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiSubagent {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "ai_scope_default")]
    pub scope: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiCommand {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "ai_scope_default")]
    pub scope: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub content: String,
}

fn ai_scope_default() -> String {
    "user".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    #[serde(default)]
    pub rules: Vec<AiRule>,
    #[serde(default)]
    pub skills: Vec<AiSkill>,
    #[serde(default)]
    pub subagents: Vec<AiSubagent>,
    #[serde(default)]
    pub commands: Vec<AiCommand>,
    /// Model IDs enabled for the AI panel dropdown. If None or empty, all built-in models are shown.
    #[serde(default)]
    pub model_ids_enabled: Option<Vec<String>>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            rules: vec![
                AiRule {
                    id: "core-rustmodlica-style".to_string(),
                    name: "Core rustmodlica code style".to_string(),
                    scope: "rustmodlica".to_string(),
                    enabled: true,
                    content: "- Dialog with the user in Chinese, but keep all source code, identifiers, comments and string literals in ASCII and English.\n- Do not generate Chinese comments or non-ASCII string literals.\n- Do not create tests, README files, or audit/check scripts unless explicitly requested.\n- When modifying C++ or Rust code, never remove member variable or pointer initializations.\n- Prefer refactoring by extracting helpers or splitting files without changing algorithms or semantics unless explicitly requested.\n- If a single source file grows beyond about 800 lines, propose a plan to split it into multiple classes or modules, but keep behavior identical.\n- For C++ builds, always remind the user to validate with: cmake --build build --config Release --parallel.".to_string(),
                },
                AiRule {
                    id: "indexing-behavior".to_string(),
                    name: "Indexing & Docs behavior".to_string(),
                    scope: "rustmodlica".to_string(),
                    enabled: true,
                    content: "- Respect the Indexing & Docs UI semantics: Codebase Indexing, Index New Folders, Ignore files, and Instant Grep.\n- Use ignore files (.modaiignore, .cursorignore) and settings to control indexing scope instead of hard-coding paths.\n- Keep all index paths normalized with forward slashes and based on the project root.\n- Avoid O(N^2) path matching when filtering indexed files.".to_string(),
                },
            ],
            skills: vec![AiSkill {
                id: "skill-index-tuning".to_string(),
                name: "Index tuning for rustmodlica".to_string(),
                description: "Help adjust and debug indexing behavior for ModAI IDE.".to_string(),
                scope: "rustmodlica".to_string(),
                enabled: true,
                content: "When the user asks about indexing, index.db, .modaiignore/.cursorignore, or the Indexing & Docs panel:\n- Read index_manager.rs, index_db.rs, app_settings.rs, SettingsContent.tsx, and i18n.ts before proposing changes.\n- Prefer adding configuration or ignore rules instead of changing algorithms.\n- Explain how your changes affect which files and repositories are indexed.\n- Always remind the user to rebuild and validate after changes.".to_string(),
            }],
            subagents: vec![AiSubagent {
                id: "subagent-jit-compiler".to_string(),
                name: "JIT compiler engineer".to_string(),
                description: "Focused assistant for Rust JIT compiler changes using self-iteration tools.".to_string(),
                scope: "rustmodlica".to_string(),
                enabled: true,
                content: "Act as a Rust compiler engineer specializing in the rustmodlica JIT compiler.\nUse self-iteration tools when appropriate and keep patches small and focused.\nNever change public APIs or error messages without an explicit user request.".to_string(),
            }],
            commands: vec![AiCommand {
                id: "cmd-refactor-file".to_string(),
                name: "Refactor single file safely".to_string(),
                description: "Standard workflow to refactor one file without changing behavior.".to_string(),
                scope: "all".to_string(),
                enabled: true,
                content: "Workflow:\n1. Understand the file purpose and dependencies.\n2. Identify purely structural changes (extraction, naming, dead-code removal) that do not change semantics.\n3. Propose a patch limited to that file.\n4. Ask the user to run the existing build and tests, not new ones.\nConstraints:\n- Do not remove any member initialization or pointer initialization in C++.\n- Do not introduce global state or hidden side effects.".to_string(),
            }],
            model_ids_enabled: None,
        }
    }
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
    #[serde(default)]
    pub index_cache: IndexCacheSettings,
    #[serde(default)]
    pub indexing: IndexingSettings,
    #[serde(default)]
    pub ai: AiConfig,
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
