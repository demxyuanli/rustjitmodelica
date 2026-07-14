fn run_simulation_sync(request: RunSimulationRequest) -> Result<SimulationResult, String> {
    let _timer = ScopedTimer::new("run_simulation_cmd");
    let _salsa_env = SalsaEnvDefaults::install();
    let model_name = resolve_model_name(&request.code, request.model_name.as_ref())?;
    let _eq_expand_mode_env =
        ScopedEnvVar::set("RUSTMODLICA_EQ_EXPAND_PARALLEL_MODE", normalize_eq_expand_parallel_mode(request.options.as_ref()));
    let mut compiler = rustmodlica::Compiler::new();
    compiler.options = build_compiler_options(request.options);
    compiler.options.compile_stop = CompileStopPhase::Full;
    with_loader_paths(
        &mut compiler,
        request.project_dir.as_ref(),
        request.resolver_context.as_ref(),
    );
    let out = compiler
        .compile_from_source(&model_name, &request.code)
        .map_err(|e| e.to_string())?;
    let artifacts = match out {
        rustmodlica::CompileOutput::FunctionRun(_) => {
            return Err("Simulation requested for a function entry".to_string());
        }
        rustmodlica::CompileOutput::FlatSnapshotDone => {
            return Err("Flat snapshot only; simulation is not available".to_string());
        }
        rustmodlica::CompileOutput::ValidationParseOk
        | rustmodlica::CompileOutput::ValidationFlattenOk { .. }
        | rustmodlica::CompileOutput::ValidationAnalyzed(_) => {
            return Err("Simulation requires full compile (tiered validation is not allowed)".to_string());
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
        &model_name,
        compiler.loader.library_paths.clone(),
    )
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn run_simulation_cmd(request: RunSimulationRequest) -> Result<SimulationResult, String> {
    tokio::task::spawn_blocking(move || run_simulation_sync(request))
        .await
        .map_err(|e| format!("blocking task join error: {e}"))?
}

#[tauri::command]
pub async fn run_simulation_cmd_v2(
    app: AppHandle,
    request: RunSimulationRequest,
) -> Result<JitApiEnvelope<SimulationResult>, String> {
    let task_name = "simulate";
    emit_jit_progress(&app, task_name, "started", 0, "Simulation started");
    let started = Instant::now();
    let mut task = tokio::task::spawn_blocking(move || run_simulation_sync(request));
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let join = loop {
        tokio::select! {
            join_res = &mut task => {
                break join_res;
            }
            _ = heartbeat.tick() => {
                let elapsed = started.elapsed().as_secs().max(1);
                emit_jit_progress(
                    &app,
                    task_name,
                    "running",
                    elapsed,
                    format!("Simulation running ({}s)...", elapsed),
                );
            }
        }
    };
    match join
        .map_err(|e| {
            emit_jit_error(&app, task_name, "failed", started.elapsed().as_secs(), format!("Simulation join error: {e}"), Some("join"));
            format!("blocking task join error: {e}")
        })?
    {
        Ok(data) => Ok(envelope_ok("simulate", data)),
        Err(message) => {
            let d = diagnostics_from_error_message(&message);
            emit_jit_error(
                &app,
                task_name,
                "failed",
                started.elapsed().as_secs().max(1),
                format!("Simulation failed: {}", message),
                Some("simulate"),
            );
            Ok(envelope_err(
                "simulate",
                vec![JitApiError {
                    code: d.code,
                    message: d.message,
                    path: d.path,
                    line: d.line,
                    column: d.column,
                }],
            ))
        }
    }
    .map(|env| {
        if env.ok {
            emit_jit_progress(
                &app,
                task_name,
                "completed",
                started.elapsed().as_secs().max(1),
                "Simulation completed",
            );
        }
        env
    })
}

fn get_equation_graph_sync(
    code: String,
    model_name: String,
    project_dir: Option<String>,
    graph_mode: Option<EquationGraphMode>,
    changed_keys: Option<Vec<rustmodlica::NodeKey>>,
) -> Result<rustmodlica::EquationGraph, String> {
    let loader_paths = collect_loader_paths(project_dir.as_ref(), None);
    crate::equation_graph_actor::build_equation_graph_blocking(
        crate::equation_graph_actor::EquationGraphBuildRequest {
            code,
            model_name,
            project_dir,
            loader_paths,
            graph_mode: graph_mode.unwrap_or(EquationGraphMode::Compact),
            changed_keys,
        },
    )
}

fn perf_delta(
    before: &std::collections::HashMap<String, u64>,
    after: &std::collections::HashMap<String, u64>,
    key: &str,
) -> u64 {
    let b = before.get(key).copied().unwrap_or(0);
    let a = after.get(key).copied().unwrap_or(0);
    a.saturating_sub(b)
}

#[tauri::command]
pub async fn get_equation_graph(
    code: String,
    model_name: String,
    project_dir: Option<String>,
    graph_mode: Option<EquationGraphMode>,
    changed_keys: Option<Vec<rustmodlica::NodeKey>>,
    metrics_session_id: Option<String>,
) -> Result<rustmodlica::EquationGraph, String> {
    let _ = metrics_session_id;
    tokio::task::spawn_blocking(move || {
        get_equation_graph_sync(code, model_name, project_dir, graph_mode, changed_keys)
    })
        .await
        .map_err(|e| format!("blocking task join error: {e}"))?
}

#[tauri::command]
pub async fn get_equation_graph_v2(
    app: AppHandle,
    code: String,
    model_name: String,
    project_dir: Option<String>,
    graph_mode: Option<EquationGraphMode>,
    changed_keys: Option<Vec<rustmodlica::NodeKey>>,
    metrics_session_id: Option<String>,
) -> Result<JitApiEnvelope<rustmodlica::EquationGraph>, String> {
    let task_name = "equation-graph";
    let metrics_sid = metrics_session_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    if let Some(sid) = metrics_sid.as_deref() {
        emit_jit_progress_for_session(
            &app,
            sid,
            task_name,
            "started",
            0,
            "Equation graph build started",
            None,
            None,
            None,
        );
    } else {
        emit_jit_progress(&app, task_name, "started", 0, "Equation graph build started");
    }
    let started = Instant::now();
    let perf_before = rustmodlica::query_db::perf_snapshot();
    let mut task = tokio::task::spawn_blocking(move || {
        get_equation_graph_sync(code, model_name, project_dir, graph_mode, changed_keys)
    });
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let join = loop {
        tokio::select! {
            join_res = &mut task => {
                break join_res;
            }
            _ = heartbeat.tick() => {
                let elapsed = started.elapsed().as_secs().max(1);
                if let Some(sid) = metrics_sid.as_deref() {
                    emit_jit_progress_for_session(
                        &app,
                        sid,
                        task_name,
                        "running",
                        elapsed,
                        format!("Equation graph building ({}s)...", elapsed),
                        None,
                        None,
                        None,
                    );
                } else {
                    emit_jit_progress(
                        &app,
                        task_name,
                        "running",
                        elapsed,
                        format!("Equation graph building ({}s)...", elapsed),
                    );
                }
            }
        }
    };
    match join
        .map_err(|e| {
            if let Some(sid) = metrics_sid.as_deref() {
                emit_jit_progress_for_session(
                    &app,
                    sid,
                    task_name,
                    "failed",
                    started.elapsed().as_secs().max(1),
                    format!("Equation graph join error: {e}"),
                    None,
                    None,
                    Some("join"),
                );
            }
            emit_jit_error(
                &app,
                task_name,
                "failed",
                started.elapsed().as_secs(),
                format!("Equation graph join error: {e}"),
                Some("join"),
            );
            format!("blocking task join error: {e}")
        })?
    {
        Ok(data) => {
            let perf_after = rustmodlica::query_db::perf_snapshot();
            let match_skip = perf_delta(
                &perf_before,
                &perf_after,
                "eqgraph_caller_hash_match_skip_count",
            );
            let mismatch_dirty = perf_delta(
                &perf_before,
                &perf_after,
                "eqgraph_caller_hash_mismatch_dirty_count",
            );
            let out_of_range_dirty = perf_delta(
                &perf_before,
                &perf_after,
                "eqgraph_caller_hash_out_of_range_dirty_count",
            );
            let var_dirty =
                perf_delta(&perf_before, &perf_after, "eqgraph_caller_var_dirty_count");
            let denom = match_skip + mismatch_dirty;
            let ratio = if denom > 0 {
                (match_skip as f64) / (denom as f64)
            } else {
                0.0
            };
            if let Some(sid) = metrics_sid.as_deref() {
                emit_jit_progress_for_session(
                    &app,
                    sid,
                    task_name,
                    "metrics",
                    started.elapsed().as_secs().max(1),
                    format!(
                        "eqgraph caller-hash metrics: skip_match={} mismatch_dirty={} out_of_range_dirty={} var_dirty={} skip_ratio={:.3}",
                        match_skip, mismatch_dirty, out_of_range_dirty, var_dirty, ratio
                    ),
                    None,
                    None,
                    None,
                );
            } else {
                emit_jit_progress(
                    &app,
                    task_name,
                    "metrics",
                    started.elapsed().as_secs().max(1),
                    format!(
                        "eqgraph caller-hash metrics: skip_match={} mismatch_dirty={} out_of_range_dirty={} var_dirty={} skip_ratio={:.3}",
                        match_skip, mismatch_dirty, out_of_range_dirty, var_dirty, ratio
                    ),
                );
            }
            Ok(envelope_ok("equationGraph", data))
        }
        Err(message) => {
            if let Some(sid) = metrics_sid.as_deref() {
                emit_jit_progress_for_session(
                    &app,
                    sid,
                    task_name,
                    "failed",
                    started.elapsed().as_secs().max(1),
                    format!("Equation graph failed: {}", message),
                    None,
                    None,
                    Some("equation-graph"),
                );
            }
            let d = diagnostics_from_error_message(&message);
            emit_jit_error(
                &app,
                task_name,
                "failed",
                started.elapsed().as_secs().max(1),
                format!("Equation graph failed: {}", message),
                Some("equation-graph"),
            );
            Ok(envelope_err(
                "equationGraph",
                vec![JitApiError {
                    code: d.code,
                    message: d.message,
                    path: d.path,
                    line: d.line,
                    column: d.column,
                }],
            ))
        }
    }
    .map(|env| {
        if env.ok {
            if let Some(sid) = metrics_sid.as_deref() {
                emit_jit_progress_for_session(
                    &app,
                    sid,
                    task_name,
                    "completed",
                    started.elapsed().as_secs().max(1),
                    "Equation graph build completed",
                    None,
                    None,
                    None,
                );
            } else {
                emit_jit_progress(
                    &app,
                    task_name,
                    "completed",
                    started.elapsed().as_secs().max(1),
                    "Equation graph build completed",
                );
            }
        }
        env
    })
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
    started_at: Instant,
    last_progress_emit_at: Instant,
    last_progress_emit_step: usize,
    completion_emitted: bool,
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
    resolver_context: Option<ResolverContext>,
}

fn start_simulation_session_sync(request: StartSessionRequest) -> Result<String, String> {
    let _timer = ScopedTimer::new("start_simulation_session");
    let _salsa_env = SalsaEnvDefaults::install();
    let model_name = resolve_model_name(&request.code, request.model_name.as_ref())?;
    let _eq_expand_mode_env =
        ScopedEnvVar::set("RUSTMODLICA_EQ_EXPAND_PARALLEL_MODE", normalize_eq_expand_parallel_mode(request.options.as_ref()));
    let mut compiler = rustmodlica::Compiler::new();
    compiler.options = build_compiler_options(request.options);
    compiler.options.compile_stop = CompileStopPhase::Full;
    with_loader_paths(
        &mut compiler,
        request.project_dir.as_ref(),
        request.resolver_context.as_ref(),
    );
    let out = compiler
        .compile_from_source(&model_name, &request.code)
        .map_err(|e| e.to_string())?;
    let artifacts = match out {
        rustmodlica::CompileOutput::FunctionRun(_) => {
            return Err("Simulation requested for a function entry".to_string());
        }
        rustmodlica::CompileOutput::FlatSnapshotDone => {
            return Err("Flat snapshot only; simulation is not available".to_string());
        }
        rustmodlica::CompileOutput::ValidationParseOk
        | rustmodlica::CompileOutput::ValidationFlattenOk { .. }
        | rustmodlica::CompileOutput::ValidationAnalyzed(_) => {
            return Err("Simulation requires full compile (tiered validation is not allowed)".to_string());
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
        &model_name,
        compiler.loader.library_paths.clone(),
    )
    .map_err(|e| e.to_string())?;

    let session_id = format!("sim_{}", uuid_simple());
    let session = SimulationSession {
        result,
        current_step: 0,
        state_names,
        output_names,
        paused: false,
        started_at: Instant::now(),
        last_progress_emit_at: Instant::now(),
        last_progress_emit_step: 0,
        completion_emitted: false,
    };

    SESSIONS.lock().unwrap().insert(session_id.clone(), session);
    Ok(session_id)
}

#[tauri::command]
pub async fn start_simulation_session(app: AppHandle, request: StartSessionRequest) -> Result<String, String> {
    let task_name = "start-session";
    emit_jit_progress(&app, task_name, "started", 0, "Step-debug session compile started");
    let started = Instant::now();
    let mut task = tokio::task::spawn_blocking(move || start_simulation_session_sync(request));
    let mut heartbeat = tokio::time::interval(Duration::from_secs(5));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let join = loop {
        tokio::select! {
            join_res = &mut task => {
                break join_res;
            }
            _ = heartbeat.tick() => {
                let elapsed = started.elapsed().as_secs().max(1);
                emit_jit_progress(
                    &app,
                    task_name,
                    "running",
                    elapsed,
                    format!("Step-debug session compiling ({}s)...", elapsed),
                );
            }
        }
    };
    let sid = join
        .map_err(|e| {
            emit_jit_error(
                &app,
                task_name,
                "failed",
                started.elapsed().as_secs(),
                format!("Step-debug session join error: {e}"),
                Some("join"),
            );
            format!("blocking task join error: {e}")
        })?
        .map_err(|e| {
            emit_jit_error(
                &app,
                task_name,
                "failed",
                started.elapsed().as_secs(),
                format!("Step-debug session failed: {e}"),
                Some("start-session"),
            );
            e
        })?;
    emit_jit_progress_for_session(
        &app,
        &sid,
        task_name,
        "ready",
        started.elapsed().as_secs().max(1),
        "Step-debug session created",
        Some(0),
        None,
        Some("create"),
    );
    emit_jit_progress(
        &app,
        task_name,
        "completed",
        started.elapsed().as_secs().max(1),
        "Step-debug session ready",
    );
    Ok(sid)
}

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}_{}", d.as_millis(), d.subsec_nanos())
}

#[tauri::command]
pub fn simulation_step(app: AppHandle, session_id: String) -> Result<StepState, String> {
    const STEP_EMIT_EVERY: usize = 25;
    const STEP_EMIT_MIN_INTERVAL_SECS: u64 = 1;
    let mut sessions = SESSIONS.lock().unwrap();
    let session = match sessions.get_mut(&session_id) {
        Some(s) => s,
        None => {
            emit_jit_error(&app, "step-session", "failed", 0, "Step-debug session not found", Some("session-not-found"));
            return Err("Session not found".to_string());
        }
    };

    let total_steps = session.result.time.len();
    if session.current_step >= total_steps {
        if !session.completion_emitted {
            let elapsed = session.started_at.elapsed().as_secs().max(1);
            emit_jit_progress(
                &app,
                "step-session",
                "completed",
                elapsed,
                format!("Step-debug playback completed at {} steps", total_steps),
            );
            emit_jit_progress_for_session(
                &app,
                &session_id,
                "step-session",
                "completed",
                elapsed,
                format!("Step-debug playback completed at {} steps", total_steps),
                Some(total_steps),
                Some(total_steps),
                Some("completed"),
            );
            session.completion_emitted = true;
        }
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
    let should_emit_by_step = session.current_step.saturating_sub(session.last_progress_emit_step) >= STEP_EMIT_EVERY;
    let should_emit_by_time = session.last_progress_emit_at.elapsed() >= Duration::from_secs(STEP_EMIT_MIN_INTERVAL_SECS);
    if should_emit_by_step && should_emit_by_time {
        let elapsed = session.started_at.elapsed().as_secs().max(1);
        emit_jit_progress_for_session(
            &app,
            &session_id,
            "step-session",
            if session.paused { "paused" } else { "running" },
            elapsed,
            format!("Step-debug progress: {}/{} steps", session.current_step, total_steps),
            Some(session.current_step),
            Some(total_steps),
            None,
        );
        session.last_progress_emit_at = Instant::now();
        session.last_progress_emit_step = session.current_step;
    }

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
pub fn simulation_command(app: AppHandle, session_id: String, command: String) -> Result<(), String> {
    let mut sessions = SESSIONS.lock().unwrap();
    match command.as_str() {
        "pause" => {
            if let Some(session) = sessions.get_mut(&session_id) {
                session.paused = true;
                let total_steps = session.result.time.len();
                emit_jit_control(
                    &app,
                    "step-session",
                    "paused",
                    session.started_at.elapsed().as_secs().max(1),
                    format!("Step-debug paused at step {}/{}", session.current_step, total_steps),
                    Some(session.current_step),
                    Some(total_steps),
                    Some("pause"),
                );
                emit_jit_progress_for_session(
                    &app,
                    &session_id,
                    "step-session",
                    "paused",
                    session.started_at.elapsed().as_secs().max(1),
                    "Step-debug paused",
                    Some(session.current_step),
                    Some(total_steps),
                    Some("pause"),
                );
            }
        }
        "run" => {
            if let Some(session) = sessions.get_mut(&session_id) {
                session.paused = false;
                let total_steps = session.result.time.len();
                emit_jit_control(
                    &app,
                    "step-session",
                    "running",
                    session.started_at.elapsed().as_secs().max(1),
                    format!("Step-debug resumed at step {}/{}", session.current_step, total_steps),
                    Some(session.current_step),
                    Some(total_steps),
                    Some("run"),
                );
                emit_jit_progress_for_session(
                    &app,
                    &session_id,
                    "step-session",
                    "running",
                    session.started_at.elapsed().as_secs().max(1),
                    "Step-debug resumed",
                    Some(session.current_step),
                    Some(total_steps),
                    Some("run"),
                );
            }
        }
        "stop" => {
            if let Some(session) = sessions.remove(&session_id) {
                let total_steps = session.result.time.len();
                emit_jit_control(
                    &app,
                    "step-session",
                    "stopped",
                    session.started_at.elapsed().as_secs().max(1),
                    format!("Step-debug stopped at step {}/{}", session.current_step, total_steps),
                    Some(session.current_step),
                    Some(total_steps),
                    Some("stop"),
                );
            }
        }
        "reset" => {
            if let Some(session) = sessions.remove(&session_id) {
                let total_steps = session.result.time.len();
                emit_jit_control(
                    &app,
                    "step-session",
                    "reset",
                    session.started_at.elapsed().as_secs().max(1),
                    format!("Step-debug reset at step {}/{}", session.current_step, total_steps),
                    Some(session.current_step),
                    Some(total_steps),
                    Some("reset"),
                );
            }
        }
        _ => {
            emit_jit_error(&app, "step-session", "failed", 0, format!("Unknown simulation command: {}", command), Some("unknown-command"));
            return Err(format!("Unknown command: {}", command));
        }
    }
    Ok(())
}

#[tauri::command]
pub fn get_monitor_events(session_id: Option<String>, limit: Option<usize>) -> Result<Vec<JitProgressEventRecord>, String> {
    let max_n = limit.unwrap_or(200).max(1).min(1000);
    let path = if let Some(sid) = session_id {
        monitor_events_file_path(&sid)?
    } else {
        let dir = monitor_events_dir()?;
        let mut latest: Option<(std::time::SystemTime, PathBuf)> = None;
        for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
            let entry = match entry {
                Ok(v) => v,
                Err(_) => continue,
            };
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
            if latest.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                latest = Some((mtime, path));
            }
        }
        match latest {
            Some((_, p)) => p,
            None => return Ok(Vec::new()),
        }
    };
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let mut records: Vec<JitProgressEventRecord> = serde_json::from_str(&content).unwrap_or_default();
    if records.len() > max_n {
        records.drain(0..(records.len() - max_n));
    }
    Ok(records)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorEventSessionEntry {
    pub session_id: String,
    pub modified_ms: Option<u64>,
    pub event_count: usize,
}

#[tauri::command]
pub fn list_monitor_event_sessions(limit: Option<usize>) -> Result<Vec<MonitorEventSessionEntry>, String> {
    let max_n = limit.unwrap_or(50).max(1).min(200);
    let dir = monitor_events_dir()?;
    let mut rows: Vec<(std::time::SystemTime, String, PathBuf)> = Vec::new();
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())? {
        let entry = match entry {
            Ok(v) => v,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => continue,
        };
        let mtime = entry
            .metadata()
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);
        rows.push((mtime, stem, path));
    }
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    rows.truncate(max_n);
    Ok(rows
        .into_iter()
        .map(|(t, session_id, path)| {
            let modified_ms = t
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_millis() as u64);
            let event_count = fs::read_to_string(&path)
                .ok()
                .and_then(|content| serde_json::from_str::<Vec<JitProgressEventRecord>>(&content).ok())
                .map(|v| v.len())
                .unwrap_or(0);
            MonitorEventSessionEntry {
                session_id,
                modified_ms,
                event_count,
            }
        })
        .collect())
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
