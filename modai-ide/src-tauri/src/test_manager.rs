// Test library management: list, CRUD, execute, regression suite.

use serde::Serialize;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestCaseInfo {
    pub name: String,
    pub path: String,
    pub size_bytes: u64,
    pub last_modified: String,
    pub category: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestRunResult {
    pub name: String,
    pub passed: bool,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestSuiteResult {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub results: Vec<TestRunResult>,
    pub duration_ms: u64,
}

fn categorize_test(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("init") {
        "initialization".to_string()
    } else if lower.contains("array") || lower.contains("for") || lower.contains("loop") {
        "array".to_string()
    } else if lower.contains("connect") || lower.contains("pin") || lower.contains("circuit") {
        "connect".to_string()
    } else if lower.contains("when") || lower.contains("discrete") || lower.contains("reinit")
        || lower.contains("event") || lower.contains("pre") || lower.contains("edge")
    {
        "discrete".to_string()
    } else if lower.contains("algebraic") || lower.contains("solvable") || lower.contains("tearing")
        || lower.contains("blt") || lower.contains("jacobian")
    {
        "algebraic".to_string()
    } else if lower.contains("msl") || lower.contains("library") || lower.contains("siunits")
        || lower.contains("blocks")
    {
        "msl".to_string()
    } else if lower.contains("func") {
        "function".to_string()
    } else if lower.contains("record") || lower.contains("block") {
        "structure".to_string()
    } else if lower.contains("bad") || lower.contains("error") || lower.contains("unknown") {
        "error".to_string()
    } else if lower.contains("adaptive") || lower.contains("bouncing") || lower.contains("pendulum") {
        "solver".to_string()
    } else if lower.contains("backend") || lower.contains("dae") {
        "tooling".to_string()
    } else {
        "basic".to_string()
    }
}

pub fn list_test_library(repo_root: &Path) -> Result<Vec<TestCaseInfo>, String> {
    let test_dir = repo_root.join("TestLib");
    if !test_dir.is_dir() {
        return Ok(vec![]);
    }
    let mut cases = Vec::new();
    collect_test_files(&test_dir, "TestLib", &mut cases)?;
    cases.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(cases)
}

fn collect_test_files(dir: &Path, prefix: &str, out: &mut Vec<TestCaseInfo>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if path.is_dir() {
            collect_test_files(&path, &format!("{}/{}", prefix, name), out)?;
        } else if path.extension().map_or(false, |e| e == "mo") {
            let stem = path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();
            let model_name = format!("{}/{}", prefix, stem);
            let meta = fs::metadata(&path).map_err(|e| e.to_string())?;
            let modified = meta
                .modified()
                .ok()
                .and_then(|t| {
                    t.duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .map(|d| d.as_secs())
                })
                .unwrap_or(0);
            let rel_path = format!("{}/{}", prefix, name);
            out.push(TestCaseInfo {
                name: model_name.clone(),
                path: rel_path,
                size_bytes: meta.len(),
                last_modified: format!("{}", modified),
                category: categorize_test(&stem),
            });
        }
    }
    Ok(())
}

pub fn read_test_file(repo_root: &Path, name: &str) -> Result<String, String> {
    let mo_path = resolve_test_path(repo_root, name)?;
    fs::read_to_string(&mo_path).map_err(|e| e.to_string())
}

pub fn write_test_file(repo_root: &Path, name: &str, content: &str) -> Result<(), String> {
    let mo_path = resolve_test_path_for_write(repo_root, name)?;
    if let Some(parent) = mo_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(&mo_path, content).map_err(|e| e.to_string())
}

pub fn delete_test_file(repo_root: &Path, name: &str) -> Result<(), String> {
    let mo_path = resolve_test_path(repo_root, name)?;
    fs::remove_file(&mo_path).map_err(|e| e.to_string())
}

fn resolve_test_path(repo_root: &Path, name: &str) -> Result<std::path::PathBuf, String> {
    let normalized = name.replace('\\', "/");
    let with_ext = if normalized.ends_with(".mo") {
        normalized
    } else {
        let parts: Vec<&str> = normalized.rsplitn(2, '/').collect();
        if parts.len() == 2 {
            format!("{}/{}.mo", parts[1], parts[0])
        } else {
            format!("{}.mo", normalized)
        }
    };
    let full = repo_root.join(&with_ext);
    if !full.exists() {
        return Err(format!("Test file not found: {}", with_ext));
    }
    Ok(full)
}

fn resolve_test_path_for_write(repo_root: &Path, name: &str) -> Result<std::path::PathBuf, String> {
    let normalized = name.replace('\\', "/");
    let with_ext = if normalized.ends_with(".mo") {
        normalized
    } else {
        let parts: Vec<&str> = normalized.rsplitn(2, '/').collect();
        if parts.len() == 2 {
            format!("{}/{}.mo", parts[1], parts[0])
        } else {
            format!("TestLib/{}.mo", normalized)
        }
    };
    let full = repo_root.join(&with_ext);
    let root_canonical = repo_root.canonicalize().map_err(|e| e.to_string())?;
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let parent_canonical = parent.canonicalize().map_err(|e| e.to_string())?;
        if !parent_canonical.starts_with(&root_canonical) {
            return Err("Path is outside repository".to_string());
        }
    }
    Ok(full)
}

