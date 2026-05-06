fn make_run_id() -> String {
    format!("run_{}", chrono::Utc::now().format("%Y%m%dT%H%M%S%.3fZ"))
}

fn append_event_dual(
    primary: &PathBuf,
    run_scoped: &PathBuf,
    env: &EventEnvelope,
) -> Result<()> {
    append_event_line(primary, env)?;
    append_event_line(run_scoped, env)?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn run_cmd(
    config_path: PathBuf,
    tier: Option<String>,
    tags: Option<Vec<String>>,
    workers: Option<usize>,
    baseline_cli: Option<PathBuf>,
    incremental_cli: Option<String>,
    manifest_cli: Option<PathBuf>,
    data_root_cli: PathBuf,
    out_dir_legacy: Option<PathBuf>,
    ndjson: bool,
    summary_compat: bool,
    progress: bool,
) -> Result<()> {
    run_cmd_impl(
        config_path,
        tier,
        tags,
        workers,
        baseline_cli,
        incremental_cli,
        manifest_cli,
        data_root_cli,
        out_dir_legacy,
        ndjson,
        summary_compat,
        progress,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
#[allow(dead_code)]
pub(crate) fn run_cmd_tui(
    config_path: PathBuf,
    tier: Option<String>,
    tags: Option<Vec<String>>,
    workers: Option<usize>,
    baseline_cli: Option<PathBuf>,
    incremental_cli: Option<String>,
    manifest_cli: Option<PathBuf>,
    data_root_cli: PathBuf,
    out_dir_legacy: Option<PathBuf>,
    ndjson: bool,
    summary_compat: bool,
) -> Result<()> {
    run_cmd_impl(
        config_path,
        tier,
        tags,
        workers,
        baseline_cli,
        incremental_cli,
        manifest_cli,
        data_root_cli,
        out_dir_legacy,
        ndjson,
        summary_compat,
        false,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn run_cmd_impl(
    config_path: PathBuf,
    tier: Option<String>,
    tags: Option<Vec<String>>,
    workers: Option<usize>,
    baseline_cli: Option<PathBuf>,
    incremental_cli: Option<String>,
    manifest_cli: Option<PathBuf>,
    data_root_cli: PathBuf,
    out_dir_legacy: Option<PathBuf>,
    ndjson: bool,
    summary_compat: bool,
    progress: bool,
    emit_console: bool,
) -> Result<()> {
    let filter = Filter { tier, tags };
    if emit_console {
        ui::print_section(i18n::tr("section_prepare_run"));
    }
    let prep = prepare_session(
        config_path.clone(),
        filter.clone(),
        data_root_cli,
        out_dir_legacy,
        baseline_cli,
        incremental_cli,
        manifest_cli,
    )?;

    let repo_root = prep.repo_root;
    let data_root = prep.data_root;
    let artifact_dir = prep.artifact_dir;
    let cfg = prep.cfg;
    let inc = prep.incremental;
    let ordered_ids = prep.ordered_case_ids;
    let baseline_report = prep.baseline_report;
    let plan = prep.plan;

    let workers = workers.unwrap_or(cfg.execution.workers).max(1);
    let fail_fast = cfg.execution.fail_fast;
    let fail_flag = AtomicBool::new(false);
    let run_id = make_run_id();
    let resolved_options = merge_run_options(
        &CliOverrides {
            workers: Some(workers),
            solver: None,
            t_end: None,
            dt: None,
            fail_fast: Some(fail_fast),
            incremental: None,
        },
        None,
        &cfg,
        run_id.clone(),
    );
    validate_run_options(&resolved_options)?;
    write_run_options_snapshot(&run_id, &resolved_options, &data_root)?;

    let ctx = RunContext {
        repo_root: &repo_root,
        defaults: &cfg.defaults,
        out_dir: &artifact_dir,
    };

    let ndjson_path = data_root.join("cases.ndjson");
    let events_path = data_root.join("events.ndjson");
    let runs_root = data_root.join(".regress").join("runs");
    let run_dir = runs_root.join(&run_id);
    std::fs::create_dir_all(&run_dir)?;
    let run_ndjson_path = run_dir.join("cases.ndjson");
    let run_events_path = run_dir.join("events.ndjson");
    let run_report_path = run_dir.join("report.json");
    ensure_event_file(&events_path)?;
    ensure_event_file(&run_events_path)?;
    if ndjson && ndjson_path.exists() {
        std::fs::remove_file(&ndjson_path)?;
    }
    let mut event_seq = 1u64;
    append_event_dual(
        &events_path,
        &run_events_path,
        &EventEnvelope::new(
            run_id.clone(),
            event_seq,
            ExecutionEvent::RunStarted {
                total: plan.iter().filter(|x| matches!(x, PlanEntry::Run(_))).count(),
            },
        ),
    )?;

    let mut fresh: HashMap<String, regress_harness::report::CaseResult> = HashMap::new();

    let run_jobs: Vec<regress_harness::config::CaseDef> = plan
        .iter()
        .filter_map(|e| match e {
            PlanEntry::Run(c) => Some(c.clone()),
            _ => None,
        })
        .collect();
    let total_jobs = run_jobs.len();
    for case in &run_jobs {
        event_seq += 1;
        append_event_dual(
            &events_path,
            &run_events_path,
            &EventEnvelope::new(
                run_id.clone(),
                event_seq,
                ExecutionEvent::CaseQueued {
                    case_id: case.id.clone(),
                },
            ),
        )?;
    }

    let skipped: Vec<regress_harness::report::CaseResult> = plan
        .iter()
        .filter_map(|e| match e {
            PlanEntry::SkippedUnchanged(c) | PlanEntry::SkippedScope(c) => Some(c.clone()),
            _ => None,
        })
        .collect();
    for s in skipped {
        fresh.insert(s.case_id.clone(), s);
    }

    use rayon::prelude::*;
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .context("thread pool")?;

    let started = Instant::now();
    let pb = if progress {
        let bar = ProgressBar::new(run_jobs.len() as u64);
        if let Ok(style) = ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
        ) {
            bar.set_style(style.progress_chars("#>-"));
        }
        Some(std::sync::Arc::new(bar))
    } else {
        None
    };
    let run_results: Vec<regress_harness::report::CaseResult> = if fail_fast {
        let mut v: Vec<regress_harness::report::CaseResult> = Vec::new();
        let mut passed = 0usize;
        let mut failed = 0usize;
        let mut skipped_cnt = 0usize;
        for (job_idx, case) in run_jobs.iter().enumerate() {
            event_seq += 1;
            append_event_dual(
                &events_path,
                &run_events_path,
                &EventEnvelope::new(
                    run_id.clone(),
                    event_seq,
                    ExecutionEvent::CaseStarted {
                        case_id: case.id.clone(),
                        worker: 0,
                    },
                ),
            )?;
            let traced = regress_harness::runner::run_case_with_trace(&ctx, case);
            for phase in &traced.phases {
                event_seq += 1;
                append_event_dual(
                    &events_path,
                    &run_events_path,
                    &EventEnvelope::new(
                        run_id.clone(),
                        event_seq,
                        ExecutionEvent::CasePhase {
                            case_id: case.id.clone(),
                            phase: phase.phase,
                        },
                    ),
                )?;
            }
            for log in traced.logs.iter().take(8) {
                event_seq += 1;
                append_event_dual(
                    &events_path,
                    &run_events_path,
                    &EventEnvelope::new(
                        run_id.clone(),
                        event_seq,
                        ExecutionEvent::CaseLog {
                            case_id: case.id.clone(),
                            level: log.level,
                            message: log.message.clone(),
                        },
                    ),
                )?;
            }
            let r = traced.result;
            if progress {
                eprintln!(
                    "[regress-harness] done {}/{} case_id={} status={:?} duration_ms={}",
                    job_idx + 1,
                    total_jobs,
                    r.case_id,
                    r.status,
                    r.duration_ms
                );
            }
            if let Some(bar) = &pb {
                bar.inc(1);
                bar.set_message(r.case_id.clone());
            }
            let status = r.status.clone();
            event_seq += 1;
            append_event_dual(
                &events_path,
                &run_events_path,
                &EventEnvelope::new(
                    run_id.clone(),
                    event_seq,
                    ExecutionEvent::CaseFinished {
                        case_id: r.case_id.clone(),
                        status: status.clone(),
                        duration_ms: r.duration_ms,
                        classification: r.classification.clone(),
                    },
                ),
            )?;
            if status == regress_harness::report::CaseStatus::Pass {
                passed += 1;
            } else if status == regress_harness::report::CaseStatus::Fail {
                failed += 1;
            } else if status == regress_harness::report::CaseStatus::SkippedUnchanged {
                skipped_cnt += 1;
            }
            event_seq += 1;
            append_event_dual(
                &events_path,
                &run_events_path,
                &EventEnvelope::new(
                    run_id.clone(),
                    event_seq,
                    ExecutionEvent::RunProgress {
                        completed: v.len() + 1,
                        passed,
                        failed,
                        skipped: skipped_cnt,
                    },
                ),
            )?;
            if status == regress_harness::report::CaseStatus::Fail {
                fail_flag.store(true, Ordering::Relaxed);
            }
            v.push(r);
            if fail_flag.load(Ordering::Relaxed) {
                event_seq += 1;
                append_event_dual(
                    &events_path,
                    &run_events_path,
                    &EventEnvelope::new(
                        run_id.clone(),
                        event_seq,
                        ExecutionEvent::RunAborted {
                            reason: "fail_fast triggered".to_string(),
                        },
                    ),
                )?;
                break;
            }
        }
        v
    } else {
        let progress_done = std::sync::Arc::new(AtomicUsize::new(0));
        let traces: Vec<regress_harness::runner::CaseRunTrace> = pool.install(|| {
            let pbar = pb.clone();
            let progress_done = std::sync::Arc::clone(&progress_done);
            run_jobs
                .par_iter()
                .map(|case| {
                    let traced = regress_harness::runner::run_case_with_trace(&ctx, case);
                    if progress {
                        let n = progress_done.fetch_add(1, Ordering::Relaxed) + 1;
                        eprintln!(
                            "[regress-harness] done {}/{} case_id={} status={:?} duration_ms={}",
                            n,
                            total_jobs,
                            traced.result.case_id,
                            traced.result.status,
                            traced.result.duration_ms
                        );
                    }
                    if let Some(bar) = &pbar {
                        bar.inc(1);
                        bar.set_message(traced.result.case_id.clone());
                    }
                    traced
                })
                .collect()
        });
        let mut out = Vec::with_capacity(traces.len());
        let mut completed = 0usize;
        let mut passed = 0usize;
        let mut failed = 0usize;
        let mut skipped_cnt = 0usize;
        for traced in traces {
            let case_id = traced.result.case_id.clone();
            event_seq += 1;
            append_event_dual(
                &events_path,
                &run_events_path,
                &EventEnvelope::new(
                    run_id.clone(),
                    event_seq,
                    ExecutionEvent::CaseStarted {
                        case_id: case_id.clone(),
                        worker: workers,
                    },
                ),
            )?;
            for phase in traced.phases {
                event_seq += 1;
                append_event_dual(
                    &events_path,
                    &run_events_path,
                    &EventEnvelope::new(
                        run_id.clone(),
                        event_seq,
                        ExecutionEvent::CasePhase {
                            case_id: case_id.clone(),
                            phase: phase.phase,
                        },
                    ),
                )?;
            }
            for log in traced.logs.into_iter().take(8) {
                event_seq += 1;
                append_event_dual(
                    &events_path,
                    &run_events_path,
                    &EventEnvelope::new(
                        run_id.clone(),
                        event_seq,
                        ExecutionEvent::CaseLog {
                            case_id: case_id.clone(),
                            level: log.level,
                            message: log.message,
                        },
                    ),
                )?;
            }
            let status = traced.result.status.clone();
            event_seq += 1;
            append_event_dual(
                &events_path,
                &run_events_path,
                &EventEnvelope::new(
                    run_id.clone(),
                    event_seq,
                    ExecutionEvent::CaseFinished {
                        case_id: case_id.clone(),
                        status: status.clone(),
                        duration_ms: traced.result.duration_ms,
                        classification: traced.result.classification.clone(),
                    },
                ),
            )?;
            completed += 1;
            if status == regress_harness::report::CaseStatus::Pass {
                passed += 1;
            } else if status == regress_harness::report::CaseStatus::Fail {
                failed += 1;
            } else if status == regress_harness::report::CaseStatus::SkippedUnchanged {
                skipped_cnt += 1;
            }
            event_seq += 1;
            append_event_dual(
                &events_path,
                &run_events_path,
                &EventEnvelope::new(
                    run_id.clone(),
                    event_seq,
                    ExecutionEvent::RunProgress {
                        completed,
                        passed,
                        failed,
                        skipped: skipped_cnt,
                    },
                ),
            )?;
            out.push(traced.result);
        }
        out
    };
    if let Some(bar) = &pb {
        bar.finish_with_message("done");
    }
    for r in &run_results {
        fresh.insert(r.case_id.clone(), r.clone());
    }
    if ndjson {
        for r in &run_results {
            append_ndjson_line(&ndjson_path, r).context("ndjson append")?;
        }
    }
    for r in &run_results {
        append_ndjson_line(&run_ndjson_path, r).context("run ndjson append")?;
    }
    let _elapsed = started.elapsed();

    let merged = merge_ordered(baseline_report.as_ref(), fresh, &ordered_ids);

    let strat_name = format!("{:?}", inc.strategy);
    let mut report = Report {
        schema_version: 1,
        generated_at: chrono::Utc::now().to_rfc3339(),
        repo_root: repo_root.to_string_lossy().to_string(),
        config_path: config_path.to_string_lossy().to_string(),
        tier_filter: filter.tier.clone(),
        tags_filter: filter.tags.clone(),
        incremental_strategy: strat_name,
        summary: Default::default(),
        cases: merged,
    };
    report.summarize();

    let report_path = data_root.join("report.json");
    write_report_json(&report_path, &report)?;
    write_report_json(&run_report_path, &report)?;

    let manifest_out = RegressManifest {
        version: 1,
        created_at: report.generated_at.clone(),
        config_path: report.config_path.clone(),
        tier_filter: filter.tier.clone(),
        tags_filter: filter.tags.clone(),
        case_ids: ordered_ids.clone(),
    };
    write_manifest(&data_root.join("regress_manifest.json"), &manifest_out)?;

    if summary_compat {
        write_summary_compat(&data_root.join("summary_compat.txt"), &report)?;
    }

    if report.summary.failed > 0 {
        let ev = EventEnvelope::new(
            run_id.clone(),
            event_seq + 1,
            ExecutionEvent::RunFinished {
                summary: RunSummaryLite {
                    total: report.summary.total,
                    passed: report.summary.passed,
                    failed: report.summary.failed,
                    skipped: report.summary.skipped,
                },
            },
        );
        let _ = append_event_dual(&events_path, &run_events_path, &ev);
        let _ = std::fs::create_dir_all(&runs_root);
        let _ = std::fs::write(runs_root.join("latest"), run_id.as_bytes());
        bail!(
            "regression failed: {} failed, {} passed, {} skipped",
            report.summary.failed,
            report.summary.passed,
            report.summary.skipped
        );
    }
    event_seq += 1;
    append_event_dual(
        &events_path,
        &run_events_path,
        &EventEnvelope::new(
            run_id.clone(),
            event_seq,
            ExecutionEvent::RunFinished {
                summary: RunSummaryLite {
                    total: report.summary.total,
                    passed: report.summary.passed,
                    failed: report.summary.failed,
                    skipped: report.summary.skipped,
                },
            },
        ),
    )?;
    std::fs::write(runs_root.join("latest"), run_id.as_bytes())?;
    if emit_console {
        ui::print_ok(&ui::run_done_line(
            report.summary.passed,
            report.summary.failed,
            report.summary.skipped,
        ));
    }
    Ok(())
}
