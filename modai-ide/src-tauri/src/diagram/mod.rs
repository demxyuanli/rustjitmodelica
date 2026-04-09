mod types;
mod equation;

pub use types::*;
pub use equation::*;

/// New `.mo` source plus optional warning when `.modai/diagram-state.json` persistence fails.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyDiagramEditsOutput {
    pub new_source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

use rustmodlica::annotation::{
    self, format_icon_diagram_record, IconDiagramAnnotation, LineAnnotation, Placement, Point,
};
use rustmodlica::ast::{
    connector_path_to_expr, expr_to_connector_path, ClassItem, Declaration, Equation, Expression,
    Model,
};
use rustmodlica::parser;
use rustmodlica::unparse;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::time::Duration;

const STATE_FILE_NAME: &str = ".modai/diagram-state.json";
const LEGACY_LAYOUT_FILE_NAME: &str = ".modai/diagram-layout.json";
/// Matches `COORD_KEY_DECIMALS` in `structureEditor/docSync.ts`.
const COORD_DECIMALS: i32 = 4;
const MAX_DIAGRAM_STATE_FILE_BYTES: u64 = 10 * 1024 * 1024;

fn round_coord_f64(n: f64) -> f64 {
    let f = 10_f64.powi(COORD_DECIMALS);
    (n * f).round() / f
}

fn round_layout_point(p: &LayoutPoint) -> LayoutPoint {
    LayoutPoint {
        x: round_coord_f64(p.x),
        y: round_coord_f64(p.y),
    }
}

fn round_line_annotation(line: &LineAnnotation) -> LineAnnotation {
    LineAnnotation {
        points: line
            .points
            .iter()
            .map(|p| Point {
                x: round_coord_f64(p.x),
                y: round_coord_f64(p.y),
            })
            .collect(),
        color: line.color.clone(),
        thickness: line.thickness,
        pattern: line.pattern.clone(),
        smooth: line.smooth.clone(),
    }
}

fn round_persistent_state_in_place(state: &mut DiagramPersistentState) {
    for v in state.layout.values_mut() {
        *v = round_layout_point(v);
    }
    for line in state.connection_lines.values_mut() {
        *line = round_line_annotation(line);
    }
}

fn round_persistent_state(state: &DiagramPersistentState) -> DiagramPersistentState {
    let mut out = state.clone();
    round_persistent_state_in_place(&mut out);
    out
}

/// Rejects path traversal and absolute paths; `relative_path` is a project-relative file path.
fn validate_relative_path(relative_path: &str) -> Result<(), String> {
    let s = relative_path.trim();
    if s.is_empty() {
        return Err("relative path is empty".to_string());
    }
    let norm = normalize_relative_path(s);
    if norm.starts_with('/') {
        return Err("relative path must not be absolute".to_string());
    }
    if norm.contains(':') {
        return Err("relative path is invalid".to_string());
    }
    for part in norm.split('/') {
        if part == ".." {
            return Err("relative path must not contain parent segments".to_string());
        }
    }
    Ok(())
}

