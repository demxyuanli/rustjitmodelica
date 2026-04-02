use crate::report::CaseStatus;
use crate::runtime::events::{ExecutionEvent, RunSummaryLite};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionStateStore {
    pub run_id: String,
    pub total: usize,
    pub completed: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub running: usize,
    pub running_cases: BTreeSet<String>,
    pub case_status: BTreeMap<String, CaseStatus>,
    pub last_error: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSnapshot {
    pub run_id: String,
    pub summary: RunSummaryLite,
    pub top_running: Vec<String>,
    pub top_failed: Vec<String>,
    pub updated_at: String,
}

impl ExecutionStateStore {
    pub fn new(run_id: String, total: usize) -> Self {
        Self {
            run_id,
            total,
            completed: 0,
            passed: 0,
            failed: 0,
            skipped: 0,
            running: 0,
            running_cases: BTreeSet::new(),
            case_status: BTreeMap::new(),
            last_error: None,
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn apply_event(&mut self, ev: &ExecutionEvent) {
        match ev {
            ExecutionEvent::RunStarted { total } => {
                self.total = *total;
            }
            ExecutionEvent::CaseQueued { case_id } => {
                self.running_cases.remove(case_id);
            }
            ExecutionEvent::CaseStarted { case_id, .. } => {
                self.running_cases.insert(case_id.clone());
            }
            ExecutionEvent::CaseFinished {
                case_id, status, ..
            } => {
                self.running_cases.remove(case_id);
                self.case_status.insert(case_id.clone(), status.clone());
            }
            ExecutionEvent::RunProgress {
                completed,
                passed,
                failed,
                skipped,
            } => {
                self.completed = *completed;
                self.passed = *passed;
                self.failed = *failed;
                self.skipped = *skipped;
            }
            ExecutionEvent::RunFinished { summary } => {
                self.total = summary.total;
                self.completed = summary.total;
                self.passed = summary.passed;
                self.failed = summary.failed;
                self.skipped = summary.skipped;
            }
            ExecutionEvent::RunAborted { reason } => {
                self.last_error = Some(reason.clone());
            }
            ExecutionEvent::CasePhase { .. } | ExecutionEvent::CaseLog { .. } => {}
        }
        self.running = self.running_cases.len();
        self.updated_at = chrono::Utc::now().to_rfc3339();
    }

    pub fn snapshot(&self) -> ExecutionSnapshot {
        let top_running = self
            .running_cases
            .iter()
            .cloned()
            .take(8)
            .collect();
        let top_failed = self
            .case_status
            .iter()
            .filter(|(_, s)| **s == CaseStatus::Fail)
            .map(|(k, _)| k.clone())
            .take(8)
            .collect();
        ExecutionSnapshot {
            run_id: self.run_id.clone(),
            summary: RunSummaryLite {
                total: self.total,
                passed: self.passed,
                failed: self.failed,
                skipped: self.skipped,
            },
            top_running,
            top_failed,
            updated_at: self.updated_at.clone(),
        }
    }
}
