use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JitValidateOptions {
    pub t_end: Option<f64>,
    pub dt: Option<f64>,
    pub atol: Option<f64>,
    pub rtol: Option<f64>,
    pub solver: Option<String>,
    pub output_interval: Option<f64>,
}

pub fn repo_root() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "cannot determine repository root".to_string())
}

pub fn jit_compiler_root() -> Result<PathBuf, String> {
    Ok(repo_root()?.join("jit-compiler"))
}

pub fn project_dir_canonical(project_dir: &str) -> Result<PathBuf, String> {
    Path::new(project_dir)
        .canonicalize()
        .map_err(|e| e.to_string())
}

pub fn compiler_options_to_args(opts: Option<&JitValidateOptions>) -> Vec<String> {
    let mut args = Vec::new();
    if let Some(o) = opts {
        if let Some(v) = o.t_end {
            args.push(format!("--t-end={}", v));
        }
        if let Some(v) = o.dt {
            args.push(format!("--dt={}", v));
        }
        if let Some(v) = o.atol {
            args.push(format!("--atol={}", v));
        }
        if let Some(v) = o.rtol {
            args.push(format!("--rtol={}", v));
        }
        if let Some(ref v) = o.solver {
            args.push(format!("--solver={}", v));
        }
        if let Some(v) = o.output_interval {
            args.push(format!("--output-interval={}", v));
        }
    }
    args
}

pub fn run_compiler_with_stdin(
    args: Vec<String>,
    stdin_input: Option<&str>,
    cwd: &Path,
) -> Result<(String, String, i32), String> {
    let (exe, extra_args) = crate::compiler_config::resolve_compiler_exe(cwd)?;
    let mut cmd = Command::new(&exe);
    cmd.args(&extra_args).args(&args).current_dir(cwd);
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Failed to run compiler: {}", e))?;
    if let Some(input) = stdin_input {
        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(input.as_bytes());
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|e| format!("Compiler process error: {}", e))?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let code = output.status.code().unwrap_or(-1);
    Ok((stdout, stderr, code))
}
