use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use regress_harness::incremental::{merge_ordered, PlanEntry};
use regress_harness::report::{
    append_ndjson_line, write_manifest, write_report_json, write_summary_compat, RegressManifest,
    Report,
};
use regress_harness::runtime::events::{EventEnvelope, ExecutionEvent, RunSummaryLite};
use regress_harness::runtime::errors::{anyhow_to_issue, resolve_exit_code};
use regress_harness::runtime::monitor::{append_event_line, ensure_event_file};
use regress_harness::runtime::options::{
    merge_run_options, validate_run_options, write_run_options_snapshot, CliOverrides,
};
use regress_harness::runner::RunContext;
use regress_harness::session_prep::prepare_session;
use regress_harness::tiers::Filter;
use indicatif::{ProgressBar, ProgressStyle};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

mod commands;
mod agent_repl;
mod i18n;
mod repl;
mod ps1_profiles;
mod scope;
mod sync_tools;
mod fmi_tools;
mod sparse_dense;
mod event_scan_matrix;
mod coverage_status;
mod ui;
mod jit_validate;
mod jit_phase1;
mod jit_script_mode;

use commands::{
    cmd_agent_context, cmd_list_cases, cmd_monitor, cmd_plan, cmd_status, cmd_validate_config,
    parse_monitor_source, OutputFormat,
};

#[derive(Parser, Debug)]
#[command(
    name = "regress-harness",
    version,
    about = "JSON-driven regression runner."
)]
struct Cli {
    /// Language: en | zh-CN (also supports env RUSTMODLICA_LANG)
    #[arg(long, global = true)]
    lang: Option<String>,
    /// If omitted, starts a simple interactive menu (inquire-based).
    #[command(subcommand)]
    command: Option<Commands>,
}

