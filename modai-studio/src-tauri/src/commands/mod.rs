use crate::catalog::builtin_catalog;
use crate::diagram::{
    CatalogEntry, DiagramDocument, OpenProjectResult, SimProfile, StudioProjectManifest,
    StudioReport, StudioSimOptions,
};
use crate::emit_mo::{emit_modelica, emit_modelica_with_dict};
use crate::fmu_import::{self, FmuImportResult};
use crate::project::{create_project, open_project, save_project};
use crate::sim_bridge::{self, validate_model, RestartMode};
use crate::wrap_mo::{self, WrapMoResult};
use rustmodlica::{SimControl, SimulationResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};

#[tauri::command]
pub fn studio_catalog_list(root_dir: Option<String>) -> Vec<CatalogEntry> {
    let mut cat = builtin_catalog();
    if let Some(root) = root_dir {
        let p = PathBuf::from(&root);
        cat.extend(fmu_import::list_imported_fmus(p.as_path()));
        cat.extend(wrap_mo::list_wrapped(p.as_path()));
        cat.extend(crate::user_catalog::list_user_catalog(p.as_path()));
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

/// Path to auto-open on GUI startup (`MODAI_STUDIO_OPEN` or `--project=`).
#[tauri::command]
pub fn studio_startup_project_path() -> Option<String> {
    crate::startup::resolve_startup_project().map(|p| p.display().to_string())
}

#[tauri::command]
pub fn studio_list_examples() -> Vec<crate::examples::ExampleInfo> {
    crate::examples::list_examples()
}

#[tauri::command]
pub fn studio_open_example(id: String) -> Result<OpenProjectResult, String> {
    crate::examples::open_example(&id)
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
    /// When true, validate runs Full compile including JIT codegen.
    #[serde(default)]
    pub full_jit: bool,
    #[serde(default)]
    pub param_dictionary: Vec<crate::diagram::ParamDictionaryEntry>,
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
            return Ok(emit_modelica_with_dict(
                diagram,
                &req.subsystems,
                Some(&sim),
                &req.param_dictionary,
                req.root_dir.as_deref().map(Path::new),
            ));
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
pub async fn studio_validate(req: ValidateRequest) -> StudioReport {
    match tokio::task::spawn_blocking(move || studio_validate_blocking(req)).await {
        Ok(report) => report,
        Err(e) => StudioReport {
            success: false,
            stage: "validate".into(),
            errors: vec![crate::diagram::StudioDiagnostic {
                message: format!("validate join error: {e}"),
                path: None,
                line: None,
                column: None,
            }],
            warnings: Vec::new(),
        },
    }
}

fn studio_validate_blocking(req: ValidateRequest) -> StudioReport {
    let libs = resolved_library_paths(req.root_dir.as_deref(), &req.library_paths);
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
                let mut report = validate_model(
                    &code,
                    &req.model_name,
                    &libs,
                    &req.options,
                    req.full_jit,
                );
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
        Ok(code) => validate_model(
            &code,
            &req.model_name,
            &libs,
            &req.options,
            req.full_jit,
        ),
        Err(r) => r,
    }
}

fn resolved_library_paths(root_dir: Option<&str>, paths: &[String]) -> Vec<String> {
    match root_dir {
        Some(r) => crate::user_catalog::ensure_project_library_paths(Path::new(r), paths),
        None => paths.to_vec(),
    }
}

include!("sim_cmds.rs");
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
    /// Write Coder-grade step API + CMake + interface.json (default true).
    #[serde(default = "default_true")]
    pub coder_package: bool,
    /// "cs" | "me" when packaging FMU (default cs).
    #[serde(default = "default_fmi_kind")]
    pub fmi_kind: String,
    #[serde(default)]
    pub model_identifier: Option<String>,
    #[serde(default)]
    pub guid: Option<String>,
    #[serde(default = "default_true")]
    pub emit_from_diagram: bool,
    #[serde(default)]
    pub param_dictionary: Vec<crate::diagram::ParamDictionaryEntry>,
}

fn default_fmi_kind() -> String {
    "cs".into()
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
        full_jit: false,
        param_dictionary: req.param_dictionary.clone(),
    };
    match resolve_code(&vreq) {
        Ok(code) => {
            let libs = resolved_library_paths(req.root_dir.as_deref(), &req.library_paths);
            let fmi_opts = rustmodlica::FmiExportOptions {
                model_identifier_override: req
                    .model_identifier
                    .as_ref()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
                guid_override: req
                    .guid
                    .as_ref()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty()),
            };
            sim_bridge::emit_c_or_fmu(
                &code,
                &req.model_name,
                &libs,
                &req.options,
                &req.out_dir,
                req.package_fmu,
                req.coder_package,
                &req.fmi_kind,
                fmi_opts,
            )
        }
        Err(report) => crate::sim_bridge::EmitArtifactsResult {
            success: false,
            out_dir: req.out_dir,
            files: Vec::new(),
            fmu_path: None,
            report: Some(report),
            package_kind: None,
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

#[tauri::command]
pub fn studio_pick_mo_file() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("Select Modelica model (.mo)")
        .add_filter("Modelica", &["mo"])
        .pick_file()
        .map(|p| p.display().to_string())
}

#[tauri::command]
pub fn studio_pick_data_file() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("Select data file")
        .add_filter("Data", &["csv", "txt", "dat"])
        .pick_file()
        .map(|p| p.display().to_string())
}