fn read_state_file_limited(path: &Path) -> Result<String, String> {
    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.len() > MAX_DIAGRAM_STATE_FILE_BYTES {
        return Err(format!(
            "diagram state file exceeds {} bytes",
            MAX_DIAGRAM_STATE_FILE_BYTES
        ));
    }
    std::fs::read_to_string(path).map_err(|e| e.to_string())
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagramPersistentState {
    #[serde(default)]
    layout: BTreeMap<String, LayoutPoint>,
    #[serde(default)]
    connection_lines: BTreeMap<String, LineAnnotation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    diagram_annotation: Option<IconDiagramAnnotation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon_annotation: Option<IconDiagramAnnotation>,
}

fn state_file_path(project_dir: &str) -> std::path::PathBuf {
    Path::new(project_dir).join(STATE_FILE_NAME)
}

fn legacy_layout_file_path(project_dir: &str) -> std::path::PathBuf {
    Path::new(project_dir).join(LEGACY_LAYOUT_FILE_NAME)
}

fn normalize_relative_path(relative_path: &str) -> String {
    relative_path.replace('\\', "/")
}

fn load_legacy_layout_from_file(
    project_dir: &str,
    relative_path: &str,
) -> Option<HashMap<String, LayoutPoint>> {
    let path = legacy_layout_file_path(project_dir);
    let content = read_state_file_limited(&path).ok()?;
    let all: HashMap<String, HashMap<String, LayoutPoint>> =
        serde_json::from_str(&content).ok()?;
    all.get(&normalize_relative_path(relative_path)).cloned()
}

fn load_persistent_state(
    project_dir: &str,
    relative_path: &str,
) -> Option<DiagramPersistentState> {
    validate_relative_path(relative_path).ok()?;
    let path = state_file_path(project_dir);
    if path.exists() {
        match read_state_file_limited(&path) {
            Ok(content) => {
                if let Ok(all) = serde_json::from_str::<BTreeMap<String, DiagramPersistentState>>(&content)
                {
                    let key = normalize_relative_path(relative_path);
                    if let Some(mut state) = all.get(&key).cloned() {
                        round_persistent_state_in_place(&mut state);
                        return Some(state);
                    }
                }
            }
            Err(e) => {
                tracing::warn!(target: "modai_diagram", "diagram state file not loaded: {}", e);
            }
        }
    }
    load_legacy_layout_from_file(project_dir, relative_path).map(|layout| {
        let mut state = DiagramPersistentState {
            layout: layout.into_iter().collect(),
            ..DiagramPersistentState::default()
        };
        round_persistent_state_in_place(&mut state);
        state
    })
}

/// Async read of `.modai/diagram-state.json` (and legacy layout file) so the Tauri async runtime
/// is not blocked on disk I/O before CPU-heavy parse runs in `spawn_blocking`.
async fn load_persistent_state_async(
    project_dir: &str,
    relative_path: &str,
) -> Option<DiagramPersistentState> {
    validate_relative_path(relative_path).ok()?;
    let path = state_file_path(project_dir);
    if let Ok(meta) = tokio::fs::metadata(&path).await {
        if meta.len() > MAX_DIAGRAM_STATE_FILE_BYTES {
            tracing::warn!(
                target: "modai_diagram",
                "diagram state file too large: {} bytes",
                meta.len()
            );
            return None;
        }
        if let Ok(content) = tokio::fs::read_to_string(&path).await {
            if let Ok(all) = serde_json::from_str::<BTreeMap<String, DiagramPersistentState>>(&content)
            {
                let key = normalize_relative_path(relative_path);
                if let Some(mut state) = all.get(&key).cloned() {
                    round_persistent_state_in_place(&mut state);
                    return Some(state);
                }
            }
        }
    }
    load_legacy_layout_from_file_async(project_dir, relative_path).await
}

async fn load_legacy_layout_from_file_async(
    project_dir: &str,
    relative_path: &str,
) -> Option<DiagramPersistentState> {
    let path = legacy_layout_file_path(project_dir);
    let meta = tokio::fs::metadata(&path).await.ok()?;
    if meta.len() > MAX_DIAGRAM_STATE_FILE_BYTES {
        tracing::warn!(
            target: "modai_diagram",
            "legacy diagram layout file too large: {} bytes",
            meta.len()
        );
        return None;
    }
    let content = tokio::fs::read_to_string(&path).await.ok()?;
    let all: HashMap<String, HashMap<String, LayoutPoint>> = serde_json::from_str(&content).ok()?;
    let layout = all.get(&normalize_relative_path(relative_path))?.clone();
    let mut state = DiagramPersistentState {
        layout: layout.into_iter().collect(),
        ..DiagramPersistentState::default()
    };
    round_persistent_state_in_place(&mut state);
    Some(state)
}

fn save_persistent_state(
    project_dir: &str,
    relative_path: &str,
    state: &DiagramPersistentState,
) -> Result<(), String> {
    validate_relative_path(relative_path)?;
    let state = round_persistent_state(state);
    let path = state_file_path(project_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let key = normalize_relative_path(relative_path);
    let mut all: BTreeMap<String, DiagramPersistentState> = match read_state_file_limited(&path) {
        Ok(c) => serde_json::from_str(&c).unwrap_or_default(),
        Err(e) => {
            if path.exists() {
                tracing::warn!(
                    target: "modai_diagram",
                    "could not read existing diagram state for merge: {}",
                    e
                );
            }
            BTreeMap::new()
        }
    };
    all.insert(key, state);
    let content = serde_json::to_string_pretty(&all).map_err(|e| e.to_string())?;
    std::fs::write(&path, content).map_err(|e| e.to_string())?;
    Ok(())
}

fn parse_layout_from_annotation(
    annotation: Option<&String>,
) -> Option<HashMap<String, LayoutPoint>> {
    let s = annotation.as_ref()?.as_str();
    let key = "__DiagramLayout=\"";
    let start = s.find(key)?;
    let rest = &s[start + key.len()..];
    let mut end = 0usize;
    let mut i = 0;
    let bytes = rest.as_bytes();
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            i += 2;
            continue;
        }
        if bytes[i] == b'"' {
            end = i;
            break;
        }
        i += 1;
    }
    let value = &rest[..end];
    let json_str = value.replace("\\\"", "\"").replace("\\\\", "\\");
    let parsed: HashMap<String, LayoutPoint> = serde_json::from_str(&json_str).ok()?;
    Some(parsed)
}

/// Remove __DiagramLayout="..." from annotation inner (value may contain commas and escaped quotes).
fn strip_diagram_layout_from_annotation_inner(inner: &str) -> String {
    const KEY: &str = "__DiagramLayout=\"";
    let mut out = String::new();
    let mut rest = inner;
    while let Some(start) = rest.find(KEY) {
        out.push_str(&rest[..start]);
        rest = &rest[start + KEY.len()..];
        let mut i = 0usize;
        let bytes = rest.as_bytes();
        while i < bytes.len() {
            if bytes[i] == b'\\' && i + 1 < bytes.len() {
                i += 2;
                continue;
            }
            if bytes[i] == b'"' {
                rest = &rest[i + 1..];
                break;
            }
            i += 1;
        }
    }
    out.push_str(rest);
    out.trim_matches(',').trim().to_string()
}

