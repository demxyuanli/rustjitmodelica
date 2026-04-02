use crate::report::CaseStatus;
use serde::{Deserialize, Serialize};

pub const EVENT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSummaryLite {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhaseKind {
    Prepare,
    Compile,
    Simulate,
    Compare,
    Finalize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionEvent {
    RunStarted {
        total: usize,
    },
    CaseQueued {
        case_id: String,
    },
    CaseStarted {
        case_id: String,
        worker: usize,
    },
    CasePhase {
        case_id: String,
        phase: PhaseKind,
    },
    CaseLog {
        case_id: String,
        level: LogLevel,
        message: String,
    },
    CaseFinished {
        case_id: String,
        status: CaseStatus,
        duration_ms: u64,
        classification: Option<String>,
    },
    RunProgress {
        completed: usize,
        passed: usize,
        failed: usize,
        skipped: usize,
    },
    RunFinished {
        summary: RunSummaryLite,
    },
    RunAborted {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub schema_version: u32,
    pub event_version: String,
    pub ts: String,
    pub run_id: String,
    pub seq: u64,
    pub payload: ExecutionEvent,
}

impl EventEnvelope {
    pub fn new(run_id: impl Into<String>, seq: u64, payload: ExecutionEvent) -> Self {
        Self {
            schema_version: EVENT_SCHEMA_VERSION,
            event_version: "v1".to_string(),
            ts: chrono::Utc::now().to_rfc3339(),
            run_id: run_id.into(),
            seq,
            payload,
        }
    }
}
