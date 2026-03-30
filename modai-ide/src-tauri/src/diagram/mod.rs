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

fn try_load_type_icon(
    type_name: &str,
    project_dir: Option<&str>,
) -> Option<IconDiagramAnnotation> {
    let pdir = project_dir?;
    let base = Path::new(pdir);
    let candidates = vec![
        format!("{}.mo", type_name),
        format!("{}.mo", type_name.replace('.', "/")),
    ];
    for c in &candidates {
        let path = base.join(c);
        if path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(item) = parser::parse(&content) {
                    if let ClassItem::Model(m) = item {
                        if let Some(ref ann_str) = m.annotation {
                            if let Some(ad) = annotation::parse_annotation(ann_str) {
                                if ad.icon.is_some() {
                                    return ad.icon;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    None
}

fn determine_connector_kind(type_name: &str, is_input: bool, is_output: bool) -> Option<String> {
    let lower = type_name.to_lowercase();
    if lower.contains("flange") || lower.contains("rotational") || lower.contains("translational") {
        return Some("mechanical".to_string());
    }
    if lower.contains("pin") || lower.contains("positivepin") || lower.contains("negativepin") {
        return Some("electrical".to_string());
    }
    if lower.contains("heatport") || lower.contains("thermal") {
        return Some("thermal".to_string());
    }
    if lower.contains("fluidport") || lower.contains("fluid") {
        return Some("fluid".to_string());
    }
    if is_input || lower.contains("realinput") || lower.contains("integerinput") || lower.contains("booleaninput") {
        return Some("signal_input".to_string());
    }
    if is_output || lower.contains("realoutput") || lower.contains("integeroutput") || lower.contains("booleanoutput") {
        return Some("signal_output".to_string());
    }
    None
}

fn extract_diagram_from_model(m: &Model, project_dir: Option<&str>) -> DiagramModel {
    let class_annotation = m
        .annotation
        .as_ref()
        .and_then(|s| annotation::parse_annotation(s));

    let components: Vec<ComponentInstance> = m
        .declarations
        .iter()
        .filter(|d| !d.is_parameter)
        .map(|d| {
            let decl_ann = d
                .annotation
                .as_ref()
                .and_then(|s| annotation::parse_annotation(s));

            let placement = decl_ann.as_ref().and_then(|a| a.placement.clone());

            let (rotation, origin) = placement
                .as_ref()
                .and_then(|p| p.transformation.as_ref())
                .map(|t| (t.rotation, t.origin.clone()))
                .unwrap_or((None, None));

            let icon = try_load_type_icon(&d.type_name, project_dir);

            let params = extract_params(d);

            let connector_kind =
                determine_connector_kind(&d.type_name, d.is_input, d.is_output);

            ComponentInstance {
                name: d.name.clone(),
                type_name: d.type_name.clone(),
                placement,
                icon,
                rotation,
                origin,
                params,
                connector_kind,
                is_input: d.is_input,
                is_output: d.is_output,
            }
        })
        .collect();

    let mut connections = Vec::new();
    for eq in &m.equations {
        if let Equation::Connect(a, b) = eq {
            if let (Some(from), Some(to)) = (expr_to_connector_path(a), expr_to_connector_path(b))
            {
                connections.push(Connection {
                    from,
                    to,
                    line: None,
                });
            }
        }
    }

    let layout = parse_layout_from_annotation(m.annotation.as_ref());

    let diagram_annotation = class_annotation.as_ref().and_then(|a| a.diagram.clone());
    let icon_annotation = class_annotation.as_ref().and_then(|a| a.icon.clone());

    DiagramModel {
        model_name: m.name.clone(),
        components,
        connections,
        layout,
        diagram_annotation,
        icon_annotation,
    }
}

/// Parses source and returns diagram data for the top-level model.
/// If project_dir and relative_path are set, internal diagram state is loaded from .modai/diagram-state.json.
///
/// `persistent_hint`: `None` — read state synchronously inside this function; `Some(inner)` — state
/// was already read on the async runtime (`inner == None` means no saved state, same as a miss on disk).
fn get_diagram_data_from_source_impl(
    source: &str,
    project_dir: Option<&str>,
    relative_path: Option<&str>,
    persistent_hint: Option<Option<DiagramPersistentState>>,
) -> Result<DiagramModel, String> {
    let item = parser::parse(source).map_err(|e| e.to_string())?;
    let m = match item {
        ClassItem::Model(model) => model,
        ClassItem::Function(_) => return Err("File defines a function, not a model".to_string()),
    };
    let mut diagram = extract_diagram_from_model(&m, project_dir);
    if let (Some(pdir), Some(rpath)) = (project_dir, relative_path) {
        validate_relative_path(rpath).map_err(|e| e.to_string())?;
        tracing::info!(target: "modai_diagram", "diagram data for {}", rpath);
        let state_opt = match persistent_hint {
            None => load_persistent_state(pdir, rpath),
            Some(pre) => pre,
        };
        if let Some(state) = state_opt {
            if !state.layout.is_empty() {
                diagram.layout = Some(state.layout.into_iter().collect());
            }
            if let Some(annotation) = state.diagram_annotation {
                diagram.diagram_annotation = Some(annotation);
            }
            if let Some(annotation) = state.icon_annotation {
                diagram.icon_annotation = Some(annotation);
            }
            for connection in &mut diagram.connections {
                let key = connection_key(&connection.from, &connection.to);
                if let Some(line) = state.connection_lines.get(&key) {
                    connection.line = Some(line.clone());
                }
            }
        } else {
            if let Err(e) = save_persistent_state(
                pdir,
                rpath,
                &DiagramPersistentState {
                    layout: diagram
                        .layout
                        .clone()
                        .unwrap_or_default()
                        .into_iter()
                        .collect::<BTreeMap<_, _>>(),
                    connection_lines: diagram
                        .connections
                        .iter()
                        .filter_map(|connection| {
                            connection.line.as_ref().map(|line| {
                                (connection_key(&connection.from, &connection.to), line.clone())
                            })
                        })
                        .collect::<BTreeMap<_, _>>(),
                    diagram_annotation: diagram.diagram_annotation.clone(),
                    icon_annotation: diagram.icon_annotation.clone(),
                },
            ) {
                tracing::warn!(
                    target: "modai_diagram",
                    "failed to save initial diagram persistent state: {}",
                    e
                );
            }
        }
    }
    Ok(diagram)
}

pub fn get_diagram_data_from_source(
    source: &str,
    project_dir: Option<&str>,
    relative_path: Option<&str>,
) -> Result<DiagramModel, String> {
    get_diagram_data_from_source_impl(source, project_dir, relative_path, None)
}

/// Loads persistent diagram JSON asynchronously, then parses source on the blocking pool.
pub async fn load_and_build_graphical_document_from_source(
    source: String,
    project_dir: Option<String>,
    relative_path: Option<String>,
) -> Result<GraphicalDocumentModel, String> {
    if let (Some(_), Some(r)) = (&project_dir, &relative_path) {
        validate_relative_path(r).map_err(|e| e.to_string())?;
    }
    let persistent_hint: Option<Option<DiagramPersistentState>> =
        match (&project_dir, &relative_path) {
            (Some(p), Some(r)) => Some(load_persistent_state_async(p, r).await),
            _ => None,
        };
    tokio::time::timeout(
        Duration::from_secs(30),
        async move {
            tokio::task::spawn_blocking(move || {
                get_diagram_data_from_source_impl(
                    &source,
                    project_dir.as_deref(),
                    relative_path.as_deref(),
                    persistent_hint,
                )
                .map(GraphicalDocumentModel::from_diagram_model)
            })
            .await
            .map_err(|e| format!("join error: {e}"))?
        },
    )
    .await
    .map_err(|_| "diagram load timed out after 30s".to_string())?
}

/// Reads file and returns diagram data.
pub fn get_diagram_data(project_dir: &str, relative_path: &str) -> Result<DiagramModel, String> {
    validate_relative_path(relative_path).map_err(|e| e.to_string())?;
    let path = std::path::Path::new(project_dir).join(relative_path);
    let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    get_diagram_data_from_source(&source, Some(project_dir), Some(relative_path))
}

/// Synchronous build (blocking I/O + parse). Prefer `load_and_build_graphical_document_from_source` from async commands.
#[allow(dead_code)]
pub fn get_graphical_document_from_source(
    source: &str,
    project_dir: Option<&str>,
    relative_path: Option<&str>,
) -> Result<GraphicalDocumentModel, String> {
    get_diagram_data_from_source_impl(source, project_dir, relative_path, None)
        .map(GraphicalDocumentModel::from_diagram_model)
}

pub fn get_graphical_document(
    project_dir: &str,
    relative_path: &str,
) -> Result<GraphicalDocumentModel, String> {
    get_diagram_data(project_dir, relative_path).map(GraphicalDocumentModel::from_diagram_model)
}

fn declaration_from_component(c: &ComponentInstance) -> Declaration {
    let mut decl = Declaration {
        type_name: c.type_name.clone(),
        name: c.name.clone(),
        replaceable: false,
        constrainedby_type: None,
        is_parameter: false,
        is_flow: false,
        is_stream: false,
        is_discrete: false,
        is_input: c.is_input,
        is_output: c.is_output,
        is_inner: false,
        is_outer: false,
        is_public: false,
        is_protected: false,
        start_value: None,
        array_size: None,
        modifications: vec![],
        is_rest: false,
        annotation: None,
        condition: None,
    };
    apply_component_to_declaration(&mut decl, c);
    decl
}

fn parse_param_expression(value: &str) -> Option<Expression> {
    parser::parse_expression_from_str(value).ok()
}

fn apply_component_to_declaration(decl: &mut Declaration, component: &ComponentInstance) {
    decl.type_name = component.type_name.clone();
    decl.is_input = component.is_input;
    decl.is_output = component.is_output;
    decl.annotation = upsert_annotation_item(
        decl.annotation.as_ref(),
        "Placement(",
        component.placement.as_ref().map(format_placement),
    );

    let incoming = component.params.clone().unwrap_or_default();
    let mut incoming_mods = HashMap::new();
    for param in &incoming {
        if param.name.trim().is_empty() {
            continue;
        }
        incoming_mods.insert(param.name.clone(), param.value.clone());
    }
    if let Some(start) = incoming_mods.remove("start") {
        decl.start_value = parse_param_expression(&start).or_else(|| decl.start_value.clone());
    }
    for modification in &mut decl.modifications {
        if let Some(new_value) = incoming_mods.remove(&modification.name) {
            modification.value = parse_param_expression(&new_value).or(modification.value.clone());
        }
    }
    let mut remaining_mods: Vec<(String, String)> = incoming_mods.into_iter().collect();
    remaining_mods.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    for (name, value) in remaining_mods {
        if let Some(expr) = parse_param_expression(&value) {
            decl.modifications.push(rustmodlica::ast::Modification {
                name,
                value: Some(expr),
                each: false,
                redeclare: false,
                redeclare_type: None,
                is_inner: false,
                is_outer: false,
                is_public: false,
                is_protected: false,
                is_operator_function: false,
            });
        }
    }
}

fn merge_declarations(
    existing: &[Declaration],
    components: &[ComponentInstance],
) -> Vec<Declaration> {
    let mut component_map = HashMap::new();
    for component in components {
        component_map.insert(component.name.clone(), component.clone());
    }
    let mut used = HashSet::new();
    let mut merged = Vec::new();

    for decl in existing {
        if decl.is_parameter {
            merged.push(decl.clone());
            continue;
        }
        if let Some(component) = component_map.get(&decl.name) {
            let mut updated = decl.clone();
            apply_component_to_declaration(&mut updated, component);
            merged.push(updated);
            used.insert(component.name.clone());
        }
    }

    for component in components {
        if !used.contains(&component.name) {
            merged.push(declaration_from_component(component));
        }
    }

    merged
}

fn merge_connections(existing: &[Equation], connections: &[Connection]) -> Vec<Equation> {
    let mut remaining = HashMap::new();
    for connection in connections {
        remaining.insert(connection_key(&connection.from, &connection.to), connection.clone());
    }

    let mut merged = Vec::new();
    for eq in existing {
        if let Equation::Connect(a, b) = eq {
            if let (Some(from), Some(to)) = (expr_to_connector_path(a), expr_to_connector_path(b)) {
                let key = connection_key(&from, &to);
                if remaining.remove(&key).is_some() {
                    merged.push(eq.clone());
                }
                continue;
            }
        }
        merged.push(eq.clone());
    }

    for connection in connections {
        let key = connection_key(&connection.from, &connection.to);
        if remaining.contains_key(&key) {
            merged.push(Equation::Connect(
                connector_path_to_expr(&connection.from),
                connector_path_to_expr(&connection.to),
            ));
        }
    }

    merged
}

/// Builds new .mo source from original source and new diagram components/connections.
/// Layout is stored in .modai/diagram-layout.json when project_dir and relative_path are set; never written into .mo.
pub fn apply_diagram_edits(
    source: &str,
    components: &[ComponentInstance],
    connections: &[Connection],
    layout: Option<&HashMap<String, LayoutPoint>>,
    diagram_annotation: Option<&IconDiagramAnnotation>,
    icon_annotation: Option<&IconDiagramAnnotation>,
    project_dir: Option<&str>,
    relative_path: Option<&str>,
) -> Result<ApplyDiagramEditsOutput, String> {
    let item = parser::parse(source).map_err(|e| e.to_string())?;
    let mut m = match item {
        ClassItem::Model(model) => model,
        ClassItem::Function(_) => return Err("File defines a function, not a model".to_string()),
    };

    m.declarations = merge_declarations(&m.declarations, components);
    m.equations = merge_connections(&m.equations, connections);

    let mut persistent_warning: Option<String> = None;
    if let (Some(pdir), Some(rpath)) = (project_dir, relative_path) {
        validate_relative_path(rpath).map_err(|e| e.to_string())?;
        tracing::info!(target: "modai_diagram", "apply_diagram_edits for {}", rpath);
        let state = DiagramPersistentState {
            layout: layout
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .collect::<BTreeMap<_, _>>(),
            connection_lines: connections
                .iter()
                .filter_map(|connection| {
                    connection
                        .line
                        .as_ref()
                        .map(|line| (connection_key(&connection.from, &connection.to), line.clone()))
                })
                .collect::<BTreeMap<_, _>>(),
            diagram_annotation: diagram_annotation.cloned(),
            icon_annotation: icon_annotation.cloned(),
        };
        if let Err(e) = save_persistent_state(pdir, rpath, &state) {
            tracing::warn!(
                target: "modai_diagram",
                "failed to save diagram persistent state after edits: {}",
                e
            );
            persistent_warning = Some(format!(
                "Failed to save .modai diagram state (layout persists in editor only until fixed): {}",
                e
            ));
        }
    }
    let inner_base = m
        .annotation
        .as_ref()
        .and_then(|ann| annotation_inner(ann))
        .map(|s| strip_diagram_layout_from_annotation_inner(&s))
        .unwrap_or_default();

    let mut items: Vec<String> = split_top_level_items(&inner_base)
        .into_iter()
        .filter(|item| {
            let t = item.trim();
            if icon_annotation.is_some() && t.starts_with("Icon(") {
                return false;
            }
            if diagram_annotation.is_some() && t.starts_with("Diagram(") {
                return false;
            }
            true
        })
        .collect();

    if let Some(icon) = icon_annotation {
        items.push(format_icon_diagram_record("Icon", icon));
    }
    if let Some(diagram) = diagram_annotation {
        items.push(format_icon_diagram_record("Diagram", diagram));
    }

    m.annotation = if items.is_empty() {
        None
    } else {
        Some(format!("annotation({});", items.join(", ")))
    };

    Ok(ApplyDiagramEditsOutput {
        new_source: unparse::model_to_mo(&m),
        warning: persistent_warning,
    })
}

pub fn apply_graphical_document_edits(
    source: &str,
    document: &GraphicalDocumentModel,
    project_dir: Option<&str>,
    relative_path: Option<&str>,
) -> Result<ApplyDiagramEditsOutput, String> {
    let diagram = document.clone().into_diagram_model();
    apply_diagram_edits(
        source,
        &diagram.components,
        &diagram.connections,
        diagram.layout.as_ref(),
        diagram.diagram_annotation.as_ref(),
        diagram.icon_annotation.as_ref(),
        project_dir,
        relative_path,
    )
}
