use rustmodlica::{
    Artifacts, CompileOutput, Compiler, WarningInfo, run_simulation, run_simulation_collect,
};
use rustmodlica::error;
use rustmodlica::fmi;
use rustmodlica::i18n;
use rustmodlica::script;
use serde::Serialize;
use std::env;
use std::fs;
use std::io::Read;
use std::process::ExitCode;
use std::thread;
use std::time::Instant;

type RunError = error::AppError;

fn maybe_write_perf_json(
    perf_json_path: &Option<String>,
    compiler: &mut Compiler,
    model_name: &str,
    warnings_count: usize,
) -> Result<(), RunError> {
    let Some(path) = perf_json_path.as_ref() else {
        return Ok(());
    };
    let payload = serde_json::json!({
        "model": model_name,
        "warnings_count": warnings_count,
        "compile_perf": compiler.take_compile_perf_report()
    });
    let text = serde_json::to_string_pretty(&payload)
        .map_err(|e| RunError::Message(format!("serialize perf json failed: {}", e)))?;
    fs::write(path, text)
        .map_err(|e| RunError::Message(format!("write perf json '{}' failed: {}", path, e)))?;
    Ok(())
}

fn emit_validate_json(
    success: bool,
    warnings: &[WarningInfo],
    errors: &[String],
    state_vars: &[String],
    output_vars: &[String],
) {
    let warnings_json: Vec<serde_json::Value> = warnings
        .iter()
        .map(|w| {
            serde_json::json!({
                "path": w.path,
                "line": w.line,
                "column": w.column,
                "message": w.message
            })
        })
        .collect();
    let out = serde_json::json!({
        "success": success,
        "warnings": warnings_json,
        "errors": errors,
        "state_vars": state_vars,
        "output_vars": output_vars
    });
    println!("{}", serde_json::to_string(&out).unwrap_or_default());
}

