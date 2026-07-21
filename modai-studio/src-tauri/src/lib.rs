mod catalog;
mod commands;
mod diagram;
mod emit_mo;
mod fmu_import;
mod project;
mod sim_bridge;

use commands::{
    studio_cancel_simulation, studio_catalog_list, studio_emit_artifacts, studio_emit_preview,
    studio_import_fmu, studio_list_imported_fmus, studio_pick_directory,
    studio_pick_export_directory, studio_pick_fmu_file, studio_project_create, studio_project_open,
    studio_project_save, studio_simulate, studio_validate,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            studio_catalog_list,
            studio_project_create,
            studio_project_open,
            studio_project_save,
            studio_emit_preview,
            studio_validate,
            studio_simulate,
            studio_cancel_simulation,
            studio_emit_artifacts,
            studio_pick_directory,
            studio_pick_export_directory,
            studio_pick_fmu_file,
            studio_import_fmu,
            studio_list_imported_fmus,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Modai Studio");
}
