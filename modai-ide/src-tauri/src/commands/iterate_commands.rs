use std::process::Command;

use crate::iterate;

use super::common::jit_compiler_root;

#[tauri::command]
pub fn self_iterate(
    diff: Option<String>,
    quick: Option<bool>,
) -> Result<iterate::IterationResult, String> {
    let root = jit_compiler_root()?;
    iterate::self_iterate_impl(&root, diff.as_deref(), quick.unwrap_or(true))
}

#[tauri::command]
pub fn apply_patch_to_workspace(diff: String) -> Result<(), String> {
    let work_dir = jit_compiler_root()?;
    iterate::apply_diff_to_dir(&diff, &work_dir)
}

#[tauri::command]
pub fn commit_patch(message: String) -> Result<(), String> {
    let work_dir = jit_compiler_root()?;
    let add = Command::new("git")
        .args(["add", "-A"])
        .current_dir(&work_dir)
        .output()
        .map_err(|e| e.to_string())?;
    if !add.status.success() {
        return Err(format!(
            "git add failed: {}",
            String::from_utf8_lossy(&add.stderr)
        ));
    }
    let commit = Command::new("git")
        .args(["commit", "-m", &message])
        .current_dir(&work_dir)
        .output()
        .map_err(|e| e.to_string())?;
    if !commit.status.success() {
        return Err(format!(
            "git commit failed: {}",
            String::from_utf8_lossy(&commit.stderr)
        ));
    }
    Ok(())
}