/// INT-1: REPL loop. Commands: <var_name> (print value), simulate, list, quit/exit.
fn run_repl_loop(artifacts: Artifacts) -> Result<(), RunError> {
    use std::io::{self, BufRead, Write};
    println!(
        "REPL: type variable name to inspect, 'simulate' to run, 'list' for vars, 'quit' to exit."
    );
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    loop {
        write!(stdout, "> ").ok();
        stdout.flush().ok();
        let mut line = String::new();
        if stdin.lock().read_line(&mut line).is_err() || line.is_empty() {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_lowercase();
        if lower == "quit" || lower == "exit" {
            break;
        }
        if lower == "simulate" {
            println!("{}", i18n::msg0("starting_simulation"));
            run_simulation(
                artifacts.calc_derivs,
                artifacts.when_count,
                artifacts.crossings_count,
                artifacts.states.clone(),
                artifacts.discrete_vals.clone(),
                artifacts.params.clone(),
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
                artifacts.result_file.as_deref(),
                &artifacts.clock_partition_schedule,
                None,
            )?;
            println!("{}", i18n::msg0("simulation_completed"));
            continue;
        }
        if lower == "list" || lower == "vars" {
            for n in &artifacts.state_vars {
                println!("  state {}", n);
            }
            for n in &artifacts.param_vars {
                println!("  param {}", n);
            }
            for n in &artifacts.discrete_vars {
                println!("  discrete {}", n);
            }
            continue;
        }
        let name = line;
        if let Some(&i) = artifacts.state_var_index.get(name) {
            if i < artifacts.states.len() {
                println!("{}", artifacts.states[i]);
                continue;
            }
        }
        if let Some((i, _)) = artifacts
            .param_vars
            .iter()
            .enumerate()
            .find(|(_, s)| *s == name)
        {
            if i < artifacts.params.len() {
                println!("{}", artifacts.params[i]);
                continue;
            }
        }
        if let Some((i, _)) = artifacts
            .discrete_vars
            .iter()
            .enumerate()
            .find(|(_, s)| *s == name)
        {
            if i < artifacts.discrete_vals.len() {
                println!("{}", artifacts.discrete_vals[i]);
                continue;
            }
        }
        println!("unknown variable: {}", name);
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize)]
struct ScanMetrics {
    switch_count: usize,
    first_event: Option<f64>,
    final_h: f64,
    final_v: f64,
}

#[derive(Debug, Clone, Serialize)]
struct ScanRow {
    event_count_deadband: f64,
    tail_velocity_deadband: f64,
    solver_metrics: Vec<SolverScanMetrics>,
    score: f64,
}

#[derive(Debug, Clone, Serialize)]
struct EventScanOutput {
    models: Vec<EventScanModelOutput>,
    aggregate_best: Option<AggregateScanRow>,
    aggregate_topn: Vec<AggregateScanRow>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
enum EventScanModelStatus {
    Stable,
    Nondeterministic,
    Unsupported,
    ConfigError,
}

#[derive(Debug, Clone, Serialize)]
struct SolverScanMetrics {
    solver: String,
    metrics: ScanMetrics,
}

#[derive(Debug, Clone, Serialize)]
struct EventScanModelOutput {
    model: String,
    status: EventScanModelStatus,
    supported_solvers: Vec<String>,
    unsupported_reason: Option<String>,
    config_error: Option<String>,
    baseline_rk4: Option<ScanMetrics>,
    baseline_rk45: Option<ScanMetrics>,
    best: Option<ScanRow>,
    topn: Vec<ScanRow>,
}

#[derive(Debug, Clone, Serialize)]
struct AggregateScanRow {
    event_count_deadband: f64,
    tail_velocity_deadband: f64,
    total_score: f64,
    avg_score: f64,
    max_score: f64,
    models_covered: usize,
}

#[derive(Debug, Clone)]
struct AggregateScanAccumulator {
    event_count_deadband: f64,
    tail_velocity_deadband: f64,
    total_score: f64,
    max_score: f64,
    models_covered: usize,
}

#[derive(Debug, Clone, Copy)]
enum AggregateMode {
    Sum,
    Avg,
    Max,
}

impl AggregateMode {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "sum" => Some(Self::Sum),
            "avg" => Some(Self::Avg),
            "max" => Some(Self::Max),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum AggregateReportMode {
    Full,
    Compact,
}

impl AggregateReportMode {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "full" => Some(Self::Full),
            "compact" => Some(Self::Compact),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum QuietMode {
    None,
    Events,
    All,
}

impl QuietMode {
    fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "none" => Some(Self::None),
            "events" => Some(Self::Events),
            "all" => Some(Self::All),
            _ => None,
        }
    }
}

fn parse_scan_list_f64(raw: &str) -> Result<Vec<f64>, RunError> {
    let mut out = Vec::new();
    for s in raw.split(',') {
        let v = s.trim().parse::<f64>().map_err(|e| {
            format!("invalid scan list value '{}': {}", s.trim(), e)
        })?;
        if !v.is_finite() {
            return Err(format!("invalid non-finite scan value '{}'", s.trim()).into());
        }
        out.push(v);
    }
    if out.is_empty() {
        return Err("scan value list cannot be empty".into());
    }
    Ok(out)
}

fn parse_scan_list_string(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn scan_metrics_from_csv(csv_content: &str) -> ScanMetrics {
    let mut rows: Vec<(f64, f64, f64)> = Vec::new();
    for line in csv_content.lines().skip(1) {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() < 3 {
            continue;
        }
        let t = match parts[0].trim().parse::<f64>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let h = match parts[1].trim().parse::<f64>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let v = match parts[2].trim().parse::<f64>() {
            Ok(v) => v,
            Err(_) => continue,
        };
        rows.push((t, h, v));
    }

    let (final_h, final_v) = rows.last().map(|(_, h, v)| (*h, *v)).unwrap_or((0.0, 0.0));
    let mut switch_count = 0usize;
    let mut first_event = None;
    let mut i = 1usize;
    while i < rows.len() {
        if rows[i - 1].2 < 0.0 && rows[i].2 > 0.0 {
            switch_count += 1;
            if first_event.is_none() {
                first_event = Some(rows[i].0);
            }
        }
        i += 1;
    }

    ScanMetrics {
        switch_count,
        first_event,
        final_h,
        final_v,
    }
}

fn run_collect_with_solver(artifacts: &Artifacts, solver: &str) -> Result<ScanMetrics, RunError> {
    let temp_name = format!(
        "rustmodlica_event_scan_{}_{}_{}.csv",
        solver,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    );
    let temp_path = std::env::temp_dir().join(temp_name);
    let temp_path_str = temp_path.to_string_lossy().to_string();
    run_simulation(
        artifacts.calc_derivs,
        artifacts.when_count,
        artifacts.crossings_count,
        artifacts.states.clone(),
        artifacts.discrete_vals.clone(),
        artifacts.params.clone(),
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
        solver,
        artifacts.output_interval,
        Some(temp_path_str.as_str()),
        &artifacts.clock_partition_schedule,
        None,
    )?;
    let csv = std::fs::read_to_string(&temp_path)
        .map_err(|e| format!("failed to read scan csv '{}': {}", temp_path_str, e))?;
    let _ = std::fs::remove_file(&temp_path);
    Ok(scan_metrics_from_csv(&csv))
}

fn metric_score(base: &ScanMetrics, others: &[ScanMetrics]) -> f64 {
    if others.is_empty() {
        return f64::INFINITY;
    }
    let base_first = base.first_event.unwrap_or(1e9);
    let mut total = 0.0;
    for m in others {
        let first = m.first_event.unwrap_or(1e9);
        let switch_penalty = (m.switch_count as f64 - base.switch_count as f64).abs();
        let first_penalty = (first - base_first).abs();
        let final_h_penalty = (m.final_h - base.final_h).abs();
        total += switch_penalty * 10.0 + first_penalty * 100.0 + final_h_penalty;
    }
    total / others.len() as f64
}

fn detect_supported_solvers(artifacts: &Artifacts) -> (Vec<String>, Option<String>) {
    let mut solvers = vec![
        "rk4".to_string(),
        "rk45".to_string(),
        "implicit".to_string(),
        "cvode".to_string(),
        "ida".to_string(),
    ];
    if artifacts.states.is_empty() {
        solvers.retain(|s| s != "cvode" && s != "ida");
        return (
            solvers,
            Some("cvode/ida require non-empty state vectors".to_string()),
        );
    }
    if !artifacts.newton_tearing_var_names.is_empty() {
        solvers.retain(|s| s != "cvode" && s != "ida");
        return (
            solvers,
            Some("cvode/ida are not supported for models with Newton tearing".to_string()),
        );
    }
    (solvers, None)
}

fn run_event_scan(args: &[String]) -> Result<(), RunError> {
    let mut model_names = vec!["BouncingBall".to_string()];
    let mut lib_paths: Vec<String> = Vec::new();
    let mut t_end = 4.0_f64;
    let mut dt = 0.001_f64;
    let mut output_interval = 0.001_f64;
    let mut top_n = 5usize;
    let mut aggregate_mode = AggregateMode::Sum;
    let mut aggregate_report_mode = AggregateReportMode::Full;
    let mut quiet_mode = QuietMode::None;
    let mut output_file: Option<String> = None;
    let mut count_values = vec![4e-4, 5e-4, 6e-4, 8e-4];
    let mut tail_v_values = vec![2e-2, 3e-2, 4e-2, 5e-2];

    let mut i = 2usize;
    while i < args.len() {
        let a = &args[i];
        if let Some(v) = a.strip_prefix("--model=") {
            model_names = vec![v.to_string()];
            i += 1;
        } else if let Some(v) = a.strip_prefix("--models=") {
            let parsed = parse_scan_list_string(v);
            if !parsed.is_empty() {
                model_names = parsed;
            }
            i += 1;
        } else if let Some(v) = a.strip_prefix("--lib-path=") {
            if !v.trim().is_empty() {
                lib_paths.push(v.to_string());
            }
            i += 1;
        } else if let Some(v) = a.strip_prefix("--t-end=") {
            t_end = v.parse().unwrap_or(t_end);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--dt=") {
            dt = v.parse().unwrap_or(dt);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--output-interval=") {
            output_interval = v.parse().unwrap_or(output_interval);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--top-n=") {
            top_n = v.parse().unwrap_or(top_n);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--aggregate-mode=") {
            aggregate_mode = AggregateMode::parse(v)
                .ok_or_else(|| format!("invalid --aggregate-mode value '{}', expected sum|avg|max", v))?;
            i += 1;
        } else if let Some(v) = a.strip_prefix("--aggregate-report=") {
            aggregate_report_mode = AggregateReportMode::parse(v).ok_or_else(|| {
                format!(
                    "invalid --aggregate-report value '{}', expected full|compact",
                    v
                )
            })?;
            i += 1;
        } else if let Some(v) = a.strip_prefix("--count-values=") {
            count_values = parse_scan_list_f64(v)?;
            i += 1;
        } else if let Some(v) = a.strip_prefix("--tail-velocity-values=") {
            tail_v_values = parse_scan_list_f64(v)?;
            i += 1;
        } else if let Some(v) = a.strip_prefix("--output-file=") {
            output_file = Some(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--quiet=") {
            quiet_mode = QuietMode::parse(v)
                .ok_or_else(|| format!("invalid --quiet value '{}', expected none|events|all", v))?;
            i += 1;
        } else if a == "--quiet" {
            quiet_mode = QuietMode::All;
            i += 1;
        } else if !a.starts_with('-') {
            model_names = vec![a.clone()];
            i += 1;
        } else {
            return Err(format!("unknown event-scan argument: {}", a).into());
        }
    }
    if lib_paths.is_empty() {
        return Err("event-scan requires at least one --lib-path=<dir>".into());
    }

    let old_count = env::var("RUSTMODLICA_EVENT_COUNT_DEADBAND").ok();
    let old_tail_v = env::var("RUSTMODLICA_TAIL_VELOCITY_DEADBAND").ok();
    let old_event_log = env::var("RUSTMODLICA_SUNDIALS_EVENT_LOG").ok();
    if quiet_mode == QuietMode::Events || quiet_mode == QuietMode::All {
        env::set_var("RUSTMODLICA_SUNDIALS_EVENT_LOG", "0");
    }
    let scan_result = (|| -> Result<EventScanOutput, RunError> {
        let mut models = Vec::new();
        let mut aggregate_map: std::collections::HashMap<(u64, u64), AggregateScanAccumulator> =
            std::collections::HashMap::new();

        let mut scored_model_count = 0usize;
        for model_name in &model_names {
            let mut compiler = Compiler::new();
            compiler.options.quiet = quiet_mode == QuietMode::All;
            compiler.options.t_end = t_end;
            compiler.options.dt = dt;
            compiler.options.output_interval = output_interval;
            for p in &lib_paths {
                compiler.loader.add_path(p.into());
            }
            let out = match compiler.compile(model_name) {
                Ok(v) => v,
                Err(e) => {
                    models.push(EventScanModelOutput {
                        model: model_name.clone(),
                        status: EventScanModelStatus::ConfigError,
                        supported_solvers: Vec::new(),
                        unsupported_reason: None,
                        config_error: Some(e.to_string()),
                        baseline_rk4: None,
                        baseline_rk45: None,
                        best: None,
                        topn: Vec::new(),
                    });
                    continue;
                }
            };
            let artifacts = match out {
                CompileOutput::Simulation(a) => a,
                CompileOutput::FunctionRun(_) => {
                    models.push(EventScanModelOutput {
                        model: model_name.clone(),
                        status: EventScanModelStatus::ConfigError,
                        supported_solvers: Vec::new(),
                        unsupported_reason: None,
                        config_error: Some(format!(
                            "event-scan requires a simulation model, got function '{}'",
                            model_name
                        )),
                        baseline_rk4: None,
                        baseline_rk45: None,
                        best: None,
                        topn: Vec::new(),
                    });
                    continue;
                }
                CompileOutput::FlatSnapshotDone => {
                    models.push(EventScanModelOutput {
                        model: model_name.clone(),
                        status: EventScanModelStatus::ConfigError,
                        supported_solvers: Vec::new(),
                        unsupported_reason: None,
                        config_error: Some(format!(
                            "event-scan cannot run with flat-snapshot-only output for model '{}'",
                            model_name
                        )),
                        baseline_rk4: None,
                        baseline_rk45: None,
                        best: None,
                        topn: Vec::new(),
                    });
                    continue;
                }
            };
            let (supported_solvers, unsupported_reason) = detect_supported_solvers(&artifacts);
            let baseline_rk4 = match run_collect_with_solver(&artifacts, "rk4") {
                Ok(v) => v,
                Err(e) => {
                    models.push(EventScanModelOutput {
                        model: model_name.clone(),
                        status: EventScanModelStatus::ConfigError,
                        supported_solvers,
                        unsupported_reason: None,
                        config_error: Some(e.to_string()),
                        baseline_rk4: None,
                        baseline_rk45: None,
                        best: None,
                        topn: Vec::new(),
                    });
                    continue;
                }
            };
            let baseline_rk45 = match run_collect_with_solver(&artifacts, "rk45") {
                Ok(v) => v,
                Err(e) => {
                    models.push(EventScanModelOutput {
                        model: model_name.clone(),
                        status: EventScanModelStatus::ConfigError,
                        supported_solvers,
                        unsupported_reason: None,
                        config_error: Some(e.to_string()),
                        baseline_rk4: Some(baseline_rk4),
                        baseline_rk45: None,
                        best: None,
                        topn: Vec::new(),
                    });
                    continue;
                }
            };
            let candidate_solvers: Vec<String> = supported_solvers
                .iter()
                .filter(|s| s.as_str() != "rk4" && s.as_str() != "rk45")
                .cloned()
                .collect();
            if candidate_solvers.is_empty() {
                models.push(EventScanModelOutput {
                    model: model_name.clone(),
                    status: EventScanModelStatus::Unsupported,
                    supported_solvers,
                    unsupported_reason,
                    config_error: None,
                    baseline_rk4: Some(baseline_rk4),
                    baseline_rk45: Some(baseline_rk45),
                    best: None,
                    topn: Vec::new(),
                });
                continue;
            }
            let mut rows = Vec::new();
            let mut model_error: Option<String> = None;
            for c in &count_values {
                for tv in &tail_v_values {
                    env::set_var("RUSTMODLICA_EVENT_COUNT_DEADBAND", c.to_string());
                    env::set_var("RUSTMODLICA_TAIL_VELOCITY_DEADBAND", tv.to_string());
                    let mut solver_metrics = Vec::new();
                    for solver_name in &candidate_solvers {
                        match run_collect_with_solver(&artifacts, solver_name) {
                            Ok(m) => solver_metrics.push(SolverScanMetrics {
                                solver: solver_name.clone(),
                                metrics: m,
                            }),
                            Err(e) => {
                                model_error = Some(format!(
                                    "solver '{}' failed for model '{}': {}",
                                    solver_name, model_name, e
                                ));
                                break;
                            }
                        }
                    }
                    if model_error.is_some() {
                        break;
                    }
                    let score_inputs: Vec<ScanMetrics> =
                        solver_metrics.iter().map(|v| v.metrics.clone()).collect();
                    let score = metric_score(&baseline_rk4, &score_inputs);
                    let key = (c.to_bits(), tv.to_bits());
                    let entry = aggregate_map
                        .entry(key)
                        .or_insert_with(|| AggregateScanAccumulator {
                            event_count_deadband: *c,
                            tail_velocity_deadband: *tv,
                            total_score: 0.0,
                            max_score: f64::NEG_INFINITY,
                            models_covered: 0,
                        });
                    entry.total_score += score;
                    if score > entry.max_score {
                        entry.max_score = score;
                    }
                    entry.models_covered += 1;
                    rows.push(ScanRow {
                        event_count_deadband: *c,
                        tail_velocity_deadband: *tv,
                        solver_metrics,
                        score,
                    });
                }
                if model_error.is_some() {
                    break;
                }
            }
            if let Some(err) = model_error {
                models.push(EventScanModelOutput {
                    model: model_name.clone(),
                    status: EventScanModelStatus::ConfigError,
                    supported_solvers,
                    unsupported_reason: None,
                    config_error: Some(err),
                    baseline_rk4: Some(baseline_rk4),
                    baseline_rk45: Some(baseline_rk45),
                    best: None,
                    topn: Vec::new(),
                });
                continue;
            }

            rows.sort_by(|a, b| {
                a.score
                    .partial_cmp(&b.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then_with(|| {
                        let a_sum: usize = a
                            .solver_metrics
                            .iter()
                            .map(|s| s.metrics.switch_count)
                            .sum();
                        let b_sum: usize = b
                            .solver_metrics
                            .iter()
                            .map(|s| s.metrics.switch_count)
                            .sum();
                        a_sum.cmp(&b_sum)
                    })
                    .then_with(|| a.event_count_deadband.to_bits().cmp(&b.event_count_deadband.to_bits()))
                    .then_with(|| a.tail_velocity_deadband.to_bits().cmp(&b.tail_velocity_deadband.to_bits()))
            });
            let best = rows.first().cloned();
            let topn = match aggregate_report_mode {
                AggregateReportMode::Full => rows.into_iter().take(top_n.max(1)).collect::<Vec<_>>(),
                AggregateReportMode::Compact => Vec::new(),
            };
            let status = if best
                .as_ref()
                .map(|b| b.score.is_finite())
                .unwrap_or(false)
            {
                EventScanModelStatus::Stable
            } else {
                EventScanModelStatus::Nondeterministic
            };
            scored_model_count += 1;
            models.push(EventScanModelOutput {
                model: model_name.clone(),
                status,
                supported_solvers,
                unsupported_reason: None,
                config_error: None,
                baseline_rk4: Some(baseline_rk4),
                baseline_rk45: Some(baseline_rk45),
                best,
                topn,
            });
        }

        let expected_model_count = scored_model_count;
        let mut aggregate_rows = aggregate_map
            .into_values()
            .filter(|v| expected_model_count > 0 && v.models_covered == expected_model_count)
            .map(|v| AggregateScanRow {
                event_count_deadband: v.event_count_deadband,
                tail_velocity_deadband: v.tail_velocity_deadband,
                total_score: v.total_score,
                avg_score: v.total_score / expected_model_count as f64,
                max_score: v.max_score,
                models_covered: v.models_covered,
            })
            .collect::<Vec<_>>();

        aggregate_rows.sort_by(|a, b| {
            let lhs = match aggregate_mode {
                AggregateMode::Sum => a.total_score,
                AggregateMode::Avg => a.avg_score,
                AggregateMode::Max => a.max_score,
            };
            let rhs = match aggregate_mode {
                AggregateMode::Sum => b.total_score,
                AggregateMode::Avg => b.avg_score,
                AggregateMode::Max => b.max_score,
            };
            lhs.partial_cmp(&rhs)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    a.total_score
                        .partial_cmp(&b.total_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| {
                    a.avg_score
                        .partial_cmp(&b.avg_score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| a.event_count_deadband.to_bits().cmp(&b.event_count_deadband.to_bits()))
                .then_with(|| {
                    a.tail_velocity_deadband
                        .to_bits()
                        .cmp(&b.tail_velocity_deadband.to_bits())
                })
        });

        let aggregate_best = aggregate_rows.first().cloned();
        let aggregate_topn = match aggregate_report_mode {
            AggregateReportMode::Full => aggregate_rows
                .into_iter()
                .take(top_n.max(1))
                .collect::<Vec<_>>(),
            AggregateReportMode::Compact => Vec::new(),
        };
        Ok(EventScanOutput {
            models,
            aggregate_best,
            aggregate_topn,
        })
    })();

    match old_count {
        Some(v) => env::set_var("RUSTMODLICA_EVENT_COUNT_DEADBAND", v),
        None => env::remove_var("RUSTMODLICA_EVENT_COUNT_DEADBAND"),
    }
    match old_tail_v {
        Some(v) => env::set_var("RUSTMODLICA_TAIL_VELOCITY_DEADBAND", v),
        None => env::remove_var("RUSTMODLICA_TAIL_VELOCITY_DEADBAND"),
    }
    match old_event_log {
        Some(v) => env::set_var("RUSTMODLICA_SUNDIALS_EVENT_LOG", v),
        None => env::remove_var("RUSTMODLICA_SUNDIALS_EVENT_LOG"),
    }

    let output = scan_result?;
    let json = serde_json::to_string_pretty(&output)
        .map_err(|e| format!("failed to serialize event-scan output: {}", e))?;
    if let Some(path) = output_file {
        std::fs::write(&path, json.as_bytes())
            .map_err(|e| format!("failed to write event-scan output file '{}': {}", path, e))?;
        let (best_c, best_tv) = output
            .aggregate_best
            .as_ref()
            .map(|b| (b.event_count_deadband, b.tail_velocity_deadband))
            .unwrap_or((f64::NAN, f64::NAN));
        println!(
            "event-scan wrote {} model results, aggregate best ({:.6}, {:.6}) to {}",
            output.models.len(),
            best_c,
            best_tv,
            path
        );
    } else {
        println!("{}", json);
    }
    Ok(())
}

fn run(args: Vec<String>) -> Result<(), RunError> {
    if args.len() >= 2 && args[1] == "event-scan" {
        return run_event_scan(&args);
    }
    let mut backend_dae_info = false;
    let mut index_reduction_method = "none".to_string();
    let mut tearing_method = "first".to_string();
    let mut generate_dynamic_jacobian = "none".to_string();
    let mut t_end = 10.0_f64;
    let mut dt = 0.01_f64;
    let mut atol = 1e-6_f64;
    let mut rtol = 1e-3_f64;
    let mut function_args: Option<Vec<f64>> = None;
    let mut solver = "rk45".to_string();
    let mut output_interval = 0.05_f64;
    let mut result_file: Option<String> = None;
    let mut warnings_level = "all".to_string();
    let mut emit_c_dir: Option<String> = None;
    let mut emit_fmu_dir: Option<String> = None;
    let mut emit_fmu_me_dir: Option<String> = None;
    let mut external_libs: Vec<String> = Vec::new();
    let mut lib_paths: Vec<String> = Vec::new();
    let mut repl = false;
    let mut script_path: Option<String> = None;
    let mut model_name = None;
    let mut validate_only = false;
    let mut output_format: Option<String> = None;
    let mut perf_json_path: Option<String> = None;
    let mut emit_flat_snapshot: Option<String> = None;
    let mut flat_snapshot_only = false;
    let mut coarse_constrainedby_only = false;
    let mut i = 1;
    while i < args.len() {
        let a = &args[i];
        if let Some(lang) = a.strip_prefix("--lang=") {
            let _ = std::env::set_var("RUSTMODLICA_LANG", lang);
            i += 1;
        } else if a == "--backend-dae-info" {
            backend_dae_info = true;
            i += 1;
        } else if a == "-d" && i + 1 < args.len() {
            if args[i + 1] == "backenddaeinfo" {
                backend_dae_info = true;
            }
            i += 2;
        } else if a.starts_with("-d=")
            && a.strip_prefix("-d=")
                .map(|s| s == "backenddaeinfo")
                .unwrap_or(false)
        {
            backend_dae_info = true;
            i += 1;
        } else if let Some(v) = a.strip_prefix("--index-reduction-method=") {
            index_reduction_method = v.to_string();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--tearing-method=") {
            tearing_method = v.to_string();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--generate-dynamic-jacobian=") {
            generate_dynamic_jacobian = v.to_string();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--t-end=") {
            t_end = v.parse().unwrap_or(10.0);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--dt=") {
            dt = v.parse().unwrap_or(0.01);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--atol=") {
            atol = v.parse().unwrap_or(1e-6);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--rtol=") {
            rtol = v.parse().unwrap_or(1e-3);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--function-args=") {
            let parsed: Result<Vec<f64>, _> =
                v.split(',').map(|s| s.trim().parse::<f64>()).collect();
            function_args = parsed.ok();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--solver=") {
            solver = v.to_string();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--output-interval=") {
            output_interval = v.parse().unwrap_or(0.05);
            i += 1;
        } else if let Some(v) = a.strip_prefix("--result-file=") {
            result_file = Some(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--emit-flat-snapshot=") {
            emit_flat_snapshot = Some(v.to_string());
            i += 1;
        } else if a == "--flat-snapshot-only" {
            flat_snapshot_only = true;
            i += 1;
        } else if a == "--coarse-constrainedby" {
            coarse_constrainedby_only = true;
            i += 1;
        } else if let Some(v) = a.strip_prefix("--warnings=") {
            warnings_level = v.to_string();
            i += 1;
        } else if let Some(v) = a.strip_prefix("--emit-c=") {
            emit_c_dir = Some(v.to_string());
            i += 1;
        } else if a == "--repl" {
            repl = true;
            i += 1;
        } else if let Some(v) = a.strip_prefix("--script=") {
            script_path = Some(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--emit-fmu=") {
            emit_fmu_dir = Some(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--emit-fmu-me=") {
            emit_fmu_me_dir = Some(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--external-lib=") {
            external_libs.push(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--lib-path=") {
            if !v.trim().is_empty() {
                lib_paths.push(v.to_string());
            }
            i += 1;
        } else if let Some(v) = a.strip_prefix("--modelica-stdlib=") {
            if !v.trim().is_empty() {
                lib_paths.push(v.to_string());
            }
            i += 1;
        } else if a == "--validate" {
            validate_only = true;
            i += 1;
        } else if a == "-" {
            model_name = Some("-".to_string());
            i += 1;
            break;
        } else if let Some(v) = a.strip_prefix("--output-format=") {
            output_format = Some(v.to_string());
            i += 1;
        } else if let Some(v) = a.strip_prefix("--perf-json=") {
            perf_json_path = Some(v.to_string());
            i += 1;
        } else if !a.starts_with('-') {
            model_name = Some(a.clone());
            i += 1;
            break;
        } else {
            return Err(format!("unknown argument: {}", a).into());
        }
    }
    if i < args.len() {
        return Err("Unknown or extra arguments after model name.".into());
    }
    if let Some(ref path) = script_path {
        let mut compiler = Compiler::new();
        compiler.options.backend_dae_info = backend_dae_info;
        compiler.options.index_reduction_method = index_reduction_method;
        compiler.options.tearing_method = tearing_method;
        compiler.options.generate_dynamic_jacobian = generate_dynamic_jacobian;
        compiler.options.t_end = t_end;
        compiler.options.dt = dt;
        compiler.options.atol = atol;
        compiler.options.rtol = rtol;
        compiler.options.function_args = function_args;
        compiler.options.solver = solver;
        compiler.options.output_interval = output_interval;
        compiler.options.result_file = result_file;
        compiler.options.warnings_level = warnings_level;
        compiler.options.emit_c_dir = emit_c_dir.clone();
        compiler.options.external_libs = external_libs;
        for p in &lib_paths {
            compiler.loader.add_path(p.into());
        }
        compiler.loader.add_path(".".into());
        compiler.loader.add_path("StandardLib".into());
        compiler.loader.add_path("TestLib".into());
        compiler.loader.add_path("ModelicaTest".into());
        let mut runner = script::ScriptRunner::new(compiler);
        if path == "-" {
            runner.run_script_named(std::io::stdin(), "<stdin>")?;
        } else {
            let f = std::fs::File::open(path)
                .map_err(|e| format!("Cannot open script file {}: {}", path, e))?;
            runner.run_script_named(f, path)?;
        }
        return Ok(());
    }
    let model_name = match model_name {
        Some(n) => n,
        None => {
            let msg = format!(
                "Usage: {} [options] <model_name>\n  event-scan [scan-options] [model_name]\n\n  --lang=en|zh  message language\n  --validate  compile only, output JSON to stdout\n  --output-format=json  simulation: output time series as JSON to stdout\n  --perf-json=<path>  write structured compile perf report JSON\n  --solver=rk4|rk45|implicit|cvode|ida  (cvode/ida need --features sundials; default: rk45)\n  --warnings=all|none|error  (default: all)\n  --backend-dae-info  print backend DAE statistics\n  --index-reduction-method=<none|dummyDerivative|pantelides|pantelidesDummy|debugPrint>\n  --t-end=<float>  --dt=<float>  --atol=<float>  --rtol=<float>\n  --output-interval=<float>  (default 0.05)\n  --result-file=<path>  write CSV time series to file\n  --emit-flat-snapshot=<path>  Tier S flat JSON after flatten (before inline)\n  --flat-snapshot-only  stop after snapshot (requires --emit-flat-snapshot)\n  --coarse-constrainedby  legacy constrainedby check instead of extends-closure\n  --emit-c=<dir>  emit C source (model.c, model.h) to directory\n  --repl  after compile, enter REPL (inspect vars, simulate, quit)\n  --script=<path>  run .mos script (AST parser + strict executor by default); use - for stdin\n  Env: RUSTMODLICA_SCRIPT_ENGINE=mos|legacy; RUSTMODLICA_NEWTON_SPARSE_POLICY=auto|dense|sparse; RUSTMODLICA_STRICT_NEWTON=1\n  --emit-fmu=<dir>  emit C + modelDescription.xml + fmi2_cs.c for FMI 2.0 CS\n  --emit-fmu-me=<dir>  emit C + modelDescription.xml + fmi2_me.c for FMI 2.0 ME\n  --external-lib=<path>  load shared library for external function symbols (EXT-1; repeatable)\n  --lib-path=<dir>  add a Modelica library root to the loader search path (repeatable)\n  --modelica-stdlib=<dir>  alias of --lib-path\n  --function-args=<f1,f2,...>  function input values\n\n  event-scan options:\n  --model=<name>  single target model (default: BouncingBall)\n  --models=<m1,m2,...>  multi-model batch scan\n  --lib-path=<dir>  repeatable model library roots\n  --t-end=<float> --dt=<float> --output-interval=<float>\n  --count-values=<v1,v2,...>  values for RUSTMODLICA_EVENT_COUNT_DEADBAND\n  --tail-velocity-values=<v1,v2,...>  values for RUSTMODLICA_TAIL_VELOCITY_DEADBAND\n  --aggregate-mode=sum|avg|max  aggregate sort strategy (default: sum)\n  --aggregate-report=full|compact  output detail level (default: full)\n  --output-file=<path>  write JSON result to file (stdout prints summary)\n  --quiet  alias of --quiet=all\n  --quiet=none|events|all  control scan logging granularity\n  --top-n=<N>  output top N combinations per model and aggregate\n\n  Use model_name '-' to read Modelica source from stdin.",
                args[0]
            );
            return Err(msg.into());
        }
    };
    let mut compiler = Compiler::new();
    compiler.options.backend_dae_info = backend_dae_info;
    compiler.options.index_reduction_method = index_reduction_method;
    compiler.options.tearing_method = tearing_method;
    compiler.options.generate_dynamic_jacobian = generate_dynamic_jacobian;
    compiler.options.t_end = t_end;
    compiler.options.dt = dt;
    compiler.options.atol = atol;
    compiler.options.rtol = rtol;
    compiler.options.function_args = function_args;
    compiler.options.solver = solver;
    compiler.options.output_interval = output_interval;
    compiler.options.result_file = result_file;
    compiler.options.warnings_level = warnings_level;
    compiler.options.emit_flat_snapshot = emit_flat_snapshot.clone();
    compiler.options.flat_snapshot_only = flat_snapshot_only;
    compiler.options.coarse_constrainedby_only = coarse_constrainedby_only;
    if emit_fmu_dir.is_some() && emit_c_dir.is_none() {
        emit_c_dir = emit_fmu_dir.clone();
    }
    if emit_fmu_me_dir.is_some() && emit_c_dir.is_none() {
        emit_c_dir = emit_fmu_me_dir.clone();
    }
    compiler.options.emit_c_dir = emit_c_dir;
    compiler.options.external_libs = external_libs;
    let run_repl = repl;
    let json_mode = validate_only || output_format.as_deref() == Some("json");
    if validate_only {
        compiler.options.quiet = true;
    }
    for p in &lib_paths {
        compiler.loader.add_path(p.into());
    }
    compiler.loader.add_path(".".into());
    compiler.loader.add_path("StandardLib".into());
    compiler.loader.add_path("TestLib".into());
    compiler.loader.add_path("ModelicaTest".into());

    let effective_model = if model_name == "-" {
        let mut code = String::new();
        std::io::stdin()
            .read_to_string(&mut code)
            .map_err(|e| format!("Failed to read stdin: {}", e))?;
        compiler
            .loader
            .load_model_from_source("<stdin>", &code)
            .map_err(|e| format!("{}", e))?;
        "<stdin>".to_string()
    } else {
        model_name.clone()
    };

    if flat_snapshot_only && emit_flat_snapshot.is_none() {
        return Err("--flat-snapshot-only requires --emit-flat-snapshot=<path>".into());
    }

    if !json_mode {
        println!(
            "{}",
            i18n::msg("compiling", &[&effective_model as &dyn std::fmt::Display])
        );
    }
    let perf_enabled = std::env::var("RUSTMODLICA_PERF_TRACE")
        .ok()
        .map(|v| {
            let t = v.trim();
            t == "1" || t.eq_ignore_ascii_case("true") || t.eq_ignore_ascii_case("yes")
        })
        .unwrap_or(false);
    let compile_t0 = if perf_enabled { Some(Instant::now()) } else { None };
    let out = match compiler.compile(&effective_model) {
        Ok(o) => o,
        Err(e) => {
            let warnings = compiler.take_warnings();
            if validate_only {
                emit_validate_json(false, &warnings, &[e.to_string()], &[], &[]);
                return Ok(());
            }
            return Err(e.into());
        }
    };
    if let Some(t0) = compile_t0 {
        eprintln!("[perf] compile_ms={}", t0.elapsed().as_millis());
    }
    let warnings = compiler.take_warnings();
    maybe_write_perf_json(&perf_json_path, &mut compiler, &effective_model, warnings.len())?;
    let warn_level = compiler.options.warnings_level.as_str();
    if validate_only {
        match &out {
            CompileOutput::FunctionRun(_) => {
                emit_validate_json(true, &warnings, &[], &[], &[]);
            }
            CompileOutput::Simulation(artifacts) => {
                emit_validate_json(
                    true,
                    &warnings,
                    &[],
                    &artifacts.state_vars,
                    &artifacts.output_vars,
                );
            }
            CompileOutput::FlatSnapshotDone => {
                emit_validate_json(true, &warnings, &[], &[], &[]);
            }
        }
        return Ok(());
    }
    if warn_level != "none" {
        for w in &warnings {
            if warn_level == "error" {
                return Err(w.to_string().into());
            }
            eprintln!("{}", w);
        }
        if !warnings.is_empty() && warn_level == "all" {
            eprintln!("{}", i18n::msg("warnings_generated", &[&warnings.len()]));
        }
    }
    match out {
        CompileOutput::FlatSnapshotDone => {
            if !json_mode {
                println!("Tier S flat snapshot written.");
            }
            return Ok(());
        }
        CompileOutput::FunctionRun(value) => {
            if !json_mode {
                println!("{}", i18n::msg("result", &[&value]));
            }
            return Ok(());
        }
        CompileOutput::Simulation(artifacts) => {
            println!("{}", i18n::msg0("compilation_successful"));
            println!("{}", i18n::msg("states", &[&artifacts.states.len()]));
            println!(
                "{}",
                i18n::msg("discrete_vars", &[&artifacts.discrete_vars.len()])
            );
            println!("{}", i18n::msg("parameters", &[&artifacts.params.len()]));
            println!("{}", i18n::msg("outputs", &[&artifacts.output_vars.len()]));
            println!("{}", i18n::msg("when_statements", &[&artifacts.when_count]));
            println!(
                "{}",
                i18n::msg("zero_crossings", &[&artifacts.crossings_count])
            );
            if let Some(ref dir) = emit_fmu_dir {
                let path = std::path::Path::new(dir);
                match fmi::emit_fmu_artifacts(
                    path,
                    &effective_model,
                    &artifacts.state_vars,
                    &artifacts.param_vars,
                    &artifacts.output_vars,
                    0.0,
                    artifacts.t_end,
                    artifacts.dt,
                ) {
                    Ok(files) => {
                        let paths: Vec<String> =
                            files.iter().map(|p| p.display().to_string()).collect();
                        println!("FMI CS: emitted {}", paths.join(", "));
                    }
                    Err(e) => return Err(format!("FMI CS emit failed: {}", e).into()),
                }
            }
            if let Some(ref dir) = emit_fmu_me_dir {
                let path = std::path::Path::new(dir);
                match fmi::emit_fmu_me_artifacts(
                    path,
                    &effective_model,
                    &artifacts.state_vars,
                    &artifacts.param_vars,
                    &artifacts.output_vars,
                    0.0,
                    artifacts.t_end,
                    artifacts.dt,
                ) {
                    Ok(files) => {
                        let paths: Vec<String> =
                            files.iter().map(|p| p.display().to_string()).collect();
                        println!("FMI ME: emitted {}", paths.join(", "));
                    }
                    Err(e) => return Err(format!("FMI ME emit failed: {}", e).into()),
                }
            }
            if run_repl {
                run_repl_loop(artifacts)?;
                return Ok(());
            }
            if emit_fmu_dir.is_some() || emit_fmu_me_dir.is_some() {
                return Ok(());
            }
            if output_format.as_deref() == Some("json") {
                let sim_t0 = if perf_enabled { Some(Instant::now()) } else { None };
                let result = run_simulation_collect(
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
                )?;
                if let Some(t0) = sim_t0 {
                    eprintln!("[perf] sim_ms={}", t0.elapsed().as_millis());
                }
                println!("{}", serde_json::to_string(&result).unwrap_or_default());
                return Ok(());
            }
            if !json_mode {
                println!("{}", i18n::msg0("starting_simulation"));
            }
            let sim_t0 = if perf_enabled { Some(Instant::now()) } else { None };
            run_simulation(
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
                artifacts.result_file.as_deref(),
                &artifacts.clock_partition_schedule,
                None,
            )?;
            if let Some(t0) = sim_t0 {
                eprintln!("[perf] sim_ms={}", t0.elapsed().as_millis());
            }
            if !json_mode {
                println!("{}", i18n::msg0("simulation_completed"));
            }
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    const STACK_SIZE: usize = 8 * 1024 * 1024;
    let child = thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(move || run(args))
        .map_err(|e| error::AppError::ThreadSpawn(e.to_string()));
    let child = match child {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{}", e);
            return ExitCode::from(1);
        }
    };
    match child.join() {
        Err(_) => {
            eprintln!("{}", error::AppError::ThreadPanic);
            ExitCode::from(1)
        }
        Ok(Err(e)) => {
            eprintln!("{}", e);
            ExitCode::from(1)
        }
        Ok(Ok(())) => ExitCode::from(0),
    }
}
