//! Catalog FQN gate inventory (Q4-A P519+) and minimal-model validate runner.

use crate::catalog::builtin_catalog;
use crate::diagram::{CatalogEntry, CatalogParam, PortDef};
use crate::diagram::StudioSimOptions;
use crate::sim_bridge;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GateInventoryEntry {
    pub id: String,
    pub type_name: String,
    pub domain: String,
    pub category: String,
    /// signal-core | physical-core | routing | multibody | draw | terminal | other
    pub group: String,
    /// validate | validate_sim | skip
    pub gate: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<String>,
    pub requires_library: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GateInventoryDoc {
    pub schema_version: u32,
    pub updated_at: String,
    pub note: String,
    pub entries: Vec<GateInventoryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GateRunResult {
    pub id: String,
    pub type_name: String,
    pub group: String,
    pub gate: String,
    pub ok: bool,
    pub stage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(default)]
    pub skipped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GateRunReport {
    pub schema_version: u32,
    pub updated_at: String,
    pub groups: Vec<String>,
    pub total: usize,
    pub passed: usize,
    pub failed: usize,
    pub skipped: usize,
    pub results: Vec<GateRunResult>,
}

fn today() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{secs}")
}

fn safe_ident(id: &str) -> String {
    let mut out = String::from("G_");
    for c in id.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    out
}

fn classify(e: &CatalogEntry) -> GateInventoryEntry {
    let tags = e.tags.clone();
    let is_draw = e.domain == "draw"
        || tags.iter().any(|t| t == "draw" || t == "annotation")
        || e.type_name.starts_with("Studio.Draw.");
    if is_draw {
        return GateInventoryEntry {
            id: e.id.clone(),
            type_name: e.type_name.clone(),
            domain: e.domain.clone(),
            category: e.category.clone(),
            group: "draw".into(),
            gate: "skip".into(),
            skip_reason: Some("draw annotation; not emitted to Modelica".into()),
            requires_library: e.requires_library.clone(),
            tags,
        };
    }
    if e.fmu_asset.is_some() || tags.iter().any(|t| t == "fmu") {
        return GateInventoryEntry {
            id: e.id.clone(),
            type_name: e.type_name.clone(),
            domain: e.domain.clone(),
            category: e.category.clone(),
            group: "other".into(),
            gate: "skip".into(),
            skip_reason: Some("FMU asset; gated via import cosim path".into()),
            requires_library: e.requires_library.clone(),
            tags,
        };
    }
    let cat = e.category.to_lowercase();
    if cat == "terminals"
        || e.type_name.ends_with(".RealInput")
        || e.type_name.ends_with(".RealOutput")
        || e.id.contains("terminals.")
    {
        return GateInventoryEntry {
            id: e.id.clone(),
            type_name: e.type_name.clone(),
            domain: e.domain.clone(),
            category: e.category.clone(),
            group: "terminal".into(),
            gate: "skip".into(),
            skip_reason: Some("terminal ports require parent subsystem harness".into()),
            requires_library: e.requires_library.clone(),
            tags,
        };
    }
    // Goto/From alone are incomplete; covered by emit golden P524.
    if e.id.contains("goto") || e.id.contains(".from") || e.id.ends_with(".from") {
        return GateInventoryEntry {
            id: e.id.clone(),
            type_name: e.type_name.clone(),
            domain: e.domain.clone(),
            category: e.category.clone(),
            group: "routing".into(),
            gate: "skip".into(),
            skip_reason: Some("Goto/From need paired tag harness (emit golden)".into()),
            requires_library: e.requires_library.clone(),
            tags,
        };
    }

    let group = if e.type_name.contains("MultiBody") || cat == "multibody" || e.domain == "multibody"
    {
        "multibody"
    } else if e.domain == "electrical"
        || e.domain == "mechanical"
        || e.type_name.contains("Electrical.Analog")
        || e.type_name.contains("Mechanics.Rotational")
        || e.type_name.contains("Mechanics.Translational")
    {
        "physical-core"
    } else if cat == "routing" || e.type_name.contains("Blocks.Routing") {
        "routing"
    } else if e.domain == "signal"
        || e.type_name.contains("Blocks.")
        || cat == "sources"
        || cat == "math"
        || cat == "continuous"
        || cat == "discrete"
        || cat == "tables"
        || cat == "logical"
        || cat == "nonlinear"
        || cat == "sinks"
    {
        "signal-core"
    } else {
        "other"
    };

    let is_source = cat == "sources"
        || tags.iter().any(|t| t == "source")
        || e.ports.iter().all(|p| p.direction != "in");

    let gate = if group == "signal-core" && is_source {
        "validate_sim"
    } else if group == "multibody" {
        // Instantiate-only analyze; full sim needs world/joints harness.
        "validate"
    } else if group == "physical-core" || group == "signal-core" || group == "routing" {
        "validate"
    } else {
        "validate"
    };

    GateInventoryEntry {
        id: e.id.clone(),
        type_name: e.type_name.clone(),
        domain: e.domain.clone(),
        category: e.category.clone(),
        group: group.into(),
        gate: gate.into(),
        skip_reason: None,
        requires_library: e.requires_library.clone(),
        tags,
    }
}

pub fn build_inventory() -> GateInventoryDoc {
    let mut entries: Vec<_> = builtin_catalog().iter().map(classify).collect();
    entries.sort_by(|a, b| a.id.cmp(&b.id));
    GateInventoryDoc {
        schema_version: 1,
        updated_at: today(),
        note: "Q4-A catalog gate matrix. gate=validate|validate_sim|skip. Draw/terminals/Goto-From skipped with reason.".into(),
        entries,
    }
}

pub fn dump_inventory(path: &Path) -> Result<(), String> {
    let doc = build_inventory();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?;
    fs::write(path, json).map_err(|e| e.to_string())?;
    Ok(())
}

fn format_params(params: &[CatalogParam]) -> String {
    if params.is_empty() {
        return String::new();
    }
    let inner = params
        .iter()
        .map(|p| {
            // Quote string-typed or obviously non-numeric values.
            let v = &p.value;
            let needs_quote = p.value_type.as_deref() == Some("String")
                || (!v.is_empty()
                    && !v.chars().next().unwrap().is_ascii_digit()
                    && !v.starts_with('-')
                    && v != "true"
                    && v != "false"
                    && !v.contains('[')
                    && !v.contains('{'));
            if needs_quote && !v.starts_with('"') {
                format!("{}=\"{}\"", p.name, v.replace('"', ""))
            } else {
                format!("{}={}", p.name, v)
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("({inner})")
}

fn bool_in_ports(ports: &[PortDef]) -> bool {
    ports.iter().any(|p| {
        p.direction == "in"
            && (p.type_name.contains("Boolean") || p.id.starts_with("u") && p.type_name.contains("Boolean"))
    })
}

/// Minimal Modelica model that instantiates the catalog type (signal inputs driven).
pub fn minimal_model_source(entry: &CatalogEntry) -> (String, String) {
    let model = safe_ident(&entry.id);
    let inst = entry.default_instance_prefix.clone();
    let params = format_params(&entry.default_params);
    let mut decls = Vec::new();
    let mut eqs = Vec::new();

    let signal_ins: Vec<&PortDef> = entry
        .ports
        .iter()
        .filter(|p| p.direction == "in" && p.kind == "signal")
        .collect();

    let use_bool = bool_in_ports(&entry.ports);
    for (i, p) in signal_ins.iter().enumerate() {
        let src = format!("src{i}");
        if p.type_name.contains("Boolean") || (use_bool && p.type_name.contains("Boolean")) {
            decls.push(format!(
                "  Modelica.Blocks.Sources.BooleanConstant {src}(k=true);"
            ));
        } else if p.type_name.contains("Integer") {
            decls.push(format!("  Modelica.Blocks.Sources.IntegerConstant {src}(k=1);"));
        } else {
            decls.push(format!("  Modelica.Blocks.Sources.Constant {src}(k=1);"));
        }
        eqs.push(format!("  connect({src}.y, {inst}.{});", p.id));
    }

    decls.push(format!("  {} {}{};", entry.type_name, inst, params));

    let eq_block = if eqs.is_empty() {
        String::new()
    } else {
        format!("equation\n{}\n", eqs.join("\n"))
    };
    let code = format!(
        "model {model}\n{}\n{eq_block}end {model};\n",
        decls.join("\n")
    );
    (model, code)
}

fn find_catalog(id: &str) -> Option<CatalogEntry> {
    builtin_catalog().into_iter().find(|e| e.id == id)
}

pub fn run_gate(groups: &[&str], out_path: &Path) -> Result<GateRunReport, String> {
    let inv = build_inventory();
    let group_set: Vec<String> = if groups.is_empty() {
        vec![
            "signal-core".into(),
            "physical-core".into(),
            "routing".into(),
        ]
    } else {
        groups.iter().map(|s| (*s).to_string()).collect()
    };

    let sim = StudioSimOptions {
        stop_time: 0.1,
        number_of_intervals: 2,
        solver: "rk4".into(),
        tolerance: 1e-4,
    };

    let mut results = Vec::new();
    let mut passed = 0usize;
    let mut failed = 0usize;
    let mut skipped = 0usize;

    for ge in &inv.entries {
        if !group_set.iter().any(|g| g == &ge.group) {
            continue;
        }
        if ge.gate == "skip" {
            skipped += 1;
            results.push(GateRunResult {
                id: ge.id.clone(),
                type_name: ge.type_name.clone(),
                group: ge.group.clone(),
                gate: ge.gate.clone(),
                ok: true,
                stage: "skip".into(),
                error: ge.skip_reason.clone(),
                skipped: true,
            });
            continue;
        }
        let Some(entry) = find_catalog(&ge.id) else {
            failed += 1;
            results.push(GateRunResult {
                id: ge.id.clone(),
                type_name: ge.type_name.clone(),
                group: ge.group.clone(),
                gate: ge.gate.clone(),
                ok: false,
                stage: "missing".into(),
                error: Some("catalog entry not found".into()),
                skipped: false,
            });
            continue;
        };
        let (model_name, code) = minimal_model_source(&entry);
        let report = sim_bridge::validate_model(&code, &model_name, &[], &sim, false);
        if report.success {
            passed += 1;
            results.push(GateRunResult {
                id: ge.id.clone(),
                type_name: ge.type_name.clone(),
                group: ge.group.clone(),
                gate: ge.gate.clone(),
                ok: true,
                stage: report.stage,
                error: None,
                skipped: false,
            });
        } else {
            failed += 1;
            let err = report
                .errors
                .first()
                .map(|e| e.message.clone())
                .unwrap_or_else(|| "validate failed".into());
            results.push(GateRunResult {
                id: ge.id.clone(),
                type_name: ge.type_name.clone(),
                group: ge.group.clone(),
                gate: ge.gate.clone(),
                ok: false,
                stage: report.stage,
                error: Some(err),
                skipped: false,
            });
        }
    }

    let report = GateRunReport {
        schema_version: 1,
        updated_at: today(),
        groups: group_set,
        total: results.len(),
        passed,
        failed,
        skipped,
        results,
    };
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    fs::write(out_path, json).map_err(|e| e.to_string())?;
    Ok(report)
}

/// CLI: `--dump-catalog-gate=<path>` or `--run-catalog-gate=<reportPath> [--gate-groups=a,b]`.
pub fn try_run_catalog_gate_cli() -> Option<std::process::ExitCode> {
    let args: Vec<String> = std::env::args().collect();
    let mut dump: Option<String> = None;
    let mut run: Option<String> = None;
    let mut groups: Vec<String> = Vec::new();
    for a in &args {
        if let Some(p) = a.strip_prefix("--dump-catalog-gate=") {
            dump = Some(p.to_string());
        }
        if let Some(p) = a.strip_prefix("--run-catalog-gate=") {
            run = Some(p.to_string());
        }
        if let Some(g) = a.strip_prefix("--gate-groups=") {
            groups = g.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect();
        }
    }
    if dump.is_none() && run.is_none() {
        return None;
    }
    if let Some(p) = dump {
        match dump_inventory(Path::new(&p)) {
            Ok(()) => {
                eprintln!("catalog gate inventory written: {p}");
            }
            Err(e) => {
                eprintln!("dump-catalog-gate failed: {e}");
                return Some(std::process::ExitCode::from(1));
            }
        }
    }
    if let Some(p) = run {
        let grefs: Vec<&str> = groups.iter().map(|s| s.as_str()).collect();
        match run_gate(&grefs, Path::new(&p)) {
            Ok(rep) => {
                eprintln!(
                    "catalog gate: total={} passed={} failed={} skipped={} -> {p}",
                    rep.total, rep.passed, rep.failed, rep.skipped
                );
                if rep.failed > 0 {
                    return Some(std::process::ExitCode::from(2));
                }
            }
            Err(e) => {
                eprintln!("run-catalog-gate failed: {e}");
                return Some(std::process::ExitCode::from(1));
            }
        }
    }
    Some(std::process::ExitCode::SUCCESS)
}