fn find_exe(repo_root: &Path) -> Result<std::path::PathBuf, String> {
    let release_exe = repo_root
        .join("target/release/rustmodlica")
        .with_extension(std::env::consts::EXE_EXTENSION);
    if release_exe.exists() {
        return Ok(release_exe);
    }
    let debug_exe = repo_root
        .join("target/debug/rustmodlica")
        .with_extension(std::env::consts::EXE_EXTENSION);
    if debug_exe.exists() {
        return Ok(debug_exe);
    }
    Err("rustmodlica executable not found (run cargo build first)".to_string())
}

pub fn run_single_test(repo_root: &Path, name: &str) -> Result<TestRunResult, String> {
    let exe = find_exe(repo_root)?;
    let start = Instant::now();
    let output = Command::new(&exe)
        .args(["--t-end=1", name])
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("Failed to run test {}: {}", name, e))?;
    let duration = start.elapsed();
    let exit_code = output.status.code().unwrap_or(-1);
    Ok(TestRunResult {
        name: name.to_string(),
        passed: output.status.success(),
        exit_code,
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        duration_ms: duration.as_millis() as u64,
    })
}

pub fn run_test_suite(
    repo_root: &Path,
    names: &[String],
    _suite: Option<&str>,
) -> Result<TestSuiteResult, String> {
    let exe = find_exe(repo_root)?;
    let start = Instant::now();
    let mut results = Vec::new();
    let mut passed = 0usize;
    let mut failed = 0usize;
    for name in names {
        let t_start = Instant::now();
        let output = Command::new(&exe)
            .args(["--t-end=1", name.as_str()])
            .current_dir(repo_root)
            .output();
        let t_dur = t_start.elapsed();
        match output {
            Ok(out) => {
                let ok = out.status.success();
                if ok { passed += 1; } else { failed += 1; }
                results.push(TestRunResult {
                    name: name.clone(),
                    passed: ok,
                    exit_code: out.status.code().unwrap_or(-1),
                    stdout: String::from_utf8_lossy(&out.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&out.stderr).to_string(),
                    duration_ms: t_dur.as_millis() as u64,
                });
            }
            Err(e) => {
                failed += 1;
                results.push(TestRunResult {
                    name: name.clone(),
                    passed: false,
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: e.to_string(),
                    duration_ms: t_dur.as_millis() as u64,
                });
            }
        }
    }
    Ok(TestSuiteResult {
        total: names.len(),
        passed,
        failed,
        results,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}

pub fn run_full_regression(repo_root: &Path) -> Result<TestSuiteResult, String> {
    let test_lib = repo_root.join("TestLib");
    if !test_lib.is_dir() {
        return Err("TestLib/ directory not found".to_string());
    }
    let cases = list_test_library(repo_root)?;
    let names: Vec<String> = cases.iter().map(|c| c.name.clone()).collect();
    run_test_suite(repo_root, &names, Some("full"))
}