fn split_top_level_items(input: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut depth_paren = 0i32;
    let mut depth_brace = 0i32;
    let mut in_string = false;
    let mut escape = false;
    for ch in input.chars() {
        if in_string {
            current.push(ch);
            if escape {
                escape = false;
            } else if ch == '\\' {
                escape = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => {
                in_string = true;
                current.push(ch);
            }
            '(' => {
                depth_paren += 1;
                current.push(ch);
            }
            ')' => {
                depth_paren -= 1;
                current.push(ch);
            }
            '{' => {
                depth_brace += 1;
                current.push(ch);
            }
            '}' => {
                depth_brace -= 1;
                current.push(ch);
            }
            ',' if depth_paren == 0 && depth_brace == 0 => {
                let trimmed = current.trim();
                if !trimmed.is_empty() {
                    items.push(trimmed.to_string());
                }
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    let trimmed = current.trim();
    if !trimmed.is_empty() {
        items.push(trimmed.to_string());
    }
    items
}

fn annotation_inner(raw: &str) -> Option<String> {
    raw.trim()
        .strip_suffix(';')
        .unwrap_or(raw.trim())
        .trim()
        .strip_prefix("annotation(")
        .and_then(|s| s.strip_suffix(')'))
        .map(|s| s.trim().to_string())
}

fn format_point(p: &Point) -> String {
    format!("{{{},{}}}", p.x, p.y)
}

fn format_extent(p1: &Point, p2: &Point) -> String {
    format!("{{{},{}}}", format_point(p1), format_point(p2))
}

fn format_transformation(t: &annotation::Transformation) -> String {
    let mut parts = Vec::new();
    if let Some(ref origin) = t.origin {
        parts.push(format!("origin={}", format_point(origin)));
    }
    if let Some(ref extent) = t.extent {
        parts.push(format!("extent={}", format_extent(&extent.p1, &extent.p2)));
    }
    if let Some(rotation) = t.rotation {
        parts.push(format!("rotation={}", rotation));
    }
    format!("transformation({})", parts.join(", "))
}

fn format_placement(placement: &Placement) -> String {
    let mut parts = Vec::new();
    if let Some(ref transformation) = placement.transformation {
        parts.push(format_transformation(transformation));
    }
    if let Some(ref transformation) = placement.icon_transformation {
        parts.push(format!(
            "iconTransformation({})",
            format_transformation(transformation)
                .trim_start_matches("transformation(")
                .trim_end_matches(')')
        ));
    }
    if let Some(visible) = placement.visible {
        parts.push(format!("visible={}", if visible { "true" } else { "false" }));
    }
    format!("Placement({})", parts.join(", "))
}

fn upsert_annotation_item(
    existing: Option<&String>,
    prefix: &str,
    replacement: Option<String>,
) -> Option<String> {
    let inner = existing.and_then(|raw| annotation_inner(raw));
    let mut items = inner
        .as_deref()
        .map(split_top_level_items)
        .unwrap_or_default()
        .into_iter()
        .filter(|item| !item.trim().starts_with(prefix))
        .collect::<Vec<_>>();
    if let Some(value) = replacement {
        items.push(value);
    }
    if items.is_empty() {
        None
    } else {
        Some(format!("annotation({});", items.join(", ")))
    }
}

fn connection_key(from: &str, to: &str) -> String {
    format!("{}->{}", from, to)
}

fn expr_to_display_string(e: &Expression) -> String {
    match e {
        Expression::Number(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                format!("{}", n)
            }
        }
        Expression::Variable(id) => rustmodlica::string_intern::resolve_id(*id),
        Expression::StringLiteral(s) => format!("\"{}\"", s),
        Expression::BinaryOp(l, op, r) => {
            let ops = match op {
                rustmodlica::ast::Operator::Add => "+",
                rustmodlica::ast::Operator::Sub => "-",
                rustmodlica::ast::Operator::Mul => "*",
                rustmodlica::ast::Operator::Div => "/",
                _ => "?",
            };
            format!(
                "{}{}{}",
                expr_to_display_string(l),
                ops,
                expr_to_display_string(r)
            )
        }
        _ => String::new(),
    }
}

fn extract_params(d: &Declaration) -> Option<Vec<ParamValue>> {
    let mut params = Vec::new();
    if let Some(ref sv) = d.start_value {
        let vs = expr_to_display_string(sv);
        if !vs.is_empty() {
            params.push(ParamValue {
                name: "start".to_string(),
                value: vs,
            });
        }
    }
    for m in &d.modifications {
        if let Some(ref val) = m.value {
            let vs = expr_to_display_string(val);
            if !vs.is_empty() {
                params.push(ParamValue {
                    name: m.name.clone(),
                    value: vs,
                });
            }
        }
    }
    if params.is_empty() {
        None
    } else {
        Some(params)
    }
}


include!("diagram_mod_tail.rs");
