// ModAI IDE: Tauri commands for JIT validation, simulation, and AI (rustmodlica).

use tauri::Emitter;

mod ai;
mod chunker;
mod db;
mod diagram;
mod file_watcher;
mod git;
mod index_db;
mod index_manager;
mod iterate;
mod source_manager;
mod test_manager;
mod traceability;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

const COMPILE_STACK_SIZE: usize = 32 * 1024 * 1024;

fn run_with_large_stack<R, F>(f: F) -> R
where
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    thread::Builder::new()
        .stack_size(COMPILE_STACK_SIZE)
        .spawn(f)
        .expect("compile thread spawn")
        .join()
        .expect("compile thread join")
}

use rustmodlica::ast::ClassItem;
use rustmodlica::parser;
use rustmodlica::{Compiler, CompilerOptions, CompileOutput, run_simulation_collect, SimulationResult};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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

fn add_compiler_library_paths(compiler: &mut Compiler, project_dir: Option<&str>) {
    if let Some(dir) = project_dir {
        let dir = dir.trim();
        if !dir.is_empty() {
            let path = PathBuf::from(dir);
            let path = if path.exists() {
                path.canonicalize().unwrap_or(path)
            } else {
                path
            };
            compiler.loader.add_path(path.clone());
            if path.is_dir() {
                if let Ok(entries) = fs::read_dir(&path) {
                    for e in entries.flatten() {
                        let p = e.path();
                        if p.is_dir() {
                            let canonical = p.canonicalize().unwrap_or(p);
                            compiler.loader.add_path(canonical);
                        }
                    }
                }
            }
        }
    }
    compiler.loader.add_path(PathBuf::from("."));
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir.parent().unwrap_or(&manifest_dir);
    compiler.loader.add_path(repo_root.to_path_buf());
    compiler.loader.add_path(repo_root.join("StandardLib"));
    compiler.loader.add_path(repo_root.join("TestLib"));
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JitValidateRequest {
    code: String,
    model_name: String,
    options: Option<JitValidateOptions>,
    project_dir: Option<String>,
}

#[tauri::command]
fn jit_validate(request: JitValidateRequest) -> Result<JitValidateResult, String> {
    run_with_large_stack(move || {
        let mut compiler = Compiler::new();
        compiler.options = options_to_compiler_options(request.options);
        add_compiler_library_paths(&mut compiler, request.project_dir.as_deref());

        let out = match compiler.compile_from_source(&request.model_name, &request.code) {
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
    })
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RunSimulationRequest {
    code: String,
    model_name: String,
    options: Option<JitValidateOptions>,
    project_dir: Option<String>,
}

#[tauri::command]
fn run_simulation_cmd(request: RunSimulationRequest) -> Result<SimulationResult, String> {
    let artifacts = run_with_large_stack(move || {
        let mut compiler = Compiler::new();
        compiler.options = options_to_compiler_options(request.options);
        add_compiler_library_paths(&mut compiler, request.project_dir.as_deref());

        let out = compiler
            .compile_from_source(&request.model_name, &request.code)
            .map_err(|e| e.to_string())?;

        match out {
            CompileOutput::Simulation(a) => Ok(a),
            CompileOutput::FunctionRun(_) => Err("Model is a function, not a simulation.".to_string()),
        }
    })?;

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

fn repo_root() -> Result<PathBuf, String> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .ok_or_else(|| "cannot determine repository root".to_string())
}

#[tauri::command]
fn self_iterate(diff: Option<String>) -> Result<iterate::IterationResult, String> {
    let root = repo_root()?;
    iterate::self_iterate_impl(&root, diff.as_deref())
}

#[tauri::command]
fn apply_patch_to_workspace(diff: String) -> Result<(), String> {
    let work_dir = repo_root()?;
    let mut child = Command::new("patch")
        .args(["-p1"])
        .current_dir(&work_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("patch command failed (install patch?): {}", e))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(diff.as_bytes()).map_err(|e| e.to_string())?;
    }
    let out = child.wait_with_output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(format!("Patch apply failed: {}", stderr))
    }
}

#[tauri::command]
fn commit_patch(message: String) -> Result<(), String> {
    let work_dir = repo_root()?;
    let add = Command::new("git")
        .args(["add", "-A"])
        .current_dir(&work_dir)
        .output()
        .map_err(|e| e.to_string())?;
    if !add.status.success() {
        let stderr = String::from_utf8_lossy(&add.stderr);
        return Err(format!("git add failed: {}", stderr));
    }
    let commit = Command::new("git")
        .args(["commit", "-m", &message])
        .current_dir(&work_dir)
        .output()
        .map_err(|e| e.to_string())?;
    if !commit.status.success() {
        let stderr = String::from_utf8_lossy(&commit.stderr);
        return Err(format!("git commit failed: {}", stderr));
    }
    Ok(())
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

#[derive(serde::Serialize)]
struct SearchMatch {
    file: String,
    line: u32,
    column: u32,
    line_content: String,
}

fn walk_dir_recursive(dir: &Path, base: &Path, results: &mut Vec<PathBuf>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with('.') || name == "node_modules" || name == "target" || name == "build" {
                continue;
            }
        }
        if path.is_dir() {
            walk_dir_recursive(&path, base, results);
        } else if path.is_file() {
            results.push(path);
        }
    }
}

