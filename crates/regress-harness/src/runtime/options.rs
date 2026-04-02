use crate::config::HarnessConfig;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CliOverrides {
    pub workers: Option<usize>,
    pub solver: Option<String>,
    pub t_end: Option<f64>,
    pub dt: Option<f64>,
    pub fail_fast: Option<bool>,
    pub incremental: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CollectionMeta {
    pub workers: Option<usize>,
    pub solver: Option<String>,
    pub t_end: Option<f64>,
    pub dt: Option<f64>,
    pub fail_fast: Option<bool>,
    pub incremental: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunOptionsResolved {
    pub schema_version: u32,
    pub run_id: String,
    pub source_priority: Vec<String>,
    pub workers: usize,
    pub solver: String,
    pub t_end: f64,
    pub dt: f64,
    pub fail_fast: bool,
    pub incremental: String,
    pub generated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OptionConflict {
    pub code: String,
    pub field: String,
    pub message: String,
    pub fatal: bool,
}

pub fn merge_run_options(
    cli: &CliOverrides,
    collection: Option<&CollectionMeta>,
    cfg: &HarnessConfig,
    run_id: String,
) -> RunOptionsResolved {
    let col = collection.cloned().unwrap_or_default();
    let workers = cli
        .workers
        .or(col.workers)
        .unwrap_or(cfg.execution.workers)
        .max(1);
    let solver = cli
        .solver
        .clone()
        .or(col.solver)
        .unwrap_or_else(|| cfg.defaults.solver.clone());
    let t_end = cli.t_end.or(col.t_end).unwrap_or(cfg.defaults.t_end);
    let dt = cli.dt.or(col.dt).unwrap_or(cfg.defaults.dt);
    let fail_fast = cli
        .fail_fast
        .or(col.fail_fast)
        .unwrap_or(cfg.execution.fail_fast);
    let incremental = cli
        .incremental
        .clone()
        .or(col.incremental)
        .unwrap_or_else(|| format!("{:?}", cfg.incremental.strategy).to_ascii_lowercase());
    RunOptionsResolved {
        schema_version: 1,
        run_id,
        source_priority: vec![
            "cli".to_string(),
            "collection".to_string(),
            "config".to_string(),
            "case".to_string(),
        ],
        workers,
        solver,
        t_end,
        dt,
        fail_fast,
        incremental,
        generated_at: chrono::Utc::now().to_rfc3339(),
    }
}

pub fn detect_option_conflicts(opt: &RunOptionsResolved) -> Vec<OptionConflict> {
    let mut out = Vec::new();
    if opt.workers == 0 {
        out.push(OptionConflict {
            code: "E_OPTION_001".to_string(),
            field: "workers".to_string(),
            message: "workers must be >= 1".to_string(),
            fatal: true,
        });
    }
    if opt.dt <= 0.0 {
        out.push(OptionConflict {
            code: "E_OPTION_001".to_string(),
            field: "dt".to_string(),
            message: "dt must be > 0".to_string(),
            fatal: true,
        });
    }
    if opt.t_end <= 0.0 {
        out.push(OptionConflict {
            code: "E_OPTION_001".to_string(),
            field: "t_end".to_string(),
            message: "t_end must be > 0".to_string(),
            fatal: true,
        });
    }
    out
}

pub fn validate_run_options(opt: &RunOptionsResolved) -> anyhow::Result<()> {
    let conflicts = detect_option_conflicts(opt);
    if let Some(c) = conflicts.iter().find(|x| x.fatal) {
        anyhow::bail!("{}: {}", c.code, c.message);
    }
    Ok(())
}

pub fn write_run_options_snapshot(
    run_id: &str,
    opt: &RunOptionsResolved,
    data_root: &Path,
) -> anyhow::Result<()> {
    let run_dir = data_root.join("runs").join(run_id);
    std::fs::create_dir_all(&run_dir)?;
    let path = run_dir.join("run_options.json");
    let text = serde_json::to_string_pretty(opt)?;
    std::fs::write(path, text)?;
    Ok(())
}
