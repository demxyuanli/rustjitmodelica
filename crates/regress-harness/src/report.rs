//! JSON report, NDJSON records, and regression manifest (last-run structure).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::config::CaseDef;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub schema_version: u32,
    pub generated_at: String,
    pub repo_root: String,
    pub config_path: String,
    pub tier_filter: Option<String>,
    pub tags_filter: Option<Vec<String>>,
    pub incremental_strategy: String,
    pub summary: ReportSummary,
    pub cases: Vec<CaseResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ReportSummary {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CaseStatus {
    Pass,
    Fail,
    SkippedUnchanged,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Artifacts {
    #[serde(default)]
    pub rust_csv: Option<String>,
    #[serde(default)]
    pub working_dir: Option<String>,
    #[serde(default)]
    pub repro_bundle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmcCompareResult {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_abs_diff: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_column_index: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub threshold: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseResult {
    pub case_id: String,
    #[serde(default = "default_empty_vec")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    pub status: CaseStatus,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classification: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default)]
    pub stderr_tail: String,
    #[serde(default)]
    pub stdout_tail: String,
    pub stdout_len: usize,
    pub stderr_len: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub omc_compare: Option<OmcCompareResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_hash: Option<String>,
    #[serde(default)]
    pub warmup_failed_count: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warmup_failed_models: Vec<String>,
    pub artifacts: Artifacts,
}

fn default_empty_vec() -> Vec<String> {
    Vec::new()
}

impl Report {
    pub fn summarize(&mut self) {
        let mut passed = 0usize;
        let mut failed = 0usize;
        let mut skipped = 0usize;
        for c in &self.cases {
            match c.status {
                CaseStatus::Pass => passed += 1,
                CaseStatus::Fail => failed += 1,
                CaseStatus::SkippedUnchanged => skipped += 1,
            }
        }
        self.summary = ReportSummary {
            total: self.cases.len(),
            passed,
            failed,
            skipped,
        };
    }
}

pub fn write_report_json(path: &Path, report: &Report) -> std::io::Result<()> {
    let s = serde_json::to_string_pretty(report)?;
    std::fs::write(path, s)
}

pub fn read_report_json(path: &Path) -> Result<Report, anyhow::Error> {
    let text = std::fs::read_to_string(path)?;
    let r: Report = serde_json::from_str(&text)?;
    Ok(r)
}

/// Saved after each run under the central data root as `regress_manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegressManifest {
    pub version: u32,
    pub created_at: String,
    pub config_path: String,
    pub tier_filter: Option<String>,
    pub tags_filter: Option<Vec<String>>,
    /// Case ids in run order; used for `last_structure*` incremental scope.
    pub case_ids: Vec<String>,
}

pub fn read_manifest(path: &Path) -> Result<RegressManifest, anyhow::Error> {
    let text = std::fs::read_to_string(path)?;
    let m: RegressManifest = serde_json::from_str(&text)?;
    Ok(m)
}

pub fn write_manifest(path: &Path, manifest: &RegressManifest) -> std::io::Result<()> {
    let s = serde_json::to_string_pretty(manifest)?;
    std::fs::write(path, s)
}

/// Keep manifest order; skip ids missing from `cases`.
pub fn filter_cases_by_manifest(cases: Vec<CaseDef>, manifest: &RegressManifest) -> Vec<CaseDef> {
    let mut map: HashMap<String, CaseDef> = cases.into_iter().map(|c| (c.id.clone(), c)).collect();
    let mut out = Vec::new();
    for id in &manifest.case_ids {
        if let Some(c) = map.remove(id) {
            out.push(c);
        }
    }
    out
}

pub fn append_ndjson_line(path: &Path, case: &CaseResult) -> std::io::Result<()> {
    use std::io::Write;
    let line = serde_json::to_string(case)?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    writeln!(f, "{line}")?;
    Ok(())
}

/// Optional legacy `summary.txt` lines: `OK id ...` / `!! id ...` / `-- id ...`.
pub fn write_summary_compat(path: &Path, report: &Report) -> std::io::Result<()> {
    use std::io::Write;
    let mut lines = Vec::new();
    lines.push(format!(
        "# regress-harness {}  cases={} passed={} failed={} skipped={}",
        report.generated_at,
        report.summary.total,
        report.summary.passed,
        report.summary.failed,
        report.summary.skipped
    ));
    for c in &report.cases {
        let sym = match c.status {
            CaseStatus::Pass => "OK",
            CaseStatus::Fail => "!!",
            CaseStatus::SkippedUnchanged => "--",
        };
        let detail = match c.status {
            CaseStatus::Pass => "pass".to_string(),
            CaseStatus::Fail => c
                .classification
                .clone()
                .unwrap_or_else(|| format!("exit={}", c.exit_code.unwrap_or(-1))),
            CaseStatus::SkippedUnchanged => "skipped_unchanged".to_string(),
        };
        lines.push(format!("{sym} {}  {detail}", c.case_id));
    }
    let mut f = std::fs::File::create(path)?;
    for ln in lines {
        writeln!(f, "{ln}")?;
    }
    Ok(())
}
