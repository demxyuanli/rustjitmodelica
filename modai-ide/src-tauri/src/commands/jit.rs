use rustmodlica::ast::ClassItem;
use rustmodlica::parser;
use rustmodlica::SimulationResult;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use crate::component_library;
use crate::app_settings;
use crate::profiler::ScopedTimer;
use rustmodlica::equation_graph::EquationGraphMode;
use rustmodlica::CompileStopPhase;

use super::common::{JitValidateOptions, ResolverContext};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarningItem {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticErrorItem {
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitValidateResult {
    pub schema_version: String,
    pub success: bool,
    pub warnings: Vec<WarningItem>,
    pub errors: Vec<String>,
    pub diagnostics: Vec<DiagnosticErrorItem>,
    pub state_vars: Vec<String>,
    pub output_vars: Vec<String>,
    /// Compiler phase timings and counts for IDE output panel (replaces relying on backend stdout).
    pub compile_trace: Vec<String>,
    /// Completed tier: `full`, `parse`, `flatten`, or `analyze`.
    pub validation_stop_phase: Option<String>,
    /// True when stopped before JIT (`parse` / `flatten` / `analyze`).
    pub validation_partial: bool,
    /// Equation/component provenance summary and optional param-impact probe (after flatten+inline tiers).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<JitProvenanceReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JitProvenanceReport {
    pub equation_count: usize,
    pub variable_count: usize,
    pub parameter_closure_count: usize,
    pub instance_count: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub param_change_impact: Option<rustmodlica::ImpactAnalysisResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_change_impact: Option<rustmodlica::ImpactAnalysisResult>,
    /// Heuristic only; does not enable partial codegen.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub incremental_codegen_worthwhile_hint: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JitApiMeta {
    pub schema_version: String,
    pub operation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JitApiError {
    pub code: String,
    pub message: String,
    pub path: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JitApiEnvelope<T> {
    pub ok: bool,
    pub meta: JitApiMeta,
    pub data: Option<T>,
    pub errors: Vec<JitApiError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JitValidateRequest {
    code: String,
    model_name: Option<String>,
    project_dir: Option<String>,
    options: Option<JitValidateOptions>,
    resolver_context: Option<ResolverContext>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ValidateCliWarning {
    path: String,
    line: usize,
    column: usize,
    message: String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct ValidateCliOutput {
    success: bool,
    warnings: Vec<ValidateCliWarning>,
    errors: Vec<String>,
    state_vars: Vec<String>,
    output_vars: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSimulationRequest {
    code: String,
    model_name: Option<String>,
    project_dir: Option<String>,
    options: Option<JitValidateOptions>,
    resolver_context: Option<ResolverContext>,
}

const JIT_API_SCHEMA_VERSION: &str = "jit.api.v1";
const JIT_PROGRESS_EVENT: &str = "modai-jit-progress";
const MONITOR_EVENTS_DIR: &str = "monitor-events";
const MONITOR_EVENTS_PER_SESSION: usize = 300;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JitProgressEvent {
    category: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    task: String,
    stage: String,
    elapsed_sec: u64,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    current_step: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total_steps: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JitProgressEventRecord {
    pub ts_millis: u64,
    #[serde(flatten)]
    event: JitProgressEvent,
}

fn monitor_events_dir() -> Result<PathBuf, String> {
    let dir = crate::app_data::app_data_root()?.join(MONITOR_EVENTS_DIR);
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn monitor_events_file_path(session_id: &str) -> Result<PathBuf, String> {
    let safe = session_id
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect::<String>();
    Ok(monitor_events_dir()?.join(format!("{}.json", safe)))
}

fn current_ts_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn persist_monitor_event(record: JitProgressEventRecord) {
    let sid = record
        .event
        .session_id
        .clone()
        .unwrap_or_else(|| "global".to_string());
    let path = match monitor_events_file_path(&sid) {
        Ok(p) => p,
        Err(_) => return,
    };
    let mut records: Vec<JitProgressEventRecord> = match fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    };
    records.push(record);
    if records.len() > MONITOR_EVENTS_PER_SESSION {
        let drop_n = records.len() - MONITOR_EVENTS_PER_SESSION;
        records.drain(0..drop_n);
    }
    if let Ok(content) = serde_json::to_string(&records) {
        let _ = fs::write(path, content);
    }
}

fn emit_and_persist(app: &AppHandle, payload: JitProgressEvent) {
    let _ = app.emit(JIT_PROGRESS_EVENT, payload.clone());
    persist_monitor_event(JitProgressEventRecord {
        ts_millis: current_ts_millis(),
        event: payload,
    });
}

fn emit_jit_progress(app: &AppHandle, task: &str, stage: &str, elapsed_sec: u64, message: impl Into<String>) {
    let payload = JitProgressEvent {
        category: "progress".to_string(),
        session_id: None,
        task: task.to_string(),
        stage: stage.to_string(),
        elapsed_sec,
        message: message.into(),
        current_step: None,
        total_steps: None,
        reason: None,
    };
    emit_and_persist(app, payload);
}

fn emit_jit_progress_for_session(
    app: &AppHandle,
    session_id: &str,
    task: &str,
    stage: &str,
    elapsed_sec: u64,
    message: impl Into<String>,
    current_step: Option<usize>,
    total_steps: Option<usize>,
    reason: Option<&str>,
) {
    let payload = JitProgressEvent {
        category: "progress".to_string(),
        session_id: Some(session_id.to_string()),
        task: task.to_string(),
        stage: stage.to_string(),
        elapsed_sec,
        message: message.into(),
        current_step,
        total_steps,
        reason: reason.map(|s| s.to_string()),
    };
    emit_and_persist(app, payload);
}

fn emit_jit_control(
    app: &AppHandle,
    task: &str,
    stage: &str,
    elapsed_sec: u64,
    message: impl Into<String>,
    current_step: Option<usize>,
    total_steps: Option<usize>,
    reason: Option<&str>,
) {
    let payload = JitProgressEvent {
        category: "control".to_string(),
        session_id: None,
        task: task.to_string(),
        stage: stage.to_string(),
        elapsed_sec,
        message: message.into(),
        current_step,
        total_steps,
        reason: reason.map(|s| s.to_string()),
    };
    emit_and_persist(app, payload);
}

fn emit_jit_error(
    app: &AppHandle,
    task: &str,
    stage: &str,
    elapsed_sec: u64,
    message: impl Into<String>,
    reason: Option<&str>,
) {
    let payload = JitProgressEvent {
        category: "error".to_string(),
        session_id: None,
        task: task.to_string(),
        stage: stage.to_string(),
        elapsed_sec,
        message: message.into(),
        current_step: None,
        total_steps: None,
        reason: reason.map(|s| s.to_string()),
    };
    emit_and_persist(app, payload);
}

fn classify_error_code(message: &str) -> Cow<'static, str> {
    let m = message.to_lowercase();
    if m.contains("parse") {
        Cow::Borrowed("PARSE_ERROR")
    } else if m.contains("model not found") || m.contains("could not find model") {
        Cow::Borrowed("MODEL_NOT_FOUND")
    } else if m.contains("constrainedby") {
        Cow::Borrowed("FLATTEN_CONSTRAINEDBY")
    } else if m.contains("newton") {
        Cow::Borrowed("SIM_NEWTON_FAILURE")
    } else if m.contains("simulation") {
        Cow::Borrowed("SIMULATION_ERROR")
    } else {
        Cow::Borrowed("JIT_ERROR")
    }
}

fn parse_location_from_error(message: &str) -> (Option<String>, Option<usize>, Option<usize>) {
    for line in message.lines() {
        let text = line.trim();
        if let Some(rest) = text.strip_prefix("-->") {
            let loc = rest.trim();
            let mut parts = loc.rsplitn(3, ':').collect::<Vec<_>>();
            if parts.len() == 3 {
                parts.reverse();
                let path = parts[0].trim().to_string();
                let line_no = parts[1].trim().parse::<usize>().ok();
                let col_no = parts[2].trim().parse::<usize>().ok();
                return (Some(path), line_no, col_no);
            }
            return (Some(loc.to_string()), None, None);
        }
    }
    (None, None, None)
}

fn diagnostics_from_error_message(message: &str) -> DiagnosticErrorItem {
    let (path, line, column) = parse_location_from_error(message);
    DiagnosticErrorItem {
        code: classify_error_code(message).into_owned(),
        message: message.to_string(),
        path,
        line,
        column,
    }
}

fn to_api_errors(items: &[DiagnosticErrorItem]) -> Vec<JitApiError> {
    items
        .iter()
        .map(|item| JitApiError {
            code: item.code.clone(),
            message: item.message.clone(),
            path: item.path.clone(),
            line: item.line,
            column: item.column,
        })
        .collect()
}

fn envelope_ok<T>(operation: &str, data: T) -> JitApiEnvelope<T> {
    JitApiEnvelope {
        ok: true,
        meta: JitApiMeta {
            schema_version: JIT_API_SCHEMA_VERSION.to_string(),
            operation: operation.to_string(),
        },
        data: Some(data),
        errors: Vec::new(),
    }
}

fn envelope_err<T>(operation: &str, errors: Vec<JitApiError>) -> JitApiEnvelope<T> {
    JitApiEnvelope {
        ok: false,
        meta: JitApiMeta {
            schema_version: JIT_API_SCHEMA_VERSION.to_string(),
            operation: operation.to_string(),
        },
        data: None,
        errors,
    }
}

fn envelope_err_with_data<T>(operation: &str, data: T, errors: Vec<JitApiError>) -> JitApiEnvelope<T> {
    JitApiEnvelope {
        ok: false,
        meta: JitApiMeta {
            schema_version: JIT_API_SCHEMA_VERSION.to_string(),
            operation: operation.to_string(),
        },
        data: Some(data),
        errors,
    }
}

fn build_compile_trace(
    perf: Option<&rustmodlica::compiler::CompilePerfReport>,
    state_vars_len: usize,
    output_vars_len: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!(
        "compile: eq_expand_parallel_mode={}",
        current_eq_expand_parallel_mode_from_env()
    ));
    if let Some(p) = perf {
        lines.push(format!("compile: load_model {} ms", p.load_model_ms));
        lines.push(format!("compile: flatten_inline {} ms", p.flatten_inline_ms));
        lines.push(format!(
            "compile: trackA flatten_wall_us={} inline_wall_us={}",
            p.flatten_wall_us, p.inline_wall_us
        ));
        lines.push(format!("compile: analyze {} ms", p.analyze_ms));
        lines.push(format!("compile: backend_dae {} ms", p.backend_dae_ms));
        lines.push(format!(
            "compile: backend_dae_cache_status={}",
            p.backend_dae_cache_status
        ));
        lines.push(format!(
            "compile: param_only_update={}",
            p.param_only_update
        ));
        lines.push(format!("compile: external_resolve {} ms", p.external_resolve_ms));
        lines.push(format!(
            "compile: trackB codegen_wall_us={} codegen_wall_ms={} jit_ms={}",
            p.codegen_wall_us, p.codegen_wall_ms, p.jit_ms
        ));
        lines.push(format!(
            "compile: layout states={} discrete={} params={} alg_eq={} diff_eq={}",
            p.state_count,
            p.discrete_count,
            p.param_count,
            p.alg_eq_count,
            p.diff_eq_count
        ));
        if p.jit_compile_ok {
            lines.push("compile: JIT codegen OK".to_string());
        } else if let Some(ref je) = p.jit_error {
            lines.push(format!("compile: JIT error: {je}"));
        }
        if p.fallback_total > 0 {
            lines.push(format!("compile: fallbacks total={}", p.fallback_total));
        }
    } else {
        lines.push("compile: (no perf report)".to_string());
    }
    lines.push(format!(
        "validate: state_vars={} output_vars={}",
        state_vars_len, output_vars_len
    ));
    lines
}

fn build_compile_trace_from_perf_value(
    perf: &serde_json::Value,
    state_vars_len: usize,
    output_vars_len: usize,
) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!(
        "compile: eq_expand_parallel_mode={}",
        current_eq_expand_parallel_mode_from_env()
    ));
    let u64_field = |key: &str| perf.get(key).and_then(|v| v.as_u64()).unwrap_or(0);
    let str_field = |key: &str| {
        perf.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    };
    let bool_field = |key: &str| perf.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
    lines.push(format!("compile: load_model {} ms", u64_field("load_model_ms")));
    lines.push(format!(
        "compile: flatten_inline {} ms",
        u64_field("flatten_inline_ms")
    ));
    lines.push(format!(
        "compile: trackA flatten_wall_us={} inline_wall_us={}",
        u64_field("flatten_wall_us"),
        u64_field("inline_wall_us")
    ));
    lines.push(format!("compile: analyze {} ms", u64_field("analyze_ms")));
    lines.push(format!("compile: backend_dae {} ms", u64_field("backend_dae_ms")));
    lines.push(format!(
        "compile: backend_dae_cache_status={}",
        str_field("backend_dae_cache_status")
    ));
    lines.push(format!(
        "compile: param_only_update={}",
        bool_field("param_only_update")
    ));
    lines.push(format!(
        "compile: external_resolve {} ms",
        u64_field("external_resolve_ms")
    ));
    lines.push(format!(
        "compile: trackB codegen_wall_us={} codegen_wall_ms={} jit_ms={}",
        u64_field("codegen_wall_us"),
        u64_field("codegen_wall_ms"),
        u64_field("jit_ms")
    ));
    lines.push(format!(
        "compile: layout states={} discrete={} params={} alg_eq={} diff_eq={}",
        u64_field("state_count"),
        u64_field("discrete_count"),
        u64_field("param_count"),
        u64_field("alg_eq_count"),
        u64_field("diff_eq_count")
    ));
    if bool_field("jit_compile_ok") {
        lines.push("compile: JIT codegen OK".to_string());
    } else if let Some(je) = perf.get("jit_error").and_then(|v| v.as_str()) {
        if !je.is_empty() {
            lines.push(format!("compile: JIT error: {je}"));
        }
    }
    let fallback_total = u64_field("fallback_total");
    if fallback_total > 0 {
        lines.push(format!("compile: fallbacks total={fallback_total}"));
    }
    if bool_field("salsa_process_db_hit") {
        lines.push("compile: salsa_process_db_hit=true".to_string());
    }
    lines.push(format!(
        "validate: state_vars={state_vars_len} output_vars={output_vars_len}"
    ));
    lines
}

fn validate_tier_cli_string(opts: Option<&JitValidateOptions>) -> String {
    opts.and_then(|o| o.validation_tier.as_deref())
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "full".to_string())
        .to_string()
}

fn ide_validate_inprocess_forced() -> bool {
    std::env::var("RUSTMODLICA_IDE_VALIDATE_INPROCESS")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false)
}

fn map_worker_validate_response(
    resp: crate::validate_stdio_worker::ValidateWorkerResponse,
    provenance: Option<JitProvenanceReport>,
) -> JitValidateResult {
    let warnings: Vec<rustmodlica::WarningInfo> = resp
        .warnings
        .into_iter()
        .map(|w| rustmodlica::WarningInfo {
            path: w.path,
            line: w.line,
            column: w.column,
            message: w.message,
            source: None,
        })
        .collect();
    let state_len = resp.state_vars.len();
    let output_len = resp.output_vars.len();
    let compile_trace = if let Some(ref perf) = resp.compile_perf {
        build_compile_trace_from_perf_value(perf, state_len, output_len)
    } else {
        build_compile_trace(None, state_len, output_len)
    };
    let diagnostics: Vec<DiagnosticErrorItem> = resp
        .errors
        .iter()
        .map(|m| diagnostics_from_error_message(m))
        .collect();
    jit_validate_result_body(
        resp.success,
        warnings,
        resp.errors,
        diagnostics,
        resp.state_vars,
        resp.output_vars,
        compile_trace,
        resp.validation_stop_phase,
        resp.validation_partial,
        provenance,
    )
}

fn parse_validation_tier(s: &str) -> Option<CompileStopPhase> {
    match s.trim().to_ascii_lowercase().as_str() {
        "full" => Some(CompileStopPhase::Full),
        "parse" => Some(CompileStopPhase::Parse),
        "flatten" => Some(CompileStopPhase::Flatten),
        "analyze" => Some(CompileStopPhase::Analyze),
        _ => None,
    }
}

fn normalize_eq_expand_parallel_mode(opts: Option<&JitValidateOptions>) -> &'static str {
    match opts
        .and_then(|o| o.eq_expand_parallel_mode.as_deref())
        .map(|s| s.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("guarded") => "guarded",
        Some("on") => "on",
        _ => "off",
    }
}

fn current_eq_expand_parallel_mode_from_env() -> &'static str {
    match std::env::var("RUSTMODLICA_EQ_EXPAND_PARALLEL_MODE")
        .ok()
        .map(|s| s.trim().to_ascii_lowercase())
        .as_deref()
    {
        Some("guarded") => "guarded",
        Some("on") => "on",
        _ => "off",
    }
}

struct ScopedEnvVar {
    key: &'static str,
    prev: Option<String>,
}

/// Default Salsa + process DB when env vars are unset (IDE validate/sim hot path).
struct SalsaEnvDefaults {
    _salsa: Option<ScopedEnvVar>,
    _process_db: Option<ScopedEnvVar>,
}

impl SalsaEnvDefaults {
    fn install() -> Self {
        Self {
            _salsa: if std::env::var("RUSTMODLICA_SALSA").is_err() {
                Some(ScopedEnvVar::set("RUSTMODLICA_SALSA", "1"))
            } else {
                None
            },
            _process_db: if std::env::var("RUSTMODLICA_SALSA_PROCESS_DB").is_err() {
                Some(ScopedEnvVar::set("RUSTMODLICA_SALSA_PROCESS_DB", "1"))
            } else {
                None
            },
        }
    }
}

impl ScopedEnvVar {
    fn set(key: &'static str, value: &str) -> Self {
        let prev = std::env::var(key).ok();
        unsafe { std::env::set_var(key, value) };
        Self { key, prev }
    }
}

impl Drop for ScopedEnvVar {
    fn drop(&mut self) {
        if let Some(ref v) = self.prev {
            unsafe { std::env::set_var(self.key, v) };
        } else {
            unsafe { std::env::remove_var(self.key) };
        }
    }
}

fn build_compiler_options(opts: Option<JitValidateOptions>) -> rustmodlica::CompilerOptions {
    let mut out = rustmodlica::CompilerOptions::default();
    if let Some(opts) = opts {
        if let Some(v) = opts.t_end {
            out.t_end = v;
        }
        if let Some(v) = opts.dt {
            out.dt = v;
        }
        if let Some(v) = opts.atol {
            out.atol = v;
        }
        if let Some(v) = opts.rtol {
            out.rtol = v;
        }
        if let Some(v) = opts.solver {
            out.solver = v;
        }
        if let Some(v) = opts.output_interval {
            out.output_interval = v;
        }
        if let Some(v) = opts.coarse_constrainedby_only {
            out.coarse_constrainedby_only = v;
        }
        if let Some(ref t) = opts.validation_tier {
            if let Some(p) = parse_validation_tier(t) {
                out.compile_stop = p;
            }
        }
    }
    out
}

fn map_validate_warnings(warnings: Vec<rustmodlica::WarningInfo>) -> Vec<WarningItem> {
    warnings
        .into_iter()
        .map(|w| WarningItem {
            path: w.path,
            line: w.line,
            column: w.column,
            message: w.message,
        })
        .collect()
}

fn jit_validate_result_body(
    success: bool,
    warnings: Vec<rustmodlica::WarningInfo>,
    errors: Vec<String>,
    diagnostics: Vec<DiagnosticErrorItem>,
    state_vars: Vec<String>,
    output_vars: Vec<String>,
    compile_trace: Vec<String>,
    validation_stop_phase: Option<String>,
    validation_partial: bool,
    provenance: Option<JitProvenanceReport>,
) -> JitValidateResult {
    JitValidateResult {
        schema_version: JIT_API_SCHEMA_VERSION.to_string(),
        success,
        warnings: map_validate_warnings(warnings),
        errors,
        diagnostics,
        state_vars,
        output_vars,
        compile_trace,
        validation_stop_phase,
        validation_partial,
        provenance,
    }
}

fn build_provenance_report(
    compiler: &rustmodlica::Compiler,
    param_probe: Option<&Vec<String>>,
    instance_probe: Option<&str>,
) -> Option<JitProvenanceReport> {
    let ix = compiler.last_provenance_index.as_ref()?;
    let st = ix.stats();
    let param_change_impact = param_probe
        .filter(|v| !v.is_empty())
        .map(|params| rustmodlica::analyze_change_impact(ix.as_ref(), params));
    let instance_change_impact = instance_probe
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|path| rustmodlica::analyze_instance_change_impact(ix.as_ref(), path));
    let incremental_codegen_worthwhile_hint = param_change_impact
        .as_ref()
        .map(|p| rustmodlica::incremental_codegen_worthwhile_hint(ix.as_ref(), p))
        .or_else(|| {
            instance_change_impact
                .as_ref()
                .map(|i| rustmodlica::incremental_codegen_worthwhile_hint(ix.as_ref(), i))
        });
    Some(JitProvenanceReport {
        equation_count: st.equation_count,
        variable_count: st.variable_count,
        parameter_closure_count: st.parameter_count,
        instance_count: st.instance_count,
        param_change_impact,
        instance_change_impact,
        incremental_codegen_worthwhile_hint,
    })
}

