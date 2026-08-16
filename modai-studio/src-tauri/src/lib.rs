mod catalog;
mod commands;
mod diagram;
mod emit_mo;
mod examples;
mod fmu_import;
mod project;
mod sim_bridge;
pub mod startup;
mod user_catalog;
mod wrap_mo;

use commands::{
    studio_cancel_simulation, studio_catalog_list, studio_emit_artifacts, studio_emit_preview,
    studio_export_catalog_pack, studio_import_catalog_pack, studio_import_fmu,
    studio_list_examples, studio_list_imported_fmus, studio_open_example,
    studio_pause_simulation, studio_pick_catalog_pack_directory, studio_pick_directory,
    studio_pick_export_directory, studio_pick_fmu_file, studio_pick_mo_file, studio_pick_data_file,
    studio_read_text_file, studio_project_create, studio_project_open, studio_project_save,
    studio_resume_simulation, studio_sim_is_paused, studio_simulate, studio_startup_project_path,
    studio_step_simulation, studio_validate, studio_wrap_mo,
};
use diagram::StudioSimOptions;
use examples::{list_examples, resolve_example_dir};
use project::open_project;
use sim_bridge::{emit_c_or_fmu, simulate_model, validate_model};
use startup::{path_display, resolve_smoke_target, SmokeTarget};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn smoke_fail(root: &Path, error: &str) -> bool {
    eprintln!(
        "{{\"ok\":false,\"rootDir\":{},\"error\":{}}}",
        serde_json::to_string(&path_display(root)).unwrap_or_else(|_| "\"\"".into()),
        serde_json::to_string(error).unwrap_or_else(|_| "\"error\"".into())
    );
    false
}

