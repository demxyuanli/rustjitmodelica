// ModAI IDE: Tauri commands for JIT validation, simulation, and AI (rustmodlica).

mod ai;
mod db;
mod iterate;

use std::fs;
use std::path::Path;

use rustmodlica::{Compiler, CompilerOptions, CompileOutput, run_simulation_collect, SimulationResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JitValidateOptions {
    pub t_end: Option<f64>,
    pub dt: Option<f64>,
    pub atol: Option<f64>,
    pub rtol: Option<f64>,
    pub solver: Option<String>,
    pub output_interval: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WarningItem {
    pub path: String,
    pub line: usize,
    pub column: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct JitValidateResult {
    pub success: bool,
    pub warnings: Vec<WarningItem>,
    pub errors: Vec<String>,
    pub state_vars: Vec<String>,
    pub output_vars: Vec<String>,
}

fn options_to_compiler_options(opts: Option<JitValidateOptions>) -> CompilerOptions {
    let mut c = CompilerOptions::default();
    if let Some(o) = opts {
        if let Some(v) = o.t_end {
            c.t_end = v;
        }
        if let Some(v) = o.dt {
            c.dt = v;
        }
        if let Some(v) = o.atol {
            c.atol = v;
        }
        if let Some(v) = o.rtol {
            c.rtol = v;
        }
        if let Some(v) = o.solver {
            c.solver = v;
        }
        if let Some(v) = o.output_interval {
            c.output_interval = v;
        }
    }
    c
}

#[tauri::command]
fn jit_validate(
    code: String,
    model_name: String,
    options: Option<JitValidateOptions>,
) -> Result<JitValidateResult, String> {
    let mut compiler = Compiler::new();
    compiler.options = options_to_compiler_options(options);
    compiler.loader.add_path(std::path::PathBuf::from("."));
    compiler.loader.add_path(std::path::PathBuf::from("StandardLib"));
    compiler.loader.add_path(std::path::PathBuf::from("TestLib"));

    let out = match compiler.compile_from_source(&model_name, &code) {
        Ok(o) => o,
        Err(e) => {
            let err_msg = e.to_string();
            let warnings = compiler
                .take_warnings()
                .into_iter()
                .map(|w| WarningItem {
                    path: w.path,
                    line: w.line,
                    column: w.column,
                    message: w.message,
                })
                .collect();
            return Ok(JitValidateResult {
                success: false,
                warnings,
                errors: vec![err_msg],
                state_vars: vec![],
                output_vars: vec![],
            });
        }
    };

    let warnings = compiler
        .take_warnings()
        .into_iter()
        .map(|w| WarningItem {
            path: w.path,
            line: w.line,
            column: w.column,
            message: w.message,
        })
        .collect();

    match out {
        CompileOutput::Simulation(artifacts) => Ok(JitValidateResult {
            success: true,
            warnings,
            errors: vec![],
            state_vars: artifacts.state_vars.clone(),
            output_vars: artifacts.output_vars.clone(),
        }),
        CompileOutput::FunctionRun(_) => Ok(JitValidateResult {
            success: true,
            warnings,
            errors: vec![],
            state_vars: vec![],
            output_vars: vec![],
        }),
    }
}

#[tauri::command]
fn run_simulation_cmd(
    code: String,
    model_name: String,
    options: Option<JitValidateOptions>,
) -> Result<SimulationResult, String> {
    let mut compiler = Compiler::new();
    compiler.options = options_to_compiler_options(options);
    compiler.loader.add_path(std::path::PathBuf::from("."));
    compiler.loader.add_path(std::path::PathBuf::from("StandardLib"));
    compiler.loader.add_path(std::path::PathBuf::from("TestLib"));

    let out = compiler
        .compile_from_source(&model_name, &code)
        .map_err(|e| e.to_string())?;

    let artifacts = match out {
        CompileOutput::Simulation(a) => a,
        CompileOutput::FunctionRun(_) => return Err("Model is a function, not a simulation.".to_string()),
    };

    run_simulation_collect(
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
}

#[tauri::command]
fn get_api_key() -> Result<String, String> {
    ai::get_api_key()
}

#[tauri::command]
fn set_api_key(api_key: String) -> Result<(), String> {
    ai::set_api_key(&api_key)
}

#[tauri::command]
async fn ai_code_gen(prompt: String) -> Result<String, String> {
    let api_key = ai::get_api_key().map_err(|e| e.to_string())?;
    ai::deepseek_call(prompt, api_key).await
}

#[tauri::command]
fn self_iterate(diff: Option<String>) -> Result<iterate::IterationResult, String> {
    let repo_root = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .parent()
        .ok_or("no parent dir")?
        .to_path_buf();
    iterate::self_iterate_impl(&repo_root, diff.as_deref())
}

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn open_project_dir() -> Option<String> {
    rfd::FileDialog::new().pick_folder().and_then(|p| p.to_str().map(String::from))
}

fn list_mo_files_impl(dir: &Path) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    if !dir.is_dir() {
        return Ok(out);
    }
    for e in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let p = e.path();
        if p.is_dir() {
            let sub = list_mo_files_impl(&p)?;
            for s in sub {
                out.push(format!("{}/{}", p.file_name().and_then(|n| n.to_str()).unwrap_or(""), s));
            }
        } else if p.extension().map_or(false, |e| e == "mo") {
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            out.push(name);
        }
    }
    Ok(out)
}

#[tauri::command]
fn list_mo_files(project_dir: String) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    let dir = Path::new(&project_dir);
    if !dir.is_dir() {
        return Ok(out);
    }
    for e in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let p = e.path();
        if p.is_dir() {
            let sub = list_mo_files_impl(&p)?;
            let prefix = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            for s in sub {
                out.push(format!("{}/{}", prefix, s));
            }
        } else if p.extension().map_or(false, |e| e == "mo") {
            out.push(p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string());
        }
    }
    out.sort();
    Ok(out)
}

#[tauri::command]
fn read_project_file(project_dir: String, relative_path: String) -> Result<String, String> {
    let path = Path::new(&project_dir).join(&relative_path);
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    let dir_canonical = Path::new(&project_dir).canonicalize().map_err(|e| e.to_string())?;
    if !canonical.starts_with(&dir_canonical) {
        return Err("Path is outside project directory".to_string());
    }
    fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn ai_generate_compiler_patch(target: String) -> Result<String, String> {
    ai::generate_compiler_patch(target).await
}

#[tauri::command]
fn list_iteration_history(limit: i32) -> Result<Vec<db::IterationRecord>, String> {
    db::list_iteration_history(limit)
}

#[tauri::command]
fn save_iteration(
    target: String,
    diff: Option<String>,
    success: bool,
    message: String,
) -> Result<i64, String> {
    db::save_iteration(
        &target,
        diff.as_deref(),
        success,
        &message,
    )
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            jit_validate,
            run_simulation_cmd,
            get_api_key,
            set_api_key,
            ai_code_gen,
            self_iterate,
            open_project_dir,
            list_mo_files,
            read_project_file,
            ai_generate_compiler_patch,
            list_iteration_history,
            save_iteration,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
