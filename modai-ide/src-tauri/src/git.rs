// Git integration: status, diff, log, stage, commit via subprocess.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct GitStatus {
    pub branch: String,
    pub staged: Vec<String>,
    pub modified: Vec<String>,
    pub deleted: Vec<String>,
    pub untracked: Vec<String>,
    pub renamed: Vec<GitRenamed>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GitRenamed {
    pub from: String,
    pub to: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GitLogEntry {
    pub hash: String,
    pub subject: String,
    pub author: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GitLogGraphEntry {
    pub hash: String,
    pub parents: Vec<String>,
    pub subject: String,
    pub author: String,
    pub date: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct GitCommitFile {
    pub status: String,
    pub path: String,
}

fn ensure_git_repo(project_dir: &Path) -> Result<(), String> {
    let out = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(project_dir)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err("Not a git repository".to_string());
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if s != "true" {
        return Err("Not a git repository".to_string());
    }
    Ok(())
}

fn validate_path_under(project_dir: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let path = project_dir.join(relative_path);
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    let dir_canonical = project_dir.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(&dir_canonical) {
        return Err("Path is outside project directory".to_string());
    }
    Ok(canonical)
}

pub fn git_status_impl(project_dir: &Path) -> Result<GitStatus, String> {
    ensure_git_repo(project_dir)?;

    let branch_out = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(project_dir)
        .output()
        .map_err(|e| e.to_string())?;
    let branch = if branch_out.status.success() {
        String::from_utf8_lossy(&branch_out.stdout).trim().to_string()
    } else {
        "HEAD".to_string()
    };

    let out = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(project_dir)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git status failed: {}", stderr));
    }
    let text = String::from_utf8_lossy(&out.stdout);

    let mut staged = Vec::new();
    let mut modified = Vec::new();
    let mut deleted = Vec::new();
    let mut untracked = Vec::new();
    let mut renamed = Vec::new();

    for line in text.lines() {
        let line = line.trim();
        if line.len() < 3 {
            continue;
        }
        let x = line.chars().next().unwrap_or(' ');
        let y = line.chars().nth(1).unwrap_or(' ');
        let rest = line[2..].trim_start();
        let (path, path_from) = if rest.contains(" -> ") {
            let parts: Vec<&str> = rest.splitn(2, " -> ").collect();
            if parts.len() >= 2 {
                (parts[1].to_string(), Some(parts[0].trim().to_string()))
            } else {
                (rest.to_string(), None)
            }
        } else {
            (rest.to_string(), None)
        };
        if path.is_empty() {
            continue;
        }

        if x == '?' && y == '?' {
            untracked.push(path);
            continue;
        }
        if y == '?' && x == ' ' {
            untracked.push(path);
            continue;
        }
        if x == 'R' || x == 'C' {
            if let Some(ref from) = path_from {
                renamed.push(GitRenamed {
                    from: from.clone(),
                    to: path.clone(),
                });
            }
        }
        if x != ' ' && x != '?' {
            staged.push(path.clone());
        }
        if y == 'M' || y == 'A' {
            modified.push(path.clone());
        } else if y == 'D' {
            deleted.push(path);
        }
    }

    Ok(GitStatus {
        branch,
        staged,
        modified,
        deleted,
        untracked,
        renamed,
    })
}

pub fn git_diff_file_impl(
    project_dir: &Path,
    relative_path: &str,
    base: Option<&str>,
) -> Result<String, String> {
    ensure_git_repo(project_dir)?;
    let _ = validate_path_under(project_dir, relative_path)?;

    let base = base.unwrap_or("HEAD");
    let out = Command::new("git")
        .args(["diff", base, "--", relative_path])
        .current_dir(project_dir)
        .output()
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn git_diff_file_staged_impl(
    project_dir: &Path,
    relative_path: &str,
) -> Result<String, String> {
    ensure_git_repo(project_dir)?;
    let _ = validate_path_under(project_dir, relative_path)?;

    let out = Command::new("git")
        .args(["diff", "--cached", "--", relative_path])
        .current_dir(project_dir)
        .output()
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn git_show_file_impl(
    project_dir: &Path,
    revision: &str,
    relative_path: &str,
) -> Result<String, String> {
    ensure_git_repo(project_dir)?;
    let _ = validate_path_under(project_dir, relative_path)?;

    let spec = format!("{}:{}", revision, relative_path.replace('\\', "/"));
    let out = Command::new("git")
        .args(["show", &spec])
        .current_dir(project_dir)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git show failed: {}", stderr));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

pub fn git_log_impl(
    project_dir: &Path,
    relative_path: Option<&str>,
    limit: u32,
) -> Result<Vec<GitLogEntry>, String> {
    ensure_git_repo(project_dir)?;
    if let Some(p) = relative_path {
        let _ = validate_path_under(project_dir, p)?;
    }

    let n = limit.min(500).to_string();
    let mut args = vec!["log", "-n", &n, "--format=%H%n%s%n%an%n%ai"];
    if let Some(p) = relative_path {
        args.push("--");
        args.push(p);
    }

    let out = Command::new("git")
        .args(&args)
        .current_dir(project_dir)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("does not have any commits") {
            return Ok(Vec::new());
        }
        return Err(format!("git log failed: {}", stderr));
    }

    let text = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = text.lines().collect();
    let mut entries = Vec::new();
    let mut i = 0;
    while i + 3 < lines.len() {
        entries.push(GitLogEntry {
            hash: lines[i].trim().to_string(),
            subject: lines[i + 1].trim().to_string(),
            author: lines[i + 2].trim().to_string(),
            date: lines[i + 3].trim().to_string(),
        });
        i += 4;
    }
    Ok(entries)
}

pub fn git_log_graph_impl(project_dir: &Path, limit: u32) -> Result<Vec<GitLogGraphEntry>, String> {
    ensure_git_repo(project_dir)?;
    let n = limit.min(200).to_string();
    let out = Command::new("git")
        .args(["log", "-n", &n, "--format=%H%n%P%n%s%n%an%n%ai"])
        .current_dir(project_dir)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        if stderr.contains("does not have any commits") {
            return Ok(Vec::new());
        }
        return Err(format!("git log failed: {}", stderr));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<&str> = text.lines().collect();
    let mut entries = Vec::new();
    let mut i = 0;
    while i + 4 < lines.len() {
        let hash = lines[i].trim().to_string();
        let parents_str = lines[i + 1].trim();
        let parents: Vec<String> = if parents_str.is_empty() {
            Vec::new()
        } else {
            parents_str.split_whitespace().map(String::from).collect()
        };
        entries.push(GitLogGraphEntry {
            hash,
            parents,
            subject: lines[i + 2].trim().to_string(),
            author: lines[i + 3].trim().to_string(),
            date: lines[i + 4].trim().to_string(),
        });
        i += 5;
    }
    Ok(entries)
}

pub fn git_stage_impl(project_dir: &Path, paths: &[String]) -> Result<(), String> {
    ensure_git_repo(project_dir)?;
    for p in paths {
        let _ = validate_path_under(project_dir, p)?;
    }
    let out = Command::new("git")
        .arg("add")
        .args(paths)
        .current_dir(project_dir)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git add failed: {}", stderr));
    }
    Ok(())
}

pub fn git_unstage_impl(project_dir: &Path, paths: &[String]) -> Result<(), String> {
    ensure_git_repo(project_dir)?;
    for p in paths {
        let _ = validate_path_under(project_dir, p)?;
    }
    let mut args = vec!["reset", "HEAD"];
    args.extend(paths.iter().map(String::as_str));
    let out = Command::new("git")
        .args(&args)
        .current_dir(project_dir)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git reset failed: {}", stderr));
    }
    Ok(())
}

pub fn git_commit_impl(project_dir: &Path, message: &str) -> Result<(), String> {
    ensure_git_repo(project_dir)?;
    let out = Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(project_dir)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git commit failed: {}", stderr));
    }
    Ok(())
}

pub fn git_commit_files_impl(project_dir: &Path, hash: &str) -> Result<Vec<GitCommitFile>, String> {
    ensure_git_repo(project_dir)?;
    let out = Command::new("git")
        .args(["show", "--name-status", "--format=", hash])
        .current_dir(project_dir)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git show failed: {}", stderr));
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let mut files = Vec::new();
    let status_chars = ['A', 'M', 'D', 'R', 'C', 'T', 'U', 'X', 'B'];
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let first = line.chars().next().unwrap_or(' ');
        if !status_chars.contains(&first) {
            continue;
        }
        let (status, path) = if let Some(rest) = line.get(1..) {
            let path = rest.trim_start().trim_start_matches('\t').trim_start();
            (first.to_string(), path.to_string())
        } else {
            continue;
        };
        if !path.is_empty() {
            files.push(GitCommitFile {
                status,
                path,
            });
        }
    }
    Ok(files)
}

pub fn git_is_repo_impl(project_dir: &Path) -> bool {
    ensure_git_repo(project_dir).is_ok()
}

pub fn git_init_impl(project_dir: &Path) -> Result<(), String> {
    let out = Command::new("git")
        .arg("init")
        .current_dir(project_dir)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git init failed: {}", stderr.trim()));
    }
    Ok(())
}
