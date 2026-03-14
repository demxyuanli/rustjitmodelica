use crate::test_manager;

use super::common::jit_compiler_root;

#[tauri::command]
pub fn list_test_library() -> Result<Vec<test_manager::TestCaseInfo>, String> {
    test_manager::list_test_library(&jit_compiler_root()?)
}

#[tauri::command]
pub fn read_test_file(name: String) -> Result<String, String> {
    test_manager::read_test_file(&jit_compiler_root()?, &name)
}

#[tauri::command]
pub fn write_test_file(name: String, content: String) -> Result<(), String> {
    test_manager::write_test_file(&jit_compiler_root()?, &name, &content)
}

#[tauri::command]
pub fn delete_test_file(name: String) -> Result<(), String> {
    test_manager::delete_test_file(&jit_compiler_root()?, &name)
}

#[tauri::command]
pub fn run_single_test(name: String) -> Result<test_manager::TestRunResult, String> {
    test_manager::run_single_test(&jit_compiler_root()?, &name)
}

#[tauri::command]
pub fn run_test_suite(
    names: Vec<String>,
    suite: Option<String>,
) -> Result<test_manager::TestSuiteResult, String> {
    test_manager::run_test_suite(&jit_compiler_root()?, &names, suite.as_deref())
}

#[tauri::command]
pub fn run_full_regression() -> Result<test_manager::TestSuiteResult, String> {
    test_manager::run_full_regression(&jit_compiler_root()?)
}
