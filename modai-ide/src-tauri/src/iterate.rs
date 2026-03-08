// Self-iteration: sandbox build/test and optional benchmark for compiler patches.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

/// Model name (path) and expected outcome: "pass" (exit 0) or "fail" (non-zero).
const MO_CASES: &[(&str, &str)] = &[
    ("TestLib/InitDummy", "pass"),
    ("TestLib/JacobianTest", "pass"),
    ("TestLib/AlgebraicLoop2Eq", "pass"),
    ("TestLib/SolvableBlock4Res", "pass"),
    ("TestLib/WhenTest", "pass"),
    ("TestLib/BouncingBall", "pass"),
    ("TestLib/FuncInline", "pass"),
    ("TestLib/NoEventTest", "pass"),
    ("TestLib/TerminalWhen", "pass"),
    ("TestLib/SimpleBlockTest", "pass"),
    ("TestLib/SimpleTest", "pass"),
    ("TestLib/BadConnect", "fail"),
];

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
pub struct MoRunDetail {
    pub name: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, serde::Serialize)]
pub struct MoRunResult {
    pub passed: usize,
    pub failed: usize,
    pub details: Vec<MoRunDetail>,
}

#[derive(Debug, serde::Serialize)]
pub struct IterationResult {
    pub success: bool,
    pub build_ok: bool,
    pub test_ok: bool,
    pub message: String,
    pub diff: Option<String>,
    pub mo_run: Option<MoRunResult>,
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
                mo_run: None,
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
            mo_run: None,
        });
    }

    let test = Command::new("cargo")
        .args(["test", "--release"])
        .current_dir(&work_dir)
        .output()
        .map_err(|e| e.to_string())?;

    let test_ok = test.status.success();
    let mut message = if test_ok {
        "Build and test OK.".to_string()
    } else {
        format!("Test failed: {}", String::from_utf8_lossy(&test.stderr))
    };

    let mo_run = if test_ok {
        let exe = work_dir
            .join("target/release/rustmodlica")
            .with_extension(std::env::consts::EXE_EXTENSION);
        if exe.exists() {
            let mut details = Vec::with_capacity(MO_CASES.len());
            let mut passed = 0usize;
            let mut failed = 0usize;
            for (model_name, expected) in MO_CASES {
                let out = match Command::new(&exe)
                    .args([*model_name, "--t-end", "1"])
                    .current_dir(&work_dir)
                    .output()
                {
                    Ok(o) => o,
                    Err(_) => {
                        details.push(MoRunDetail {
                            name: (*model_name).to_string(),
                            expected: (*expected).to_string(),
                            actual: "fail".to_string(),
                        });
                        failed += 1;
                        continue;
                    }
                };
                let actual = if out.status.success() { "pass" } else { "fail" };
                let ok = actual == *expected;
                if ok {
                    passed += 1;
                } else {
                    failed += 1;
                }
                details.push(MoRunDetail {
                    name: (*model_name).to_string(),
                    expected: (*expected).to_string(),
                    actual: actual.to_string(),
                });
            }
            if failed > 0 {
                message = format!("Build and test OK; mo cases: {} passed, {} failed.", passed, failed);
            } else {
                message = format!("Build and test OK; mo cases: {} passed.", passed);
            }
            Some(MoRunResult { passed, failed, details })
        } else {
            None
        }
    } else {
        None
    };

    let mo_ok = mo_run.as_ref().map_or(true, |r| r.failed == 0);
    let success = build.status.success() && test_ok && mo_ok;

    Ok(IterationResult {
        success,
        build_ok: build.status.success(),
        test_ok,
        message,
        diff: diff_content.map(String::from),
        mo_run,
    })
}