fn resolve_model_name(source: &str, requested: Option<&String>) -> Result<String, String> {
    if let Some(name) = requested.filter(|value| !value.trim().is_empty()) {
        return Ok(name.clone());
    }
    let item = parser::parse(source).map_err(|e| e.to_string())?;
    Ok(match item {
        ClassItem::Model(model) => model.name,
        ClassItem::Function(function) => function.name,
    })
}

/// Collect the loader paths for a project + optional explicit resolver
/// context, using the same precedence rules as `with_loader_paths`. Lets
/// the equation-graph actor build a deterministic cache key without
/// constructing a Compiler first.
pub(crate) fn collect_loader_paths(
    project_dir: Option<&String>,
    resolver_context: Option<&ResolverContext>,
) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    if let Some(ctx) = resolver_context {
        for p in &ctx.library_paths {
            out.push(PathBuf::from(p));
        }
        return out;
    }
    if let Ok(paths) = component_library::compiler_loader_paths(project_dir.map(Path::new)) {
        for path in paths {
            out.push(path);
        }
    }

    let mut added_modelica = false;
    if let Ok(settings) = app_settings::load_settings() {
        let raw = settings.extensions.modelica_stdlib_path;
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            let as_root = p.join("Modelica").join("package.mo");
            let as_modelica = p.join("package.mo");
            if as_root.is_file() {
                out.push(p);
                added_modelica = true;
            } else if as_modelica.is_file() {
                if let Some(parent) = p.parent() {
                    out.push(parent.to_path_buf());
                    added_modelica = true;
                }
            }
        }
    }

    if !added_modelica {
        if let Ok(root) = component_library::installed_libraries_root() {
            let modelica_package = PathBuf::from("Modelica").join("package.mo");
            if root.join(&modelica_package).is_file() {
                out.push(root);
            } else if let Ok(entries) = std::fs::read_dir(&root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() && path.join(&modelica_package).is_file() {
                        out.push(path);
                        break;
                    }
                }
            }
        }
    }

    out
}

