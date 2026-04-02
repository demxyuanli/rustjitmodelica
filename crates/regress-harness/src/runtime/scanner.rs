use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanSnapshot {
    pub scan_id: String,
    pub root_path: String,
    pub scanned_at: String,
    pub options_hash: String,
    pub models: Vec<ModelRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRef {
    pub case_id: String,
    pub model_path: String,
    pub file_path: String,
    pub model_type: String,
    pub tags: Vec<String>,
    pub dir_group: String,
}

#[derive(Debug, Clone)]
pub struct ScanProgress {
    pub scanned_dirs: usize,
    pub pending_dirs: usize,
    pub found_models: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    pub include_mos: bool,
}

fn detect_model_type(p: &Path) -> String {
    let ext = p
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if ext == "mos" {
        "script".to_string()
    } else {
        "model".to_string()
    }
}

fn normalize_case_id(rel_stem: &str) -> String {
    let mut s = String::with_capacity(rel_stem.len() + 5);
    s.push_str("scan_");
    for ch in rel_stem.chars() {
        if ch.is_ascii_alphanumeric() {
            s.push(ch.to_ascii_lowercase());
        } else {
            s.push('_');
        }
    }
    s.trim_matches('_').to_string()
}

fn rel_to_unix(root: &Path, p: &Path) -> Option<String> {
    let rel = p.strip_prefix(root).ok()?;
    Some(rel.to_string_lossy().replace('\\', "/"))
}

fn first_dir_group(rel_unix: &str) -> String {
    rel_unix
        .split('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("misc")
        .to_ascii_lowercase()
}

fn options_hash(root: &Path, options: &ScanOptions) -> String {
    let mut h = Sha256::new();
    h.update(root.to_string_lossy().as_bytes());
    h.update(if options.include_mos { b"mos=1" } else { b"mos=0" });
    format!("{:x}", h.finalize())
}

pub fn scan_models_with_progress<F>(
    root: &Path,
    options: &ScanOptions,
    mut on_progress: F,
) -> Result<ScanSnapshot>
where
    F: FnMut(ScanProgress),
{
    let root = std::fs::canonicalize(root).with_context(|| format!("canonicalize {}", root.display()))?;
    let mut queue = VecDeque::<PathBuf>::new();
    queue.push_back(root.clone());
    let mut scanned_dirs = 0usize;
    let mut models: Vec<ModelRef> = Vec::new();

    while let Some(dir) = queue.pop_front() {
        scanned_dirs += 1;
        let entries = match std::fs::read_dir(&dir) {
            Ok(v) => v,
            Err(_) => {
                on_progress(ScanProgress {
                    scanned_dirs,
                    pending_dirs: queue.len(),
                    found_models: models.len(),
                });
                continue;
            }
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                queue.push_back(p);
                continue;
            }
            let ext = p
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let include = ext == "mo" || (options.include_mos && ext == "mos");
            if !include {
                continue;
            }
            let Some(file_path) = rel_to_unix(&root, &p) else {
                continue;
            };
            let stem = file_path
                .strip_suffix(".mo")
                .or_else(|| file_path.strip_suffix(".mos"))
                .unwrap_or(&file_path)
                .to_string();
            let dir_group = first_dir_group(&file_path);
            let model_type = detect_model_type(&p);
            let case_id = normalize_case_id(&stem);
            models.push(ModelRef {
                case_id,
                model_path: stem.clone(),
                file_path: file_path.clone(),
                model_type,
                tags: vec![dir_group.clone()],
                dir_group,
            });
        }
        on_progress(ScanProgress {
            scanned_dirs,
            pending_dirs: queue.len(),
            found_models: models.len(),
        });
    }

    models.sort_by(|a, b| a.case_id.cmp(&b.case_id));
    let scanned_at = chrono::Utc::now().to_rfc3339();
    let scan_id = format!("scan_{}", chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ"));
    Ok(ScanSnapshot {
        scan_id,
        root_path: root.to_string_lossy().to_string(),
        scanned_at,
        options_hash: options_hash(&root, options),
        models,
    })
}

pub fn save_scan_snapshot(base_data_root: &Path, snapshot: &ScanSnapshot) -> Result<PathBuf> {
    let dir = base_data_root.join(".regress").join("scans");
    std::fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let path = dir.join(format!("{}.json", snapshot.scan_id));
    let text = serde_json::to_string_pretty(snapshot)?;
    std::fs::write(&path, text).with_context(|| format!("write {}", path.display()))?;
    Ok(path)
}
