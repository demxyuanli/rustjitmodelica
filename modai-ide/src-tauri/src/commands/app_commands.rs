use crate::{app_settings, compiler_config, db};

#[tauri::command]
pub fn open_devtools(_window: tauri::WebviewWindow) {
    #[cfg(debug_assertions)]
    {
        _window.open_devtools();
    }
}

#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
pub fn get_compiler_config() -> Result<Option<compiler_config::CompilerConfig>, String> {
    compiler_config::load_config()
}

#[tauri::command]
pub fn set_compiler_config(config: compiler_config::CompilerConfig) -> Result<(), String> {
    compiler_config::save_config(&config)
}

#[tauri::command]
pub fn get_app_settings() -> Result<app_settings::AppSettings, String> {
    app_settings::load_settings()
}

#[tauri::command]
pub fn set_app_settings(settings: app_settings::AppSettings) -> Result<(), String> {
    app_settings::save_settings(&settings)
}

#[tauri::command]
pub fn get_app_data_root() -> Result<String, String> {
    crate::app_data::app_data_root().map(|p| p.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn rebuild_component_library_index() -> Result<(), String> {
    let conn = crate::component_library_index::open_connection()?;
    crate::component_library_index::clear_all(&conn)
}

#[tauri::command]
pub fn list_iteration_history(limit: i32) -> Result<Vec<db::IterationRecord>, String> {
    db::list_iteration_history(limit)
}

#[tauri::command]
pub fn get_iteration(id: i64) -> Result<Option<db::IterationRecord>, String> {
    db::get_iteration_by_id(id)
}

#[tauri::command]
pub fn save_iteration(
    target: String,
    diff: Option<String>,
    success: bool,
    message: String,
    git_commit: Option<String>,
) -> Result<i64, String> {
    db::save_iteration(
        &target,
        diff.as_deref(),
        success,
        &message,
        git_commit.as_deref(),
    )
}
