//! Wrap a Modelica `.mo` class as a Studio catalog block:
//! auto-extract multiple inputs/outputs (and connectors) + parameters.

use crate::diagram::{CatalogEntry, CatalogParam, PortDef};
use rustmodlica::annotation;
use rustmodlica::ast::{ClassItem, Declaration, Expression, Model};
use rustmodlica::parser;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WrapMoResult {
    pub success: bool,
    pub error: Option<String>,
    pub catalog_entry: Option<CatalogEntry>,
    pub model_name: String,
    pub type_name: String,
    pub asset_path: String,
    pub input_count: u32,
    pub output_count: u32,
    pub connector_count: u32,
    pub param_count: u32,
    pub has_icon: bool,
}

fn safe_id(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "block".into()
    } else if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("b_{out}")
    } else {
        out
    }
}

/// Public wrapper for pack / catalog id sanitizing.
pub fn safe_id_pub(s: &str) -> String {
    safe_id(s)
}

fn expr_lit(e: &Expression) -> Option<String> {
    match e {
        Expression::Number(n) => Some(format!("{n}")),
        Expression::Variable(v) => Some(v.as_str()),
        Expression::ArrayLiteral(items) => {
            let parts: Vec<String> = items.iter().filter_map(expr_lit).collect();
            if parts.len() == items.len() {
                Some(format!("{{{}}}", parts.join(",")))
            } else {
                None
            }
        }
        _ => None,
    }
}

fn is_signal_port_type(type_name: &str) -> bool {
    let t = type_name.to_ascii_lowercase();
    t.contains("realinput")
        || t.contains("realoutput")
        || t.contains("integerinput")
        || t.contains("integeroutput")
        || t.contains("booleaninput")
        || t.contains("booleanoutput")
        || t.ends_with("input")
        || t.ends_with("output")
}

fn decl_to_port(d: &Declaration) -> Option<PortDef> {
    if d.is_parameter || d.is_protected {
        return None;
    }
    if d.is_input {
        return Some(PortDef {
            id: d.name.clone(),
            kind: "signal".into(),
            direction: "in".into(),
            type_name: if d.type_name.is_empty() {
                "RealInput".into()
            } else {
                d.type_name.clone()
            },
        });
    }
    if d.is_output {
        return Some(PortDef {
            id: d.name.clone(),
            kind: "signal".into(),
            direction: "out".into(),
            type_name: if d.type_name.is_empty() {
                "RealOutput".into()
            } else {
                d.type_name.clone()
            },
        });
    }
    // Physical / bidirectional connectors (Pin, Flange, …) without input/output prefix
    if is_signal_port_type(&d.type_name) {
        let lower = d.type_name.to_ascii_lowercase();
        let direction = if lower.contains("input") {
            "in"
        } else if lower.contains("output") {
            "out"
        } else {
            "bidirectional"
        };
        return Some(PortDef {
            id: d.name.clone(),
            kind: if direction == "bidirectional" {
                "connector".into()
            } else {
                "signal".into()
            },
            direction: direction.into(),
            type_name: d.type_name.clone(),
        });
    }
    let lower = d.type_name.to_ascii_lowercase();
    if lower.contains("pin")
        || lower.contains("flange")
        || lower.contains("frame_")
        || lower.contains("heatport")
        || lower.contains("connector")
    {
        return Some(PortDef {
            id: d.name.clone(),
            kind: "connector".into(),
            direction: "bidirectional".into(),
            type_name: d.type_name.clone(),
        });
    }
    None
}

fn decl_to_param(d: &Declaration) -> Option<CatalogParam> {
    if !d.is_parameter || d.is_protected {
        return None;
    }
    let value = d
        .start_value
        .as_ref()
        .and_then(expr_lit)
        .or_else(|| {
            d.modifications
                .iter()
                .find(|m| m.name.is_empty() || m.name == d.name)
                .and_then(|m| m.value.as_ref())
                .and_then(expr_lit)
        })
        .unwrap_or_else(|| "0".into());
    Some(CatalogParam::typed(d.name.clone(), value, &d.type_name))
}

