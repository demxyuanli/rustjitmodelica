// Compiler source file operations: tree listing, read/write, branch management.

use serde::Serialize;
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceTreeEntry {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<SourceTreeEntry>>,
    pub is_dir: bool,
}

fn build_tree(dir: &Path, prefix: &str) -> Vec<SourceTreeEntry> {
    let mut entries = Vec::new();
    let read_dir = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return entries,
    };
    let mut items: Vec<_> = read_dir.filter_map(|e| e.ok()).collect();
    items.sort_by(|a, b| {
        let a_is_dir = a.path().is_dir();
        let b_is_dir = b.path().is_dir();
        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });
    for item in items {
        let p = item.path();
        let name = p
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        if name.starts_with('.') || name == "target" {
            continue;
        }
        let rel = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };
        if p.is_dir() {
            let children = build_tree(&p, &rel);
            if !children.is_empty() {
                entries.push(SourceTreeEntry {
                    name,
                    path: None,
                    children: Some(children),
                    is_dir: true,
                });
            }
        } else {
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("");
            if ext == "rs" || ext == "pest" || ext == "toml" {
                entries.push(SourceTreeEntry {
                    name,
                    path: Some(rel),
                    children: None,
                    is_dir: false,
                });
            }
        }
    }
    entries
}

pub fn list_source_tree(repo_root: &Path) -> Result<SourceTreeEntry, String> {
    let src_dir = repo_root.join("src");
    if !src_dir.is_dir() {
        return Err("src/ directory not found".to_string());
    }
    let children = build_tree(&src_dir, "src");
    Ok(SourceTreeEntry {
        name: "src".to_string(),
        path: None,
        children: Some(children),
        is_dir: true,
    })
}

pub fn read_file(repo_root: &Path, rel_path: &str) -> Result<String, String> {
    let normalized = rel_path.replace('\\', "/");
    if !normalized.starts_with("src/") && !normalized.starts_with("Cargo.toml") {
        return Err("Only files under src/ or Cargo.toml are accessible".to_string());
    }
    let full = repo_root.join(&normalized);
    let canonical = full.canonicalize().map_err(|e| e.to_string())?;
    let root_canonical = repo_root.canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(&root_canonical) {
        return Err("Path is outside repository".to_string());
    }
    fs::read_to_string(&full).map_err(|e| e.to_string())
}

pub fn write_file(repo_root: &Path, rel_path: &str, content: &str) -> Result<(), String> {
    let normalized = rel_path.replace('\\', "/");
    if !normalized.starts_with("src/") {
        return Err("Only files under src/ can be written".to_string());
    }
    let full = repo_root.join(&normalized);
    let root_canonical = repo_root.canonicalize().map_err(|e| e.to_string())?;
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let parent_canonical = full
        .parent()
        .ok_or("no parent")?
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !parent_canonical.starts_with(&root_canonical) {
        return Err("Path is outside repository".to_string());
    }
    fs::write(&full, content).map_err(|e| e.to_string())
}

pub fn create_iteration_branch(repo_root: &Path, name: &str) -> Result<String, String> {
    let sanitized: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let branch_name = format!("iter/{}", sanitized);
    let out = Command::new("git")
        .args(["checkout", "-b", &branch_name])
        .current_dir(repo_root)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git checkout -b failed: {}", stderr));
    }
    Ok(branch_name)
}

pub fn list_iteration_branches(repo_root: &Path) -> Result<Vec<String>, String> {
    let out = Command::new("git")
        .args(["branch", "--list", "iter/*"])
        .current_dir(repo_root)
        .output()
        .map_err(|e| e.to_string())?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let branches: Vec<String> = stdout
        .lines()
        .map(|l| l.trim().trim_start_matches("* ").to_string())
        .filter(|l| !l.is_empty())
        .collect();
    Ok(branches)
}

pub fn switch_branch(repo_root: &Path, name: &str) -> Result<(), String> {
    let out = Command::new("git")
        .args(["checkout", name])
        .current_dir(repo_root)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git checkout failed: {}", stderr));
    }
    Ok(())
}

pub fn merge_branch(repo_root: &Path, name: &str) -> Result<(), String> {
    let out = Command::new("git")
        .args(["merge", name])
        .current_dir(repo_root)
        .output()
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        return Err(format!("git merge failed: {}", stderr));
    }
    Ok(())
}
