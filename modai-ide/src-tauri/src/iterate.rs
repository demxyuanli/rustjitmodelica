// Self-iteration: sandbox build/test and optional benchmark for compiler patches.
// Enhanced with dynamic test suite selection and traceability config integration.

use std::fs;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::process::Command;

use patch_apply::{apply as patch_apply_fn, Patch};

const SMOKE_CASES: &[(&str, &str)] = &[
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

/// Normalize diff so patch_apply parser accepts it: (1) context lines must start with space - replace Unicode space with ASCII; (2) blank lines in hunk body must be " \n" per unified diff, not bare "\n".
fn normalize_diff_line_starts(diff: &str) -> String {
    let mut out = String::with_capacity(diff.len());
    for line in diff.lines() {
        if line.is_empty() {
            out.push_str(" \n");
            continue;
        }
        let mut it = line.chars();
        if let Some(c) = it.next() {
            let first = if c == '+' || c == '-' {
                c
            } else if c.is_whitespace() && c != '\n' && c != '\r' {
                ' '
            } else {
                c
            };
            out.push(first);
            out.extend(it);
        }
        out.push('\n');
    }
    out
}

/// Apply a unified diff to a directory (like `patch -p1`). Uses patch_apply crate; no external `patch` binary.
/// Catches panic from patch_apply (parser "remaining input" or apply index out of bounds) and returns Err instead of crashing.
/// Normalizes line endings and ensures the diff ends with \\n. Replaces Unicode space at line start with ASCII space so parser accepts context lines (e.g. " pub mod annotation;").
pub fn apply_diff_to_dir(diff: &str, work_dir: &Path) -> Result<(), String> {
    let diff: String = diff.replace("\r\n", "\n").replace('\r', "\n");
    let diff = diff.trim_end().to_string();
    let diff = normalize_diff_line_starts(&diff);
    let diff = if diff.is_empty() {
        diff
    } else if diff.ends_with('\n') {
        diff
    } else {
        format!("{}\n", diff)
    };
    let work_dir = work_dir.to_path_buf();
    let result = catch_unwind(AssertUnwindSafe(|| apply_diff_to_dir_impl(diff.as_str(), &work_dir)));
    match result {
        Ok(Ok(())) => Ok(()),
        Ok(Err(e)) => Err(e),
        Err(panic_payload) => {
            let msg = panic_payload
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| panic_payload.downcast_ref::<&'static str>().copied())
                .unwrap_or("patch parse/apply panic");
            Err(format!("Patch failed: {}", msg))
        }
    }
}

fn apply_diff_to_dir_impl(diff: &str, work_dir: &Path) -> Result<(), String> {
    let patches = Patch::from_multiple(diff).map_err(|e| format!("Parse patch: {}", e))?;
    for patch in patches {
        let path_str = patch
            .new
            .path
            .strip_prefix("a/")
            .or_else(|| patch.new.path.strip_prefix("b/"))
            .unwrap_or(patch.new.path.as_ref());
        let rel_path = path_str.replace('/', std::path::MAIN_SEPARATOR_STR);
        let file_path = work_dir.join(&rel_path);
        let existing = fs::read_to_string(&file_path).unwrap_or_default();
        let patch_owned = patch.clone();
        let new_content = match catch_unwind(AssertUnwindSafe(|| {
            patch_apply_fn(existing, patch_owned)
        })) {
            Ok(s) => s,
            Err(panic_payload) => {
                let msg = panic_payload
                    .downcast_ref::<String>()
                    .map(|s| s.as_str())
                    .or_else(|| panic_payload.downcast_ref::<&'static str>().copied())
                    .unwrap_or("internal error");
                return Err(format!("Patch apply failed for {}: {}", rel_path, msg));
            }
        };
        if let Some(parent) = file_path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::write(&file_path, new_content).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Copy directory tree iteratively (no recursion) to avoid stack overflow on large repos.
fn copy_dir_all(src: &Path, dst: &Path, exclude: &[&str]) -> Result<(), String> {
    let mut stack: Vec<(PathBuf, PathBuf)> = vec![(src.to_path_buf(), dst.to_path_buf())];
    while let Some((s, d)) = stack.pop() {
        if s.is_file() {
            if let Some(p) = d.parent() {
                fs::create_dir_all(p).map_err(|e| e.to_string())?;
            }
            fs::copy(&s, &d).map_err(|e| e.to_string())?;
            continue;
        }
        if s.is_dir() {
            let name = s.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if exclude.contains(&name) {
                continue;
            }
            fs::create_dir_all(&d).map_err(|e| e.to_string())?;
            for e in fs::read_dir(&s).map_err(|e| e.to_string())? {
                let e = e.map_err(|e| e.to_string())?;
                let p = e.path();
                let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                if exclude.contains(&name.as_str()) {
                    continue;
                }
                stack.push((p, d.join(&name)));
            }
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
    /// True when only cargo check was run; full build/test/mo not done yet.
    pub quick_run: bool,
}

fn load_cases_from_config(rustmodlica_path: &Path) -> Vec<(String, String)> {
    let config_path = rustmodlica_path.join("jit_traceability.json");
    if let Ok(content) = fs::read_to_string(&config_path) {
        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
            if let Some(cases) = val.get("cases").and_then(|c| c.as_array()) {
                return cases
                    .iter()
                    .filter_map(|c| {
                        let name = c.get("name")?.as_str()?.to_string();
                        let expected = c.get("expected")?.as_str()?.to_string();
                        Some((name, expected))
                    })
                    .collect();
            }
        }
    }
    SMOKE_CASES
        .iter()
        .map(|(n, e)| (n.to_string(), e.to_string()))
        .collect()
}

fn run_mo_cases(
    exe: &Path,
    work_dir: &Path,
    cases: &[(String, String)],
) -> MoRunResult {
    let mut details = Vec::with_capacity(cases.len());
    let mut passed = 0usize;
    let mut failed = 0usize;
    for (model_name, expected) in cases {
        let out = match Command::new(exe)
            .args(["--t-end=1", model_name.as_str()])
            .current_dir(work_dir)
            .output()
        {
            Ok(o) => o,
            Err(_) => {
                details.push(MoRunDetail {
                    name: model_name.clone(),
                    expected: expected.clone(),
                    actual: "fail".to_string(),
                });
                failed += 1;
                continue;
            }
        };
        let actual = if out.status.success() { "pass" } else { "fail" };
        let ok = actual == expected;
        if ok {
            passed += 1;
        } else {
            failed += 1;
        }
        details.push(MoRunDetail {
            name: model_name.clone(),
            expected: expected.clone(),
            actual: actual.to_string(),
        });
    }
    MoRunResult { passed, failed, details }
}

pub fn self_iterate_impl(
    rustmodlica_path: &Path,
    diff_content: Option<&str>,
    quick: bool,
) -> Result<IterationResult, String> {
    let start = std::time::Instant::now();
    let temp = tempfile::tempdir().map_err(|e| e.to_string())?;
    let sandbox = temp.path();
    copy_dir_all(
        rustmodlica_path,
        &sandbox.join("rustmodlica"),
        &["target", ".git"],
    )?;

    let work_dir = sandbox.join("rustmodlica");
    if let Some(diff) = diff_content {
        if let Err(e) = apply_diff_to_dir(diff, &work_dir) {
            return Ok(IterationResult {
                success: false,
                build_ok: false,
                test_ok: false,
                message: format!("Patch apply failed: {}", e),
                diff: Some(diff.to_string()),
                mo_run: None,
                quick_run: false,
            });
        }
    }

    if quick {
        let check = Command::new("cargo")
            .args(["check"])
            .current_dir(&work_dir)
            .output()
            .map_err(|e| e.to_string())?;
        let ok = check.status.success();
        let message = if ok {
            format!("Check OK. ({}ms) Run full build to compile, test and run mo cases.", start.elapsed().as_millis())
        } else {
            format!("Check failed: {}", String::from_utf8_lossy(&check.stderr))
        };
        return Ok(IterationResult {
            success: ok,
            build_ok: ok,
            test_ok: true,
            message,
            diff: diff_content.map(String::from),
            mo_run: None,
            quick_run: true,
        });
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
            quick_run: false,
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
            let cases = load_cases_from_config(rustmodlica_path);
            let result = run_mo_cases(&exe, &work_dir, &cases);
            if result.failed > 0 {
                message = format!(
                    "Build and test OK; mo cases: {} passed, {} failed. ({}ms)",
                    result.passed, result.failed, start.elapsed().as_millis()
                );
            } else {
                message = format!(
                    "Build and test OK; mo cases: {} passed. ({}ms)",
                    result.passed, start.elapsed().as_millis()
                );
            }
            Some(result)
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
        quick_run: false,
    })
}

#[cfg(test)]
mod tests {
    use super::{self_iterate_impl, IterationResult};
    use std::path::PathBuf;

    fn repo_root_for_test() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("jit-compiler")
    }

    /// Full JIT self-iterate pipeline: sandbox copy, release build, test, mo cases.
    /// Asserts that the pipeline succeeds and TestLib/SmoothTest (compiler extension regression) passes.
    #[test]
    fn self_iterate_full_pipeline_includes_smooth_test() {
        let root = repo_root_for_test();
        let result: Result<IterationResult, String> = self_iterate_impl(&root, None, false);
        let r = result.expect("self_iterate_impl should not return Err");
        assert!(r.build_ok, "build should succeed: {}", r.message);
        assert!(r.test_ok, "cargo test should succeed: {}", r.message);
        assert!(r.success, "overall success: {}", r.message);
        assert!(!r.quick_run, "full run should set quick_run false");
        let mo_run = r.mo_run.expect("full run should have mo_run");
        let smooth = mo_run
            .details
            .iter()
            .find(|d| d.name == "TestLib/SmoothTest")
            .expect("mo_run should include TestLib/SmoothTest (compiler extension regression case)");
        assert_eq!(smooth.actual, "pass", "TestLib/SmoothTest should pass");
    }
}
