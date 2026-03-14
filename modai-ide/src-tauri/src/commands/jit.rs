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