pub fn model_to_catalog(model: &Model, source_stem: &str) -> CatalogEntry {
    let mut ports = Vec::new();
    let mut params = Vec::new();
    for d in &model.declarations {
        if let Some(p) = decl_to_port(d) {
            ports.push(p);
        }
        if let Some(p) = decl_to_param(d) {
            params.push(p);
        }
    }
    let domain = if ports.iter().any(|p| p.kind == "connector") {
        if ports.iter().any(|p| p.type_name.to_ascii_lowercase().contains("pin")) {
            "electrical"
        } else {
            "other"
        }
    } else {
        "signal"
    };
    let prefix = safe_id(&model.name).to_ascii_lowercase();
    let icon = model
        .annotation
        .as_ref()
        .and_then(|raw| annotation::parse_icon(raw))
        .and_then(|ann| serde_json::to_value(ann).ok())
        .filter(|v| {
            v.get("graphics")
                .and_then(|g| g.as_array())
                .map(|a| !a.is_empty())
                .unwrap_or(false)
        });
    let mut tags = vec!["wrapped".into(), "modelRef".into(), "auto-ports".into()];
    if icon.is_some() {
        tags.push("has-icon".into());
    }
    CatalogEntry {
        id: format!("modelref.{}", safe_id(&model.name)),
        type_name: model.name.clone(),
        domain: domain.into(),
        category: "ModelRef".into(),
        display_key: format!("catalog.modelref.{}", safe_id(&model.name)),
        default_instance_prefix: if prefix.is_empty() {
            safe_id(source_stem).to_ascii_lowercase()
        } else {
            prefix
        },
        requires_library: "ModelRef".into(),
        default_params: params,
        ports,
        tags,
        plot_hints: Vec::new(),
        fmu_asset: None,
        fmu_guid: None,
        fmu_model_identifier: None,
        icon,
    }
}

fn pick_model(item: ClassItem) -> Option<Model> {
    match item {
        ClassItem::Model(m) => {
            if m.is_function {
                None
            } else {
                Some(m)
            }
        }
        ClassItem::Function(_) => None,
    }
}

/// Parse `.mo`, copy into `{project}/wrapped/`, return catalog entry with auto ports/params.
pub fn wrap_mo_file(mo_path: &Path, project_root: Option<&Path>) -> WrapMoResult {
    let empty = || WrapMoResult {
        success: false,
        error: Some("wrap failed".into()),
        catalog_entry: None,
        model_name: String::new(),
        type_name: String::new(),
        asset_path: String::new(),
        input_count: 0,
        output_count: 0,
        connector_count: 0,
        param_count: 0,
        has_icon: false,
    };

    let content = match fs::read_to_string(mo_path) {
        Ok(s) => s,
        Err(e) => {
            let mut r = empty();
            r.error = Some(format!("read mo: {e}"));
            return r;
        }
    };

    let item = match parser::parse(&content) {
        Ok(i) => i,
        Err(e) => {
            let mut r = empty();
            r.error = Some(format!("parse mo: {e}"));
            return r;
        }
    };

    let model = match pick_model(item) {
        Some(m) => m,
        None => {
            let mut r = empty();
            r.error = Some("file is not a model/block (functions not supported)".into());
            return r;
        }
    };

    let mut entry = model_to_catalog(
        &model,
        mo_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("block"),
    );

    let asset_path = if let Some(root) = project_root {
        let dest_dir = root.join("wrapped");
        if let Err(e) = fs::create_dir_all(&dest_dir) {
            let mut r = empty();
            r.error = Some(format!("create wrapped dir: {e}"));
            return r;
        }
        let dest = dest_dir.join(format!("{}.mo", model.name));
        if let Err(e) = fs::copy(mo_path, &dest) {
            let mut r = empty();
            r.error = Some(format!("copy mo: {e}"));
            return r;
        }
        // Relative path for reopen; not an FMU asset.
        entry.tags.push(format!("asset:wrapped/{}.mo", model.name));
        dest.display().to_string()
    } else {
        mo_path.display().to_string()
    };

    let input_count = entry
        .ports
        .iter()
        .filter(|p| p.direction == "in")
        .count() as u32;
    let output_count = entry
        .ports
        .iter()
        .filter(|p| p.direction == "out")
        .count() as u32;
    let connector_count = entry
        .ports
        .iter()
        .filter(|p| p.kind == "connector")
        .count() as u32;
    let param_count = entry.default_params.len() as u32;
    let has_icon = entry.icon.is_some();

    WrapMoResult {
        success: true,
        error: None,
        model_name: model.name.clone(),
        type_name: entry.type_name.clone(),
        catalog_entry: Some(entry),
        asset_path,
        input_count,
        output_count,
        connector_count,
        param_count,
        has_icon,
    }
}

pub fn list_wrapped(project_root: &Path) -> Vec<CatalogEntry> {
    let dir = project_root.join("wrapped");
    let Ok(rd) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in rd.flatten() {
        let path = e.path();
        if path.extension().and_then(|x| x.to_str()) != Some("mo") {
            continue;
        }
        let res = wrap_mo_file(&path, None);
        if let Some(mut entry) = res.catalog_entry {
            let file = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("block.mo");
            entry.tags.push(format!("asset:wrapped/{file}"));
            out.push(entry);
        }
    }
    out
}

#[allow(dead_code)]
pub fn wrapped_dir(project_root: &Path) -> PathBuf {
    project_root.join("wrapped")
}
