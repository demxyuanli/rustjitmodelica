mod cancel_registry;
pub mod fast_restart;

pub use cancel_registry::{
    is_paused, register, request_cancel, request_pause, request_resume, request_step, unregister,
};
pub use fast_restart::{
    param_overrides_from_diagram, param_overrides_from_diagram_with_dict, simulate_with_fast_restart,
    structure_key, RestartMode,
};

use crate::diagram::{StudioDiagnostic, StudioReport, StudioSimOptions};
use crate::project::resolve_default_msl_root;
use rustmodlica::{
    run_simulation_collect, CompileOutput, CompileStopPhase, Compiler, CompilerOptions, SimControl,
    SimulationResult,
};
use std::path::PathBuf;

pub(crate) fn build_compiler(library_paths: &[String], options: CompilerOptions) -> Compiler {
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

pub(crate) fn options_from_sim(sim: &StudioSimOptions) -> CompilerOptions {
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
    full_jit: bool,
) -> StudioReport {
    let mut opts = options_from_sim(sim);
    opts.validate_only = true;
    // Default Analyze skips JIT codegen; Full JIT when caller opts in.
    opts.compile_stop = if full_jit {
        CompileStopPhase::Full
    } else {
        CompileStopPhase::Analyze
    };
    let stage = if full_jit {
        "validate_full"
    } else {
        "validate_analyze"
    };
    let mut compiler = build_compiler(library_paths, opts);
    match compiler.compile_from_source(model_name, code) {
        Ok(_) => {
            let mut warnings: Vec<StudioDiagnostic> = compiler
                .take_warnings()
                .into_iter()
                .map(|w| StudioDiagnostic {
                    message: annotate_fallback_message(w.message),
                    path: None,
                    line: None,
                    column: None,
                })
                .collect();
            // Surface compiler placeholder/fallback noise explicitly (Q4-A P525).
            warnings.retain(|w| !w.message.trim().is_empty());
            StudioReport {
                success: true,
                stage: stage.into(),
                errors: Vec::new(),
                warnings,
            }
        }
        Err(e) => StudioReport {
            success: false,
            stage: stage.into(),
            errors: vec![StudioDiagnostic {
                message: annotate_fallback_message(e.to_string()),
                path: Some(format!("{model_name}.mo")),
                line: None,
                column: None,
            }],
            warnings: Vec::new(),
        },
    }
}

fn annotate_fallback_message(message: String) -> String {
    let lower = message.to_ascii_lowercase();
    let hit = lower.contains("placeholder")
        || lower.contains("fallback")
        || lower.contains("not implemented")
        || lower.contains("unsupported");
    if hit && !message.contains("[fallback]") {
        format!("[fallback] {message}")
    } else {
        message
    }
}

/// Compile Modelica source to simulation artifacts (kept alive by caller for calc_derivs).
pub fn compile_artifacts(
    code: &str,
    model_name: &str,
    library_paths: &[String],
    sim: &StudioSimOptions,
) -> Result<rustmodlica::Artifacts, StudioReport> {
    let mut opts = options_from_sim(sim);
    // Tight cosim drives fixed RK4; force rk4 for predictable one-step semantics.
    opts.solver = "rk4".into();
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
    match out {
        CompileOutput::Simulation(a) => Ok(a),
        _ => Err(StudioReport {
            success: false,
            stage: "compile".into(),
            errors: vec![StudioDiagnostic {
                message: "unexpected compile output (not a simulatable model)".into(),
                path: None,
                line: None,
                column: None,
            }],
            warnings: Vec::new(),
        }),
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
    /// "c" | "coder" | "fmu" | "fmu-me" | "coder+fmu" | "coder+fmu-me"
    pub package_kind: Option<String>,
}

/// Compile with `--emit-c` semantics; optionally package FMI 2.0 CS or ME + `.fmu`.
/// When `coder_package` is true, also writes rml_step / CMake / interface.json deployment files.
pub fn emit_c_or_fmu(
    code: &str,
    model_name: &str,
    library_paths: &[String],
    sim: &StudioSimOptions,
    out_dir: &str,
    package_fmu: bool,
    coder_package: bool,
    fmi_kind: &str,
    fmi_opts: rustmodlica::FmiExportOptions,
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
            package_kind: None,
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
                package_kind: None,
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
                package_kind: None,
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

    if coder_package {
        let intervals = sim.number_of_intervals.max(1);
        let sample = sim.stop_time / f64::from(intervals);
        let info = rustmodlica::CoderPackageInfo {
            model_name: model_name.to_string(),
            state_vars: artifacts.state_vars.clone(),
            param_vars: artifacts.param_vars.clone(),
            output_vars: artifacts.output_vars.clone(),
            sample_time: sample,
            stop_time: sim.stop_time,
            has_jacobian: artifacts.symbolic_ode_jacobian.is_some(),
        };
        match rustmodlica::write_coder_package(&dir, &info) {
            Ok(extra) => {
                for p in extra {
                    files.push(p.display().to_string());
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
                            message: format!("coder package failed: {e}"),
                            path: None,
                            line: None,
                            column: None,
                        }],
                        warnings: Vec::new(),
                    }),
                    package_kind: None,
                };
            }
        }
    }

    let mut fmu_path = None;
    let kind = fmi_kind.trim().to_ascii_lowercase();
    let want_fmu = package_fmu && (kind == "cs" || kind == "me" || kind.is_empty());
    let use_me = kind == "me";
    if want_fmu {
        let emit_res = if use_me {
            rustmodlica::fmi::emit_fmu_me_artifacts_with_options(
                &dir,
                model_name,
                &artifacts.state_vars,
                &artifacts.param_vars,
                &artifacts.output_vars,
                0.0,
                artifacts.t_end,
                artifacts.dt,
                &fmi_opts,
            )
        } else {
            rustmodlica::fmi::emit_fmu_artifacts_with_options(
                &dir,
                model_name,
                &artifacts.state_vars,
                &artifacts.param_vars,
                &artifacts.output_vars,
                0.0,
                artifacts.t_end,
                artifacts.dt,
                &fmi_opts,
            )
        };
        match emit_res {
            Ok(written) => {
                for p in written {
                    files.push(p.display().to_string());
                }
                let id = rustmodlica::fmi::resolve_model_identifier(
                    model_name,
                    fmi_opts.model_identifier_override.as_deref(),
                );
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
                            package_kind: None,
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
                    package_kind: None,
                };
            }
        }
    }

    let package_kind = match (coder_package, want_fmu, use_me) {
        (true, true, true) => Some("coder+fmu-me".into()),
        (true, true, false) => Some("coder+fmu".into()),
        (true, false, _) => Some("coder".into()),
        (false, true, true) => Some("fmu-me".into()),
        (false, true, false) => Some("fmu".into()),
        (false, false, _) => Some("c".into()),
    };

    EmitArtifactsResult {
        success: true,
        out_dir: out_dir.to_string(),
        files,
        fmu_path,
        report: None,
        package_kind,
    }
}
