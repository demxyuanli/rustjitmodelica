use anyhow::{bail, Context, Result};
use clap::{builder::Styles, Parser, Subcommand};
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
use std::collections::{BTreeMap, HashMap};
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
mod jit_phase1;
mod jit_script_mode;

use commands::{
    cmd_agent_context, cmd_list_cases, cmd_monitor, cmd_plan, cmd_status, cmd_validate_config,
    parse_monitor_source, OutputFormat,
};

const LONG_ABOUT: &str = "\
JSON-driven regression runner for rustmodlica (parallel execution, incremental plans, reports).

Launch with no subcommand to open an interactive session (terminal-first workflow).";

const AFTER_LONG_HELP: &str = "\
Command groups:
  Session        repl (default), no-args entry
  Workflow       run, plan, list-cases, status, monitor
  Config         validate-config
  Agents         agent-context, agent
  JIT            jit <subcommand>
  Automation     repl-exec -c \"<one repl line>\"

Tips:
  Use --no-color when piping or logging to files.
  In the interactive session, leading '/' is optional (e.g. /help and help).";

#[derive(Parser, Debug)]
#[command(
    name = "regress-harness",
    version,
    about = "JSON-driven regression runner.",
    long_about = LONG_ABOUT,
    after_long_help = AFTER_LONG_HELP,
    styles = Styles::styled(),
)]
struct Cli {
    /// Disable ANSI colors (also respects env NO_COLOR).
    #[arg(long, global = true)]
    no_color: bool,
    /// Extra diagnostics on stderr (reserved for future use).
    #[arg(short, long, global = true)]
    verbose: bool,
    /// Language: en | zh-CN (also supports env RUSTMODLICA_LANG)
    #[arg(long, global = true)]
    lang: Option<String>,
    /// If omitted, starts an interactive session (inquire REPL).
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
    /// Interactive REPL (same as launching with no subcommand)
    Repl,
    /// Run one REPL-syntax command and exit (for Ink / automation)
    ReplExec {
        /// Same input as in the interactive REPL (e.g. `scope list`, `run --config cfg.json`)
        #[arg(short = 'c', long)]
        command: String,
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
    /// Show execution plan (incremental) without running
    Plan {
        #[command(flatten)]
        filter: HarnessFilterArgs,
        #[arg(long, default_value = "human")]
        format: String,
    },
    /// List selected cases (tier/tags) without running
    ListCases {
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
    /// Validate JSON config (version, duplicate ids)
    ValidateConfig {
        #[arg(long)]
        config: PathBuf,
    },
    /// Emit machine-readable context for agents (paths, failures, suggested CLI)
    AgentContext {
        #[arg(long, default_value = "build/regression_data")]
        data_root: PathBuf,
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// AI agent integration (JSON line protocol)
    Agent {
        #[command(subcommand)]
        command: AgentCommands,
    },
    /// JIT-specific gates and helpers
    Jit {
        #[command(subcommand)]
        command: JitCommands,
    },
}

#[derive(Subcommand, Debug)]
#[command(subcommand_help_heading = "JIT commands")]
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
        /// Incremental mode: only run models affected by changed dependency files.
        #[arg(long)]
        incremental: bool,
        /// Delete all out_dir/cache_* before running (reproducible L2 state across A/B compares).
        #[arg(long)]
        purge_scenario_caches: bool,
        /// Optional shared cache root for all scenarios.
        /// When set, scenario cache dirs become <shared_cache_dir>/<scenario_id>.
        #[arg(long)]
        shared_cache_dir: Option<PathBuf>,
        /// Force-enable flatten full cache for every scenario.
        #[arg(long)]
        force_flatten_full_cache: bool,
        /// PoC mode: execute each scenario via worker entrypoint.
        #[arg(long)]
        worker_per_scenario: bool,
        /// Extra env for each rustmodlica child, repeatable (`KEY=VAL`, e.g. `RUSTMODLICA_CRANELIFT_OPT_LEVEL=none`).
        #[arg(long = "set-env", value_name = "KEY=VAL")]
        set_env: Vec<String>,
    },
    /// Compare validate-perf report against a baseline JSON
    CompareBaseline {
        /// Path to current validate-perf report.json
        #[arg(long)]
        report: PathBuf,
        /// Path to baseline JSON (default: baseline/20260417_jit_cranelift_none/jit_perf_baseline.json)
        #[arg(long)]
        baseline: Option<PathBuf>,
    },
    /// Generate or update a baseline JSON from a validate-perf report
    UpdateBaseline {
        /// Path to current validate-perf report.json
        #[arg(long)]
        report: PathBuf,
        /// Output path for baseline JSON
        #[arg(long)]
        output: PathBuf,
        /// Confirm overwriting existing baseline
        #[arg(long)]
        confirm: bool,
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
#[command(subcommand_help_heading = "Agent commands")]
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
    if cli.no_color {
        std::env::set_var("NO_COLOR", "1");
    }
    if cli.verbose {
        std::env::set_var("HARNESS_VERBOSE", "1");
    }
    i18n::set_language(cli.lang.as_deref());
    match cli.command {
        None | Some(Commands::Repl) => repl::run_repl(),
        Some(Commands::ReplExec { command }) => repl::run_single_command(&command),
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
                let s = regress_harness::jit_validate::legacy::testlib_validate_batch(
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
                incremental,
                purge_scenario_caches,
                shared_cache_dir,
                force_flatten_full_cache,
                worker_per_scenario,
                set_env,
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

                let mut child_set: BTreeMap<String, String> = BTreeMap::new();
                for pair in &set_env {
                    let (k, v) = pair.split_once('=').with_context(|| {
                        format!("--set-env expects KEY=VAL, got {pair}")
                    })?;
                    if k.is_empty() {
                        bail!("empty env key in --set-env");
                    }
                    child_set.insert(k.to_string(), v.to_string());
                }
                let child_env = regress_harness::jit_validate::EnvOverlay {
                    set: child_set,
                    unset: Vec::new(),
                };

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
                    incremental,
                    purge_scenario_caches,
                    shared_cache_dir,
                    force_flatten_full_cache,
                    worker_per_scenario,
                    child_env,
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
            JitCommands::CompareBaseline { report, baseline } => {
                let repo_root = discover_repo_root()?;
                let report_path = if report.is_absolute() { report } else { repo_root.join(&report) };
                let baseline_path = baseline
                    .map(|p| if p.is_absolute() { p } else { repo_root.join(p) })
                    .unwrap_or_else(|| {
                        repo_root.join(regress_harness::jit_validate::baseline::DEFAULT_JIT_COMPARE_BASELINE_REL)
                    });

                let report_text = std::fs::read_to_string(&report_path)
                    .with_context(|| format!("read report {}", report_path.display()))?;
                let report: regress_harness::jit_validate::artifacts::ValidatePerfReport =
                    serde_json::from_str(&report_text)?;

                let bl = regress_harness::jit_validate::baseline::load_baseline(&baseline_path)?;
                let result = regress_harness::jit_validate::baseline::compare_report_to_baseline(&report, &bl);

                let json = serde_json::to_string_pretty(&result)?;
                println!("{}", json);

                if result.overall_verdict == regress_harness::jit_validate::baseline::Verdict::Fail {
                    bail!("baseline comparison failed");
                }
                Ok(())
            }
            JitCommands::UpdateBaseline { report, output, confirm } => {
                if output.exists() && !confirm {
                    bail!("output file exists; use --confirm to overwrite");
                }
                let repo_root = discover_repo_root()?;
                let report_path = if report.is_absolute() { report } else { repo_root.join(&report) };
                let output_path = if output.is_absolute() { output } else { repo_root.join(&output) };

                let report_text = std::fs::read_to_string(&report_path)
                    .with_context(|| format!("read report {}", report_path.display()))?;
                let report: regress_harness::jit_validate::artifacts::ValidatePerfReport =
                    serde_json::from_str(&report_text)?;

                let git_head = std::process::Command::new("git")
                    .args(["rev-parse", "HEAD"])
                    .output()
                    .ok()
                    .and_then(|o| if o.status.success() { Some(String::from_utf8_lossy(&o.stdout).trim().to_string()) } else { None });

                let bl = regress_harness::jit_validate::baseline::update_baseline_from_report(
                    &report,
                    regress_harness::jit_validate::baseline::BaselineThresholds::default(),
                    git_head,
                );
                regress_harness::jit_validate::baseline::save_baseline(&bl, &output_path)?;
                println!("baseline saved to {} ({} benchmarks)", output_path.display(), bl.benchmarks.len());
                Ok(())
            }
        },
    }
}


include!("main_tail.rs");
