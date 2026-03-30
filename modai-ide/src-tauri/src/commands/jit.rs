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
    if let Some(p) = perf {
        lines.push(format!("compile: load_model {} ms", p.load_model_ms));
        lines.push(format!("compile: flatten_inline {} ms", p.flatten_inline_ms));
        lines.push(format!("compile: analyze {} ms", p.analyze_ms));
        lines.push(format!("compile: backend_dae {} ms", p.backend_dae_ms));
        lines.push(format!("compile: external_resolve {} ms", p.external_resolve_ms));
        lines.push(format!("compile: jit {} ms", p.jit_ms));
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

fn parse_validation_tier(s: &str) -> Option<CompileStopPhase> {
    match s.trim().to_ascii_lowercase().as_str() {
        "full" => Some(CompileStopPhase::Full),
        "parse" => Some(CompileStopPhase::Parse),
        "flatten" => Some(CompileStopPhase::Flatten),
        "analyze" => Some(CompileStopPhase::Analyze),
        _ => None,
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
    }
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

fn jit_validate_sync(request: JitValidateRequest) -> Result<JitValidateResult, String> {
    let _timer = ScopedTimer::new("jit_validate");
    let model_name = resolve_model_name(&request.code, request.model_name.as_ref())?;
    let mut compiler = rustmodlica::Compiler::new();
    compiler.options = build_compiler_options(request.options);
    compiler.options.validate_only = true;
    with_loader_paths(
        &mut compiler,
        request.project_dir.as_ref(),
        request.resolver_context.as_ref(),
    );
    let result = compiler.compile_from_source(&model_name, &request.code);
    let warnings = compiler.take_warnings();
    let perf = compiler.take_compile_perf_report();
    match result {
        Ok(rustmodlica::CompileOutput::FunctionRun(_)) => {
            let compile_trace = build_compile_trace(perf.as_ref(), 0, 0);
            Ok(jit_validate_result_body(
                true,
                warnings,
                vec![],
                vec![],
                vec![],
                vec![],
                compile_trace,
                Some("full".to_string()),
                false,
            ))
        }
        Ok(rustmodlica::CompileOutput::Simulation(artifacts)) => {
            let state_vars = artifacts.state_vars;
            let output_vars = artifacts.output_vars;
            let compile_trace = build_compile_trace(
                perf.as_ref(),
                state_vars.len(),
                output_vars.len(),
            );
            Ok(jit_validate_result_body(
                true,
                warnings,
                vec![],
                vec![],
                state_vars,
                output_vars,
                compile_trace,
                Some("full".to_string()),
                false,
            ))
        }
        Ok(rustmodlica::CompileOutput::FlatSnapshotDone) => {
            let compile_trace = build_compile_trace(perf.as_ref(), 0, 0);
            Ok(jit_validate_result_body(
                true,
                warnings,
                vec![],
                vec![],
                vec![],
                vec![],
                compile_trace,
                Some("full".to_string()),
                false,
            ))
        }
        Ok(rustmodlica::CompileOutput::ValidationParseOk) => {
            let compile_trace = build_compile_trace(perf.as_ref(), 0, 0);
            Ok(jit_validate_result_body(
                true,
                warnings,
                vec![],
                vec![],
                vec![],
                vec![],
                compile_trace,
                Some("parse".to_string()),
                true,
            ))
        }
        Ok(rustmodlica::CompileOutput::ValidationFlattenOk { .. }) => {
            let compile_trace = build_compile_trace(perf.as_ref(), 0, 0);
            Ok(jit_validate_result_body(
                true,
                warnings,
                vec![],
                vec![],
                vec![],
                vec![],
                compile_trace,
                Some("flatten".to_string()),
                true,
            ))
        }
        Ok(rustmodlica::CompileOutput::ValidationAnalyzed(s)) => {
            let state_vars = s.state_vars;
            let output_vars = s.output_vars;
            let compile_trace = build_compile_trace(
                perf.as_ref(),
                state_vars.len(),
                output_vars.len(),
            );
            Ok(jit_validate_result_body(
                true,
                warnings,
                vec![],
                vec![],
                state_vars,
                output_vars,
                compile_trace,
                Some("analyze".to_string()),
                true,
            ))
        }
        Err(err) => {
            let message = err.to_string();
            let diagnostics = vec![diagnostics_from_error_message(&message)];
            let compile_trace = build_compile_trace(perf.as_ref(), 0, 0);
            Ok(jit_validate_result_body(
                false,
                warnings,
                vec![message],
                diagnostics,
                vec![],
                vec![],
                compile_trace,
                None,
                false,
            ))
        }
    }
}

#[tauri::command]
pub async fn jit_validate(request: JitValidateRequest) -> Result<JitValidateResult, String> {
    tokio::task::spawn_blocking(move || jit_validate_sync(request))
        .await
        .map_err(|e| format!("blocking task join error: {e}"))?
}

#[tauri::command]
pub async fn jit_validate_v2(
    app: AppHandle,
    request: JitValidateRequest,
) -> Result<JitApiEnvelope<JitValidateResult>, String> {
    let task_name = "validate";
    emit_jit_progress(&app, task_name, "started", 0, "Validation started");
    let started = Instant::now();
    let mut task = tokio::task::spawn_blocking(move || jit_validate_sync(request));
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let result = loop {
        tokio::select! {
            join_res = &mut task => {
                break join_res;
            }
            _ = heartbeat.tick() => {
                let elapsed = started.elapsed().as_secs().max(1);
                emit_jit_progress(
                    &app,
                    task_name,
                    "running",
                    elapsed,
                    format!("Validation running ({}s)...", elapsed),
                );
            }
        }
    }
    .map_err(|e| {
        emit_jit_error(&app, task_name, "failed", started.elapsed().as_secs(), format!("Validation join error: {e}"), Some("join"));
        format!("blocking task join error: {e}")
    })??;
    emit_jit_progress(
        &app,
        task_name,
        if result.success { "completed" } else { "failed" },
        started.elapsed().as_secs().max(1),
        if result.success { "Validation completed" } else { "Validation completed with errors" },
    );
    if result.success {
        Ok(envelope_ok("validate", result))
    } else {
        let errors = if !result.diagnostics.is_empty() {
            to_api_errors(&result.diagnostics)
        } else {
            result
                .errors
                .iter()
                .map(|message| {
                    let d = diagnostics_from_error_message(message);
                    JitApiError {
                        code: d.code,
                        message: d.message,
                        path: d.path,
                        line: d.line,
                        column: d.column,
                    }
                })
                .collect()
        };
        Ok(envelope_err_with_data("validate", result, errors))
    }
}

fn run_simulation_sync(request: RunSimulationRequest) -> Result<SimulationResult, String> {
    let _timer = ScopedTimer::new("run_simulation_cmd");
    let model_name = resolve_model_name(&request.code, request.model_name.as_ref())?;
    let mut compiler = rustmodlica::Compiler::new();
    compiler.options = build_compiler_options(request.options);
    compiler.options.compile_stop = CompileStopPhase::Full;
    with_loader_paths(
        &mut compiler,
        request.project_dir.as_ref(),
        request.resolver_context.as_ref(),
    );
    let out = compiler
        .compile_from_source(&model_name, &request.code)
        .map_err(|e| e.to_string())?;
    let artifacts = match out {
        rustmodlica::CompileOutput::FunctionRun(_) => {
            return Err("Simulation requested for a function entry".to_string());
        }
        rustmodlica::CompileOutput::FlatSnapshotDone => {
            return Err("Flat snapshot only; simulation is not available".to_string());
        }
        rustmodlica::CompileOutput::ValidationParseOk
        | rustmodlica::CompileOutput::ValidationFlattenOk { .. }
        | rustmodlica::CompileOutput::ValidationAnalyzed(_) => {
            return Err("Simulation requires full compile (tiered validation is not allowed)".to_string());
        }
        rustmodlica::CompileOutput::Simulation(artifacts) => artifacts,
    };
    rustmodlica::run_simulation_collect(
        artifacts.calc_derivs,
        artifacts.when_count,
        artifacts.crossings_count,
        artifacts.states,
        artifacts.discrete_vals,
        artifacts.params,
        &artifacts.state_vars,
        &artifacts.discrete_vars,
        &artifacts.output_vars,
        &artifacts.output_start_vals,
        &artifacts.state_var_index,
        artifacts.t_end,
        artifacts.dt,
        artifacts.numeric_ode_jacobian,
        artifacts.symbolic_ode_jacobian.as_ref(),
        &artifacts.newton_tearing_var_names,
        artifacts.atol,
        artifacts.rtol,
        artifacts.differential_index,
        artifacts.ida_component_id.as_slice(),
        &artifacts.solver,
        artifacts.output_interval,
        &artifacts.clock_partition_schedule,
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_simulation_cmd(request: RunSimulationRequest) -> Result<SimulationResult, String> {
    tokio::task::spawn_blocking(move || run_simulation_sync(request))
        .await
        .map_err(|e| format!("blocking task join error: {e}"))?
}

#[tauri::command]
pub async fn run_simulation_cmd_v2(
    app: AppHandle,
    request: RunSimulationRequest,
) -> Result<JitApiEnvelope<SimulationResult>, String> {
    let task_name = "simulate";
    emit_jit_progress(&app, task_name, "started", 0, "Simulation started");
    let started = Instant::now();
    let mut task = tokio::task::spawn_blocking(move || run_simulation_sync(request));
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let join = loop {
        tokio::select! {
            join_res = &mut task => {
                break join_res;
            }
            _ = heartbeat.tick() => {
                let elapsed = started.elapsed().as_secs().max(1);
                emit_jit_progress(
                    &app,
                    task_name,
                    "running",
                    elapsed,
                    format!("Simulation running ({}s)...", elapsed),
                );
            }
        }
    };
    match join
        .map_err(|e| {
            emit_jit_error(&app, task_name, "failed", started.elapsed().as_secs(), format!("Simulation join error: {e}"), Some("join"));
            format!("blocking task join error: {e}")
        })?
    {
        Ok(data) => Ok(envelope_ok("simulate", data)),
        Err(message) => {
            let d = diagnostics_from_error_message(&message);
            emit_jit_error(
                &app,
                task_name,
                "failed",
                started.elapsed().as_secs().max(1),
                format!("Simulation failed: {}", message),
                Some("simulate"),
            );
            Ok(envelope_err(
                "simulate",
                vec![JitApiError {
                    code: d.code,
                    message: d.message,
                    path: d.path,
                    line: d.line,
                    column: d.column,
                }],
            ))
        }
    }
    .map(|env| {
        if env.ok {
            emit_jit_progress(
                &app,
                task_name,
                "completed",
                started.elapsed().as_secs().max(1),
                "Simulation completed",
            );
        }
        env
    })
}

fn get_equation_graph_sync(
    code: String,
    model_name: String,
    project_dir: Option<String>,
    graph_mode: Option<EquationGraphMode>,
) -> Result<rustmodlica::EquationGraph, String> {
    let mut compiler = rustmodlica::Compiler::new();
    with_loader_paths(&mut compiler, project_dir.as_ref(), None);
    compiler
        .get_equation_graph_from_source(
            &model_name,
            &code,
            graph_mode.unwrap_or(EquationGraphMode::Compact),
        )
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_equation_graph(
    code: String,
    model_name: String,
    project_dir: Option<String>,
    graph_mode: Option<EquationGraphMode>,
) -> Result<rustmodlica::EquationGraph, String> {
    tokio::task::spawn_blocking(move || get_equation_graph_sync(code, model_name, project_dir, graph_mode))
        .await
        .map_err(|e| format!("blocking task join error: {e}"))?
}

#[tauri::command]
pub async fn get_equation_graph_v2(
    app: AppHandle,
    code: String,
    model_name: String,
    project_dir: Option<String>,
    graph_mode: Option<EquationGraphMode>,
) -> Result<JitApiEnvelope<rustmodlica::EquationGraph>, String> {
    let task_name = "equation-graph";
    emit_jit_progress(&app, task_name, "started", 0, "Equation graph build started");
    let started = Instant::now();
    let mut task = tokio::task::spawn_blocking(move || get_equation_graph_sync(code, model_name, project_dir, graph_mode));
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let join = loop {
        tokio::select! {
            join_res = &mut task => {
                break join_res;
            }
            _ = heartbeat.tick() => {
                let elapsed = started.elapsed().as_secs().max(1);
                emit_jit_progress(
                    &app,
                    task_name,
                    "running",
                    elapsed,
                    format!("Equation graph building ({}s)...", elapsed),
                );
            }
        }
    };
    match join
        .map_err(|e| {
            emit_jit_error(
                &app,
                task_name,
                "failed",
                started.elapsed().as_secs(),
                format!("Equation graph join error: {e}"),
                Some("join"),
            );
            format!("blocking task join error: {e}")
        })?
    {
        Ok(data) => Ok(envelope_ok("equationGraph", data)),
        Err(message) => {
            let d = diagnostics_from_error_message(&message);
            emit_jit_error(
                &app,
                task_name,
                "failed",
                started.elapsed().as_secs().max(1),
                format!("Equation graph failed: {}", message),
                Some("equation-graph"),
            );
            Ok(envelope_err(
                "equationGraph",
                vec![JitApiError {
                    code: d.code,
                    message: d.message,
                    path: d.path,
                    line: d.line,
                    column: d.column,
                }],
            ))
        }
    }
    .map(|env| {
        if env.ok {
            emit_jit_progress(
                &app,
                task_name,
                "completed",
                started.elapsed().as_secs().max(1),
                "Equation graph build completed",
            );
        }
        env
    })
}

// --- Simulation session support for step-by-step debugging ---

use std::sync::Mutex;
use std::collections::HashMap;
use once_cell::sync::Lazy;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepState {
    pub time: f64,
    pub states: Vec<f64>,
    pub state_names: Vec<String>,
    pub discrete_vals: Vec<f64>,
    pub outputs: Vec<f64>,
    pub output_names: Vec<String>,
    pub active_events: Vec<String>,
    pub step_index: usize,
}

struct SimulationSession {
    result: SimulationResult,
    current_step: usize,
    state_names: Vec<String>,
    output_names: Vec<String>,
    paused: bool,
    started_at: Instant,
    last_progress_emit_at: Instant,
    last_progress_emit_step: usize,
    completion_emitted: bool,
}

static SESSIONS: Lazy<Mutex<HashMap<String, SimulationSession>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartSessionRequest {
    code: String,
    model_name: Option<String>,
    project_dir: Option<String>,
    options: Option<JitValidateOptions>,
    resolver_context: Option<ResolverContext>,
}

fn start_simulation_session_sync(request: StartSessionRequest) -> Result<String, String> {
    let _timer = ScopedTimer::new("start_simulation_session");
    let model_name = resolve_model_name(&request.code, request.model_name.as_ref())?;
    let mut compiler = rustmodlica::Compiler::new();
    compiler.options = build_compiler_options(request.options);
    compiler.options.compile_stop = CompileStopPhase::Full;
    with_loader_paths(
        &mut compiler,
        request.project_dir.as_ref(),
        request.resolver_context.as_ref(),
    );
    let out = compiler
        .compile_from_source(&model_name, &request.code)
        .map_err(|e| e.to_string())?;
    let artifacts = match out {
        rustmodlica::CompileOutput::FunctionRun(_) => {
            return Err("Simulation requested for a function entry".to_string());
        }
        rustmodlica::CompileOutput::FlatSnapshotDone => {
            return Err("Flat snapshot only; simulation is not available".to_string());
        }
        rustmodlica::CompileOutput::ValidationParseOk
        | rustmodlica::CompileOutput::ValidationFlattenOk { .. }
        | rustmodlica::CompileOutput::ValidationAnalyzed(_) => {
            return Err("Simulation requires full compile (tiered validation is not allowed)".to_string());
        }
        rustmodlica::CompileOutput::Simulation(artifacts) => artifacts,
    };

    let state_names = artifacts.state_vars.clone();
    let output_names = artifacts.output_vars.clone();

    let result = rustmodlica::run_simulation_collect(
        artifacts.calc_derivs,
        artifacts.when_count,
        artifacts.crossings_count,
        artifacts.states,
        artifacts.discrete_vals,
        artifacts.params,
        &artifacts.state_vars,
        &artifacts.discrete_vars,
        &artifacts.output_vars,
        &artifacts.output_start_vals,
        &artifacts.state_var_index,
        artifacts.t_end,
        artifacts.dt,
        artifacts.numeric_ode_jacobian,
        artifacts.symbolic_ode_jacobian.as_ref(),
        &artifacts.newton_tearing_var_names,
        artifacts.atol,
        artifacts.rtol,
        artifacts.differential_index,
        artifacts.ida_component_id.as_slice(),
        &artifacts.solver,
        artifacts.output_interval,
        &artifacts.clock_partition_schedule,
    )
    .map_err(|e| e.to_string())?;

    let session_id = format!("sim_{}", uuid_simple());
    let session = SimulationSession {
        result,
        current_step: 0,
        state_names,
        output_names,
        paused: false,
        started_at: Instant::now(),
        last_progress_emit_at: Instant::now(),
        last_progress_emit_step: 0,
        completion_emitted: false,
    };

    SESSIONS.lock().unwrap().insert(session_id.clone(), session);
    Ok(session_id)
}

#[tauri::command]
pub async fn start_simulation_session(app: AppHandle, request: StartSessionRequest) -> Result<String, String> {
    let task_name = "start-session";
    emit_jit_progress(&app, task_name, "started", 0, "Step-debug session compile started");
    let started = Instant::now();
    let mut task = tokio::task::spawn_blocking(move || start_simulation_session_sync(request));
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let join = loop {
        tokio::select! {
            join_res = &mut task => {
                break join_res;
            }
            _ = heartbeat.tick() => {
                let elapsed = started.elapsed().as_secs().max(1);
                emit_jit_progress(
                    &app,
                    task_name,
                    "running",
                    elapsed,
                    format!("Step-debug session compiling ({}s)...", elapsed),
                );
            }
        }
    };
    let sid = join
        .map_err(|e| {
            emit_jit_error(
                &app,
                task_name,
                "failed",
                started.elapsed().as_secs(),
                format!("Step-debug session join error: {e}"),
                Some("join"),
            );
            format!("blocking task join error: {e}")
        })?
        .map_err(|e| {
            emit_jit_error(
                &app,
                task_name,
                "failed",
                started.elapsed().as_secs(),
                format!("Step-debug session failed: {e}"),
                Some("start-session"),
            );
            e
        })?;
    emit_jit_progress_for_session(
        &app,
        &sid,
        task_name,
        "ready",
        started.elapsed().as_secs().max(1),
        "Step-debug session created",
        Some(0),
        None,
        Some("create"),
    );
    emit_jit_progress(
        &app,
        task_name,
        "completed",
        started.elapsed().as_secs().max(1),
        "Step-debug session ready",
    );
    Ok(sid)
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}_{}", d.as_millis(), d.subsec_nanos())
}

#[tauri::command]
pub fn simulation_step(app: AppHandle, session_id: String) -> Result<StepState, String> {
    const STEP_EMIT_EVERY: usize = 25;
    const STEP_EMIT_MIN_INTERVAL_SECS: u64 = 1;
    let mut sessions = SESSIONS.lock().unwrap();
    let session = match sessions.get_mut(&session_id) {
        Some(s) => s,
        None => {
            emit_jit_error(&app, "step-session", "failed", 0, "Step-debug session not found", Some("session-not-found"));
            return Err("Session not found".to_string());
        }
    };

    let total_steps = session.result.time.len();
    if session.current_step >= total_steps {
        if !session.completion_emitted {
            let elapsed = session.started_at.elapsed().as_secs().max(1);
            emit_jit_progress(
                &app,
                "step-session",
                "completed",
                elapsed,
                format!("Step-debug playback completed at {} steps", total_steps),
            );
            emit_jit_progress_for_session(
                &app,
                &session_id,
                "step-session",
                "completed",
                elapsed,
                format!("Step-debug playback completed at {} steps", total_steps),
                Some(total_steps),
                Some(total_steps),
                Some("completed"),
            );
            session.completion_emitted = true;
        }
        return Err("Simulation completed".to_string());
    }

    let idx = session.current_step;
    let time = session.result.time[idx];

    let mut states = Vec::new();
    for name in &session.state_names {
        if let Some(series) = session.result.series.get(name) {
            states.push(if idx < series.len() { series[idx] } else { 0.0 });
        }
    }

    let mut outputs = Vec::new();
    for name in &session.output_names {
        if let Some(series) = session.result.series.get(name) {
            outputs.push(if idx < series.len() { series[idx] } else { 0.0 });
        }
    }

    session.current_step += 1;
    let should_emit_by_step = session.current_step.saturating_sub(session.last_progress_emit_step) >= STEP_EMIT_EVERY;
    let should_emit_by_time = session.last_progress_emit_at.elapsed() >= Duration::from_secs(STEP_EMIT_MIN_INTERVAL_SECS);
    if should_emit_by_step && should_emit_by_time {
        let elapsed = session.started_at.elapsed().as_secs().max(1);
        emit_jit_progress_for_session(
            &app,
            &session_id,
            "step-session",
            if session.paused { "paused" } else { "running" },
            elapsed,
            format!("Step-debug progress: {}/{} steps", session.current_step, total_steps),
            Some(session.current_step),
            Some(total_steps),
            None,
        );
        session.last_progress_emit_at = Instant::now();
        session.last_progress_emit_step = session.current_step;
    }

    Ok(StepState {
        time,
        states,
        state_names: session.state_names.clone(),
        discrete_vals: vec![],
        outputs,
        output_names: session.output_names.clone(),
        active_events: vec![],
        step_index: idx,
    })
}

#[tauri::command]
pub fn simulation_command(app: AppHandle, session_id: String, command: String) -> Result<(), String> {
    let mut sessions = SESSIONS.lock().unwrap();
    match command.as_str() {
        "pause" => {
            if let Some(session) = sessions.get_mut(&session_id) {
                session.paused = true;
                let total_steps = session.result.time.len();
                emit_jit_control(
                    &app,
                    "step-session",
                    "paused",
                    session.started_at.elapsed().as_secs().max(1),
                    format!("Step-debug paused at step {}/{}", session.current_step, total_steps),
                    Some(session.current_step),
                    Some(total_steps),
                    Some("pause"),
                );
                emit_jit_progress_for_session(
                    &app,
                    &session_id,
                    "step-session",
                    "paused",
                    session.started_at.elapsed().as_secs().max(1),
                    "Step-debug paused",
                    Some(session.current_step),
                    Some(total_steps),
                    Some("pause"),
                );
            }
        }
        "run" => {
            if let Some(session) = sessions.get_mut(&session_id) {
                session.paused = false;
                let total_steps = session.result.time.len();
                emit_jit_control(
                    &app,
                    "step-session",
                    "running",
                    session.started_at.elapsed().as_secs().max(1),
                    format!("Step-debug resumed at step {}/{}", session.current_step, total_steps),
                    Some(session.current_step),
                    Some(total_steps),
                    Some("run"),
                );
                emit_jit_progress_for_session(
                    &app,
                    &session_id,
                    "step-session",
                    "running",
                    session.started_at.elapsed().as_secs().max(1),
                    "Step-debug resumed",
                    Some(session.current_step),
                    Some(total_steps),
                    Some("run"),
                );
            }
        }
        "stop" => {
            if let Some(session) = sessions.remove(&session_id) {
                let total_steps = session.result.time.len();
                emit_jit_control(
                    &app,
                    "step-session",
                    "stopped",
                    session.started_at.elapsed().as_secs().max(1),
                    format!("Step-debug stopped at step {}/{}", session.current_step, total_steps),
                    Some(session.current_step),
                    Some(total_steps),
                    Some("stop"),
                );
            }
        }
        "reset" => {
            if let Some(session) = sessions.remove(&session_id) {
                let total_steps = session.result.time.len();
                emit_jit_control(
                    &app,
                    "step-session",
                    "reset",
                    session.started_at.elapsed().as_secs().max(1),
                    format!("Step-debug reset at step {}/{}", session.current_step, total_steps),
                    Some(session.current_step),
                    Some(total_steps),
                    Some("reset"),
                );
            }
        }
        _ => {
            emit_jit_error(&app, "step-session", "failed", 0, format!("Unknown simulation command: {}", command), Some("unknown-command"));
            return Err(format!("Unknown command: {}", command));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn get_monitor_events(session_id: Option<String>, limit: Option<usize>) -> Result<Vec<JitProgressEventRecord>, String> {
    let max_n = limit.unwrap_or(200).max(1).min(1000);
    let path = if let Some(sid) = session_id {
        monitor_events_file_path(&sid)?
    } else {
        let dir = monitor_events_dir()?;
        let mut latest: Option<(std::time::SystemTime, PathBuf)> = None;
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = match entry {
                Ok(v) => v,
                Err(_) => continue,
            };
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            if latest.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                latest = Some((mtime, path));
            }
        }
        match latest {
            Some((_, p)) => p,
            None => return Ok(Vec::new()),
        }
    };
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut records: Vec<JitProgressEventRecord> = serde_json::from_str(&content).unwrap_or_default();
    if records.len() > max_n {
        records.drain(0..(records.len() - max_n));
    }
    Ok(records)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorEventSessionEntry {
    pub session_id: String,
    pub modified_ms: Option<u64>,
    pub event_count: usize,
}

#[tauri::command]
pub fn list_monitor_event_sessions(limit: Option<usize>) -> Result<Vec<MonitorEventSessionEntry>, String> {
    let max_n = limit.unwrap_or(50).max(1).min(200);
    let dir = monitor_events_dir()?;
    let mut rows: Vec<(std::time::SystemTime, String, PathBuf)> = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = match entry {
            Ok(v) => v,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => continue,
        };
        let mtime = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        rows.push((mtime, stem, path));
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    rows.truncate(max_n);
    Ok(rows
        .into_iter()
        .map(|(t, session_id, path)| {
            let modified_ms = t
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_millis() as u64);
            let event_count = fs::read_to_string(&path)
                .ok()
                .and_then(|content| serde_json::from_str::<Vec<JitProgressEventRecord>>(&content).ok())
                .map(|v| v.len())
                .unwrap_or(0);
            MonitorEventSessionEntry {
                session_id,
                modified_ms,
                event_count,
            }
        })
        .collect())
}

#[tauri::command]
pub fn get_simulation_state(session_id: String) -> Result<Option<StepState>, String> {
    let sessions = SESSIONS.lock().unwrap();
    let session = match sessions.get(&session_id) {
        Some(s) => s,
        None => return Ok(None),
    };
    if session.current_step == 0 || session.result.time.is_empty() {
        return Ok(None);
    }
    let idx = session.current_step.saturating_sub(1).min(session.result.time.len() - 1);
    let time = session.result.time[idx];
    let mut states = Vec::new();
    for name in &session.state_names {
        if let Some(series) = session.result.series.get(name) {
            states.push(if idx < series.len() { series[idx] } else { 0.0 });
        }
    }
    let mut outputs = Vec::new();
    for name in &session.output_names {
        if let Some(series) = session.result.series.get(name) {
            outputs.push(if idx < series.len() { series[idx] } else { 0.0 });
        }
    }
    Ok(Some(StepState {
        time,
        states,
        state_names: session.state_names.clone(),
        discrete_vals: vec![],
        outputs,
        output_names: session.output_names.clone(),
        active_events: vec![],
        step_index: idx,
    }))
}
