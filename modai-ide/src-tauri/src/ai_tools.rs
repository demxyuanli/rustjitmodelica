use crate::iterate;
use serde_json::json;
use std::path::{Path, PathBuf};
use std::process::Command;

fn canonical_base_dir(base_dir: &str) -> Result<PathBuf, String> {
    Path::new(base_dir)
        .canonicalize()
        .map_err(|e| format!("base_dir canonicalize: {}", e))
}

fn canonical_under(base: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let rel = relative_path
        .replace('/', std::path::MAIN_SEPARATOR_STR)
        .replace('\\', std::path::MAIN_SEPARATOR_STR);
    let joined = base.join(rel);
    let canon = joined.canonicalize().map_err(|e| e.to_string())?;
    let base_canon = base.canonicalize().map_err(|e| e.to_string())?;
    if !canon.starts_with(&base_canon) {
        return Err("path escapes base_dir".to_string());
    }
    Ok(canon)
}

pub fn tools_schema() -> serde_json::Value {
    json!([
        {
            "type": "function",
            "function": {
                "name": "search_text",
                "description": "Search text in files under base_dir and return matches. Uses ripgrep.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "base_dir": { "type": "string" },
                        "pattern": { "type": "string" },
                        "glob": { "type": "string" },
                        "max_results": { "type": "integer", "minimum": 1, "maximum": 500 }
                    },
                    "required": ["base_dir", "pattern"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "read_file",
                "description": "Read a UTF-8 text file under base_dir.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "base_dir": { "type": "string" },
                        "path": { "type": "string" }
                    },
                    "required": ["base_dir", "path"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "write_file",
                "description": "Write a UTF-8 text file under base_dir. Creates parent directories if needed.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "base_dir": { "type": "string" },
                        "path": { "type": "string" },
                        "content": { "type": "string" }
                    },
                    "required": ["base_dir", "path", "content"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "apply_patch",
                "description": "Apply a unified diff to base_dir (like patch -p1).",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "base_dir": { "type": "string" },
                        "diff": { "type": "string" }
                    },
                    "required": ["base_dir", "diff"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "git_diff",
                "description": "Run git diff in base_dir and return text output.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "base_dir": { "type": "string" },
                        "args": { "type": "array", "items": { "type": "string" }, "maxItems": 30 }
                    },
                    "required": ["base_dir"]
                }
            }
        },
        {
            "type": "function",
            "function": {
                "name": "run_powershell",
                "description": "Run a PowerShell script in base_dir and return stdout/stderr/exit_code.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "base_dir": { "type": "string" },
                        "script": { "type": "string" }
                    },
                    "required": ["base_dir", "script"]
                }
            }
        }
    ])
}

pub fn exec_tool(name: &str, args: &serde_json::Value) -> Result<String, String> {
    match name {
        "search_text" => exec_search_text(args),
        "read_file" => exec_read_file(args),
        "write_file" => exec_write_file(args),
        "apply_patch" => exec_apply_patch(args),
        "git_diff" => exec_git_diff(args),
        "run_powershell" => exec_run_powershell(args),
        _ => Err("unknown tool".to_string()),
    }
}

fn exec_search_text(args: &serde_json::Value) -> Result<String, String> {
    let base_dir = args.get("base_dir").and_then(|v| v.as_str()).ok_or("base_dir")?;
    let pattern = args.get("pattern").and_then(|v| v.as_str()).ok_or("pattern")?;
    let glob = args.get("glob").and_then(|v| v.as_str());
    let max_results = args.get("max_results").and_then(|v| v.as_u64()).unwrap_or(100).min(500) as usize;
    let base = canonical_base_dir(base_dir)?;

    let mut cmd = Command::new("rg");
    cmd.arg("--no-heading").arg("--line-number").arg("--color").arg("never");
    if let Some(g) = glob {
        cmd.arg("--glob").arg(g);
    }
    cmd.arg(pattern).arg(base.as_os_str());
    let out = cmd.output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if !out.status.success() && out.status.code().unwrap_or(1) != 1 {
        return Err(format!("rg failed: {}", stderr));
    }
    let mut lines: Vec<&str> = stdout.lines().collect();
    if lines.len() > max_results {
        lines.truncate(max_results);
        lines.push("... truncated");
    }
    Ok(lines.join("\n"))
}

fn exec_read_file(args: &serde_json::Value) -> Result<String, String> {
    let base_dir = args.get("base_dir").and_then(|v| v.as_str()).ok_or("base_dir")?;
    let path = args.get("path").and_then(|v| v.as_str()).ok_or("path")?;
    let base = canonical_base_dir(base_dir)?;
    let full = canonical_under(&base, path)?;
    std::fs::read_to_string(full).map_err(|e| e.to_string())
}

fn exec_write_file(args: &serde_json::Value) -> Result<String, String> {
    let base_dir = args.get("base_dir").and_then(|v| v.as_str()).ok_or("base_dir")?;
    let path = args.get("path").and_then(|v| v.as_str()).ok_or("path")?;
    let content = args.get("content").and_then(|v| v.as_str()).ok_or("content")?;
    let base = canonical_base_dir(base_dir)?;
    let rel = path
        .replace('/', std::path::MAIN_SEPARATOR_STR)
        .replace('\\', std::path::MAIN_SEPARATOR_STR);
    let joined = base.join(rel);
    if let Some(parent) = joined.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&joined, content.as_bytes()).map_err(|e| e.to_string())?;
    Ok("ok".to_string())
}

fn exec_apply_patch(args: &serde_json::Value) -> Result<String, String> {
    let base_dir = args.get("base_dir").and_then(|v| v.as_str()).ok_or("base_dir")?;
    let diff = args.get("diff").and_then(|v| v.as_str()).ok_or("diff")?;
    let base = canonical_base_dir(base_dir)?;
    iterate::apply_diff_to_dir(diff, &base)?;
    Ok("ok".to_string())
}

fn exec_git_diff(args: &serde_json::Value) -> Result<String, String> {
    let base_dir = args.get("base_dir").and_then(|v| v.as_str()).ok_or("base_dir")?;
    let base = canonical_base_dir(base_dir)?;
    let mut cmd = Command::new("git");
    cmd.arg("diff");
    if let Some(arr) = args.get("args").and_then(|v| v.as_array()) {
        for a in arr.iter().filter_map(|v| v.as_str()) {
            cmd.arg(a);
        }
    }
    let out = cmd.current_dir(&base).output().map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    if !out.status.success() {
        return Err(format!("git diff failed: {}", stderr));
    }
    Ok(stdout)
}

fn exec_run_powershell(args: &serde_json::Value) -> Result<String, String> {
    let base_dir = args.get("base_dir").and_then(|v| v.as_str()).ok_or("base_dir")?;
    let script = args.get("script").and_then(|v| v.as_str()).ok_or("script")?;
    let base = canonical_base_dir(base_dir)?;
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
        .current_dir(&base)
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    Ok(json!({
        "exit_code": out.status.code().unwrap_or(-1),
        "stdout": stdout,
        "stderr": stderr
    })
    .to_string())
}

