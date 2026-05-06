use rustmodlica::{
    run_simulation, Artifacts, CompileOutput, CompileStopPhase, Compiler,
};
use serde::Serialize;
use std::env;

use super::RunError;

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

fn run_collect_with_solver(artifacts: &Artifacts, solver: &str, model_name: &str, lib_paths: Vec<std::path::PathBuf>) -> Result<ScanMetrics, RunError> {
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
        None,
        model_name,
        lib_paths,
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

pub(crate) fn run_event_scan(args: &[String]) -> Result<(), RunError> {
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
            compiler.options.compile_stop = CompileStopPhase::Full;
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
                CompileOutput::ValidationParseOk
                | CompileOutput::ValidationFlattenOk { .. }
                | CompileOutput::ValidationAnalyzed(_) => {
                    models.push(EventScanModelOutput {
                        model: model_name.clone(),
                        status: EventScanModelStatus::ConfigError,
                        supported_solvers: Vec::new(),
                        unsupported_reason: None,
                        config_error: Some(format!(
                            "event-scan requires full compile; model '{}' used tiered compile stop",
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
            let baseline_rk4 = match run_collect_with_solver(&artifacts, "rk4", model_name, lib_paths.iter().map(|s| std::path::PathBuf::from(s)).collect()) {
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
            let baseline_rk45 = match run_collect_with_solver(&artifacts, "rk45", model_name, lib_paths.iter().map(|s| std::path::PathBuf::from(s)).collect()) {
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
                        match run_collect_with_solver(&artifacts, solver_name, model_name, lib_paths.iter().map(|s| std::path::PathBuf::from(s)).collect()) {
                            Ok(m) => solver_metrics.push(SolverScanMetrics {
                                solver: solver_name.clone(),
                                metrics: m,
                            }),
                            Err(e) => {
                                model_error = Some(format!(
                                    "solver '{}' failed for model '{}' at count_deadband={} tail_velocity_deadband={}: {}",
                                    solver_name, model_name, c, tv, e
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