#[tauri::command]
fn search_in_project(
    project_dir: String,
    query: String,
    case_sensitive: bool,
    file_pattern: Option<String>,
    max_results: Option<usize>,
) -> Result<Vec<SearchMatch>, String> {
    let base = Path::new(&project_dir);
    if !base.is_dir() {
        return Err("Project directory does not exist".to_string());
    }
    if query.is_empty() {
        return Ok(vec![]);
    }

    let limit = max_results.unwrap_or(500);
    let query_lower = if case_sensitive { query.clone() } else { query.to_lowercase() };

    let ext_filter: Option<String> = file_pattern.and_then(|p| {
        let p = p.trim();
        if p.starts_with("*.") {
            Some(p[1..].to_string())
        } else if p.starts_with('.') {
            Some(p.to_string())
        } else {
            None
        }
    });

    let mut files = Vec::new();
    walk_dir_recursive(base, base, &mut files);

    let mut matches = Vec::new();

    for file_path in &files {
        if matches.len() >= limit {
            break;
        }

        if let Some(ref ext) = ext_filter {
            let file_ext = file_path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| format!(".{}", e))
                .unwrap_or_default();
            if file_ext != *ext {
                continue;
            }
        }

        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let relative = file_path
            .strip_prefix(base)
            .unwrap_or(file_path)
            .to_string_lossy()
            .replace('\\', "/");

        for (line_idx, line) in content.lines().enumerate() {
            if matches.len() >= limit {
                break;
            }

            let haystack = if case_sensitive { line.to_string() } else { line.to_lowercase() };
            let mut start = 0;
            while let Some(pos) = haystack[start..].find(&query_lower) {
                let col = start + pos;
                matches.push(SearchMatch {
                    file: relative.clone(),
                    line: (line_idx + 1) as u32,
                    column: (col + 1) as u32,
                    line_content: line.to_string(),
                });
                start = col + query_lower.len();
                if matches.len() >= limit {
                    break;
                }
            }
        }
    }

    Ok(matches)
}

#[tauri::command]
fn get_diagram_data(project_dir: String, relative_path: String) -> Result<diagram::DiagramModel, String> {
    diagram::get_diagram_data(&project_dir, &relative_path)
}

#[tauri::command]
fn get_diagram_data_from_source(
    source: String,
    project_dir: Option<String>,
    relative_path: Option<String>,
) -> Result<diagram::DiagramModel, String> {
    diagram::get_diagram_data_from_source(
        &source,
        project_dir.as_deref(),
        relative_path.as_deref(),
    )
}

#[tauri::command]
fn apply_diagram_edits(
    source: String,
    components: Vec<diagram::ComponentInstance>,
    connections: Vec<diagram::Connection>,
    layout: Option<std::collections::HashMap<String, diagram::LayoutPoint>>,
    project_dir: Option<String>,
    relative_path: Option<String>,
) -> Result<ApplyDiagramEditsResult, String> {
    let new_source = diagram::apply_diagram_edits(
        &source,
        &components,
        &connections,
        layout.as_ref(),
        project_dir.as_deref(),
        relative_path.as_deref(),
    )?;
    Ok(ApplyDiagramEditsResult { new_source: new_source })
}

