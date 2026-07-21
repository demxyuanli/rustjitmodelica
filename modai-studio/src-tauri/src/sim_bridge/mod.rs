mod cancel_registry;

pub use cancel_registry::{register, request_cancel, unregister};

use crate::diagram::{StudioDiagnostic, StudioReport, StudioSimOptions};
use crate::project::resolve_default_msl_root;
use rustmodlica::{
    run_simulation_collect, CompileOutput, CompileStopPhase, Compiler, CompilerOptions, SimControl,
    SimulationResult,
};
use std::path::PathBuf;

fn build_compiler(library_paths: &[String], options: CompilerOptions) -> Compiler {
    let mut compiler = Compiler::new();
    compiler.options = options;
    if let Some(msl) = resolve_default_msl_root() {
        compiler.loader.add_path(msl);
    }
    compiler.loader.add_path(PathBuf::from("."));
    for p in library_paths {
        compiler.loader.add_path(PathBuf::from(p));
    }
    compiler
}

fn options_from_sim(sim: &StudioSimOptions) -> CompilerOptions {
    let mut opts = CompilerOptions::default();
    opts.t_end = sim.stop_time;
    opts.solver = sim.solver.clone();
    opts.rtol = sim.tolerance;
    opts.atol = sim.tolerance;
    opts.quiet = true;
    let intervals = sim.number_of_intervals.max(1);
    opts.output_interval = sim.stop_time / f64::from(intervals);
    opts.dt = opts.output_interval;
    opts.compile_stop = CompileStopPhase::Full;
    opts
}

pub fn validate_model(
    code: &str,
    model_name: &str,
    library_paths: &[String],
    sim: &StudioSimOptions,
) -> StudioReport {
    let mut opts = options_from_sim(sim);
    opts.validate_only = true;
    let mut compiler = build_compiler(library_paths, opts);
    match compiler.compile_from_source(model_name, code) {
        Ok(_) => StudioReport {
            success: true,
            stage: "validate".into(),
            errors: Vec::new(),
            warnings: compiler
                .take_warnings()
                .into_iter()
                .map(|w| StudioDiagnostic {
                    message: w.message,
                    path: None,
                    line: None,
                    column: None,
                })
                .collect(),
        },
        Err(e) => StudioReport {
            success: false,
            stage: "validate".into(),
            errors: vec![StudioDiagnostic {
                message: e.to_string(),
                path: Some(format!("{model_name}.mo")),
                line: None,
                column: None,
            }],
            warnings: Vec::new(),
        },
    }
}

pub fn simulate_model(
    code: &str,
    model_name: &str,
    library_paths: &[String],
    sim: &StudioSimOptions,
    mut control: Option<SimControl>,
) -> Result<SimulationResult, StudioReport> {
    let opts = options_from_sim(sim);
    let mut compiler = build_compiler(library_paths, opts);
    let out = match compiler.compile_from_source(model_name, code) {
        Ok(o) => o,
        Err(e) => {
            return Err(StudioReport {
                success: false,
                stage: "compile".into(),
                errors: vec![StudioDiagnostic {
                    message: e.to_string(),
                    path: Some(format!("{model_name}.mo")),
                    line: None,
                    column: None,
                }],
                warnings: Vec::new(),
            });
        }
    };
    let artifacts = match out {
        CompileOutput::Simulation(a) => a,
        _other => {
            return Err(StudioReport {
                success: false,
                stage: "compile".into(),
                errors: vec![StudioDiagnostic {
                    message: "unexpected compile output (not a simulatable model)".into(),
                    path: None,
                    line: None,
                    column: None,
                }],
                warnings: Vec::new(),
            });
        }
    };
    if let Some(ctrl) = control.as_mut() {
        let min_dt = artifacts
            .output_interval
            .max(artifacts.t_end / 200.0)
            .max(1e-9);
        ctrl.set_progress_min_dt(min_dt);
    }
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
        None,
        model_name,
        compiler.loader.library_paths.clone(),
        control.as_mut(),
    )
    .map_err(|e| StudioReport {
        success: false,
        stage: "simulate".into(),
        errors: vec![StudioDiagnostic {
            message: e.to_string(),
            path: None,
            line: None,
            column: None,
        }],
        warnings: Vec::new(),
    })
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmitArtifactsResult {
    pub success: bool,
    pub out_dir: String,
    pub files: Vec<String>,
    pub fmu_path: Option<String>,
    pub report: Option<StudioReport>,
}

