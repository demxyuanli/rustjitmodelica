use crate::catalog::builtin_catalog;
use crate::diagram::{
    CatalogEntry, DiagramDocument, OpenProjectResult, StudioProjectManifest, StudioReport,
    StudioSimOptions,
};
use crate::emit_mo::emit_modelica;
use crate::fmu_import::{self, FmuImportResult};
use crate::project::{create_project, open_project, save_project};
use crate::sim_bridge::{self, simulate_model, validate_model};
use rustmodlica::{SimControl, SimulationResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub fn studio_catalog_list(root_dir: Option<String>) -> Vec<CatalogEntry> {
    let mut cat = builtin_catalog();
    if let Some(root) = root_dir {
        cat.extend(fmu_import::list_imported_fmus(PathBuf::from(root).as_path()));
    }
    cat
}

#[tauri::command]
pub fn studio_project_create(root_dir: String, model_name: String) -> Result<OpenProjectResult, String> {
    create_project(PathBuf::from(root_dir).as_path(), &model_name)
}

#[tauri::command]
pub fn studio_project_open(root_dir: String) -> Result<OpenProjectResult, String> {
    open_project(PathBuf::from(root_dir).as_path())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRequest {
    pub root_dir: String,
    pub manifest: StudioProjectManifest,
    pub diagram: DiagramDocument,
    #[serde(default)]
    pub subsystems: HashMap<String, DiagramDocument>,
}

#[tauri::command]
pub fn studio_project_save(req: SaveRequest) -> Result<String, String> {
    save_project(
        PathBuf::from(&req.root_dir).as_path(),
        &req.manifest,
        &req.diagram,
        &req.subsystems,
    )
}

#[tauri::command]
pub fn studio_emit_preview(
    diagram: DiagramDocument,
    subsystems: Option<HashMap<String, DiagramDocument>>,
    stop_time: Option<f64>,
) -> String {
    let mut sim = crate::diagram::SimulationDefaults::default();
    if let Some(t) = stop_time {
        sim.stop_time = t;
    }
    let subs = subsystems.unwrap_or_default();
    emit_modelica(&diagram, &subs, Some(&sim))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateRequest {
    pub root_dir: Option<String>,
    pub model_name: String,
    pub code: Option<String>,
    pub diagram: Option<DiagramDocument>,
    #[serde(default)]
    pub subsystems: HashMap<String, DiagramDocument>,
    pub library_paths: Vec<String>,
    pub options: StudioSimOptions,
    #[serde(default)]
    pub emit_from_diagram: bool,
}

fn resolve_code(req: &ValidateRequest) -> Result<String, StudioReport> {
    if let Some(code) = &req.code {
        return Ok(code.clone());
    }
    if req.emit_from_diagram {
        if let Some(diagram) = &req.diagram {
            let sim = crate::diagram::SimulationDefaults {
                stop_time: req.options.stop_time,
                number_of_intervals: req.options.number_of_intervals,
                solver: req.options.solver.clone(),
                tolerance: req.options.tolerance,
                max_step: None,
                min_step: None,
                logged_variables: Vec::new(),
            };
            return Ok(emit_modelica(diagram, &req.subsystems, Some(&sim)));
        }
    }
    if let Some(root) = &req.root_dir {
        let path = PathBuf::from(root).join(format!("{}.mo", req.model_name));
        return std::fs::read_to_string(path).map_err(|e| StudioReport {
            success: false,
            stage: "io".into(),
            errors: vec![crate::diagram::StudioDiagnostic {
                message: e.to_string(),
                path: None,
                line: None,
                column: None,
            }],
            warnings: Vec::new(),
        });
    }
    Err(StudioReport {
        success: false,
        stage: "io".into(),
        errors: vec![crate::diagram::StudioDiagnostic {
            message: "no model source provided".into(),
            path: None,
            line: None,
            column: None,
        }],
        warnings: Vec::new(),
    })
}

#[tauri::command]
pub fn studio_validate(req: ValidateRequest) -> StudioReport {
    if let Some(diagram) = &req.diagram {
        if fmu_import::any_fmu(diagram, &req.subsystems) {
            let mut warnings = Vec::new();
            for n in diagram.nodes.iter().filter(|n| n.node_kind == "fmu") {
                let asset = n.ui.as_ref().and_then(|u| u.fmu_asset.clone());
                if asset.as_ref().map(|a| !PathBuf::from(a).exists()).unwrap_or(true) {
                    if let Some(root) = &req.root_dir {
                        let ok = asset
                            .as_ref()
                            .map(|a| PathBuf::from(root).join(a).exists())
                            .unwrap_or(false);
                        if !ok {
                            warnings.push(crate::diagram::StudioDiagnostic {
                                message: format!(
                                    "FMU asset missing for {} (will fail at Run)",
                                    n.instance_name
                                ),
                                path: asset,
                                line: None,
                                column: None,
                            });
                        }
                    }
                }
            }
            warnings.push(crate::diagram::StudioDiagnostic {
                message: "Diagram contains FMU nodes; Run uses FMI2 CS cosim host".into(),
                path: None,
                line: None,
                column: None,
            });
            // Still emit+validate Modelica stubs for structural connect check when possible.
            if let Ok(code) = resolve_code(&req) {
                let mut report =
                    validate_model(&code, &req.model_name, &req.library_paths, &req.options);
                report.warnings.extend(warnings);
                return report;
            }
            return StudioReport {
                success: warnings.iter().all(|w| !w.message.contains("missing")),
                stage: "fmu_validate".into(),
                errors: Vec::new(),
                warnings,
            };
        }
    }
    match resolve_code(&req) {
        Ok(code) => validate_model(&code, &req.model_name, &req.library_paths, &req.options),
        Err(r) => r,
    }
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SimulateRequest {
    pub root_dir: Option<String>,
    pub model_name: String,
    pub code: Option<String>,
    pub diagram: Option<DiagramDocument>,
    #[serde(default)]
    pub subsystems: HashMap<String, DiagramDocument>,
    pub library_paths: Vec<String>,
    pub options: StudioSimOptions,
    #[serde(default = "default_true")]
    pub emit_from_diagram: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SimulateResponse {
    pub success: bool,
    pub report: Option<StudioReport>,
    pub result: Option<SimulationResult>,
    pub emitted_code: Option<String>,
    pub run_id: Option<String>,
    pub cancelled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StudioProgressEvent {
    pub run_id: String,
    pub phase: String,
    pub sim_time: Option<f64>,
    pub t_end: Option<f64>,
    pub step: Option<u64>,
    pub fraction: Option<f64>,
    pub message: String,
}

fn new_run_id() -> String {
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("studio_run_{ms}")
}

#[tauri::command]
pub async fn studio_simulate(app: AppHandle, req: SimulateRequest) -> SimulateResponse {
    let run_id = new_run_id();

    // FMU cosimulation path
    if let Some(diagram) = req.diagram.clone() {
        if fmu_import::any_fmu(&diagram, &req.subsystems) {
            let _ = app.emit(
                "studio-sim-progress",
                StudioProgressEvent {
                    run_id: run_id.clone(),
                    phase: "fmu_cosim".into(),
                    sim_time: None,
                    t_end: Some(req.options.stop_time),
                    step: None,
                    fraction: Some(0.0),
                    message: "Starting FMI2 CS cosimulation".into(),
                },
            );
            let root = req.root_dir.clone().map(PathBuf::from);
            let options = req.options.clone();
            let code = resolve_code(&ValidateRequest {
                root_dir: req.root_dir.clone(),
                model_name: req.model_name.clone(),
                code: req.code.clone(),
                diagram: Some(diagram.clone()),
                subsystems: req.subsystems.clone(),
                library_paths: req.library_paths.clone(),
                options: req.options.clone(),
                emit_from_diagram: req.emit_from_diagram,
            })
            .ok();
            let outcome = tokio::task::spawn_blocking(move || {
                fmu_import::simulate_with_fmu(&diagram, root.as_deref(), &options)
            })
            .await;
            return match outcome {
                Ok(Ok(result)) => {
                    let _ = app.emit(
                        "studio-sim-progress",
                        StudioProgressEvent {
                            run_id: run_id.clone(),
                            phase: "done".into(),
                            sim_time: None,
                            t_end: None,
                            step: None,
                            fraction: Some(1.0),
                            message: "FMU cosimulation done".into(),
                        },
                    );
                    SimulateResponse {
                        success: true,
                        report: None,
                        result: Some(result),
                        emitted_code: code,
                        run_id: Some(run_id),
                        cancelled: false,
                    }
                }
                Ok(Err(report)) => {
                    let _ = app.emit(
                        "studio-sim-progress",
                        StudioProgressEvent {
                            run_id: run_id.clone(),
                            phase: "error".into(),
                            sim_time: None,
                            t_end: None,
                            step: None,
                            fraction: None,
                            message: report
                                .errors
                                .first()
                                .map(|e| e.message.clone())
                                .unwrap_or_else(|| "fmu cosim failed".into()),
                        },
                    );
                    SimulateResponse {
                        success: false,
                        report: Some(report),
                        result: None,
                        emitted_code: code,
                        run_id: Some(run_id),
                        cancelled: false,
                    }
                }
                Err(e) => SimulateResponse {
                    success: false,
                    report: Some(StudioReport {
                        success: false,
                        stage: "fmu_cosim".into(),
                        errors: vec![crate::diagram::StudioDiagnostic {
                            message: format!("join error: {e}"),
                            path: None,
                            line: None,
                            column: None,
                        }],
                        warnings: Vec::new(),
                    }),
                    result: None,
                    emitted_code: code,
                    run_id: Some(run_id),
                    cancelled: false,
                },
            };
        }
    }

    let vreq = ValidateRequest {
        root_dir: req.root_dir.clone(),
        model_name: req.model_name.clone(),
        code: req.code.clone(),
        diagram: req.diagram.clone(),
        subsystems: req.subsystems.clone(),
        library_paths: req.library_paths.clone(),
        options: req.options.clone(),
        emit_from_diagram: req.emit_from_diagram,
    };
    let code = match resolve_code(&vreq) {
        Ok(c) => c,
        Err(r) => {
            return SimulateResponse {
                success: false,
                report: Some(r),
                result: None,
                emitted_code: None,
                run_id: Some(run_id),
                cancelled: false,
            };
        }
    };

    let latest = Arc::new(Mutex::new(None::<(f64, f64, u64)>));
    let latest_cb = latest.clone();
    let control = SimControl::new().on_progress(move |p| {
        if let Ok(mut g) = latest_cb.lock() {
            *g = Some((p.time, p.t_end, p.step));
        }
    });
    sim_bridge::register(&run_id, control.cancel_flag());

    let _ = app.emit(
        "studio-sim-progress",
        StudioProgressEvent {
            run_id: run_id.clone(),
            phase: "started".into(),
            sim_time: Some(0.0),
            t_end: Some(req.options.stop_time),
            step: Some(0),
            fraction: Some(0.0),
            message: "Simulation started".into(),
        },
    );

    let model_name = req.model_name.clone();
    let library_paths = req.library_paths.clone();
    let options = req.options.clone();
    let code_for_sim = code.clone();
    let run_id_task = run_id.clone();
    let mut task = tokio::task::spawn_blocking(move || {
        let result = simulate_model(
            &code_for_sim,
            &model_name,
            &library_paths,
            &options,
            Some(control),
        );
        sim_bridge::unregister(&run_id_task);
        result
    });

    let started = Instant::now();
    let mut heartbeat = tokio::time::interval(Duration::from_millis(250));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let join = loop {
        tokio::select! {
            join_res = &mut task => break join_res,
            _ = heartbeat.tick() => {
                let snap = latest.lock().ok().and_then(|g| *g);
                if let Some((sim_t, t_end, step)) = snap {
                    let fraction = if t_end > 0.0 { (sim_t / t_end).clamp(0.0, 1.0) } else { 0.0 };
                    let _ = app.emit(
                        "studio-sim-progress",
                        StudioProgressEvent {
                            run_id: run_id.clone(),
                            phase: "running".into(),
                            sim_time: Some(sim_t),
                            t_end: Some(t_end),
                            step: Some(step),
                            fraction: Some(fraction),
                            message: format!(
                                "t={:.4}/{:.4} step={} wall={}s",
                                sim_t,
                                t_end,
                                step,
                                started.elapsed().as_secs()
                            ),
                        },
                    );
                }
            }
        }
    };

    let outcome = match join {
        Ok(r) => r,
        Err(e) => {
            sim_bridge::unregister(&run_id);
            return SimulateResponse {
                success: false,
                report: Some(StudioReport {
                    success: false,
                    stage: "simulate".into(),
                    errors: vec![crate::diagram::StudioDiagnostic {
                        message: format!("blocking task join error: {e}"),
                        path: None,
                        line: None,
                        column: None,
                    }],
                    warnings: Vec::new(),
                }),
                result: None,
                emitted_code: Some(code),
                run_id: Some(run_id),
                cancelled: false,
            };
        }
    };

    match outcome {
        Ok(result) => {
            let _ = app.emit(
                "studio-sim-progress",
                StudioProgressEvent {
                    run_id: run_id.clone(),
                    phase: "done".into(),
                    sim_time: None,
                    t_end: None,
                    step: None,
                    fraction: Some(1.0),
                    message: "Simulation done".into(),
                },
            );
            SimulateResponse {
                success: true,
                report: None,
                result: Some(result),
                emitted_code: Some(code),
                run_id: Some(run_id),
                cancelled: false,
            }
        }
        Err(report) => {
            let cancelled = report
                .errors
                .iter()
                .any(|e| rustmodlica::is_sim_cancelled_error(&e.message));
            let _ = app.emit(
                "studio-sim-progress",
                StudioProgressEvent {
                    run_id: run_id.clone(),
                    phase: if cancelled { "cancelled".into() } else { "error".into() },
                    sim_time: None,
                    t_end: None,
                    step: None,
                    fraction: None,
                    message: report
                        .errors
                        .first()
                        .map(|e| e.message.clone())
                        .unwrap_or_else(|| "failed".into()),
                },
            );
            SimulateResponse {
                success: false,
                report: Some(report),
                result: None,
                emitted_code: Some(code),
                run_id: Some(run_id),
                cancelled,
            }
        }
    }
}

#[tauri::command]
pub fn studio_cancel_simulation(run_id: String) -> bool {
    sim_bridge::request_cancel(&run_id)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmitRequest {
    pub root_dir: Option<String>,
    pub model_name: String,
    pub code: Option<String>,
    pub diagram: Option<DiagramDocument>,
    #[serde(default)]
    pub subsystems: HashMap<String, DiagramDocument>,
    pub library_paths: Vec<String>,
    pub options: StudioSimOptions,
    pub out_dir: String,
    #[serde(default)]
    pub package_fmu: bool,
    #[serde(default = "default_true")]
    pub emit_from_diagram: bool,
}

#[tauri::command]
pub fn studio_emit_artifacts(req: EmitRequest) -> crate::sim_bridge::EmitArtifactsResult {
    let vreq = ValidateRequest {
        root_dir: req.root_dir.clone(),
        model_name: req.model_name.clone(),
        code: req.code.clone(),
        diagram: req.diagram.clone(),
        subsystems: req.subsystems.clone(),
        library_paths: req.library_paths.clone(),
        options: req.options.clone(),
        emit_from_diagram: req.emit_from_diagram,
    };
    match resolve_code(&vreq) {
        Ok(code) => sim_bridge::emit_c_or_fmu(
            &code,
            &req.model_name,
            &req.library_paths,
            &req.options,
            &req.out_dir,
            req.package_fmu,
        ),
        Err(report) => crate::sim_bridge::EmitArtifactsResult {
            success: false,
            out_dir: req.out_dir,
            files: Vec::new(),
            fmu_path: None,
            report: Some(report),
        },
    }
}

#[tauri::command]
pub fn studio_pick_directory() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("Select Studio project folder")
        .pick_folder()
        .map(|p| p.display().to_string())
}

#[tauri::command]
pub fn studio_pick_export_directory() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("Select export output folder")
        .pick_folder()
        .map(|p| p.display().to_string())
}

#[tauri::command]
pub fn studio_pick_fmu_file() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("Select FMU file")
        .add_filter("FMU", &["fmu"])
        .pick_file()
        .map(|p| p.display().to_string())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFmuRequest {
    pub fmu_path: String,
    pub project_root: Option<String>,
}

#[tauri::command]
pub fn studio_import_fmu(req: ImportFmuRequest) -> FmuImportResult {
    let root = req.project_root.as_ref().map(PathBuf::from);
    fmu_import::import_fmu(
        PathBuf::from(&req.fmu_path).as_path(),
        root.as_deref(),
    )
}

#[tauri::command]
pub fn studio_list_imported_fmus(root_dir: String) -> Vec<CatalogEntry> {
    fmu_import::list_imported_fmus(PathBuf::from(root_dir).as_path())
}
