// Self-iteration: sandbox build/test and optional benchmark for compiler patches.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

fn copy_dir_all(src: &Path, dst: &Path, exclude: &[&str]) -> Result<(), String> {
    if src.is_file() {
        if let Some(p) = dst.parent() {
            fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        fs::copy(src, dst).map_err(|e| e.to_string())?;
        return Ok(());
    }
    if src.is_dir() {
        let name = src.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if exclude.contains(&name) {
            return Ok(());
        }
        fs::create_dir_all(dst).map_err(|e| e.to_string())?;
        for e in fs::read_dir(src).map_err(|e| e.to_string())? {
            let e = e.map_err(|e| e.to_string())?;
            let p = e.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if exclude.contains(&name) {
                continue;
            }
            copy_dir_all(&p, &dst.join(name), exclude)?;
        }
    }
    Ok(())
}

#[derive(Debug, serde::Serialize)]
pub struct IterationResult {
    pub success: bool,
    pub build_ok: bool,
    pub test_ok: bool,
    pub message: String,
    pub diff: Option<String>,
}

pub fn self_iterate_impl(
    rustmodlica_path: &Path,
    diff_content: Option<&str>,
) -> Result<IterationResult, String> {
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let sandbox = temp.path();
    copy_dir_all(
        rustmodlica_path,
        &sandbox.join("rustmodlica"),
        &["target", ".git", "modai-ide"],
    )?;

    let work_dir = sandbox.join("rustmodlica");
    if let Some(diff) = diff_content {
        let patch_path = sandbox.join("patch.diff");
        fs::write(&patch_path, diff).map_err(|e| e.to_string())?;
        let mut child = Command::new("patch")
            .args(["-p1"])
            .current_dir(&work_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("patch command failed (install patch?): {}", e))?;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(diff.as_bytes()).ok();
        }
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        if !out.status.success() {
            let stderr = String::from_utf8_lossy(&out.stderr);
            return Ok(IterationResult {
                success: false,
                build_ok: false,
                test_ok: false,
                message: format!("Patch apply failed: {}", stderr),
                diff: Some(diff.to_string()),
            });
        }
    }

    let build = Command::new("cargo")
        .args(["build", "--release"])
        .current_dir(&work_dir)
        .output()
        .map_err(|e| e.to_string())?;

    if !build.status.success() {
        let stderr = String::from_utf8_lossy(&build.stderr);
        return Ok(IterationResult {
            success: false,
            build_ok: false,
            test_ok: false,
            message: format!("Build failed: {}", stderr),
            diff: diff_content.map(String::from),
        });
    }

    let test = Command::new("cargo")
        .args(["test", "--release"])
        .current_dir(&work_dir)
        .output()
        .map_err(|e| e.to_string())?;

    let test_ok = test.status.success();
    let message = if test_ok {
        "Build and test OK.".to_string()
    } else {
        format!("Test failed: {}", String::from_utf8_lossy(&test.stderr))
    };

    Ok(IterationResult {
        success: build.status.success() && test_ok,
        build_ok: build.status.success(),
        test_ok,
        message,
        diff: diff_content.map(String::from),
    })
}