/// Compile with `--emit-c` semantics; optionally package FMI 2.0 CS artifacts + `.fmu`.
pub fn emit_c_or_fmu(
    code: &str,
    model_name: &str,
    library_paths: &[String],
    sim: &StudioSimOptions,
    out_dir: &str,
    package_fmu_cs: bool,
) -> EmitArtifactsResult {
    let dir = PathBuf::from(out_dir);
    if let Err(e) = std::fs::create_dir_all(&dir) {
        return EmitArtifactsResult {
            success: false,
            out_dir: out_dir.to_string(),
            files: Vec::new(),
            fmu_path: None,
            report: Some(StudioReport {
                success: false,
                stage: "io".into(),
                errors: vec![StudioDiagnostic {
                    message: e.to_string(),
                    path: None,
                    line: None,
                    column: None,
                }],
                warnings: Vec::new(),
            }),
        };
    }

    let mut opts = options_from_sim(sim);
    opts.emit_c_dir = Some(out_dir.to_string());
    let mut compiler = build_compiler(library_paths, opts);
    let out = match compiler.compile_from_source(model_name, code) {
        Ok(o) => o,
        Err(e) => {
            return EmitArtifactsResult {
                success: false,
                out_dir: out_dir.to_string(),
                files: Vec::new(),
                fmu_path: None,
                report: Some(StudioReport {
                    success: false,
                    stage: "compile".into(),
                    errors: vec![StudioDiagnostic {
                        message: e.to_string(),
                        path: Some(format!("{model_name}.mo")),
                        line: None,
                        column: None,
                    }],
                    warnings: Vec::new(),
                }),
            };
        }
    };
    let artifacts = match out {
        CompileOutput::Simulation(a) => a,
        _ => {
            return EmitArtifactsResult {
                success: false,
                out_dir: out_dir.to_string(),
                files: Vec::new(),
                fmu_path: None,
                report: Some(StudioReport {
                    success: false,
                    stage: "compile".into(),
                    errors: vec![StudioDiagnostic {
                        message: "emit requires a simulatable model".into(),
                        path: None,
                        line: None,
                        column: None,
                    }],
                    warnings: Vec::new(),
                }),
            };
        }
    };

    let mut files: Vec<String> = Vec::new();
    for name in ["model.c", "model.h"] {
        let p = dir.join(name);
        if p.exists() {
            files.push(p.display().to_string());
        }
    }

    let mut fmu_path = None;
    if package_fmu_cs {
        let fmi_opts = rustmodlica::FmiExportOptions::default();
        match rustmodlica::fmi::emit_fmu_artifacts_with_options(
            &dir,
            model_name,
            &artifacts.state_vars,
            &artifacts.param_vars,
            &artifacts.output_vars,
            0.0,
            artifacts.t_end,
            artifacts.dt,
            &fmi_opts,
        ) {
            Ok(written) => {
                for p in written {
                    files.push(p.display().to_string());
                }
                let id = rustmodlica::fmi::resolve_model_identifier(model_name, None);
                let packaged = dir.join(format!("{id}.fmu"));
                match rustmodlica::fmi::package_fmu(&dir, &packaged) {
                    Ok(()) => {
                        fmu_path = Some(packaged.display().to_string());
                        files.push(packaged.display().to_string());
                    }
                    Err(e) => {
                        return EmitArtifactsResult {
                            success: false,
                            out_dir: out_dir.to_string(),
                            files,
                            fmu_path: None,
                            report: Some(StudioReport {
                                success: false,
                                stage: "emit".into(),
                                errors: vec![StudioDiagnostic {
                                    message: format!("FMU package failed: {e}"),
                                    path: None,
                                    line: None,
                                    column: None,
                                }],
                                warnings: Vec::new(),
                            }),
                        };
                    }
                }
            }
            Err(e) => {
                return EmitArtifactsResult {
                    success: false,
                    out_dir: out_dir.to_string(),
                    files,
                    fmu_path: None,
                    report: Some(StudioReport {
                        success: false,
                        stage: "emit".into(),
                        errors: vec![StudioDiagnostic {
                            message: format!("FMU emit failed: {e}"),
                            path: None,
                            line: None,
                            column: None,
                        }],
                        warnings: Vec::new(),
                    }),
                };
            }
        }
    }

    EmitArtifactsResult {
        success: true,
        out_dir: out_dir.to_string(),
        files,
        fmu_path,
        report: None,
    }
}
