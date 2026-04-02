use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

fn discover_rustmodlica_exe(repo_root: &Path, cargo_target_subdir: Option<&str>) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    if let Some(sub) = cargo_target_subdir {
        let sub = sub.trim().trim_start_matches(['\\', '/']);
        if !sub.is_empty() {
            candidates.push(repo_root.join("jit-compiler").join(sub).join("release").join("rustmodlica.exe"));
            candidates.push(repo_root.join("jit-compiler").join(sub).join("debug").join("rustmodlica.exe"));
            candidates.push(repo_root.join("jit-compiler").join(sub).join("release").join("rustmodlica"));
            candidates.push(repo_root.join("jit-compiler").join(sub).join("debug").join("rustmodlica"));
        }
    }

    candidates.push(repo_root.join("jit-compiler/target_regression/release/rustmodlica.exe"));
    candidates.push(repo_root.join("jit-compiler/target_regression/debug/rustmodlica.exe"));
    candidates.push(repo_root.join("target/release/rustmodlica.exe"));
    candidates.push(repo_root.join("target/debug/rustmodlica.exe"));
    candidates.push(repo_root.join("target/release/rustmodlica"));
    candidates.push(repo_root.join("target/debug/rustmodlica"));

    candidates.into_iter().find(|p| p.is_file())
}

fn run_validate(
    exe: &Path,
    lib_paths: &[PathBuf],
    model_name: &str,
    negative_extra_lib: Option<&PathBuf>,
) -> Result<String> {
    let mut cmd = Command::new(exe);
    for lp in lib_paths {
        cmd.arg(format!("--lib-path={}", lp.display()));
    }
    if let Some(neg) = negative_extra_lib {
        cmd.arg(format!("--lib-path={}", neg.display()));
    }
    cmd.arg("--validate-tier=analyze");
    cmd.arg("--validate");
    cmd.arg(model_name);
    let out = cmd.output().with_context(|| format!("spawn {}", exe.display()))?;
    let text = String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    Ok(text)
}

fn parse_success(raw: &str) -> bool {
    raw.contains("\"success\"") && raw.contains("true")
}

pub struct TestLibValidateSummary {
    pub exe: PathBuf,
    pub root_total: usize,
    pub negative_total: usize,
    pub root_pass: usize,
    pub root_unexpected_fail: Vec<String>,
    pub negative_fail_as_expected: usize,
    pub negative_unexpected_pass: Vec<String>,
}

pub fn testlib_validate_batch(repo_root: &Path, cargo_target_subdir: Option<&str>) -> Result<TestLibValidateSummary> {
    let exe = discover_rustmodlica_exe(repo_root, cargo_target_subdir)
        .ok_or_else(|| anyhow::anyhow!("rustmodlica binary not found (auto discovery failed)"))?;

    let jit = repo_root.join("jit-compiler");
    if !jit.is_dir() {
        bail!("missing directory: {}", jit.display());
    }
    let testlib_dir = jit.join("TestLib");
    let neg_dir = testlib_dir.join("negative");
    if !testlib_dir.is_dir() {
        bail!("missing directory: {}", testlib_dir.display());
    }

    let mut lib_paths = vec![testlib_dir.clone()];
    if jit.join("Modelica").join("package.mo").is_file() {
        lib_paths.insert(0, jit.clone());
    }

    let mut mos_root: Vec<PathBuf> = std::fs::read_dir(&testlib_dir)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("mo"))
        .collect();
    mos_root.sort();

    let mut mos_neg: Vec<PathBuf> = Vec::new();
    if neg_dir.is_dir() {
        mos_neg = std::fs::read_dir(&neg_dir)?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("mo"))
            .collect();
        mos_neg.sort();
    }

    let mut root_pass = 0usize;
    let mut root_fail: Vec<String> = Vec::new();
    for f in &mos_root {
        let name = f.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        let raw = run_validate(&exe, &lib_paths, &name, None)?;
        if parse_success(&raw) {
            root_pass += 1;
        } else {
            root_fail.push(name);
        }
    }

    let mut neg_ok = 0usize;
    let mut neg_unexpected_pass: Vec<String> = Vec::new();
    for f in &mos_neg {
        let name = f.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        let raw = run_validate(&exe, &lib_paths, &name, Some(&neg_dir))?;
        if parse_success(&raw) {
            neg_unexpected_pass.push(name);
        } else {
            neg_ok += 1;
        }
    }

    Ok(TestLibValidateSummary {
        exe,
        root_total: mos_root.len(),
        negative_total: mos_neg.len(),
        root_pass,
        root_unexpected_fail: root_fail,
        negative_fail_as_expected: neg_ok,
        negative_unexpected_pass: neg_unexpected_pass,
    })
}

