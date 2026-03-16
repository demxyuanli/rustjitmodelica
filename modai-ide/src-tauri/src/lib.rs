mod app_data;
mod app_settings;
mod ai;
mod ai_tools;
mod chunker;
mod commands;
mod component_library;
mod compiler_config;
mod db;
mod diagram;
mod file_watcher;
mod git;
mod index_db;
mod index_manager;
mod iterate;
mod source_manager;
mod test_manager;
mod traceability;

use commands::ai_commands::{
    ai_code_gen, ai_generate_compiler_patch, ai_generate_compiler_patch_with_context, get_api_key,
    set_api_key,
};
use commands::app_commands::{
    get_app_data_root, get_app_settings, get_compiler_config, get_iteration, greet,
    list_iteration_history, open_devtools, save_iteration, set_app_settings, set_compiler_config,
};
use commands::git_commands::{
    git_commit, git_commit_files, git_diff_file, git_diff_file_staged, git_head_commit, git_init,
    git_is_repo, git_log, git_log_graph, git_show_file, git_stage, git_status, git_unstage,
};
use commands::index_commands::{
    index_build, index_build_repo, index_file_symbols, index_find_references, index_get_context,
    index_get_dependencies, index_refresh, index_refresh_repo, index_rebuild, index_rebuild_repo,
    index_repo_file_symbols, index_repo_get_context, index_repo_root, index_repo_search_symbols,
    index_repo_stats, index_search_symbols, index_start_watcher, index_stats, index_stop_watcher,
    index_update_file,
};
use commands::iterate_commands::{apply_patch_to_project, apply_patch_to_workspace, commit_patch, self_iterate};
use commands::jit::{
    get_equation_graph, get_simulation_state, jit_validate, run_simulation_cmd,
    simulation_command, simulation_step, start_simulation_session,
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
use commands::source_commands::{
    compiler_file_git_diff, compiler_file_git_log, create_iteration_branch,
    list_compiler_source_tree, list_iteration_branches, merge_iteration_branch,
    read_compiler_file, switch_iteration_branch, write_compiler_file,
};
use commands::test_commands::{
    delete_test_file, list_test_library, read_test_file, run_full_regression, run_single_test,
    run_test_suite, write_test_file,
};
use commands::traceability_commands::{
    get_traceability_matrix, load_traceability_config, save_traceability_config,
    traceability_apply_sync, traceability_coverage_analysis, traceability_git_impact,
    traceability_impact_analysis, traceability_sync_check, traceability_validate,
    update_traceability_link,
};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            open_devtools,
            jit_validate,
            run_simulation_cmd,
            get_api_key,
            set_api_key,
            ai_code_gen,
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
            start_simulation_session,
            simulation_step,
            simulation_command,
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
            get_compiler_config,
            set_compiler_config,
            get_app_data_root,
            get_app_settings,
            set_app_settings,
            index_build,
            index_update_file,
            index_search_symbols,
            index_file_symbols,
            index_find_references,
            index_get_context,
            index_get_dependencies,
            index_stats,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
