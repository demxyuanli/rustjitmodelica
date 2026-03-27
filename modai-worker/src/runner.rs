use crate::artifact::{reason_to_text, write_run_artifacts};
use chrono::Utc;
use modai_protocol::{ReasonCode, RegressionExecutionPlan, RunRecord, SummaryLine};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Instant;

pub struct RunnerOutcome {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub duration_ms: u64,
    pub records: Vec<RunRecord>,
}

fn classify_reason(stdout: &str, stderr: &str, exit_code: i32) -> ReasonCode {
    let text = format!("{stdout}\n{stderr}").to_lowercase();
    if text.contains("model not found") {
        ReasonCode::ModelNotFound
    } else if text.contains("newton") {
        ReasonCode::NewtonNonconverged
    } else if text.contains("parse") {
        ReasonCode::ParseError
    } else if exit_code == -1 {
        ReasonCode::ProcessError
    } else {
        ReasonCode::RuntimeError
    }
}

fn locate_rustmodlica_exe(repo_root: &Path) -> PathBuf {
    let candidates = [
        repo_root.join("target").join("release").join("rustmodlica.exe"),
        repo_root
            .join("jit-compiler")
            .join("target")
            .join("release")
            .join("rustmodlica.exe"),
    ];
    for c in candidates {
        if c.exists() {
            return c;
        }
    }
    repo_root.join("target").join("release").join("rustmodlica.exe")
}

pub fn run_cases(
    repo_root: &Path,
    plan: &RegressionExecutionPlan,
    workspace_dir: &Path,
) -> Result<RunnerOutcome, String> {
    let exe = locate_rustmodlica_exe(repo_root);
    if !exe.exists() {
        return Err(format!("rustmodlica executable not found: {}", exe.display()));
    }
    let mut records = Vec::new();
    let mut summaries = Vec::new();
    let mut passed = 0usize;
    let mut failed = 0usize;
    let started = Instant::now();

    for case in &plan.planned_cases {
        let begin = Instant::now();
        let output = Command::new(&exe).arg(&case.name).current_dir(repo_root).output();
        let elapsed = begin.elapsed().as_millis() as u64;
        match output {
            Ok(out) => {
                let exit_code = out.status.code().unwrap_or(-1);
                let ok = out.status.success();
                let stdout = String::from_utf8_lossy(&out.stdout).to_string();
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                let reason = if ok {
                    ReasonCode::ExpectationMet
                } else {
                    classify_reason(&stdout, &stderr, exit_code)
                };
                if ok {
                    passed += 1;
                } else {
                    failed += 1;
                }
                records.push(RunRecord {
                    timestamp: Utc::now().to_rfc3339(),
                    case_type: "DIR_MODEL".to_string(),
                    case_name: case.name.clone(),
                    duration_ms: elapsed,
                    expect_target_ok: true,
                    actual_ok: ok,
                    exit_code,
                    status: if ok { "OK".to_string() } else { "FAILED".to_string() },
                    reason: reason.clone(),
                    detail: format!("priority={};category={}", case.priority, case.category),
                });
                summaries.push(SummaryLine {
                    state: if ok { "OK".to_string() } else { "!!".to_string() },
                    case_name: case.name.clone(),
                    reason: reason_to_text(&reason).to_string(),
                    detail: case.reason.clone(),
                });
            }
            Err(e) => {
                failed += 1;
                records.push(RunRecord {
                    timestamp: Utc::now().to_rfc3339(),
                    case_type: "DIR_MODEL".to_string(),
                    case_name: case.name.clone(),
                    duration_ms: elapsed,
                    expect_target_ok: true,
                    actual_ok: false,
                    exit_code: -1,
                    status: "FAILED".to_string(),
                    reason: ReasonCode::ProcessError,
                    detail: e.to_string(),
                });
                summaries.push(SummaryLine {
                    state: "!!".to_string(),
                    case_name: case.name.clone(),
                    reason: "process_error".to_string(),
                    detail: case.reason.clone(),
                });
            }
        }
    }

    let lock_payload = serde_json::json!({
        "schemaVersion": "libraries.lock.v1",
        "repoRoot": repo_root.to_string_lossy().replace('\\', "/"),
        "libraryRoots": [
            repo_root.join("jit-compiler").join("Modelica").to_string_lossy().replace('\\', "/"),
            repo_root.join("jit-compiler").join("ModelicaTest").to_string_lossy().replace('\\', "/")
        ],
        "executable": {
            "path": exe.to_string_lossy().replace('\\', "/")
        }
    });
    write_run_artifacts(workspace_dir, &records, &summaries, lock_payload)?;
    Ok(RunnerOutcome {
        total: plan.planned_cases.len(),
        passed,
        failed,
        duration_ms: started.elapsed().as_millis() as u64,
        records,
    })
}