#[derive(Debug, serde::Serialize)]
pub struct ApplyDiagramEditsResult {
    #[serde(rename = "newSource")]
    pub new_source: String,
}

#[tauri::command]
fn write_project_file(project_dir: String, relative_path: String, content: String) -> Result<(), String> {
    let project_canonical = Path::new(&project_dir).canonicalize().map_err(|e| e.to_string())?;
    let full = project_canonical.join(&relative_path);
    if let Some(parent) = full.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let parent_canonical = parent.canonicalize().map_err(|e| e.to_string())?;
        if !parent_canonical.starts_with(&project_canonical) {
            return Err("Path is outside project directory".to_string());
        }
    }
    fs::write(&full, content).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Serialize)]
pub struct MoTreeEntry {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<MoTreeEntry>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub class_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extends: Option<Vec<String>>,
}

fn parse_mo_deps(content: &str) -> Option<(String, Vec<String>)> {
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstantiableClass {
    pub name: String,
    pub path: Option<String>,
}

fn list_instantiable_classes_impl(
    dir: &Path,
    project_dir: &Path,
    prefix: &str,
    out: &mut Vec<InstantiableClass>,
) -> Result<(), String> {
    if !dir.is_dir() {
        return Ok(());
    }
    for e in fs::read_dir(dir).map_err(|e| e.to_string())? {
        let e = e.map_err(|e| e.to_string())?;
        let p = e.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if p.is_dir() {
            list_instantiable_classes_impl(&p, project_dir, &format!("{}{}/", prefix, name), out)?;
        } else if p.extension().map_or(false, |e| e == "mo") {
            let rel = format!("{}{}", prefix, name);
            let full = project_dir.join(&rel);
            let content = fs::read_to_string(&full).map_err(|e| e.to_string())?;
            if let Ok(item) = parser::parse(&content) {
                if let ClassItem::Model(m) = item {
                    if !m.is_connector && !m.is_function {
                        out.push(InstantiableClass {
                            name: m.name,
                            path: Some(rel),
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

#[tauri::command]
fn list_instantiable_classes(project_dir: String) -> Result<Vec<InstantiableClass>, String> {
    let dir = Path::new(&project_dir);
    let mut out = Vec::new();
    if dir.is_dir() {
        list_instantiable_classes_impl(dir, dir, "", &mut out)?;
    }
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

fn list_mo_tree_impl(dir: &Path, project_dir: &Path, prefix: &str) -> Result<Vec<MoTreeEntry>, String> {
    let mut entries = Vec::new();
    if !dir.is_dir() {
        return Ok(entries);
    }
    let mut read_dir: Vec<_> = fs::read_dir(dir).map_err(|e| e.to_string())?.collect();
    read_dir.sort_by(|a, b| {
        let a = a.as_ref().map(|e| e.path()).unwrap_or_default();
        let b = b.as_ref().map(|e| e.path()).unwrap_or_default();
        let a_is_dir = a.is_dir();
        let b_is_dir = b.is_dir();
        match (a_is_dir, b_is_dir) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => a.file_name().cmp(&b.file_name()),
        }
    });
    for e in read_dir {
        let e = e.map_err(|e| e.to_string())?;
        let p = e.path();
        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
        if p.is_dir() {
            let sub = list_mo_tree_impl(&p, project_dir, &format!("{}{}/", prefix, name))?;
            if !sub.is_empty() {
                entries.push(MoTreeEntry {
                    name,
                    path: None,
                    children: Some(sub),
                    class_name: None,
                    extends: None,
                });
            }
        } else if p.extension().map_or(false, |e| e == "mo") {
            let rel = format!("{}{}", prefix, name);
            let full = project_dir.join(&rel);
            let (class_name, extends) = fs::read_to_string(&full)
                .ok()
                .and_then(|c| parse_mo_deps(&c))
                .unwrap_or((String::new(), Vec::new()));
            let (class_name, extends) = if class_name.is_empty() {
                (None, None)
            } else {
                (Some(class_name), if extends.is_empty() { None } else { Some(extends) })
            };
            entries.push(MoTreeEntry {
                name: name.clone(),
                path: Some(rel),
                children: None,
                class_name,
                extends,
            });
        }
    }
    Ok(entries)
}

#[tauri::command]
fn list_mo_tree(project_dir: String) -> Result<MoTreeEntry, String> {
    let dir = Path::new(&project_dir);
    if !dir.is_dir() {
        return Ok(MoTreeEntry {
            name: String::new(),
            path: None,
            children: Some(Vec::new()),
            class_name: None,
            extends: None,
        });
    }
    let children = list_mo_tree_impl(dir, dir, "")?;
    Ok(MoTreeEntry {
        name: String::new(),
        path: None,
        children: Some(children),
        class_name: None,
        extends: None,
    })
}

#[tauri::command]
async fn ai_generate_compiler_patch(target: String) -> Result<String, String> {
    ai::generate_compiler_patch(target).await
}

#[tauri::command]
async fn ai_generate_compiler_patch_with_context(
    target: String,
    context_files: Vec<String>,
    test_cases: Vec<String>,
) -> Result<String, String> {
    ai::generate_compiler_patch_with_context(target, context_files, test_cases).await
}

// --- Traceability commands ---

#[tauri::command]
fn load_traceability_config() -> Result<traceability::TraceabilityConfig, String> {
    let root = repo_root()?;
    traceability::load_config(&root)
}

#[tauri::command]
fn save_traceability_config(config: traceability::TraceabilityConfig) -> Result<(), String> {
    let root = repo_root()?;
    traceability::save_config(&root, &config)
}

#[tauri::command]
fn get_traceability_matrix() -> Result<traceability::TraceabilityMatrix, String> {
    let root = repo_root()?;
    traceability::get_traceability_matrix(&root)
}

#[tauri::command]
fn traceability_impact_analysis(changed_files: Vec<String>) -> Result<traceability::ImpactAnalysisResult, String> {
    let root = repo_root()?;
    traceability::impact_analysis(&root, &changed_files)
}

#[tauri::command]
fn traceability_coverage_analysis() -> Result<traceability::CoverageAnalysisResult, String> {
    let root = repo_root()?;
    traceability::coverage_analysis(&root)
}

#[tauri::command]
fn update_traceability_link(
    link_type: String,
    source: String,
    target: String,
    add: bool,
) -> Result<(), String> {
    let root = repo_root()?;
    traceability::update_traceability_link(&root, &link_type, &source, &target, add)
}

#[tauri::command]
fn traceability_sync_check() -> Result<traceability::SyncCheckResult, String> {
    let root = repo_root()?;
    traceability::sync_check(&root)
}

#[tauri::command]
fn traceability_validate() -> Result<traceability::ValidationResult, String> {
    let root = repo_root()?;
    traceability::validate_config(&root)
}

#[tauri::command]
fn traceability_apply_sync(request: traceability::ApplySyncRequest) -> Result<(), String> {
    let root = repo_root()?;
    traceability::apply_sync(&root, &request)
}

#[tauri::command]
fn traceability_git_impact() -> Result<traceability::GitImpactResult, String> {
    let root = repo_root()?;
    traceability::git_changed_impact(&root)
}

// --- Source manager commands ---

#[tauri::command]
fn list_compiler_source_tree() -> Result<source_manager::SourceTreeEntry, String> {
    let root = repo_root()?;
    source_manager::list_source_tree(&root)
}

#[tauri::command]
fn read_compiler_file(path: String) -> Result<String, String> {
    let root = repo_root()?;
    source_manager::read_file(&root, &path)
}

#[tauri::command]
fn write_compiler_file(path: String, content: String) -> Result<(), String> {
    let root = repo_root()?;
    source_manager::write_file(&root, &path, &content)
}

#[tauri::command]
fn compiler_file_git_log(path: String, limit: Option<u32>) -> Result<Vec<git::GitLogEntry>, String> {
    let root = repo_root()?;
    git::git_log_impl(&root, Some(&path), limit.unwrap_or(20))
}

#[tauri::command]
fn compiler_file_git_diff(path: String) -> Result<String, String> {
    let root = repo_root()?;
    git::git_diff_file_impl(&root, &path, None)
}

#[tauri::command]
fn create_iteration_branch(name: String) -> Result<String, String> {
    let root = repo_root()?;
    source_manager::create_iteration_branch(&root, &name)
}

#[tauri::command]
fn list_iteration_branches() -> Result<Vec<String>, String> {
    let root = repo_root()?;
    source_manager::list_iteration_branches(&root)
}

#[tauri::command]
fn switch_iteration_branch(name: String) -> Result<(), String> {
    let root = repo_root()?;
    source_manager::switch_branch(&root, &name)
}

#[tauri::command]
fn merge_iteration_branch(name: String) -> Result<(), String> {
    let root = repo_root()?;
    source_manager::merge_branch(&root, &name)
}

// --- Test manager commands ---

#[tauri::command]
fn list_test_library() -> Result<Vec<test_manager::TestCaseInfo>, String> {
    let root = repo_root()?;
    test_manager::list_test_library(&root)
}

#[tauri::command]
fn read_test_file(name: String) -> Result<String, String> {
    let root = repo_root()?;
    test_manager::read_test_file(&root, &name)
}

#[tauri::command]
fn write_test_file(name: String, content: String) -> Result<(), String> {
    let root = repo_root()?;
    test_manager::write_test_file(&root, &name, &content)
}

#[tauri::command]
fn delete_test_file(name: String) -> Result<(), String> {
    let root = repo_root()?;
    test_manager::delete_test_file(&root, &name)
}

#[tauri::command]
fn run_single_test(name: String) -> Result<test_manager::TestRunResult, String> {
    let root = repo_root()?;
    test_manager::run_single_test(&root, &name)
}

#[tauri::command]
fn run_test_suite(names: Vec<String>, suite: Option<String>) -> Result<test_manager::TestSuiteResult, String> {
    let root = repo_root()?;
    test_manager::run_test_suite(&root, &names, suite.as_deref())
}

#[tauri::command]
fn run_full_regression() -> Result<test_manager::TestSuiteResult, String> {
    let root = repo_root()?;
    test_manager::run_full_regression(&root)
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

fn project_dir_canonical(project_dir: &str) -> Result<PathBuf, String> {
    let path = Path::new(project_dir).canonicalize().map_err(|e| e.to_string())?;
    Ok(path)
}

#[tauri::command]
fn git_is_repo(project_dir: String) -> bool {
    let Ok(dir) = project_dir_canonical(&project_dir) else {
        return false;
    };
    git::git_is_repo_impl(&dir)
}

#[tauri::command]
fn git_init(project_dir: String) -> Result<(), String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_init_impl(&dir)
}

#[tauri::command]
fn git_status(project_dir: String) -> Result<git::GitStatus, String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_status_impl(&dir)
}

#[tauri::command]
fn git_diff_file(
    project_dir: String,
    relative_path: String,
    base: Option<String>,
) -> Result<String, String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_diff_file_impl(&dir, &relative_path, base.as_deref())
}

#[tauri::command]
fn git_diff_file_staged(project_dir: String, relative_path: String) -> Result<String, String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_diff_file_staged_impl(&dir, &relative_path)
}

#[tauri::command]
fn git_show_file(
    project_dir: String,
    revision: String,
    relative_path: String,
) -> Result<String, String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_show_file_impl(&dir, &revision, &relative_path)
}

#[tauri::command]
fn git_log(
    project_dir: String,
    relative_path: Option<String>,
    limit: Option<u32>,
) -> Result<Vec<git::GitLogEntry>, String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_log_impl(&dir, relative_path.as_deref(), limit.unwrap_or(50))
}

#[tauri::command]
fn git_stage(project_dir: String, paths: Vec<String>) -> Result<(), String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_stage_impl(&dir, &paths)
}

#[tauri::command]
fn git_unstage(project_dir: String, paths: Vec<String>) -> Result<(), String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_unstage_impl(&dir, &paths)
}

#[tauri::command]
fn git_commit(project_dir: String, message: String) -> Result<(), String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_commit_impl(&dir, &message)
}

#[tauri::command]
fn git_commit_files(project_dir: String, hash: String) -> Result<Vec<git::GitCommitFile>, String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_commit_files_impl(&dir, &hash)
}

