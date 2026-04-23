mod app_data;
mod app_settings;
mod ai;
mod ai_tools;
mod chunker;
mod commands;
mod component_library;
mod component_library_index;
mod compiler_config;
mod db;
mod diagram;
mod equation_graph_actor;
mod msl_pack_bootstrap;
mod file_watcher;
mod git;
mod index_db;
mod index_manager;
mod iterate;
mod profiler;
mod source_manager;
mod test_manager;
mod traceability;

use commands::ai_commands::{
    ai_code_gen, ai_code_gen_stream, ai_generate_compiler_patch, ai_generate_compiler_patch_with_context,
    get_api_key, set_api_key, get_grok_api_key, set_grok_api_key,
};
use commands::app_commands::{
    get_app_data_root, get_app_settings, get_compiler_config, get_iteration, greet,
    list_iteration_history, open_devtools, rebuild_component_library_index, save_iteration,
    set_app_settings, set_compiler_config,
};
use commands::git_commands::{
    git_commit, git_commit_files, git_diff_file, git_diff_file_staged, git_head_commit, git_init,
    git_is_repo, git_log, git_log_graph, git_show_file, git_stage, git_status, git_unstage,
};
use commands::index_commands::{
    index_build, index_build_repo, index_component_library_get_context, index_file_symbols,
    index_find_references, index_get_context,
    index_get_dependencies, index_list_included_files, index_refresh, index_refresh_repo,
    index_rebuild, index_rebuild_repo,
    index_repo_file_symbols, index_repo_get_context, index_repo_root, index_repo_search_symbols,
    index_repo_stats, index_search_symbols, index_start_watcher, index_stats, index_stop_watcher,
    index_update_file,
};
use commands::iterate_commands::{apply_patch_to_project, apply_patch_to_workspace, commit_patch, self_iterate};
use commands::jit::{
    get_equation_graph, get_equation_graph_v2, get_monitor_events, get_simulation_state, jit_validate, jit_validate_v2,
    list_monitor_event_sessions, run_simulation_cmd, run_simulation_cmd_v2,
    simulation_command, simulation_step, start_simulation_session,
};
use commands::msl_cache::{
    msl_cache_check_update, msl_cache_clear, msl_cache_download_update, msl_cache_rebuild_local, msl_cache_status,
};
use commands::project::{
    add_component_library, apply_diagram_edits, apply_equation_edits,
    apply_graphical_document_edits, extract_equations_from_source,
    get_component_type_details, get_component_type_relation_graph, get_diagram_data,
    get_diagram_data_from_source, get_graphical_document, get_graphical_document_from_source,
    install_third_party_library_from_git, list_component_libraries, list_instantiable_classes,
    list_mo_files, list_mo_tree, open_project_dir, pick_component_library_files,
    pick_component_library_folder, query_component_library_types, read_component_type_source,
    read_project_file, remove_component_library, reopen_project_dir, search_in_project,
    set_component_library_enabled, suggest_library_for_missing_type,
    sync_all_third_party_libraries, sync_third_party_library, write_project_file,
};
use commands::regression_commands::{
    regression_cancel_workspace, regression_create_workspace, regression_get_workspace_state,
    regression_list_workspaces, regression_run_workspace,
};
use commands::source_commands::{
    compiler_file_git_diff, compiler_file_git_log, create_iteration_branch,
    list_compiler_source_tree, list_iteration_branches, merge_iteration_branch,
    read_compiler_file, switch_iteration_branch, write_compiler_file,
};
use commands::test_commands::{
    delete_test_file, list_test_library, read_test_file, run_full_regression, run_single_test,
    run_library_regression, run_test_suite, write_test_file,
};
use commands::traceability_commands::{
    get_traceability_matrix, load_traceability_config, save_traceability_config,
    traceability_build_execution_plan,
    traceability_apply_sync, traceability_coverage_analysis, traceability_git_impact,
    traceability_impact_analysis, traceability_sync_check, traceability_validate,
    update_traceability_link,
};

