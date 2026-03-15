use rustmodlica::ast::ClassItem;
use rustmodlica::parser;
use rustmodlica::SimulationResult;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::component_library;

use super::common::JitValidateOptions;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WarningItem {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitValidateResult {
    pub success: bool,
    pub warnings: Vec<WarningItem>,
    pub errors: Vec<String>,
    pub state_vars: Vec<String>,
    pub output_vars: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JitValidateRequest {
    code: String,
    model_name: Option<String>,
    project_dir: Option<String>,
    options: Option<JitValidateOptions>,
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

fn with_loader_paths(compiler: &mut rustmodlica::Compiler, project_dir: Option<&String>) {
    if let Ok(paths) = component_library::compiler_loader_paths(project_dir.map(Path::new)) {
        for path in paths {
            compiler.loader.add_path(path);
        }
    }
}

#[tauri::command]
pub fn jit_validate(request: JitValidateRequest) -> Result<JitValidateResult, String> {
    let model_name = resolve_model_name(&request.code, request.model_name.as_ref())?;
    let mut compiler = rustmodlica::Compiler::new();
    compiler.options = build_compiler_options(request.options);
    with_loader_paths(&mut compiler, request.project_dir.as_ref());
    let result = compiler.compile_from_source(&model_name, &request.code);
    let warnings = compiler.take_warnings();
    match result {
        Ok(rustmodlica::CompileOutput::FunctionRun(_)) => Ok(JitValidateResult {
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
            state_vars: vec![],
            output_vars: vec![],
        }),
        Ok(rustmodlica::CompileOutput::Simulation(artifacts)) => Ok(JitValidateResult {
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
            state_vars: artifacts.state_vars,
            output_vars: artifacts.output_vars,
        }),
        Err(err) => Ok(JitValidateResult {
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
            errors: vec![err.to_string()],
            state_vars: vec![],
            output_vars: vec![],
        }),
    }
}

#[tauri::command]
pub fn run_simulation_cmd(request: RunSimulationRequest) -> Result<SimulationResult, String> {
    let model_name = resolve_model_name(&request.code, request.model_name.as_ref())?;
    let mut compiler = rustmodlica::Compiler::new();
    compiler.options = build_compiler_options(request.options);
    with_loader_paths(&mut compiler, request.project_dir.as_ref());
    let out = compiler
        .compile_from_source(&model_name, &request.code)
        .map_err(|e| e.to_string())?;
    let artifacts = match out {
        rustmodlica::CompileOutput::FunctionRun(_) => {
            return Err("Simulation requested for a function entry".to_string());
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
        &artifacts.state_var_index,
        artifacts.t_end,
        artifacts.dt,
        artifacts.numeric_ode_jacobian,
        artifacts.symbolic_ode_jacobian.as_ref(),
        &artifacts.newton_tearing_var_names,
        artifacts.atol,
        artifacts.rtol,
        &artifacts.solver,
        artifacts.output_interval,
    )
    .map_err(|e| e.to_string())
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
}

#[tauri::command]
pub fn start_simulation_session(request: StartSessionRequest) -> Result<String, String> {
    let model_name = resolve_model_name(&request.code, request.model_name.as_ref())?;
    let mut compiler = rustmodlica::Compiler::new();
    compiler.options = build_compiler_options(request.options);
    with_loader_paths(&mut compiler, request.project_dir.as_ref());
    let out = compiler
        .compile_from_source(&model_name, &request.code)
        .map_err(|e| e.to_string())?;
    let artifacts = match out {
        rustmodlica::CompileOutput::FunctionRun(_) => {
            return Err("Simulation requested for a function entry".to_string());
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
        &artifacts.state_var_index,
        artifacts.t_end,
        artifacts.dt,
        artifacts.numeric_ode_jacobian,
        artifacts.symbolic_ode_jacobian.as_ref(),
        &artifacts.newton_tearing_var_names,
        artifacts.atol,
        artifacts.rtol,
        &artifacts.solver,
        artifacts.output_interval,
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

pub(crate) fn parse_modelica_deps(content: &str) -> Option<(String, Vec<String>)> {
    let item = parser::parse(content).ok()?;
    let (class_name, extends) = match &item {
        ClassItem::Model(m) => (
            m.name.clone(),
            m.extends.iter().map(|e| e.model_name.clone()).collect(),
        ),
        ClassItem::Function(f) => (
            f.name.clone(),
            f.extends.iter().map(|e| e.model_name.clone()).collect(),
        ),
    };
    Some((class_name, extends))
}
