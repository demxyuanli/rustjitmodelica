use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlanStrategy {
    Category,
    Feature,
    Relation,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceMode {
    Persistent,
    Ephemeral,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionPlanRequest {
    pub strategy: PlanStrategy,
    pub categories: Vec<String>,
    pub feature_ids: Vec<String>,
    pub changed_files: Vec<String>,
    pub include_indirect: bool,
    pub max_cases: Option<usize>,
    pub workspace_mode: WorkspaceMode,
    pub include_modelica_examples: bool,
    pub include_modelica_test: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkspaceStatus {
    Planned,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionWorkspaceInfo {
    pub workspace_id: String,
    pub workspace_path: String,
    pub strategy: PlanStrategy,
    pub status: WorkspaceStatus,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ReasonCode {
    ExpectationMet,
    ModelNotFound,
    DependencyMissing,
    NewtonNonconverged,
    ParseError,
    RuntimeError,
    Timeout,
    ProcessError,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunRecord {
    pub timestamp: String,
    pub case_type: String,
    pub case_name: String,
    pub duration_ms: u64,
    pub expect_target_ok: bool,
    pub actual_ok: bool,
    pub exit_code: i32,
    pub status: String,
    pub reason: ReasonCode,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SummaryLine {
    pub state: String,
    pub case_name: String,
    pub reason: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedCase {
    pub name: String,
    pub reason: String,
    pub priority: u32,
    pub category: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionExecutionPlan {
    pub strategy: PlanStrategy,
    pub changed_sources: Vec<String>,
    pub affected_features: Vec<String>,
    pub planned_cases: Vec<PlannedCase>,
    pub skipped_cases: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRunResult {
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegressionWorkspaceState {
    pub info: RegressionWorkspaceInfo,
    pub plan: RegressionExecutionPlan,
    pub result: Option<WorkspaceRunResult>,
    pub records: Vec<RunRecord>,
}
