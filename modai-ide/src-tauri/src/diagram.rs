// Diagram data extraction and apply for Modelica visual programming.

use rustmodlica::annotation::{
    self, IconDiagramAnnotation, LineAnnotation, Placement, Point,
};
use rustmodlica::ast::{
    connector_path_to_expr, expr_to_connector_path, ClassItem, Declaration, Equation, Expression,
    Model,
};
use rustmodlica::parser;
use rustmodlica::unparse;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentInstance {
    pub name: String,
    pub type_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placement: Option<Placement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<IconDiagramAnnotation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Point>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Vec<ParamValue>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_kind: Option<String>,
    #[serde(default)]
    pub is_input: bool,
    #[serde(default)]
    pub is_output: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParamValue {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub from: String,
    pub to: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<LineAnnotation>,
}

/// (x, y) in diagram coordinates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagramModel {
    pub model_name: String,
    pub components: Vec<ComponentInstance>,
    pub connections: Vec<Connection>,
    /// Component instance name -> position for diagram layout persistence.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<HashMap<String, LayoutPoint>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagram_annotation: Option<IconDiagramAnnotation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_annotation: Option<IconDiagramAnnotation>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphicalModelState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<HashMap<String, LayoutPoint>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagram_annotation: Option<IconDiagramAnnotation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_annotation: Option<IconDiagramAnnotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphicalDocumentModel {
    pub model_name: String,
    pub components: Vec<ComponentInstance>,
    pub connections: Vec<Connection>,
    pub graphical: GraphicalModelState,
}

impl GraphicalDocumentModel {
    fn from_diagram_model(diagram: DiagramModel) -> Self {
        Self {
            model_name: diagram.model_name,
            components: diagram.components,
            connections: diagram.connections,
            graphical: GraphicalModelState {
                layout: diagram.layout,
                diagram_annotation: diagram.diagram_annotation,
                icon_annotation: diagram.icon_annotation,
            },
        }
    }

    fn into_diagram_model(self) -> DiagramModel {
        DiagramModel {
            model_name: self.model_name,
            components: self.components,
            connections: self.connections,
            layout: self.graphical.layout,
            diagram_annotation: self.graphical.diagram_annotation,
            icon_annotation: self.graphical.icon_annotation,
        }
    }
}

const STATE_FILE_NAME: &str = ".modai/diagram-state.json";
const LEGACY_LAYOUT_FILE_NAME: &str = ".modai/diagram-layout.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiagramPersistentState {
    #[serde(default)]
    layout: HashMap<String, LayoutPoint>,
    #[serde(default)]
    connection_lines: HashMap<String, LineAnnotation>,
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
    let content = std::fs::read_to_string(&path).ok()?;
    let all: HashMap<String, HashMap<String, LayoutPoint>> =
        serde_json::from_str(&content).ok()?;
    all.get(&normalize_relative_path(relative_path)).cloned()
}

fn load_persistent_state(
    project_dir: &str,
    relative_path: &str,
) -> Option<DiagramPersistentState> {
    let path = state_file_path(project_dir);
    let content = std::fs::read_to_string(&path).ok()?;
    let all: HashMap<String, DiagramPersistentState> = serde_json::from_str(&content).ok()?;
    let key = normalize_relative_path(relative_path);
    if let Some(state) = all.get(&key) {
        return Some(state.clone());
    }
    load_legacy_layout_from_file(project_dir, relative_path).map(|layout| DiagramPersistentState {
        layout,
        ..DiagramPersistentState::default()
    })
}

