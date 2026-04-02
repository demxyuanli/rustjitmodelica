//! CLI subcommands: validate, list, plan, status, monitor, agent-context.

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use crate::i18n::tr;
use crate::ui;

use regress_harness::incremental::PlanEntry;
use regress_harness::report::{read_report_json, CaseResult, CaseStatus, OmcCompareResult, ReportSummary};
use regress_harness::runtime::compat::{read_manifest_compat, read_report_compat};
use regress_harness::runtime::monitor::{follow_events, read_event_tail};
use regress_harness::session_prep::{
    case_kind_str, listed_case_json, plan_row_json, prepare_for_list, prepare_session,
    resolve_user_path_str, ListPrep,
};
use regress_harness::tiers::Filter;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputFormat {
    #[default]
    Human,
    Json,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self> {
        match s.to_ascii_lowercase().as_str() {
            "human" | "text" => Ok(Self::Human),
            "json" => Ok(Self::Json),
            _ => bail!("unknown format: {s} (use human or json)"),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MonitorSource {
    #[default]
    Auto,
    Event,
    Ndjson,
}

pub fn parse_monitor_source(src: Option<&str>) -> Result<MonitorSource> {
    match src.unwrap_or("auto").to_ascii_lowercase().as_str() {
        "auto" => Ok(MonitorSource::Auto),
        "event" => Ok(MonitorSource::Event),
        "ndjson" => Ok(MonitorSource::Ndjson),
        other => bail!("unknown monitor source: {other} (use auto|event|ndjson)"),
    }
}

pub fn cmd_validate_config(config: &Path) -> Result<()> {
    regress_harness::config::load_config(config).map_err(|e| anyhow::anyhow!("{e}"))?;
    println!("{}: {}", tr("ok_validate"), config.display());
    Ok(())
}

pub fn cmd_list_cases(
    config: PathBuf,
    tier: Option<String>,
    tags: Option<Vec<String>>,
    format: OutputFormat,
) -> Result<()> {
    let filter = Filter { tier, tags };
    let ListPrep { cases, .. } = prepare_for_list(&config, &filter)?;
    match format {
        OutputFormat::Json => {
            let rows: Vec<_> = cases.iter().map(listed_case_json).collect();
            println!("{}", serde_json::to_string_pretty(&rows)?);
        }
        OutputFormat::Human => {
            let id_w = 24usize;
            let kind_w = 10usize;
            let target_w = 42usize;
            let tags_w = 24usize;
            let header = format!(
                "{}  {}  {}  {}",
                ui::pad_display(&ui::truncate_display(tr("list_col_id"), id_w), id_w),
                ui::pad_display(&ui::truncate_display(tr("list_col_kind"), kind_w), kind_w),
                ui::pad_display(&ui::truncate_display(tr("list_col_target"), target_w), target_w),
                ui::pad_display(&ui::truncate_display(tr("list_col_tags"), tags_w), tags_w),
            );
            println!("{header}");
            for c in &cases {
                let k = case_kind_str(&c.kind);
                let tags = c.tags.join(",");
                let line = format!(
                    "{}  {}  {}  {}",
                    ui::pad_display(&ui::truncate_display(&c.id, id_w), id_w),
                    ui::pad_display(&ui::truncate_display(k, kind_w), kind_w),
                    ui::pad_display(&ui::truncate_display(&c.target, target_w), target_w),
                    ui::pad_display(&ui::truncate_display(&tags, tags_w), tags_w),
                );
                println!("{line}");
            }
            println!("{} {}", tr("list_total"), cases.len());
        }
    }
    Ok(())
}

pub fn cmd_plan(
    config: PathBuf,
    tier: Option<String>,
    tags: Option<Vec<String>>,
    workers: Option<usize>,
    baseline: Option<PathBuf>,
    incremental: Option<String>,
    manifest: Option<PathBuf>,
    data_root: PathBuf,
    out_dir: Option<PathBuf>,
    format: OutputFormat,
) -> Result<()> {
    let filter = Filter { tier, tags };
    let prep = prepare_session(
        config,
        filter,
        data_root,
        out_dir,
        baseline,
        incremental,
        manifest,
    )?;
    let _ = workers; // plan does not execute; kept for CLI symmetry with run

    let mut n_run = 0usize;
    let mut n_skip_u = 0usize;
    let mut n_skip_s = 0usize;
    for e in &prep.plan {
        match e {
            PlanEntry::Run(_) => n_run += 1,
            PlanEntry::SkippedUnchanged(_) => n_skip_u += 1,
            PlanEntry::SkippedScope(_) => n_skip_s += 1,
        }
    }

    match format {
        OutputFormat::Json => {
            let rows: Vec<_> = prep.plan.iter().map(plan_row_json).collect();
            let out = serde_json::json!({
                "run": n_run,
                "skipped_unchanged": n_skip_u,
                "skipped_scope": n_skip_s,
                "rows": rows,
            });
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        OutputFormat::Human => {
            println!(
                "plan: run={n_run} skipped_unchanged={n_skip_u} skipped_scope={n_skip_s}  data_root={}",
                prep.data_root.display()
            );
            for e in &prep.plan {
                let row = plan_row_json(e);
                println!(
                    "{:?}\t{}\t{}\t{}",
                    row.action, row.case_id, row.kind, row.target
                );
            }
        }
    }
    Ok(())
}

#[derive(Serialize)]
struct StatusJsonOut {
    pub report_path: String,
    pub generated_at: Option<String>,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub failed_cases: Vec<FailedCaseBrief>,
}

#[derive(Serialize)]
struct FailedCaseBrief {
    pub case_id: String,
    pub classification: Option<String>,
    pub exit_code: Option<i32>,
}

pub fn cmd_status(data_root: PathBuf, format: OutputFormat) -> Result<()> {
    let data_root = if data_root.is_absolute() {
        data_root
    } else {
        std::env::current_dir()?.join(data_root)
    };
    let data_root = std::fs::canonicalize(&data_root).unwrap_or(data_root);
    let report_path = data_root.join("report.json");
    if !report_path.exists() {
        bail!("no report at {}", report_path.display());
    }
    let report = match read_report_compat(&report_path) {
        Ok(v) => v,
        Err(_) => {
            let v = read_report_json(&report_path)?;
            regress_harness::runtime::compat::CompatReport {
                schema_version: v.schema_version,
                generated_at: v.generated_at,
                summary: v.summary,
                cases: v.cases,
            }
        }
    };
    let failed: Vec<FailedCaseBrief> = report
        .cases
        .iter()
        .filter(|c| c.status == CaseStatus::Fail)
        .map(|c| FailedCaseBrief {
            case_id: c.case_id.clone(),
            classification: c.classification.clone(),
            exit_code: c.exit_code,
        })
        .collect();

    match format {
        OutputFormat::Json => {
            let out = StatusJsonOut {
                report_path: report_path.to_string_lossy().to_string(),
                generated_at: Some(report.generated_at.clone()),
                total: report.summary.total,
                passed: report.summary.passed,
                failed: report.summary.failed,
                skipped: report.summary.skipped,
                failed_cases: failed,
            };
            println!("{}", serde_json::to_string_pretty(&out)?);
        }
        OutputFormat::Human => {
            println!("{}: {}", tr("status_report"), report_path.display());
            println!("{}: {}", tr("status_generated_at"), report.generated_at);
            println!(
                "total={} passed={} failed={} skipped={}",
                report.summary.total,
                report.summary.passed,
                report.summary.failed,
                report.summary.skipped
            );
            if !failed.is_empty() {
                println!("{}", tr("status_failed_case_ids"));
                for f in failed {
                    println!(
                        "  {}  {:?}  exit={:?}",
                        f.case_id, f.classification, f.exit_code
                    );
                }
            }
        }
    }
    Ok(())
}

fn tail_ndjson_lines(path: &Path, n: usize) -> Result<Vec<String>> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("read {}", path.display()))?;
    let mut lines: Vec<&str> = text.lines().filter(|l| !l.is_empty()).collect();
    let start = lines.len().saturating_sub(n);
    Ok(lines.drain(start..).map(|s| s.to_string()).collect())
}

pub fn cmd_monitor_ndjson(data_root: PathBuf, tail: usize, follow: bool) -> Result<()> {
    let data_root = if data_root.is_absolute() {
        data_root
    } else {
        std::env::current_dir()?.join(data_root)
    };
    let data_root = std::fs::canonicalize(&data_root).unwrap_or(data_root);
    let ndjson_path = data_root.join("cases.ndjson");

    if !follow {
        if !ndjson_path.exists() {
            bail!("no file at {}", ndjson_path.display());
        }
        for line in tail_ndjson_lines(&ndjson_path, tail)? {
            if let Ok(c) = serde_json::from_str::<CaseResult>(&line) {
                println!(
                    "{} {:?} {}ms exit={:?} {}",
                    c.case_id, c.status, c.duration_ms, c.exit_code, c.classification.as_deref().unwrap_or("")
                );
            } else {
                println!("{line}");
            }
        }
        return Ok(());
    }

    let poll = std::time::Duration::from_millis(250);
    let mut pos: u64 = 0;
    loop {
        if !ndjson_path.exists() {
            std::thread::sleep(poll);
            continue;
        }
        let mut f = std::fs::File::open(&ndjson_path)?;
        let len = f.metadata()?.len();
        if len < pos {
            pos = 0;
        }
        f.seek(SeekFrom::Start(pos))?;
        let mut chunk = String::new();
        f.read_to_string(&mut chunk)?;
        pos = f.stream_position()?;
        for line in chunk.lines() {
            if line.is_empty() {
                continue;
            }
            if let Ok(c) = serde_json::from_str::<CaseResult>(line) {
                println!(
                    "{} {:?} {}ms exit={:?} {}",
                    c.case_id,
                    c.status,
                    c.duration_ms,
                    c.exit_code,
                    c.classification.as_deref().unwrap_or("")
                );
            } else {
                println!("{line}");
            }
        }
        std::thread::sleep(poll);
    }
}

pub fn cmd_monitor_event(data_root: PathBuf, tail: usize, follow: bool) -> Result<()> {
    let data_root = if data_root.is_absolute() {
        data_root
    } else {
        std::env::current_dir()?.join(data_root)
    };
    let data_root = std::fs::canonicalize(&data_root).unwrap_or(data_root);
    let events_path = data_root.join("events.ndjson");
    if !follow {
        if !events_path.exists() {
            bail!("no file at {}", events_path.display());
        }
        for ev in read_event_tail(&events_path, tail)? {
            println!(
                "{} {} seq={} {}",
                ev.ts,
                ev.run_id,
                ev.seq,
                serde_json::to_string(&ev.payload).unwrap_or_else(|_| "{}".to_string())
            );
        }
        return Ok(());
    }
    follow_events(&events_path, 0)
}

pub fn cmd_monitor(
    data_root: PathBuf,
    tail: usize,
    follow: bool,
    source: MonitorSource,
) -> Result<()> {
    match source {
        MonitorSource::Event => cmd_monitor_event(data_root, tail, follow),
        MonitorSource::Ndjson => cmd_monitor_ndjson(data_root, tail, follow),
        MonitorSource::Auto => match cmd_monitor_event(data_root.clone(), tail, follow) {
            Ok(()) => Ok(()),
            Err(_) => cmd_monitor_ndjson(data_root, tail, follow),
        },
    }
}

#[derive(Serialize)]
struct AgentPaths {
    pub config_path: String,
    pub report_path: String,
    pub manifest_path: String,
    pub ndjson_path: String,
    pub artifacts_dir: String,
}

#[derive(Serialize)]
struct AgentFailure {
    pub case_id: String,
    pub status: String,
    pub classification: Option<String>,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub stderr_tail: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub stdout_tail: String,
    pub omc_compare: Option<OmcCompareResult>,
}

#[derive(Serialize)]
struct AgentContextOut {
    pub schema_version: u32,
    pub paths: AgentPaths,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_summary: Option<ReportSummary>,
    pub failures: Vec<AgentFailure>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub last_manifest_case_ids: Vec<String>,
    pub suggested_cli: Vec<String>,
}

fn path_display_abs(p: &Path) -> String {
    std::fs::canonicalize(p)
        .unwrap_or_else(|_| p.to_path_buf())
        .to_string_lossy()
        .to_string()
}

pub fn cmd_agent_context(data_root: PathBuf, config_override: Option<PathBuf>) -> Result<()> {
    let data_root = if data_root.is_absolute() {
        data_root
    } else {
        std::env::current_dir()?.join(data_root)
    };
    let data_root = std::fs::canonicalize(&data_root).unwrap_or(data_root.clone());

    let report_path = data_root.join("report.json");
    let manifest_path = data_root.join("regress_manifest.json");
    let ndjson_path = data_root.join("cases.ndjson");
    let artifacts_dir = data_root.join("artifacts");

    let report_opt = if report_path.exists() {
        Some(read_report_json(&report_path)?)
    } else {
        None
    };

    let config_path_str = if let Some(ref p) = config_override {
        path_display_abs(p)
    } else if let Some(ref r) = report_opt {
        match resolve_user_path_str(&r.config_path) {
            Ok(pb) => path_display_abs(&pb),
            Err(_) => r.config_path.clone(),
        }
    } else {
        String::new()
    };

    let manifest_ids = if manifest_path.exists() {
        read_manifest_compat(&manifest_path)?.case_ids
    } else {
        Vec::new()
    };

    let failures: Vec<AgentFailure> = report_opt
        .as_ref()
        .map(|r| {
            r.cases
                .iter()
                .filter(|c| c.status == CaseStatus::Fail)
                .map(|c| AgentFailure {
                    case_id: c.case_id.clone(),
                    status: format!("{:?}", c.status).to_ascii_lowercase(),
                    classification: c.classification.clone(),
                    exit_code: c.exit_code,
                    duration_ms: c.duration_ms,
                    stderr_tail: c.stderr_tail.clone(),
                    stdout_tail: c.stdout_tail.clone(),
                    omc_compare: c.omc_compare.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    let mut suggested_cli = Vec::new();
    if !config_path_str.is_empty() {
        suggested_cli.push(format!(
            "regress-harness run --config \"{}\" --data-root \"{}\"",
            config_path_str,
            data_root.display()
        ));
        if !failures.is_empty() {
            suggested_cli.push(format!(
                "regress-harness run --config \"{}\" --data-root \"{}\" --incremental rerun_failed",
                config_path_str,
                data_root.display()
            ));
        }
    }
    suggested_cli.push(format!(
        "regress-harness plan --config <path> --data-root \"{}\"",
        data_root.display()
    ));
    suggested_cli.push(format!(
        "regress-harness status --data-root \"{}\" --format json",
        data_root.display()
    ));

    let out = AgentContextOut {
        schema_version: 1,
        paths: AgentPaths {
            config_path: config_path_str.clone(),
            report_path: path_display_abs(&report_path),
            manifest_path: path_display_abs(&manifest_path),
            ndjson_path: path_display_abs(&ndjson_path),
            artifacts_dir: path_display_abs(&artifacts_dir),
        },
        report_summary: report_opt.as_ref().map(|r| r.summary.clone()),
        failures,
        last_manifest_case_ids: manifest_ids,
        suggested_cli,
    };

    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}
