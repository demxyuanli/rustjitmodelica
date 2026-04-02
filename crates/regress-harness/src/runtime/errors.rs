use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ErrorContext {
    pub command: Option<String>,
    pub run_id: Option<String>,
    pub case_id: Option<String>,
    pub path: Option<String>,
    pub details: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredIssue {
    pub severity: Severity,
    pub code: String,
    pub message: String,
    pub suggestion: Option<String>,
    pub exit_code: Option<i32>,
    pub context: ErrorContext,
    pub cause_chain: Vec<String>,
    pub ts: String,
}

pub type RHResult<T> = Result<T, StructuredIssue>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IssueBundle {
    pub issues: Vec<StructuredIssue>,
}

fn exit_code_from_error_code(code: &str) -> i32 {
    match code {
        "E_OPTION_001" => 21,
        "E_OPTION_002" => 22,
        "E_COLLECTION_001" => 23,
        "E_COLLECTION_002" => 24,
        "E_MONITOR_001" => 31,
        "E_RUNTIME_001" => 32,
        "E_EVENT_001" => 41,
        "E_EVENT_002" => 42,
        "E_IO_001" => 51,
        "E_IO_002" => 52,
        "E_SYSTEM_001" => 53,
        _ => 50,
    }
}

impl StructuredIssue {
    pub fn error(code: impl Into<String>, message: impl Into<String>) -> Self {
        let code = code.into();
        Self {
            severity: Severity::Error,
            exit_code: Some(exit_code_from_error_code(&code)),
            code,
            message: message.into(),
            suggestion: None,
            context: ErrorContext {
                details: serde_json::json!({}),
                ..ErrorContext::default()
            },
            cause_chain: Vec::new(),
            ts: chrono::Utc::now().to_rfc3339(),
        }
    }

    pub fn warning(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            severity: Severity::Warning,
            code: code.into(),
            message: message.into(),
            suggestion: None,
            exit_code: None,
            context: ErrorContext {
                details: serde_json::json!({}),
                ..ErrorContext::default()
            },
            cause_chain: Vec::new(),
            ts: chrono::Utc::now().to_rfc3339(),
        }
    }
}

pub trait IntoStructuredIssue<T> {
    fn map_issue(self, code: &str, message: &str) -> RHResult<T>;
}

impl<T> IntoStructuredIssue<T> for anyhow::Result<T> {
    fn map_issue(self, code: &str, message: &str) -> RHResult<T> {
        self.map_err(|e| anyhow_to_issue(e, code).with_message(message))
    }
}

impl StructuredIssue {
    pub fn with_message(mut self, message: &str) -> Self {
        self.message = message.to_string();
        self
    }
}

pub fn issue_to_anyhow(issue: StructuredIssue) -> anyhow::Error {
    anyhow::anyhow!("[{}] {}", issue.code, issue.message)
}

pub fn anyhow_to_issue(err: anyhow::Error, fallback_code: &str) -> StructuredIssue {
    let mut out = StructuredIssue::error(fallback_code, err.to_string());
    out.cause_chain = err.chain().map(|x| x.to_string()).collect();
    out
}

pub fn resolve_exit_code(issue: &StructuredIssue) -> i32 {
    issue.exit_code.unwrap_or(0)
}

pub fn select_primary_issue(bundle: &IssueBundle) -> Option<StructuredIssue> {
    bundle
        .issues
        .iter()
        .max_by_key(|issue| issue.exit_code.unwrap_or(0))
        .cloned()
}