fn save_persistent_state(
    project_dir: &str,
    relative_path: &str,
    state: &DiagramPersistentState,
) -> Result<(), String> {
    let path = state_file_path(project_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let key = normalize_relative_path(relative_path);
    let mut all: HashMap<String, DiagramPersistentState> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default();
    all.insert(key, state.clone());
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
        Expression::Variable(s) => s.clone(),
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
pub fn get_diagram_data_from_source(
    source: &str,
    project_dir: Option<&str>,
    relative_path: Option<&str>,
) -> Result<DiagramModel, String> {
    let item = parser::parse(source).map_err(|e| e.to_string())?;
    let m = match item {
        ClassItem::Model(model) => model,
        ClassItem::Function(_) => return Err("File defines a function, not a model".to_string()),
    };
    let mut diagram = extract_diagram_from_model(&m, project_dir);
    if let (Some(pdir), Some(rpath)) = (project_dir, relative_path) {
        if let Some(state) = load_persistent_state(pdir, rpath) {
            if !state.layout.is_empty() {
                diagram.layout = Some(state.layout);
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
            let _ = save_persistent_state(
                pdir,
                rpath,
                &DiagramPersistentState {
                    layout: diagram.layout.clone().unwrap_or_default(),
                    connection_lines: diagram
                        .connections
                        .iter()
                        .filter_map(|connection| {
                            connection.line.as_ref().map(|line| {
                                (connection_key(&connection.from, &connection.to), line.clone())
                            })
                        })
                        .collect(),
                    diagram_annotation: diagram.diagram_annotation.clone(),
                    icon_annotation: diagram.icon_annotation.clone(),
                },
            );
        }
    }
    Ok(diagram)
}

/// Reads file and returns diagram data.
pub fn get_diagram_data(project_dir: &str, relative_path: &str) -> Result<DiagramModel, String> {
    let path = std::path::Path::new(project_dir).join(relative_path);
    let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    get_diagram_data_from_source(&source, Some(project_dir), Some(relative_path))
}

pub fn get_graphical_document_from_source(
    source: &str,
    project_dir: Option<&str>,
    relative_path: Option<&str>,
) -> Result<GraphicalDocumentModel, String> {
    get_diagram_data_from_source(source, project_dir, relative_path)
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
        is_parameter: false,
        is_flow: false,
        is_discrete: false,
        is_input: c.is_input,
        is_output: c.is_output,
        start_value: None,
        array_size: None,
        modifications: vec![],
        is_rest: false,
        annotation: None,
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
    for (name, value) in incoming_mods {
        if let Some(expr) = parse_param_expression(&value) {
            decl.modifications.push(rustmodlica::ast::Modification {
                name,
                value: Some(expr),
                each: false,
                redeclare: false,
                redeclare_type: None,
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
) -> Result<String, String> {
    let item = parser::parse(source).map_err(|e| e.to_string())?;
    let mut m = match item {
        ClassItem::Model(model) => model,
        ClassItem::Function(_) => return Err("File defines a function, not a model".to_string()),
    };

    m.declarations = merge_declarations(&m.declarations, components);
    m.equations = merge_connections(&m.equations, connections);

    if let (Some(pdir), Some(rpath)) = (project_dir, relative_path) {
        let state = DiagramPersistentState {
            layout: layout.cloned().unwrap_or_default(),
            connection_lines: connections
                .iter()
                .filter_map(|connection| {
                    connection
                        .line
                        .as_ref()
                        .map(|line| (connection_key(&connection.from, &connection.to), line.clone()))
                })
                .collect(),
            diagram_annotation: diagram_annotation.cloned(),
            icon_annotation: icon_annotation.cloned(),
        };
        let _ = save_persistent_state(pdir, rpath, &state);
    }
    if let Some(ref ann) = m.annotation {
        let inner = ann
            .trim()
            .strip_suffix(';')
            .unwrap_or(ann)
            .trim()
            .strip_prefix("annotation(")
            .and_then(|s| s.strip_suffix(')'));
        if let Some(inner) = inner {
            let stripped = strip_diagram_layout_from_annotation_inner(inner);
            m.annotation = if stripped.is_empty() {
                None
            } else {
                Some(format!("annotation({});", stripped))
            };
        }
    }

    Ok(unparse::model_to_mo(&m))
}

pub fn apply_graphical_document_edits(
    source: &str,
    document: &GraphicalDocumentModel,
    project_dir: Option<&str>,
    relative_path: Option<&str>,
) -> Result<String, String> {
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

// --- Equation extraction and editing for graphical equation editor ---

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EquationEntry {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub is_when: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VariableDecl {
    pub name: String,
    pub type_name: String,
    pub variability: String,
    pub start_value: String,
    pub unit: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelEquationsAndVars {
    pub model_name: String,
    pub variables: Vec<VariableDecl>,
    pub equations: Vec<EquationEntry>,
}

pub fn extract_equations_from_source(source: &str) -> Result<ModelEquationsAndVars, String> {
    let item = parser::parse(source).map_err(|e| e.to_string())?;
    let m = match item {
        ClassItem::Model(model) => model,
        ClassItem::Function(_) => return Err("File defines a function, not a model".to_string()),
    };

    let mut variables = Vec::new();
    for decl in &m.declarations {
        let variability = if decl.is_parameter {
            "parameter"
        } else {
            "variable"
        };
        let start_value = decl
            .start_value
            .as_ref()
            .map(|v| format!("{:?}", v))
            .unwrap_or_default();
        variables.push(VariableDecl {
            name: decl.name.clone(),
            type_name: decl.type_name.clone(),
            variability: variability.to_string(),
            start_value,
            unit: String::new(),
            description: String::new(),
        });
    }

    let mut equations = Vec::new();
    for (idx, eq) in m.equations.iter().enumerate() {
        let text = unparse::equation_to_string(eq);
        let is_when = matches!(eq, Equation::When(_, _, _));
        equations.push(EquationEntry {
            id: format!("eq_{}", idx),
            text,
            is_when,
        });
    }

    Ok(ModelEquationsAndVars {
        model_name: m.name.clone(),
        variables,
        equations,
    })
}

pub fn apply_equation_edits(
    source: &str,
    variables: &[VariableDecl],
    equations: &[EquationEntry],
) -> Result<String, String> {
    let item = parser::parse(source).map_err(|e| e.to_string())?;
    let mut m = match item {
        ClassItem::Model(model) => model,
        ClassItem::Function(_) => return Err("File defines a function, not a model".to_string()),
    };

    let existing_component_names: HashSet<&str> = m
        .declarations
        .iter()
        .filter(|d| !d.is_parameter)
        .map(|d| d.name.as_str())
        .collect();

    let mut new_decls: Vec<Declaration> = Vec::new();
    for var in variables {
        if existing_component_names.contains(var.name.as_str()) {
            if let Some(existing) = m.declarations.iter().find(|d| d.name == var.name) {
                new_decls.push(existing.clone());
                continue;
            }
        }
        let start_val = if var.start_value.is_empty() {
            None
        } else {
            Some(Expression::Variable(var.start_value.clone()))
        };
        let decl = Declaration {
            name: var.name.clone(),
            type_name: var.type_name.clone(),
            replaceable: false,
            is_parameter: var.variability == "parameter",
            is_flow: false,
            is_discrete: false,
            is_input: false,
            is_output: false,
            start_value: start_val,
            array_size: None,
            modifications: vec![],
            is_rest: false,
            annotation: None,
        };
        new_decls.push(decl);
    }

    for existing in &m.declarations {
        if !variables.iter().any(|v| v.name == existing.name) {
            if existing_component_names.contains(existing.name.as_str()) {
                new_decls.push(existing.clone());
            }
        }
    }

    m.declarations = new_decls;

    let mut new_eqs: Vec<Equation> = Vec::new();
    for eq_entry in equations {
        let text = eq_entry.text.trim();
        if text.is_empty() {
            continue;
        }
        let eq_source = format!("model _Tmp\nequation\n  {};\nend _Tmp;\n", text.trim_end_matches(';'));
        if let Ok(ClassItem::Model(tmp)) = parser::parse(&eq_source) {
            for parsed_eq in tmp.equations {
                new_eqs.push(parsed_eq);
            }
        }
    }
    m.equations = new_eqs;

    Ok(unparse::model_to_mo(&m))
}
