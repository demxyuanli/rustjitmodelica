use crate::report::{read_manifest, read_report_json, CaseResult, ReportSummary};
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub summary: ReportSummary,
    pub cases: Vec<CaseResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatManifest {
    pub version: u32,
    pub created_at: String,
    pub config_path: String,
    pub case_ids: Vec<String>,
}

pub fn read_report_compat(path: &Path) -> Result<CompatReport> {
    let r = read_report_json(path)?;
    Ok(CompatReport {
        schema_version: r.schema_version,
        generated_at: r.generated_at,
        summary: r.summary,
        cases: r.cases,
    })
}

pub fn read_manifest_compat(path: &Path) -> Result<CompatManifest> {
    let m = read_manifest(path)?;
    Ok(CompatManifest {
        version: m.version,
        created_at: m.created_at,
        config_path: m.config_path,
        case_ids: m.case_ids,
    })
}