#[tauri::command]
fn git_log_graph(project_dir: String, limit: Option<u32>) -> Result<Vec<git::GitLogGraphEntry>, String> {
    let dir = project_dir_canonical(&project_dir)?;
    git::git_log_graph_impl(&dir, limit.unwrap_or(50))
}

// --- Code index commands ---

#[tauri::command]
fn index_build(project_dir: String) -> Result<index_db::IndexStats, String> {
    let idx = index_manager::CodeIndex::new(&project_dir);
    idx.build_index()
}

#[tauri::command]
fn index_update_file(project_dir: String, file_path: String) -> Result<(), String> {
    let idx = index_manager::CodeIndex::new(&project_dir);
    idx.update_file(&file_path)
}

#[tauri::command]
fn index_search_symbols(
    project_dir: String,
    query: String,
    kind: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<index_db::SymbolInfo>, String> {
    let idx = index_manager::CodeIndex::new(&project_dir);
    idx.search_symbols(&query, kind.as_deref(), limit.unwrap_or(100))
}

#[tauri::command]
fn index_file_symbols(
    project_dir: String,
    file_path: String,
) -> Result<Vec<index_db::SymbolInfo>, String> {
    let idx = index_manager::CodeIndex::new(&project_dir);
    idx.file_symbols(&file_path)
}

#[tauri::command]
fn index_find_references(
    project_dir: String,
    symbol_name: String,
) -> Result<Vec<index_db::DependencyInfo>, String> {
    let idx = index_manager::CodeIndex::new(&project_dir);
    idx.find_references(&symbol_name)
}

#[tauri::command]
fn index_get_context(
    project_dir: String,
    query: String,
    max_chunks: Option<i64>,
) -> Result<Vec<index_db::ChunkInfo>, String> {
    let idx = index_manager::CodeIndex::new(&project_dir);
    idx.get_context(&query, max_chunks.unwrap_or(10))
}

#[tauri::command]
fn index_get_dependencies(
    project_dir: String,
    file_path: String,
) -> Result<Vec<index_db::DependencyInfo>, String> {
    let idx = index_manager::CodeIndex::new(&project_dir);
    idx.get_dependencies(&file_path)
}

#[tauri::command]
fn index_stats(project_dir: String) -> Result<index_db::IndexStats, String> {
    let idx = index_manager::CodeIndex::new(&project_dir);
    idx.stats()
}

#[tauri::command]
fn index_start_watcher(app_handle: tauri::AppHandle, project_dir: String) -> Result<(), String> {
    file_watcher::start_watching(app_handle, project_dir)
}

#[tauri::command]
fn index_stop_watcher() -> Result<(), String> {
    file_watcher::stop_watching()
}

#[tauri::command]
fn index_refresh(app_handle: tauri::AppHandle, project_dir: String) -> Result<index_db::IndexStats, String> {
    let idx = index_manager::CodeIndex::new(&project_dir);
    idx.build_index_with_progress(|done, total| {
        let _ = app_handle.emit("index-progress", serde_json::json!({ "done": done, "total": total }));
    })
}

#[tauri::command]
fn index_rebuild(app_handle: tauri::AppHandle, project_dir: String) -> Result<index_db::IndexStats, String> {
    let idx = index_manager::CodeIndex::new(&project_dir);
    idx.rebuild_index_with_progress(|done, total| {
        let _ = app_handle.emit("index-progress", serde_json::json!({ "done": done, "total": total }));
    })
}

#[tauri::command]
fn index_refresh_repo(app_handle: tauri::AppHandle) -> Result<index_db::IndexStats, String> {
    let root = repo_root()?;
    let dir_str = root.to_string_lossy().to_string();
    let idx = index_manager::CodeIndex::new(&dir_str);
    idx.build_index_with_progress(|done, total| {
        let _ = app_handle.emit("index-progress", serde_json::json!({ "done": done, "total": total }));
    })
}

#[tauri::command]
fn index_rebuild_repo(app_handle: tauri::AppHandle) -> Result<index_db::IndexStats, String> {
    let root = repo_root()?;
    let dir_str = root.to_string_lossy().to_string();
    let idx = index_manager::CodeIndex::new(&dir_str);
    idx.rebuild_index_with_progress(|done, total| {
        let _ = app_handle.emit("index-progress", serde_json::json!({ "done": done, "total": total }));
    })
}

#[tauri::command]
fn index_repo_root() -> Result<String, String> {
    let root = repo_root()?;
    Ok(root.to_string_lossy().replace('\\', "/"))
}

#[tauri::command]
fn index_build_repo() -> Result<index_db::IndexStats, String> {
    let root = repo_root()?;
    let dir_str = root.to_string_lossy().to_string();
    let idx = index_manager::CodeIndex::new(&dir_str);
    idx.build_index()
}

#[tauri::command]
fn index_repo_stats() -> Result<index_db::IndexStats, String> {
    let root = repo_root()?;
    let dir_str = root.to_string_lossy().to_string();
    let idx = index_manager::CodeIndex::new(&dir_str);
    idx.stats()
}

#[tauri::command]
fn index_repo_file_symbols(file_path: String) -> Result<Vec<index_db::SymbolInfo>, String> {
    let root = repo_root()?;
    let dir_str = root.to_string_lossy().to_string();
    let idx = index_manager::CodeIndex::new(&dir_str);
    idx.file_symbols(&file_path)
}

#[tauri::command]
fn index_repo_search_symbols(
    query: String,
    kind: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<index_db::SymbolInfo>, String> {
    let root = repo_root()?;
    let dir_str = root.to_string_lossy().to_string();
    let idx = index_manager::CodeIndex::new(&dir_str);
    idx.search_symbols(&query, kind.as_deref(), limit.unwrap_or(100))
}

#[tauri::command]
fn index_repo_get_context(
    query: String,
    max_chunks: Option<i64>,
) -> Result<Vec<index_db::ChunkInfo>, String> {
    let root = repo_root()?;
    let dir_str = root.to_string_lossy().to_string();
    let idx = index_manager::CodeIndex::new(&dir_str);
    idx.get_context(&query, max_chunks.unwrap_or(10))
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
            apply_patch_to_workspace,
            commit_patch,
            open_project_dir,
            list_mo_files,
            list_mo_tree,
            read_project_file,
            write_project_file,
            search_in_project,
            get_diagram_data,
            get_diagram_data_from_source,
            apply_diagram_edits,
            list_instantiable_classes,
            ai_generate_compiler_patch,
            ai_generate_compiler_patch_with_context,
            list_iteration_history,
            save_iteration,
            git_is_repo,
            git_init,
            git_status,
            git_diff_file,
            git_diff_file_staged,
            git_show_file,
            git_log,
            git_stage,
            git_unstage,
            git_commit,
            git_commit_files,
            git_log_graph,
            load_traceability_config,
            save_traceability_config,
            get_traceability_matrix,
            traceability_impact_analysis,
            traceability_coverage_analysis,
            update_traceability_link,
            traceability_sync_check,
            traceability_validate,
            traceability_apply_sync,
            traceability_git_impact,
            list_compiler_source_tree,
            read_compiler_file,
            write_compiler_file,
            compiler_file_git_log,
            compiler_file_git_diff,
            create_iteration_branch,
            list_iteration_branches,
            switch_iteration_branch,
            merge_iteration_branch,
            list_test_library,
            read_test_file,
            write_test_file,
            delete_test_file,
            run_single_test,
            run_test_suite,
            run_full_regression,
            index_build,
            index_update_file,
            index_search_symbols,
            index_file_symbols,
            index_find_references,
            index_get_context,
            index_get_dependencies,
            index_stats,
            index_start_watcher,
            index_stop_watcher,
            index_refresh,
            index_rebuild,
            index_refresh_repo,
            index_rebuild_repo,
            index_repo_root,
            index_build_repo,
            index_repo_stats,
            index_repo_file_symbols,
            index_repo_search_symbols,
            index_repo_get_context,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