fn with_loader_paths(
    compiler: &mut rustmodlica::Compiler,
    project_dir: Option<&String>,
    resolver_context: Option<&ResolverContext>,
) {
    if let Some(ctx) = resolver_context {
        for p in &ctx.library_paths {
            compiler.loader.add_path(PathBuf::from(p));
        }
        return;
    }
    if let Ok(paths) = component_library::compiler_loader_paths(project_dir.map(Path::new)) {
        for path in paths {
            compiler.loader.add_path(path);
        }
    }

    let mut added_modelica = false;
    if let Ok(settings) = app_settings::load_settings() {
        let raw = settings.extensions.modelica_stdlib_path;
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            let p = PathBuf::from(trimmed);
            // The loader expects library roots such that joining "Modelica/..." resolves.
            // Accept either:
            // - <root> where <root>/Modelica/package.mo exists
            // - <root>/Modelica where <root>/Modelica/package.mo exists (then use parent)
            let as_root = p.join("Modelica").join("package.mo");
            let as_modelica = p.join("package.mo");
            if as_root.is_file() {
                compiler.loader.add_path(p);
                added_modelica = true;
            } else if as_modelica.is_file() {
                if let Some(parent) = p.parent() {
                    compiler.loader.add_path(parent.to_path_buf());
                    added_modelica = true;
                }
            }
        }
    }

    if !added_modelica {
        // Fallback: auto-detect a Modelica stdlib under the installed libraries root.
        if let Ok(root) = component_library::installed_libraries_root() {
            let modelica_package = PathBuf::from("Modelica").join("package.mo");
            if root.join(&modelica_package).is_file() {
                compiler.loader.add_path(root);
            } else if let Ok(entries) = std::fs::read_dir(&root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() && path.join(&modelica_package).is_file() {
                        compiler.loader.add_path(path);
                        break;
                    }
                }
            }
        }
    }
}


include!("jit_part_a.rs");
include!("jit_part_b.rs");