fn smoke_ok(code_ok: bool) -> ExitCode {
    if code_ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Headless smoke: open project, require diagram, validate, then short simulate.
/// Returns true on success.
pub fn run_smoke_open(root: &Path) -> bool {
    let opened = match open_project(root) {
        Ok(o) => o,
        Err(err) => return smoke_fail(root, &err),
    };
    let nodes = opened.diagram.nodes.len();
    let edges = opened.diagram.edges.len();
    if nodes == 0 {
        return smoke_fail(root, "diagram has no nodes (not visualizable)");
    }
    if edges == 0 {
        return smoke_fail(root, "diagram has no edges (not visualizable)");
    }

    let sim = StudioSimOptions {
        stop_time: opened.manifest.simulation_defaults.stop_time.min(1.0).max(0.1),
        number_of_intervals: 50,
        solver: opened.manifest.simulation_defaults.solver.clone(),
        tolerance: opened.manifest.simulation_defaults.tolerance,
    };
    // Match GUI: project root must be on the library path so the model .mo resolves.
    let libs = crate::user_catalog::ensure_project_library_paths(
        Path::new(&opened.root_dir),
        &opened.manifest.library_paths,
    );
    let report = validate_model(
        &opened.model_source,
        &opened.manifest.model_name,
        &libs,
        &sim,
        false,
    );
    if !report.success {
        let msg = report
            .errors
            .first()
            .map(|e| e.message.as_str())
            .unwrap_or("validate failed");
        return smoke_fail(root, msg);
    }

    let samples = match simulate_model(
        &opened.model_source,
        &opened.manifest.model_name,
        &libs,
        &sim,
        None,
    ) {
        Ok(result) => {
            let mut signal_max = 0.0_f64;
            for (k, v) in &result.series {
                if k == "time" {
                    continue;
                }
                let m = v.iter().map(|x| x.abs()).fold(0.0_f64, f64::max);
                signal_max = signal_max.max(m);
            }
            if result.series.len() <= 1 {
                return smoke_fail(root, "simulate produced no signal series");
            }
            if signal_max <= 1e-9 {
                return smoke_fail(root, "simulate signal series all near zero");
            }
            result.time.len()
        }
        Err(err) => {
            let msg = err
                .errors
                .first()
                .map(|e| e.message.as_str())
                .unwrap_or("simulate failed");
            return smoke_fail(root, msg);
        }
    };
    if samples == 0 {
        return smoke_fail(root, "simulate returned no samples");
    }

    println!(
        "{{\"ok\":true,\"rootDir\":{},\"modelName\":{},\"nodes\":{},\"edges\":{},\"validated\":true,\"simulated\":true,\"samples\":{}}}",
        serde_json::to_string(&opened.root_dir).unwrap_or_else(|_| "\"\"".into()),
        serde_json::to_string(&opened.manifest.model_name).unwrap_or_else(|_| "\"\"".into()),
        nodes,
        edges,
        samples
    );
    true
}

/// Headless Export Suite smoke: open example, emit Coder + FMU CS into a temp dir.
pub fn run_smoke_emit(root: &Path) -> bool {
    let opened = match open_project(root) {
        Ok(o) => o,
        Err(err) => return smoke_fail(root, &err),
    };
    let out_dir = std::env::temp_dir().join(format!(
        "modai_studio_smoke_emit_{}",
        opened.manifest.model_name.replace(['/', '\\', ' '], "_")
    ));
    let _ = std::fs::remove_dir_all(&out_dir);
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        return smoke_fail(root, &format!("create out dir: {e}"));
    }
    let sim = StudioSimOptions {
        stop_time: opened.manifest.simulation_defaults.stop_time.min(1.0).max(0.1),
        number_of_intervals: 50,
        solver: opened.manifest.simulation_defaults.solver.clone(),
        tolerance: opened.manifest.simulation_defaults.tolerance,
    };
    let libs = crate::user_catalog::ensure_project_library_paths(
        Path::new(&opened.root_dir),
        &opened.manifest.library_paths,
    );
    let out_s = out_dir.display().to_string();
    let coder = emit_c_or_fmu(
        &opened.model_source,
        &opened.manifest.model_name,
        &libs,
        &sim,
        &out_s,
        false,
        true,
        "cs",
        rustmodlica::FmiExportOptions::default(),
    );
    if !coder.success {
        let msg = coder
            .report
            .as_ref()
            .and_then(|r| r.errors.first())
            .map(|e| e.message.as_str())
            .unwrap_or("coder emit failed");
        return smoke_fail(root, msg);
    }
    let fmu_dir = out_dir.join("fmu_cs");
    let _ = std::fs::create_dir_all(&fmu_dir);
    let fmu_s = fmu_dir.display().to_string();
    let fmu = emit_c_or_fmu(
        &opened.model_source,
        &opened.manifest.model_name,
        &libs,
        &sim,
        &fmu_s,
        true,
        true,
        "cs",
        rustmodlica::FmiExportOptions::default(),
    );
    if !fmu.success {
        let msg = fmu
            .report
            .as_ref()
            .and_then(|r| r.errors.first())
            .map(|e| e.message.as_str())
            .unwrap_or("fmu emit failed");
        return smoke_fail(root, msg);
    }
    if fmu.fmu_path.as_ref().map(|p| PathBuf::from(p).exists()) != Some(true) {
        return smoke_fail(root, "FMU file missing after emit");
    }
    let me_dir = out_dir.join("fmu_me");
    let _ = std::fs::create_dir_all(&me_dir);
    let me_s = me_dir.display().to_string();
    let me = emit_c_or_fmu(
        &opened.model_source,
        &opened.manifest.model_name,
        &libs,
        &sim,
        &me_s,
        true,
        false,
        "me",
        rustmodlica::FmiExportOptions::default(),
    );
    if !me.success {
        let msg = me
            .report
            .as_ref()
            .and_then(|r| r.errors.first())
            .map(|e| e.message.as_str())
            .unwrap_or("fmu-me emit failed");
        return smoke_fail(root, msg);
    }
    println!(
        "{{\"ok\":true,\"rootDir\":{},\"modelName\":{},\"coderFiles\":{},\"fmuPath\":{},\"mePackageKind\":{},\"outDir\":{}}}",
        serde_json::to_string(&opened.root_dir).unwrap_or_else(|_| "\"\"".into()),
        serde_json::to_string(&opened.manifest.model_name).unwrap_or_else(|_| "\"\"".into()),
        coder.files.len(),
        serde_json::to_string(fmu.fmu_path.as_deref().unwrap_or("")).unwrap_or_else(|_| "\"\"".into()),
        serde_json::to_string(me.package_kind.as_deref().unwrap_or("")).unwrap_or_else(|_| "\"\"".into()),
        serde_json::to_string(&out_s).unwrap_or_else(|_| "\"\"".into()),
    );
    true
}

