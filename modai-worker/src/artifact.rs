use chrono::Utc;
use modai_protocol::{ReasonCode, RegressionExecutionPlan, RegressionWorkspaceState, RunRecord, SummaryLine};
use std::fs;
use std::io::Write;
use std::path::Path;

pub fn write_workspace_state(dir: &Path, state: &RegressionWorkspaceState) -> Result<(), String> {
    let path = dir.join("workspace-state.json");
    let text = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    fs::write(path, text).map_err(|e| e.to_string())
}

pub fn read_workspace_state(dir: &Path) -> Result<RegressionWorkspaceState, String> {
    let path = dir.join("workspace-state.json");
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    serde_json::from_str(&text).map_err(|e| e.to_string())
}

pub fn write_plan_cases(dir: &Path, plan: &RegressionExecutionPlan) -> Result<(), String> {
    let plan_path = dir.join("plan.json");
    let plan_text = serde_json::to_string_pretty(plan).map_err(|e| e.to_string())?;
    fs::write(plan_path, plan_text).map_err(|e| e.to_string())?;

    let mut f = fs::File::create(dir.join("cases.txt")).map_err(|e| e.to_string())?;
    for c in &plan.planned_cases {
        writeln!(f, "{}", c.name).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn write_run_artifacts(
    dir: &Path,
    records: &[RunRecord],
    summaries: &[SummaryLine],
    lock_payload: serde_json::Value,
) -> Result<(), String> {
    let summary_path = dir.join("summary.txt");
    let mut sf = fs::File::create(summary_path).map_err(|e| e.to_string())?;
    for s in summaries {
        writeln!(sf, "{} {} reason={} detail={}", s.state, s.case_name, s.reason, s.detail)
            .map_err(|e| e.to_string())?;
    }

    let ndjson = dir.join("runlog.ndjson");
    let mut nf = fs::File::create(ndjson).map_err(|e| e.to_string())?;
    for r in records {
        let line = serde_json::to_string(r).map_err(|e| e.to_string())?;
        writeln!(nf, "{line}").map_err(|e| e.to_string())?;
    }

    let csv = dir.join("runlog.csv");
    let mut cf = fs::File::create(csv).map_err(|e| e.to_string())?;
    writeln!(
        cf,
        "timestamp,case_type,case_name,duration_ms,expect_target_ok,actual_ok,exit_code,status,reason,detail"
    )
    .map_err(|e| e.to_string())?;
    for r in records {
        writeln!(
            cf,
            "{},{},{},{},{},{},{},{},{:?},{}",
            r.timestamp, r.case_type, r.case_name, r.duration_ms, r.expect_target_ok, r.actual_ok, r.exit_code, r.status, r.reason, r.detail
        )
        .map_err(|e| e.to_string())?;
    }

    let lock_file = dir.join("libraries.lock.json");
    let mut payload = lock_payload;
    payload["generatedAt"] = serde_json::Value::String(Utc::now().to_rfc3339());
    fs::write(
        lock_file,
        serde_json::to_string_pretty(&payload).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

pub fn reason_to_text(reason: &ReasonCode) -> &'static str {
    match reason {
        ReasonCode::ExpectationMet => "expectation_met",
        ReasonCode::ModelNotFound => "model_not_found",
        ReasonCode::DependencyMissing => "dependency_missing",
        ReasonCode::NewtonNonconverged => "newton_nonconverged",
        ReasonCode::ParseError => "parse_error",
        ReasonCode::RuntimeError => "runtime_error",
        ReasonCode::Timeout => "timeout",
        ReasonCode::ProcessError => "process_error",
        ReasonCode::Cancelled => "cancelled",
    }
}