/// Shared flags for run/plan/list (tier, tags, incremental, paths).
#[derive(Parser, Debug, Clone)]
struct HarnessFilterArgs {
    #[arg(long)]
    config: PathBuf,
    #[arg(long)]
    tier: Option<String>,
    #[arg(long, value_delimiter = ',')]
    tags: Option<Vec<String>>,
    #[arg(long)]
    workers: Option<usize>,
    #[arg(long)]
    baseline: Option<PathBuf>,
    #[arg(long)]
    incremental: Option<String>,
    #[arg(long)]
    manifest: Option<PathBuf>,
    #[arg(long, default_value = "build/regression_data")]
    data_root: PathBuf,
    #[arg(long)]
    out_dir: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Stream-style REPL (classic CLI interaction)
    Repl,
    /// Validate JSON config (version, duplicate ids)
    ValidateConfig {
        #[arg(long)]
        config: PathBuf,
    },
    /// List selected cases (tier/tags) without running
    ListCases {
        #[command(flatten)]
        filter: HarnessFilterArgs,
        #[arg(long, default_value = "human")]
        format: String,
    },
    /// Show execution plan (incremental) without running
    Plan {
        #[command(flatten)]
        filter: HarnessFilterArgs,
        #[arg(long, default_value = "human")]
        format: String,
    },
    /// Summarize last report.json under data root
    Status {
        #[arg(long, default_value = "build/regression_data")]
        data_root: PathBuf,
        #[arg(long, default_value = "human")]
        format: String,
    },
    /// Print tail of cases.ndjson or follow new lines
    Monitor {
        #[arg(long, default_value = "build/regression_data")]
        data_root: PathBuf,
        #[arg(long, default_value_t = 20)]
        tail: usize,
        #[arg(long)]
        follow: bool,
        #[arg(long, default_value = "auto")]
        source: String,
    },
    /// Emit machine-readable context for agents (paths, failures, suggested CLI)
    AgentContext {
        #[arg(long, default_value = "build/regression_data")]
        data_root: PathBuf,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// AI agent integration
    Agent {
        #[command(subcommand)]
        command: AgentCommands,
    },
    /// Run cases from JSON config
    Run {
        #[command(flatten)]
        filter: HarnessFilterArgs,
        #[arg(long)]
        ndjson: bool,
        #[arg(long)]
        summary_compat: bool,
        /// Log each finished case to stderr
        #[arg(long)]
        progress: bool,
    },
    /// JIT-specific gates and helpers
    Jit {
        #[command(subcommand)]
        command: JitCommands,
    },
}

#[derive(Subcommand, Debug)]
enum JitCommands {
    /// Batch validate TestLib root/negative models (PS1 parity)
    TestlibValidate {
        /// Optional cargo target subdir under jit-compiler (e.g. target_regression)
        #[arg(long)]
        cargo_target_subdir: Option<String>,
    },
    /// Emit-C check for TestLib/RecursiveFunc (PS1 parity)
    EmitCRecursiveFunc,
    /// Emit-C check for TestLib/StringArgExtFunc (PS1 parity)
    EmitCStringArgExtFunc {
        /// Optional cargo target subdir under jit-compiler (e.g. target_regression)
        #[arg(long)]
        cargo_target_subdir: Option<String>,
    },
    /// Backend DAE info check for TestLib/ClockedPartitionTest (PS1 parity)
    BackendDaeInfoClockedPartition {
        /// Optional cargo target subdir under jit-compiler (e.g. target_regression)
        #[arg(long)]
        cargo_target_subdir: Option<String>,
    },
    /// Run a built-in script-mode case (no external txt)
    ScriptMode {
        #[arg(long)]
        case: String,
    },
    /// Validate-perf bench runner: standardized artifacts + report.json
    ValidatePerf {
        /// Path to rustmodlica.exe (optional, defaults to workspace target/release)
        #[arg(long)]
        exe: Option<PathBuf>,
        /// Repeatable Modelica library roots for --lib-path
        #[arg(long, value_delimiter = ',')]
        lib_path: Option<Vec<PathBuf>>,
        /// Output directory (default: build/jit_validate_perf)
        #[arg(long)]
        out_dir: Option<PathBuf>,
        /// Validate tier (full|parse|flatten|analyze)
        #[arg(long, default_value = "analyze")]
        validate_tier: String,
        /// Validation mode (full|quick|superfast)
        #[arg(long, default_value = "full")]
        validation_mode: String,
        /// Models to run (repeatable, or comma-delimited)
        #[arg(long, value_delimiter = ',')]
        models: Vec<String>,
        /// Hot scenario run count (applies to hot_nsA)
        #[arg(long, default_value_t = 2)]
        hot_runs: usize,
        /// Enable stage timing trace (env RUSTMODLICA_STAGE_TRACE=1 for child)
        #[arg(long)]
        stage_trace: bool,
        /// Enable perf trace (env RUSTMODLICA_PERF_TRACE=1 for child)
        #[arg(long)]
        perf_trace: bool,
        /// Optional scenario allow-list (comma-delimited): cold_empty_nsCOLD,cold_qcache0,hot_nsA,legacy_salsa0
        #[arg(long, value_delimiter = ',')]
        scenarios: Option<Vec<String>>,
    },
}

fn discover_repo_root() -> Result<PathBuf> {
    let mut dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|x| x.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    for _ in 0..24 {
        if dir.join(".git").is_dir() {
            return Ok(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    Ok(std::env::current_dir()?)
}

#[derive(Subcommand, Debug)]
enum AgentCommands {
    /// Start line-based JSON REPL for AI agents
    Repl,
}

fn main() {
    if let Err(err) = run_cli() {
        let issue = anyhow_to_issue(err, "E_UNKNOWN_000");
        eprintln!(
            "[ERROR] code={} exit_code={} message=\"{}\"",
            issue.code,
            resolve_exit_code(&issue),
            issue.message
        );
        std::process::exit(resolve_exit_code(&issue));
    }
}

fn run_cli() -> Result<()> {
    let cli = Cli::parse();
    i18n::set_language(cli.lang.as_deref());
    match cli.command {
        None | Some(Commands::Repl) => repl::run_repl(),
        Some(Commands::ValidateConfig { config }) => cmd_validate_config(&config),
        Some(Commands::ListCases { filter, format }) => {
            cmd_list_cases(
                filter.config,
                filter.tier,
                filter.tags,
                OutputFormat::parse(&format)?,
            )
        }
        Some(Commands::Plan { filter, format }) => cmd_plan(
            filter.config,
            filter.tier,
            filter.tags,
            filter.workers,
            filter.baseline,
            filter.incremental,
            filter.manifest,
            filter.data_root,
            filter.out_dir,
            OutputFormat::parse(&format)?,
        ),
        Some(Commands::Status { data_root, format }) => {
            cmd_status(data_root, OutputFormat::parse(&format)?)
        }
        Some(Commands::Monitor {
            data_root,
            tail,
            follow,
            source,
        }) => cmd_monitor(data_root, tail, follow, parse_monitor_source(Some(&source))?),
        Some(Commands::AgentContext { data_root, config }) => {
            cmd_agent_context(data_root, config)
        },
        Some(Commands::Agent { command }) => match command {
            AgentCommands::Repl => agent_repl::run_agent_repl(),
        },
        Some(Commands::Run {
            filter,
            ndjson,
            summary_compat,
            progress,
        }) => run_cmd(
            filter.config,
            filter.tier,
            filter.tags,
            filter.workers,
            filter.baseline,
            filter.incremental,
            filter.manifest,
            filter.data_root,
            filter.out_dir,
            ndjson,
            summary_compat,
            progress,
        ),
        Some(Commands::Jit { command }) => match command {
            JitCommands::TestlibValidate { cargo_target_subdir } => {
                let repo_root = discover_repo_root()?;
                let s = jit_validate::legacy::testlib_validate_batch(
                    &repo_root,
                    cargo_target_subdir.as_deref(),
                )?;
                println!("rustmodlica: {}", s.exe.display());
                println!("TestLib root .mo (expect PASS): {}", s.root_total);
                println!("TestLib/negative .mo (expect FAIL): {}", s.negative_total);
                println!("PASS (root): {}", s.root_pass);
                if !s.root_unexpected_fail.is_empty() {
                    println!("FAIL (root, unexpected): {}", s.root_unexpected_fail.len());
                    for n in &s.root_unexpected_fail {
                        println!("  {n}");
                    }
                }
                println!("FAIL-as-expected (negative): {}", s.negative_fail_as_expected);
                if !s.negative_unexpected_pass.is_empty() {
                    println!(
                        "PASS (negative, unexpected -- should have failed): {}",
                        s.negative_unexpected_pass.len()
                    );
                    for n in &s.negative_unexpected_pass {
                        println!("  {n}");
                    }
                }
                let ok = s.root_unexpected_fail.is_empty() && s.negative_unexpected_pass.is_empty();
                if ok {
                    Ok(())
                } else {
                    bail!("jit testlib validate failed")
                }
            }
            JitCommands::EmitCRecursiveFunc => {
                let repo_root = discover_repo_root()?;
                let code = jit_phase1::emit_c_recursive_func(&repo_root)?;
                if code == 0 {
                    Ok(())
                } else {
                    bail!("jit emit-c recursive func failed exit_code={code}")
                }
            }
            JitCommands::EmitCStringArgExtFunc { cargo_target_subdir } => {
                let repo_root = discover_repo_root()?;
                let code =
                    jit_phase1::emit_c_string_arg_ext_func(&repo_root, cargo_target_subdir.as_deref())?;
                if code == 0 {
                    Ok(())
                } else {
                    bail!("jit emit-c check failed exit_code={code}")
                }
            }
            JitCommands::BackendDaeInfoClockedPartition { cargo_target_subdir } => {
                let repo_root = discover_repo_root()?;
                let code = jit_phase1::backend_dae_info_clocked(
                    &repo_root,
                    cargo_target_subdir.as_deref(),
                )?;
                if code == 0 {
                    Ok(())
                } else {
                    bail!("jit backend-dae-info check failed exit_code={code}")
                }
            }
            JitCommands::ScriptMode { case } => {
                let repo_root = discover_repo_root()?;
                let code = jit_script_mode::run_script_mode_case(&repo_root, &case)?;
                if code == 0 {
                    Ok(())
                } else {
                    bail!("jit script-mode failed exit_code={code}")
                }
            }
            JitCommands::ValidatePerf {
                exe,
                lib_path,
                out_dir,
                validate_tier,
                validation_mode,
                models,
                hot_runs,
                stage_trace,
                perf_trace,
                scenarios,
            } => {
                let repo_root = discover_repo_root()?;
                let exe_path = if let Some(p) = exe {
                    p
                } else {
                    // Default to workspace build output; caller can override.
                    repo_root.join("target").join("release").join("rustmodlica.exe")
                };
                let out_dir = out_dir.unwrap_or_else(|| repo_root.join("build").join("jit_validate_perf"));
                let lib_paths: Vec<PathBuf> = lib_path.unwrap_or_else(|| vec![repo_root.join("jit-compiler")]);
                let scenarios_vec = regress_harness::jit_validate::runner::default_perf_scenarios(hot_runs);

                let spec = regress_harness::jit_validate::RunSpec {
                    repo_root: repo_root.clone(),
                    exe_path,
                    lib_paths,
                    out_dir,
                    models,
                    validate: regress_harness::jit_validate::ValidateArgs {
                        validate_tier,
                        validation_mode,
                    },
                    stage_trace,
                    perf_trace,
                    scenarios: scenarios_vec,
                    scenario_filter: scenarios.unwrap_or_default(),
                };
                let report = regress_harness::jit_validate::runner::ValidatePerfRunner::run(spec)?;
                println!(
                    "jit-validate-perf: out_dir={} total={} passed={} failed={}",
                    report.out_dir, report.summary.total, report.summary.passed, report.summary.failed
                );
                if report.summary.failed > 0 {
                    bail!("jit validate-perf failed");
                }
                Ok(())
            }
        },
    }
}

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
        for case in &run_jobs {
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
                    "[regress-harness] done case_id={} status={:?} duration_ms={}",
                    r.case_id, r.status, r.duration_ms
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
        let traces: Vec<regress_harness::runner::CaseRunTrace> = pool.install(|| {
            let pbar = pb.clone();
            run_jobs
                .par_iter()
                .map(|case| {
                    let traced = regress_harness::runner::run_case_with_trace(&ctx, case);
                    if progress {
                        eprintln!(
                            "[regress-harness] done case_id={} status={:?} duration_ms={}",
                            traced.result.case_id, traced.result.status, traced.result.duration_ms
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
