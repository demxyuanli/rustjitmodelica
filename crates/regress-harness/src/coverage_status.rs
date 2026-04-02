use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
pub struct CoverageStatus {
    pub semantic_target_percent: f64,
    pub semantic_current_percent: f64,
    pub semantic_passed_items: u64,
    pub semantic_total_items: u64,
    pub modelica34_target_percent: f64,
    pub modelica34_current_percent: f64,
    pub modelica34_passed_items: u64,
    pub modelica34_total_items: u64,
    pub gaps: Vec<String>,
}

fn get_target_percent(path: &Path, key: &str, default_value: f64) -> Result<f64> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = t.split_once(',') {
            if k.trim() == key {
                if let Ok(p) = v.trim().parse::<f64>() {
                    return Ok(p);
                }
            }
        }
    }
    Ok(default_value)
}

fn coverage_from_csv_matrix(path: &Path) -> Result<(u64, u64, f64)> {
    let text = fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let mut status_idx: isize = -1;
    let mut total: u64 = 0;
    let mut passed: u64 = 0;
    let mut active = false;

    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        if !t.contains(',') {
            continue;
        }
        let cols = t
            .split(',')
            .map(|s| s.trim().to_string())
            .collect::<Vec<_>>();
        if !active && cols.iter().any(|c| c == "status") {
            status_idx = cols.iter().position(|c| c == "status").unwrap_or(0) as isize;
            active = true;
            continue;
        }
        if !active {
            continue;
        }
        if status_idx < 0 || status_idx as usize >= cols.len() {
            continue;
        }
        let status = cols[status_idx as usize].to_ascii_lowercase();
        if status == "pass" || status == "fail" || status == "pending" {
            total += 1;
            if status == "pass" {
                passed += 1;
            }
        }
    }
    if total == 0 {
        bail!("matrix has no rows with status values: {}", path.display());
    }
    let current = (100.0 * (passed as f64) / (total as f64) * 100.0).round() / 100.0;
    Ok((passed, total, current))
}

pub fn generate_coverage_status(repo_root: &Path) -> Result<PathBuf> {
    let script_dir = repo_root.join("jit-compiler").join("scripts");
    let mos_matrix = script_dir.join("mos_signal_coverage_matrix.txt");
    let core_matrix = script_dir.join("modelica34_core_coverage_matrix.txt");
    let semantic_matrix = script_dir.join("semantic_coverage_matrix.md");
    let status_json = script_dir.join("coverage_status.json");

    if !mos_matrix.exists() {
        bail!("missing file: {}", mos_matrix.display());
    }
    if !core_matrix.exists() {
        bail!("missing file: {}", core_matrix.display());
    }

    let semantic_target =
        get_target_percent(&mos_matrix, "target_semantic_coverage_percent", 98.0)?;
    let modelica_target = get_target_percent(&core_matrix, "target_modelica34_percent", 100.0)?;

    let (semantic_passed, semantic_total, semantic_current) = coverage_from_csv_matrix(&mos_matrix)?;
    let (modelica_passed, modelica_total, modelica_current) = coverage_from_csv_matrix(&core_matrix)?;

    let mut gaps = Vec::<String>::new();
    if semantic_current < semantic_target {
        gaps.push("semantic coverage below target".to_string());
    }
    if modelica_current < modelica_target {
        gaps.push("modelica34 coverage below target".to_string());
    }

    let payload = CoverageStatus {
        semantic_target_percent: semantic_target,
        semantic_current_percent: semantic_current,
        semantic_passed_items: semantic_passed,
        semantic_total_items: semantic_total,
        modelica34_target_percent: modelica_target,
        modelica34_current_percent: modelica_current,
        modelica34_passed_items: modelica_passed,
        modelica34_total_items: modelica_total,
        gaps,
    };
    fs::write(&status_json, serde_json::to_string_pretty(&payload)?)?;

    let _ = semantic_matrix; // optional informational source
    Ok(status_json)
}

pub fn coverage_gate(status_json: &Path) -> Result<bool> {
    let text = fs::read_to_string(status_json)
        .with_context(|| format!("read {}", status_json.display()))?;
    let v: Value = serde_json::from_str(&text)?;
    let sem_t = v.get("semantic_target_percent").and_then(|x| x.as_f64()).unwrap_or(98.0);
    let sem_c = v.get("semantic_current_percent").and_then(|x| x.as_f64()).unwrap_or(0.0);
    let m_t = v.get("modelica34_target_percent").and_then(|x| x.as_f64()).unwrap_or(100.0);
    let m_c = v.get("modelica34_current_percent").and_then(|x| x.as_f64()).unwrap_or(0.0);
    let gaps = v.get("gaps").and_then(|x| x.as_array()).cloned().unwrap_or_default();
    let ok = sem_c >= sem_t && m_c >= m_t && gaps.is_empty();
    Ok(ok)
}