fn init_tracing_subscriber() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        EnvFilter::new("warn,modai_diagram=info")
    });
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .try_init();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_tracing_subscriber();
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            if let Err(e) = msl_pack_bootstrap::init(app.handle()) {
                eprintln!("[msl-pack] bootstrap: {e}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            open_devtools,
            jit_validate,
            jit_validate_v2,
            run_simulation_cmd,
            run_simulation_cmd_v2,
            get_api_key,
            set_api_key,
            get_grok_api_key,
            set_grok_api_key,
            ai_code_gen,
            ai_code_gen_stream,
            self_iterate,
            apply_patch_to_project,
            apply_patch_to_workspace,
            commit_patch,
            open_project_dir,
            reopen_project_dir,
            pick_component_library_folder,
            pick_component_library_files,
            list_mo_files,
            list_mo_tree,
            read_project_file,
            write_project_file,
            search_in_project,
            get_equation_graph,
            get_equation_graph_v2,
            msl_cache_status,
            msl_cache_clear,
            msl_cache_check_update,
            msl_cache_download_update,
            msl_cache_rebuild_local,
            start_simulation_session,
            simulation_step,
            simulation_command,
            get_monitor_events,
            list_monitor_event_sessions,
            get_simulation_state,
            get_diagram_data,
            get_diagram_data_from_source,
            apply_diagram_edits,
            get_graphical_document,
            get_graphical_document_from_source,
            apply_graphical_document_edits,
            extract_equations_from_source,
            apply_equation_edits,
            list_component_libraries,
            add_component_library,
            remove_component_library,
            set_component_library_enabled,
            install_third_party_library_from_git,
            sync_third_party_library,
            sync_all_third_party_libraries,
            suggest_library_for_missing_type,
            list_instantiable_classes,
            query_component_library_types,
            get_component_type_details,
            get_component_type_relation_graph,
            read_component_type_source,
            regression_create_workspace,
            regression_run_workspace,
            regression_get_workspace_state,
            regression_list_workspaces,
            regression_cancel_workspace,
            ai_generate_compiler_patch,
            ai_generate_compiler_patch_with_context,
            list_iteration_history,
            get_iteration,
            save_iteration,
            git_head_commit,
            git_is_repo,
            git_init,
            git_status,
            git_diff_file,
            git_diff_file_staged,
            git_show_file,
            git_log,
            git_stage,
            git_unstage,
            git_commit,
            git_commit_files,
            git_log_graph,
            load_traceability_config,
            save_traceability_config,
            get_traceability_matrix,
            traceability_impact_analysis,
            traceability_coverage_analysis,
            update_traceability_link,
            traceability_sync_check,
            traceability_validate,
            traceability_apply_sync,
            traceability_git_impact,
            traceability_build_execution_plan,
            list_compiler_source_tree,
            read_compiler_file,
            write_compiler_file,
            compiler_file_git_log,
            compiler_file_git_diff,
            create_iteration_branch,
            list_iteration_branches,
            switch_iteration_branch,
            merge_iteration_branch,
            list_test_library,
            read_test_file,
            write_test_file,
            delete_test_file,
            run_single_test,
            run_test_suite,
            run_full_regression,
            run_library_regression,
            get_compiler_config,
            set_compiler_config,
            get_app_data_root,
            get_app_settings,
            rebuild_component_library_index,
            set_app_settings,
            index_build,
            index_update_file,
            index_search_symbols,
            index_file_symbols,
            index_find_references,
            index_get_context,
            index_get_dependencies,
            index_stats,
            index_list_included_files,
            index_start_watcher,
            index_stop_watcher,
            index_refresh,
            index_rebuild,
            index_refresh_repo,
            index_rebuild_repo,
            index_repo_root,
            index_build_repo,
            index_repo_stats,
            index_repo_file_symbols,
            index_repo_search_symbols,
            index_repo_get_context,
            index_component_library_get_context,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
