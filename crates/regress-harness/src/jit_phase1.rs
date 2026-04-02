use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

fn discover_rustmodlica_exe(repo_root: &Path) -> Option<PathBuf> {
    let candidates = [
        repo_root.join("jit-compiler/target_regression/release/rustmodlica.exe"),
        repo_root.join("jit-compiler/target_regression/debug/rustmodlica.exe"),
        repo_root.join("target/release/rustmodlica.exe"),
        repo_root.join("target/debug/rustmodlica.exe"),
        repo_root.join("jit-compiler/target_regression/release/rustmodlica"),
        repo_root.join("jit-compiler/target_regression/debug/rustmodlica"),
        repo_root.join("target/release/rustmodlica"),
        repo_root.join("target/debug/rustmodlica"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

fn resolve_cargo_target_dir(repo_root: &Path, cargo_target_subdir: Option<&str>) -> PathBuf {
    if let Some(sub) = cargo_target_subdir {
        let sub = sub.trim().trim_start_matches(['\\', '/']);
        if !sub.is_empty() {
            return repo_root.join("jit-compiler").join(sub);
        }
    }
    repo_root.join("jit-compiler").join("target_regression")
}

fn run_cargo_rustmodlica(
    repo_root: &Path,
    cargo_target_dir: &Path,
    args_after_dashdash: &[String],
    capture_stdout: bool,
) -> Result<(i32, String)> {
    let mut cmd = Command::new("cargo");
    cmd.current_dir(repo_root.join("jit-compiler"));
    cmd.arg("run")
        .arg("--target-dir")
        .arg(cargo_target_dir)
        .arg("-p")
        .arg("rustmodlica")
        .arg("--bin")
        .arg("rustmodlica")
        .arg("--release")
        .arg("--");
    for a in args_after_dashdash {
        cmd.arg(a);
    }
    if capture_stdout {
        let out = cmd.output().with_context(|| "spawn cargo run")?;
        let code = out.status.code().unwrap_or(1);
        let text = String::from_utf8_lossy(&out.stdout).to_string()
            + &String::from_utf8_lossy(&out.stderr);
        Ok((code, text))
    } else {
        let st = cmd.status().with_context(|| "spawn cargo run")?;
        Ok((st.code().unwrap_or(1), String::new()))
    }
}

pub fn emit_c_recursive_func(repo_root: &Path) -> Result<i32> {
    let exe = discover_rustmodlica_exe(repo_root)
        .ok_or_else(|| anyhow::anyhow!("rustmodlica binary not found (auto discovery failed)"))?;
    let jit = repo_root.join("jit-compiler");
    let out_dir = PathBuf::from("build_regress_emit");

    let mut cmd = Command::new(&exe);
    cmd.current_dir(&jit);
    cmd.arg(format!("--emit-c={}", out_dir.display()));
    cmd.arg("TestLib/RecursiveFunc");
    let st = cmd.status().with_context(|| format!("spawn {}", exe.display()))?;
    let code = st.code().unwrap_or(1);
    if code != 0 {
        return Ok(code);
    }

    let model_c = jit.join(&out_dir).join("model.c");
    if !model_c.is_file() {
        return Ok(2);
    }
    Ok(0)
}

pub fn emit_c_string_arg_ext_func(repo_root: &Path, cargo_target_subdir: Option<&str>) -> Result<i32> {
    let _exe = discover_rustmodlica_exe(repo_root)
        .ok_or_else(|| anyhow::anyhow!("rustmodlica binary not found (auto discovery failed)"))?;
    let td = resolve_cargo_target_dir(repo_root, cargo_target_subdir);

    let out_dir = PathBuf::from("build_regress_emit_string");
    let args = vec![
        format!("--emit-c={}", out_dir.display()),
        "TestLib/StringArgExtFunc".to_string(),
    ];
    let (code, _) = run_cargo_rustmodlica(repo_root, &td, &args, false)?;
    if code != 0 {
        return Ok(code);
    }

    let model_c = repo_root
        .join("jit-compiler")
        .join(&out_dir)
        .join("model.c");
    if !model_c.is_file() {
        return Ok(2);
    }
    let c = std::fs::read_to_string(&model_c)
        .with_context(|| format!("read {}", model_c.display()))?;
    let ok = c.contains("const char*") && c.contains("extLog") && c.contains("test");
    Ok(if ok { 0 } else { 3 })
}

pub fn backend_dae_info_clocked(repo_root: &Path, cargo_target_subdir: Option<&str>) -> Result<i32> {
    let _exe = discover_rustmodlica_exe(repo_root)
        .ok_or_else(|| anyhow::anyhow!("rustmodlica binary not found (auto discovery failed)"))?;
    let td = resolve_cargo_target_dir(repo_root, cargo_target_subdir);

    let args = vec![
        "--backend-dae-info".to_string(),
        "TestLib/ClockedPartitionTest".to_string(),
    ];
    let (code, out) = run_cargo_rustmodlica(repo_root, &td, &args, true)?;
    if code != 0 {
        return Ok(code);
    }
    let ok = out.to_ascii_lowercase().contains("clocked");
    Ok(if ok { 0 } else { 4 })
}