fn run_smoke_all() -> bool {
    let examples = list_examples();
    if examples.is_empty() {
        eprintln!("{{\"ok\":false,\"error\":\"no bundled examples\"}}");
        return false;
    }
    let mut failed = 0usize;
    for ex in &examples {
        if !run_smoke_open(Path::new(&ex.root_dir)) {
            failed += 1;
        }
    }
    if failed == 0 {
        println!(
            "{{\"ok\":true,\"examples\":{},\"failed\":0}}",
            examples.len()
        );
        true
    } else {
        eprintln!(
            "{{\"ok\":false,\"examples\":{},\"failed\":{}}}",
            examples.len(),
            failed
        );
        false
    }
}

/// `--dump-catalog-gate=` / `--run-catalog-gate=` headless catalog FQN gate (Q4-A).
pub fn try_run_catalog_gate_cli() -> Option<ExitCode> {
    catalog::gate::try_run_catalog_gate_cli()
}

/// `--check-emit-golden=<report.json>` subsystem / variant / Goto-From emit checks.
pub fn try_run_emit_golden_cli() -> Option<ExitCode> {
    catalog::emit_golden::try_run_emit_golden_cli()
}

/// If a smoke CLI flag is present, run headless smoke and return Some(exit).
pub fn try_run_smoke_cli() -> Option<ExitCode> {
    let target = resolve_smoke_target()?;
    Some(smoke_ok(match target {
        SmokeTarget::Path(path) => run_smoke_open(&path),
        SmokeTarget::Example(id) => match resolve_example_dir(&id) {
            Ok(path) => run_smoke_open(&path),
            Err(err) => {
                eprintln!(
                    "{{\"ok\":false,\"error\":{}}}",
                    serde_json::to_string(&err).unwrap_or_else(|_| "\"error\"".into())
                );
                false
            }
        },
        SmokeTarget::All => run_smoke_all(),
        SmokeTarget::EmitExample(id) => match resolve_example_dir(&id) {
            Ok(path) => run_smoke_emit(&path),
            Err(err) => {
                eprintln!(
                    "{{\"ok\":false,\"error\":{}}}",
                    serde_json::to_string(&err).unwrap_or_else(|_| "\"error\"".into())
                );
                false
            }
        },
    }))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            studio_catalog_list,
            studio_project_create,
            studio_project_open,
            studio_project_save,
            studio_emit_preview,
            studio_validate,
            studio_simulate,
            studio_cancel_simulation,
            studio_pause_simulation,
            studio_resume_simulation,
            studio_step_simulation,
            studio_sim_is_paused,
            studio_emit_artifacts,
            studio_pick_directory,
            studio_pick_export_directory,
            studio_pick_fmu_file,
            studio_pick_mo_file,
            studio_pick_data_file,
            studio_read_text_file,
            studio_import_fmu,
            studio_wrap_mo,
            studio_list_imported_fmus,
            studio_import_catalog_pack,
            studio_export_catalog_pack,
            studio_pick_catalog_pack_directory,
            studio_list_examples,
            studio_open_example,
            studio_startup_project_path,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Modai Studio");
}

