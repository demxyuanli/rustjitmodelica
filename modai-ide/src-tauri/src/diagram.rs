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
use std::collections::HashMap;
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

const LAYOUT_FILE_NAME: &str = ".modai/diagram-layout.json";

fn layout_file_path(project_dir: &str) -> std::path::PathBuf {
    Path::new(project_dir).join(LAYOUT_FILE_NAME)
}

/// Load diagram layout for a file from external .modai/diagram-layout.json.
fn load_layout_from_file(
    project_dir: &str,
    relative_path: &str,
) -> Option<HashMap<String, LayoutPoint>> {
    let path = layout_file_path(project_dir);
    let content = std::fs::read_to_string(&path).ok()?;
    let all: HashMap<String, HashMap<String, LayoutPoint>> =
        serde_json::from_str(&content).ok()?;
    let key = relative_path.replace('\\', "/");
    all.get(&key).cloned()
}

/// Save diagram layout for a file into external .modai/diagram-layout.json.
fn save_layout_to_file(
    project_dir: &str,
    relative_path: &str,
    layout: &HashMap<String, LayoutPoint>,
) -> Result<(), String> {
    let path = layout_file_path(project_dir);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let key = relative_path.replace('\\', "/");
    let mut all: HashMap<String, HashMap<String, LayoutPoint>> = std::fs::read_to_string(&path)
        .ok()
        .and_then(|c| serde_json::from_str(&c).ok())
        .unwrap_or_default();
    all.insert(key, layout.clone());
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
                name: String::new(),
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
/// If project_dir and relative_path are set, layout is loaded from .modai/diagram-layout.json (no layout in .mo).
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
        if let Some(layout) = load_layout_from_file(pdir, rpath) {
            diagram.layout = Some(layout);
        } else if let Some(ref layout) = diagram.layout {
            let _ = save_layout_to_file(pdir, rpath, layout);
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

fn declaration_from_component(c: &ComponentInstance) -> Declaration {
    Declaration {
        type_name: c.type_name.clone(),
        name: c.name.clone(),
        replaceable: false,
        is_parameter: false,
        is_flow: false,
        is_discrete: false,
        is_input: false,
        is_output: false,
        start_value: None,
        array_size: None,
        modifications: vec![],
        is_rest: false,
        annotation: None,
    }
}

/// Builds new .mo source from original source and new diagram components/connections.
/// Layout is stored in .modai/diagram-layout.json when project_dir and relative_path are set; never written into .mo.
pub fn apply_diagram_edits(
    source: &str,
    components: &[ComponentInstance],
    connections: &[Connection],
    layout: Option<&HashMap<String, LayoutPoint>>,
    project_dir: Option<&str>,
    relative_path: Option<&str>,
) -> Result<String, String> {
    let item = parser::parse(source).map_err(|e| e.to_string())?;
    let mut m = match item {
        ClassItem::Model(model) => model,
        ClassItem::Function(_) => return Err("File defines a function, not a model".to_string()),
    };

    m.declarations = components
        .iter()
        .map(declaration_from_component)
        .collect();

    let connect_equations: Vec<Equation> = connections
        .iter()
        .map(|c| {
            Equation::Connect(
                connector_path_to_expr(&c.from),
                connector_path_to_expr(&c.to),
            )
        })
        .collect();

    let other_equations: Vec<Equation> = m
        .equations
        .iter()
        .filter(|eq| !matches!(eq, Equation::Connect(_, _)))
        .cloned()
        .collect();

    m.equations = other_equations;
    m.equations.extend(connect_equations);

    if let (Some(lay), Some(pdir), Some(rpath)) = (layout, project_dir, relative_path) {
        let _ = save_layout_to_file(pdir, rpath, lay);
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
