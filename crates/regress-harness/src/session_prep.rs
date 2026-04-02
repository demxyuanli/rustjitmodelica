//! Resolve repo/data roots, tier filter, manifest/baseline, and execution plan (no subprocess).

use anyhow::{bail, Context, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::config::{load_config, CaseDef, CaseKind, HarnessConfig, IncrementalConfig, IncrementalStrategy};
use crate::incremental::{
    effective_plan_strategy, needs_baseline_report, needs_last_manifest, plan_runs, PlanEntry,
};
use crate::report::{filter_cases_by_manifest, read_manifest, read_report_json, Report};
use crate::tiers::{resolve_cases, Filter};

/// Resolve a user-supplied path (relative paths join cwd).
pub fn resolve_user_path_str(p: &str) -> Result<PathBuf> {
    let pb = PathBuf::from(p);
    if Path::new(p).has_root() {
        Ok(pb)
    } else {
        Ok(std::env::current_dir()?.join(p))
    }
}

/// Merge JSON incremental section with CLI flags.
pub fn apply_incremental_overrides(
    cfg_inc: &IncrementalConfig,
    baseline_cli: Option<&PathBuf>,
    incremental_cli: Option<&str>,
    manifest_cli: Option<&PathBuf>,
) -> Result<IncrementalConfig> {
    let mut out = cfg_inc.clone();
    if let Some(p) = baseline_cli {
        out.baseline_path = Some(p.to_string_lossy().to_string());
    }
    if let Some(s) = incremental_cli {
        out.strategy = match s {
            "none" => IncrementalStrategy::None,
            "rerun_failed" => IncrementalStrategy::RerunFailed,
            "skip_unchanged" => IncrementalStrategy::SkipUnchanged,
            "last_structure" => IncrementalStrategy::LastStructure,
            "last_structure_rerun_failed" => IncrementalStrategy::LastStructureRerunFailed,
            _ => bail!("unknown incremental strategy: {s}"),
        };
    }
    if let Some(p) = manifest_cli {
        out.manifest_path = Some(p.to_string_lossy().to_string());
    }
    Ok(out)
}

pub fn resolve_data_root(
    cfg: &HarnessConfig,
    data_root_cli: PathBuf,
    out_dir_legacy: Option<PathBuf>,
    repo_root: &Path,
) -> Result<PathBuf> {
    let default_cli = PathBuf::from("build/regression_data");
    let cli_override = data_root_cli != default_cli;
    let data_root = if let Some(p) = out_dir_legacy.clone() {
        p
    } else if cli_override {
        data_root_cli
    } else if let Some(p) = cfg.defaults.regression_data_root.as_ref().map(PathBuf::from) {
        p
    } else {
        data_root_cli
    };
    let data_root = if data_root.is_absolute() {
        data_root
    } else {
        repo_root.join(data_root)
    };
    Ok(std::fs::canonicalize(&data_root).unwrap_or(data_root))
}

pub struct ListPrep {
    pub cfg: HarnessConfig,
    pub repo_root: PathBuf,
    pub cases: Vec<CaseDef>,
}

pub fn prepare_for_list(config_path: &Path, filter: &Filter) -> Result<ListPrep> {
    let cfg = load_config(config_path).map_err(|e| anyhow::anyhow!("{e}"))?;
    let repo_root = cfg.resolve_repo_root();
    let repo_root = std::fs::canonicalize(&repo_root)
        .with_context(|| format!("repo_root {:?}", repo_root))?;
    let cases = resolve_cases(&cfg, filter).map_err(|e| anyhow::anyhow!(e))?;
    Ok(ListPrep {
        cfg,
        repo_root,
        cases,
    })
}

pub struct PreparedSession {
    pub config_path: PathBuf,
    pub repo_root: PathBuf,
    pub data_root: PathBuf,
    pub artifact_dir: PathBuf,
    pub cfg: HarnessConfig,
    pub filter: Filter,
    pub incremental: IncrementalConfig,
    pub manifest_path: PathBuf,
    pub ordered_case_ids: Vec<String>,
    pub baseline_report: Option<Report>,
    pub plan: Vec<PlanEntry>,
}

pub fn prepare_session(
    config_path: PathBuf,
    filter: Filter,
    data_root_cli: PathBuf,
    out_dir_legacy: Option<PathBuf>,
    baseline_cli: Option<PathBuf>,
    incremental_cli: Option<String>,
    manifest_cli: Option<PathBuf>,
) -> Result<PreparedSession> {
    let cfg = load_config(&config_path).map_err(|e| anyhow::anyhow!("{e}"))?;
    let repo_root = cfg.resolve_repo_root();
    let repo_root = std::fs::canonicalize(&repo_root)
        .with_context(|| format!("repo_root {:?}", repo_root))?;

    let data_root = resolve_data_root(&cfg, data_root_cli, out_dir_legacy, &repo_root)?;
    let artifact_dir = data_root.join("artifacts");
    std::fs::create_dir_all(&artifact_dir)?;

    let mut cases = resolve_cases(&cfg, &filter).map_err(|e| anyhow::anyhow!(e))?;
    if cases.is_empty() {
        bail!("no cases selected");
    }

    let inc = apply_incremental_overrides(
        &cfg.incremental,
        baseline_cli.as_ref(),
        incremental_cli.as_deref(),
        manifest_cli.as_ref(),
    )?;

    let manifest_path = if let Some(ref s) = inc.manifest_path {
        resolve_user_path_str(s)?
    } else {
        data_root.join("regress_manifest.json")
    };

    if needs_last_manifest(inc.strategy) {
        if !manifest_path.exists() {
            bail!(
                "incremental strategy {:?} requires manifest at {}",
                inc.strategy,
                manifest_path.display()
            );
        }
        let manifest = read_manifest(&manifest_path)?;
        cases = filter_cases_by_manifest(cases, &manifest);
        if cases.is_empty() {
            bail!(
                "no cases left after intersecting with manifest {}; check config vs manifest case_ids",
                manifest_path.display()
            );
        }
    }

    let ordered_case_ids: Vec<String> = cases.iter().map(|c| c.id.clone()).collect();

    let baseline_path_buf: Option<PathBuf> = if needs_baseline_report(inc.strategy) {
        if let Some(ref s) = inc.baseline_path {
            let pb = resolve_user_path_str(s)?;
            Some(pb)
        } else {
            let auto = data_root.join("report.json");
            if auto.exists() {
                Some(auto)
            } else {
                None
            }
        }
    } else {
        None
    };

    let baseline_report = if let Some(ref pb) = baseline_path_buf {
        if pb.exists() {
            Some(read_report_json(pb)?)
        } else {
            None
        }
    } else {
        None
    };

    let plan_strategy = effective_plan_strategy(inc.strategy);
    let plan = plan_runs(
        cases,
        baseline_report.as_ref(),
        plan_strategy,
        &repo_root,
        &cfg.defaults,
    );

    Ok(PreparedSession {
        config_path,
        repo_root,
        data_root,
        artifact_dir,
        cfg,
        filter,
        incremental: inc,
        manifest_path,
        ordered_case_ids,
        baseline_report,
        plan,
    })
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanActionJson {
    Run,
    SkippedUnchanged,
    SkippedScope,
}

#[derive(Serialize)]
pub struct PlanRowJson {
    pub action: PlanActionJson,
    pub case_id: String,
    pub kind: String,
    pub target: String,
}

pub fn case_kind_str(k: &CaseKind) -> &'static str {
    match k {
        CaseKind::Model => "model",
        CaseKind::Mos => "mos",
        CaseKind::CustomCommand => "custom_command",
    }
}

pub fn plan_row_json(entry: &PlanEntry) -> PlanRowJson {
    match entry {
        PlanEntry::Run(c) => PlanRowJson {
            action: PlanActionJson::Run,
            case_id: c.id.clone(),
            kind: case_kind_str(&c.kind).to_string(),
            target: c.target.clone(),
        },
        PlanEntry::SkippedUnchanged(c) => PlanRowJson {
            action: PlanActionJson::SkippedUnchanged,
            case_id: c.case_id.clone(),
            kind: "unknown".to_string(),
            target: String::new(),
        },
        PlanEntry::SkippedScope(c) => PlanRowJson {
            action: PlanActionJson::SkippedScope,
            case_id: c.case_id.clone(),
            kind: "unknown".to_string(),
            target: String::new(),
        },
    }
}

#[derive(Serialize)]
pub struct ListedCaseJson {
    pub id: String,
    pub kind: String,
    pub target: String,
    pub tags: Vec<String>,
}

pub fn listed_case_json(c: &CaseDef) -> ListedCaseJson {
    ListedCaseJson {
        id: c.id.clone(),
        kind: case_kind_str(&c.kind).to_string(),
        target: c.target.clone(),
        tags: c.tags.clone(),
    }
}
