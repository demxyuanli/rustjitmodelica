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
    /// When true (default), reuse compiled Artifacts if diagram structure is unchanged.
    #[serde(default = "default_true")]
    pub fast_restart: bool,
    #[serde(default)]
    pub param_dictionary: Vec<crate::diagram::ParamDictionaryEntry>,
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
    /// "fastRestart" | "fullCompile" | null when not applicable (e.g. FMU path).
    pub restart_mode: Option<String>,
    /// Wall-clock phase breakdown (F89).
    #[serde(default)]
    pub profile: Option<SimProfile>,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_time: Option<Vec<f64>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub live_series: Option<std::collections::HashMap<String, Vec<f64>>>,
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
                    live_time: None,
                    live_series: None,
                },
            );
            let root = req.root_dir.clone().map(PathBuf::from);
            let options = req.options.clone();
            let subsystems = req.subsystems.clone();
            let library_paths = req.library_paths.clone();
            let code = resolve_code(&ValidateRequest {
                root_dir: req.root_dir.clone(),
                model_name: req.model_name.clone(),
                code: req.code.clone(),
                diagram: Some(diagram.clone()),
                subsystems: req.subsystems.clone(),
                library_paths: req.library_paths.clone(),
                options: req.options.clone(),
                emit_from_diagram: req.emit_from_diagram,
                full_jit: false,
                param_dictionary: req.param_dictionary.clone(),
            })
            .ok();
            let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let pause = Arc::new(std::sync::atomic::AtomicBool::new(false));
            let step_once = Arc::new(std::sync::atomic::AtomicBool::new(false));
            sim_bridge::register(
                &run_id,
                Arc::clone(&cancel),
                Arc::clone(&pause),
                Arc::clone(&step_once),
            );
            let cosim_ctrl = fmu_import::CosimControl::from_flags(cancel, pause, step_once);
            let run_id_task = run_id.clone();
            let outcome = tokio::task::spawn_blocking(move || {
                let r = fmu_import::simulate_with_fmu(
                    &diagram,
                    &subsystems,
                    root.as_deref(),
                    &library_paths,
                    &options,
                    Some(&cosim_ctrl),
                );
                sim_bridge::unregister(&run_id_task);
                r
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
                            live_time: None,
                            live_series: None,
                        },
                    );
                    SimulateResponse {
                        success: true,
                        report: None,
                        result: Some(result),
                        emitted_code: code,
                        run_id: Some(run_id),
                        cancelled: false,
                        restart_mode: Some("fmu".into()),
                        profile: Some(SimProfile {
                            restart_mode: Some("fmu".into()),
                            ..Default::default()
                        }),
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
                            live_time: None,
                            live_series: None,
                        },
                    );
                    SimulateResponse {
                        success: false,
                        report: Some(report),
                        result: None,
                        emitted_code: code,
                        run_id: Some(run_id),
                        cancelled: false,
                        restart_mode: None,
                        profile: None,
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
                    restart_mode: None,
                    profile: None,
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
        full_jit: false,
        param_dictionary: req.param_dictionary.clone(),
    };
    let _ = app.emit(
        "studio-sim-progress",
        StudioProgressEvent {
            run_id: run_id.clone(),
            phase: "emit".into(),
            sim_time: None,
            t_end: Some(req.options.stop_time),
            step: None,
            fraction: Some(0.0),
            message: "Emitting Modelica".into(),
            live_time: None,
            live_series: None,
        },
    );
    let t_emit = Instant::now();
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
                restart_mode: None,
                profile: None,
            };
        }
    };
    let emit_ms = t_emit.elapsed().as_millis() as u64;

    let latest = Arc::new(Mutex::new(None::<(f64, f64, u64)>));
    let latest_cb = latest.clone();
    let mut control = SimControl::new().on_progress(move |p| {
        if let Ok(mut g) = latest_cb.lock() {
            *g = Some((p.time, p.t_end, p.step));
        }
    });
    let live = control.enable_live_series();
    sim_bridge::register(
        &run_id,
        control.cancel_flag(),
        control.pause_flag(),
        control.step_once_flag(),
    );

    let struct_key = sim_bridge::structure_key(
        &req.model_name,
        req.diagram.as_ref(),
        &req.subsystems,
        &req.library_paths,
        &code,
    );
    let mut dict = std::collections::HashMap::new();
    for e in &req.param_dictionary {
        let n = e.name.trim();
        if !n.is_empty() {
            dict.insert(n.to_string(), e.value.trim().to_string());
        }
    }
    let param_overrides = sim_bridge::param_overrides_from_diagram_with_dict(
        req.diagram.as_ref(),
        &req.subsystems,
        &dict,
    );
    let enable_fast = req.fast_restart;
    if !enable_fast {
        sim_bridge::fast_restart::invalidate_cache();
    }

    let _ = app.emit(
        "studio-sim-progress",
        StudioProgressEvent {
            run_id: run_id.clone(),
            phase: "compile".into(),
            sim_time: Some(0.0),
            t_end: Some(req.options.stop_time),
            step: Some(0),
            fraction: Some(0.02),
            message: if enable_fast {
                "Compile / Fast Restart".into()
            } else {
                "Compiling".into()
            },
            live_time: None,
            live_series: None,
        },
    );

    let model_name = req.model_name.clone();
    let library_paths = resolved_library_paths(req.root_dir.as_deref(), &req.library_paths);
    let options = req.options.clone();
    let code_for_sim = code.clone();
    let run_id_task = run_id.clone();
    let mut task = tokio::task::spawn_blocking(move || {
        let result = sim_bridge::simulate_with_fast_restart(
            struct_key,
            &code_for_sim,
            &model_name,
            &library_paths,
            &options,
            param_overrides,
            enable_fast,
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
                let delta = live.take_delta();
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
                            live_time: delta.as_ref().map(|d| d.time.clone()),
                            live_series: delta.map(|d| d.series),
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
                restart_mode: None,
                profile: Some(SimProfile {
                    emit_ms,
                    ..Default::default()
                }),
            };
        }
    };

    match outcome {
        Ok(sim_out) => {
            let mode_str = match sim_out.mode {
                RestartMode::FastRestart => "fastRestart",
                RestartMode::FullCompile => "fullCompile",
            };
            let mut profile = sim_out.profile;
            profile.emit_ms = emit_ms;
            profile.restart_mode = Some(mode_str.into());
            profile.finalize();
            let _ = app.emit(
                "studio-sim-progress",
                StudioProgressEvent {
                    run_id: run_id.clone(),
                    phase: "done".into(),
                    sim_time: None,
                    t_end: None,
                    step: None,
                    fraction: Some(1.0),
                    message: format!("Simulation done ({mode_str})"),
                    live_time: None,
                    live_series: None,
                },
            );
            SimulateResponse {
                success: true,
                report: None,
                result: Some(sim_out.result),
                emitted_code: Some(code),
                run_id: Some(run_id),
                cancelled: false,
                restart_mode: Some(mode_str.into()),
                profile: Some(profile),
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
                    live_time: None,
                    live_series: None,
                },
            );
            SimulateResponse {
                success: false,
                report: Some(report),
                result: None,
                emitted_code: Some(code),
                run_id: Some(run_id),
                cancelled,
                restart_mode: None,
                profile: Some(SimProfile {
                    emit_ms,
                    ..Default::default()
                }),
            }
        }
    }
}

#[tauri::command]
pub fn studio_cancel_simulation(run_id: String) -> bool {
    sim_bridge::request_cancel(&run_id)
}

#[tauri::command]
pub fn studio_pause_simulation(run_id: String) -> bool {
    sim_bridge::request_pause(&run_id)
}

#[tauri::command]
pub fn studio_resume_simulation(run_id: String) -> bool {
    sim_bridge::request_resume(&run_id)
}

#[tauri::command]
pub fn studio_step_simulation(run_id: String) -> bool {
    sim_bridge::request_step(&run_id)
}

#[tauri::command]
pub fn studio_sim_is_paused(run_id: String) -> bool {
    sim_bridge::is_paused(&run_id)
}