#[tauri::command]
pub fn studio_pick_json_file() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("Select IDE DiagramModel JSON")
        .add_filter("JSON", &["json"])
        .pick_file()
        .map(|p| p.display().to_string())
}

#[tauri::command]
pub fn studio_read_text_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| format!("read {path}: {e}"))
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WrapMoRequest {
    pub mo_path: String,
    pub project_root: Option<String>,
}

#[tauri::command]
pub fn studio_wrap_mo(req: WrapMoRequest) -> WrapMoResult {
    let root = req.project_root.as_ref().map(PathBuf::from);
    let res = wrap_mo::wrap_mo_file(PathBuf::from(&req.mo_path).as_path(), root.as_deref());
    // Ensure project can resolve the wrapped type on validate/run.
    if res.success {
        if let Some(root) = root.as_ref() {
            let wrapped = root.join("wrapped");
            if let Ok(mut opened) = open_project(root) {
                let path_s = wrapped.display().to_string();
                if !opened
                    .manifest
                    .library_paths
                    .iter()
                    .any(|p| p == &path_s || PathBuf::from(p) == wrapped)
                {
                    opened.manifest.library_paths.push(path_s);
                    let _ = save_project(
                        root,
                        &opened.manifest,
                        &opened.diagram,
                        &opened.subsystems,
                    );
                }
            }
        }
    }
    res
}

#[tauri::command]
pub fn studio_list_imported_fmus(root_dir: String) -> Vec<CatalogEntry> {
    fmu_import::list_imported_fmus(PathBuf::from(root_dir).as_path())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPackRequest {
    pub pack_dir: String,
    pub project_root: String,
}

#[tauri::command]
pub fn studio_import_catalog_pack(req: ImportPackRequest) -> crate::user_catalog::ImportPackResult {
    let root = PathBuf::from(&req.project_root);
    let res = crate::user_catalog::import_catalog_pack(PathBuf::from(&req.pack_dir).as_path(), &root);
    if res.success {
        if let Ok(mut opened) = open_project(&root) {
            let path_s = res.library_path.clone();
            if !opened
                .manifest
                .library_paths
                .iter()
                .any(|p| p == &path_s || PathBuf::from(p) == PathBuf::from(&path_s))
            {
                opened.manifest.library_paths.push(path_s);
                let _ = save_project(
                    &root,
                    &opened.manifest,
                    &opened.diagram,
                    &opened.subsystems,
                );
            }
        }
    }
    res
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportPackRequest {
    pub project_root: String,
    #[serde(default)]
    pub relative_assets: Vec<String>,
    pub out_dir: String,
    #[serde(default = "default_pack_name")]
    pub pack_name: String,
}

fn default_pack_name() -> String {
    "UserLib".into()
}

#[tauri::command]
pub fn studio_export_catalog_pack(req: ExportPackRequest) -> crate::user_catalog::ExportPackResult {
    crate::user_catalog::export_catalog_pack(
        PathBuf::from(&req.project_root).as_path(),
        &req.relative_assets,
        PathBuf::from(&req.out_dir).as_path(),
        &req.pack_name,
    )
}

#[tauri::command]
pub fn studio_pick_catalog_pack_directory() -> Option<String> {
    rfd::FileDialog::new()
        .set_title("Select catalog pack folder")
        .pick_folder()
        .map(|p| p.display().to_string())
}
