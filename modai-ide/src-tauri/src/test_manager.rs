// Test library management: list, CRUD, execute, regression suite.

use crate::compiler_config;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
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
    pub failure_kind: Option<String>,
    pub retries: u32,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRegressionOptions {
    pub include_modelica_examples: Option<bool>,
    pub include_modelica_test: Option<bool>,
    pub max_cases: Option<usize>,
    pub solver: Option<String>,
    pub t_end: Option<f64>,
    pub dt: Option<f64>,
    pub extra_args: Option<Vec<String>>,
}

fn classify_failure(stdout: &str, stderr: &str, exit_code: i32) -> String {
    let text = format!("{stdout}\n{stderr}").to_lowercase();
    if text.contains("model not found") {
        "model_not_found".to_string()
    } else if text.contains("newton") {
        "newton_nonconverged".to_string()
    } else if text.contains("parse") {
        "parse_error".to_string()
    } else if text.contains("timeout") {
        "timeout".to_string()
    } else if exit_code == -1 {
        "process_error".to_string()
    } else {
        "runtime_error".to_string()
    }
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

pub fn run_single_test(repo_root: &Path, name: &str) -> Result<TestRunResult, String> {
    let (exe, extra_args) = compiler_config::resolve_compiler_exe(repo_root)?;
    let start = Instant::now();
    let output = Command::new(&exe)
        .args(&extra_args)
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
        failure_kind: if output.status.success() {
            None
        } else {
            Some(classify_failure(
                &String::from_utf8_lossy(&output.stdout),
                &String::from_utf8_lossy(&output.stderr),
                exit_code,
            ))
        },
        retries: 0,
    })
}

