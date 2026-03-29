use rustmodlica::ast::ClassItem;
use rustmodlica::parser;
use rustmodlica::SimulationResult;
use serde::{Deserialize, Serialize};
use std::borrow::Cow;
use std::path::Path;
use std::path::PathBuf;

use crate::component_library;
use crate::app_settings;
use crate::profiler::ScopedTimer;

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
    }
    out
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

#[tauri::command]
pub fn jit_validate(request: JitValidateRequest) -> Result<JitValidateResult, String> {
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
    match result {
        Ok(rustmodlica::CompileOutput::FunctionRun(_)) => Ok(JitValidateResult {
            schema_version: JIT_API_SCHEMA_VERSION.to_string(),
            success: true,
            warnings: warnings
                .into_iter()
                .map(|w| WarningItem {
                    path: w.path,
                    line: w.line,
                    column: w.column,
                    message: w.message,
                })
                .collect(),
            errors: vec![],
            diagnostics: vec![],
            state_vars: vec![],
            output_vars: vec![],
        }),
        Ok(rustmodlica::CompileOutput::Simulation(artifacts)) => Ok(JitValidateResult {
            schema_version: JIT_API_SCHEMA_VERSION.to_string(),
            success: true,
            warnings: warnings
                .into_iter()
                .map(|w| WarningItem {
                    path: w.path,
                    line: w.line,
                    column: w.column,
                    message: w.message,
                })
                .collect(),
            errors: vec![],
            diagnostics: vec![],
            state_vars: artifacts.state_vars,
            output_vars: artifacts.output_vars,
        }),
        Ok(rustmodlica::CompileOutput::FlatSnapshotDone) => Ok(JitValidateResult {
            schema_version: JIT_API_SCHEMA_VERSION.to_string(),
            success: true,
            warnings: warnings
                .into_iter()
                .map(|w| WarningItem {
                    path: w.path,
                    line: w.line,
                    column: w.column,
                    message: w.message,
                })
                .collect(),
            errors: vec![],
            diagnostics: vec![],
            state_vars: vec![],
            output_vars: vec![],
        }),
        Err(err) => {
            let message = err.to_string();
            let diagnostics = vec![diagnostics_from_error_message(&message)];
            Ok(JitValidateResult {
            schema_version: JIT_API_SCHEMA_VERSION.to_string(),
            success: false,
            warnings: warnings
                .into_iter()
                .map(|w| WarningItem {
                    path: w.path,
                    line: w.line,
                    column: w.column,
                    message: w.message,
                })
                .collect(),
            errors: vec![message],
            diagnostics,
            state_vars: vec![],
            output_vars: vec![],
        })}
    }
}

#[tauri::command]
pub fn jit_validate_v2(request: JitValidateRequest) -> Result<JitApiEnvelope<JitValidateResult>, String> {
    let result = jit_validate(request)?;
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
        Ok(envelope_err("validate", errors))
    }
}

#[tauri::command]
pub fn run_simulation_cmd(request: RunSimulationRequest) -> Result<SimulationResult, String> {
    let _timer = ScopedTimer::new("run_simulation_cmd");
    let model_name = resolve_model_name(&request.code, request.model_name.as_ref())?;
    let mut compiler = rustmodlica::Compiler::new();
    compiler.options = build_compiler_options(request.options);
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
pub fn run_simulation_cmd_v2(
    request: RunSimulationRequest,
) -> Result<JitApiEnvelope<SimulationResult>, String> {
    match run_simulation_cmd(request) {
        Ok(data) => Ok(envelope_ok("simulate", data)),
        Err(message) => {
            let d = diagnostics_from_error_message(&message);
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
}

#[tauri::command]
pub fn get_equation_graph(
    code: String,
    model_name: String,
    project_dir: Option<String>,
) -> Result<rustmodlica::EquationGraph, String> {
    let mut compiler = rustmodlica::Compiler::new();
    if let Ok(paths) = component_library::compiler_loader_paths(project_dir.as_deref().map(Path::new)) {
        for path in paths {
            compiler.loader.add_path(path);
        }
    }
    compiler
        .get_equation_graph_from_source(&model_name, &code)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_equation_graph_v2(
    code: String,
    model_name: String,
    project_dir: Option<String>,
) -> Result<JitApiEnvelope<rustmodlica::EquationGraph>, String> {
    match get_equation_graph(code, model_name, project_dir) {
        Ok(data) => Ok(envelope_ok("equationGraph", data)),
        Err(message) => {
            let d = diagnostics_from_error_message(&message);
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

#[tauri::command]
pub fn start_simulation_session(request: StartSessionRequest) -> Result<String, String> {
    let _timer = ScopedTimer::new("start_simulation_session");
    let model_name = resolve_model_name(&request.code, request.model_name.as_ref())?;
    let mut compiler = rustmodlica::Compiler::new();
    compiler.options = build_compiler_options(request.options);
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
    };

    SESSIONS.lock().unwrap().insert(session_id.clone(), session);
    Ok(session_id)
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}_{}", d.as_millis(), d.subsec_nanos())
}

#[tauri::command]
pub fn simulation_step(session_id: String) -> Result<StepState, String> {
    let mut sessions = SESSIONS.lock().unwrap();
    let session = sessions
        .get_mut(&session_id)
        .ok_or_else(|| "Session not found".to_string())?;

    let total_steps = session.result.time.len();
    if session.current_step >= total_steps {
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
pub fn simulation_command(session_id: String, command: String) -> Result<(), String> {
    let mut sessions = SESSIONS.lock().unwrap();
    match command.as_str() {
        "pause" => {
            if let Some(session) = sessions.get_mut(&session_id) {
                session.paused = true;
            }
        }
        "run" => {
            if let Some(session) = sessions.get_mut(&session_id) {
                session.paused = false;
            }
        }
        "stop" | "reset" => {
            sessions.remove(&session_id);
        }
        _ => return Err(format!("Unknown command: {}", command)),
    }
    Ok(())
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