pub fn run_test_suite(
    repo_root: &Path,
    names: &[String],
    _suite: Option<&str>,
) -> Result<TestSuiteResult, String> {
    let (exe, extra_args) = compiler_config::resolve_compiler_exe(repo_root)?;
    let start = Instant::now();
    let mut results = Vec::new();
    let mut passed = 0usize;
    let mut failed = 0usize;
    let max_retries = 1u32;
    for name in names {
        let mut retries = 0u32;
        let mut last_result: Option<TestRunResult> = None;
        loop {
            let t_start = Instant::now();
            let output = Command::new(&exe)
                .args(&extra_args)
                .args(["--t-end=1", name.as_str()])
                .current_dir(repo_root)
                .output();
            let t_dur = t_start.elapsed();
            match output {
                Ok(out) => {
                    let ok = out.status.success();
                    let exit_code = out.status.code().unwrap_or(-1);
                    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                    let failure_kind = if ok {
                        None
                    } else {
                        Some(classify_failure(&stdout, &stderr, exit_code))
                    };
                    let current = TestRunResult {
                        name: name.clone(),
                        passed: ok,
                        exit_code,
                        stdout,
                        stderr,
                        duration_ms: t_dur.as_millis() as u64,
                        failure_kind,
                        retries,
                    };
                    let should_retry = !ok && retries < max_retries;
                    last_result = Some(current);
                    if should_retry {
                        retries += 1;
                        continue;
                    }
                }
                Err(e) => {
                    let current = TestRunResult {
                        name: name.clone(),
                        passed: false,
                        exit_code: -1,
                        stdout: String::new(),
                        stderr: e.to_string(),
                        duration_ms: t_dur.as_millis() as u64,
                        failure_kind: Some("process_error".to_string()),
                        retries,
                    };
                    let should_retry = retries < max_retries;
                    last_result = Some(current);
                    if should_retry {
                        retries += 1;
                        continue;
                    }
                }
            }
            break;
        }
        if let Some(result) = last_result {
            if result.passed {
                passed += 1;
            } else {
                failed += 1;
            }
            results.push(result);
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

fn collect_mo_files(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_mo_files(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "mo") {
            out.push(path);
        }
    }
    Ok(())
}

fn model_name_from_path(root_name: &str, root: &Path, file: &Path) -> Option<String> {
    if file.file_name().and_then(|n| n.to_str()) == Some("package.mo") {
        return None;
    }
    let rel = file.strip_prefix(root).ok()?.to_string_lossy().replace('\\', "/");
    let stem = rel.strip_suffix(".mo")?;
    if stem.is_empty() {
        return None;
    }
    Some(format!("{root_name}.{}", stem.replace('/', ".")))
}

pub fn run_library_regression(
    repo_root: &Path,
    options: Option<LibraryRegressionOptions>,
) -> Result<TestSuiteResult, String> {
    let opts = options.unwrap_or(LibraryRegressionOptions {
        include_modelica_examples: Some(true),
        include_modelica_test: Some(true),
        max_cases: Some(0),
        solver: Some("rk4".to_string()),
        t_end: Some(2.0),
        dt: Some(0.01),
        extra_args: Some(Vec::new()),
    });
    let include_examples = opts.include_modelica_examples.unwrap_or(true);
    let include_modelica_test = opts.include_modelica_test.unwrap_or(true);
    let mut models: Vec<String> = Vec::new();

    if include_examples {
        let modelica_root = repo_root.join("Modelica");
        if modelica_root.is_dir() {
            let mut files = Vec::new();
            collect_mo_files(&modelica_root, &mut files)?;
            for f in files {
                let rel = f
                    .strip_prefix(&modelica_root)
                    .ok()
                    .map(|p| p.to_string_lossy().replace('\\', "/"))
                    .unwrap_or_default();
                if rel.contains("/Examples/") {
                    if let Some(name) = model_name_from_path("Modelica", &modelica_root, &f) {
                        models.push(name);
                    }
                }
            }
        }
    }

    if include_modelica_test {
        let modelica_test_root = repo_root.join("ModelicaTest");
        if modelica_test_root.is_dir() {
            let mut files = Vec::new();
            collect_mo_files(&modelica_test_root, &mut files)?;
            for f in files {
                if let Some(name) = model_name_from_path("ModelicaTest", &modelica_test_root, &f) {
                    models.push(name);
                }
            }
        }
    }

    models.sort();
    models.dedup();
    let max_cases = opts.max_cases.unwrap_or(0);
    if max_cases > 0 && models.len() > max_cases {
        models.truncate(max_cases);
    }

    let (exe, mut base_args) = compiler_config::resolve_compiler_exe(repo_root)?;
    let start = Instant::now();
    let mut results = Vec::with_capacity(models.len());
    let mut passed = 0usize;
    let mut failed = 0usize;
    let solver = opts.solver.unwrap_or_else(|| "rk4".to_string());
    let t_end = opts.t_end.unwrap_or(2.0);
    let dt = opts.dt.unwrap_or(0.01);
    base_args.push(format!("--solver={solver}"));
    base_args.push(format!("--t-end={t_end}"));
    base_args.push(format!("--dt={dt}"));
    if let Some(extra) = opts.extra_args {
        base_args.extend(extra);
    }

    for model in models {
        let single_start = Instant::now();
        let output = Command::new(&exe)
            .args(&base_args)
            .arg(&model)
            .current_dir(repo_root)
            .output();
        let duration_ms = single_start.elapsed().as_millis() as u64;
        match output {
            Ok(out) => {
                let ok = out.status.success();
                let exit_code = out.status.code().unwrap_or(-1);
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                if ok {
                    passed += 1;
                } else {
                    failed += 1;
                }
                results.push(TestRunResult {
                    name: model,
                    passed: ok,
                    exit_code,
                    stdout: stdout.clone(),
                    stderr: stderr.clone(),
                    duration_ms,
                    failure_kind: if ok {
                        None
                    } else {
                        Some(classify_failure(&stdout, &stderr, exit_code))
                    },
                    retries: 0,
                });
            }
            Err(e) => {
                failed += 1;
                results.push(TestRunResult {
                    name: model,
                    passed: false,
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: e.to_string(),
                    duration_ms,
                    failure_kind: Some("process_error".to_string()),
                    retries: 0,
                });
            }
        }
    }

    Ok(TestSuiteResult {
        total: results.len(),
        passed,
        failed,
        results,
        duration_ms: start.elapsed().as_millis() as u64,
    })
}
