// Diagram data extraction and apply for Modelica visual programming.

use rustmodlica::ast::{connector_path_to_expr, expr_to_connector_path, ClassItem, Declaration, Equation, Expression, Model};
use rustmodlica::parser;
use rustmodlica::unparse;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentInstance {
    pub name: String,
    pub type_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Connection {
    pub from: String,
    pub to: String,
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
}

fn parse_layout_from_annotation(annotation: Option<&String>) -> Option<HashMap<String, LayoutPoint>> {
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

fn layout_annotation_value(layout: &HashMap<String, LayoutPoint>) -> String {
    let json = serde_json::to_string(layout).unwrap_or_else(|_| "{}".to_string());
    json.replace('\\', "\\\\").replace('"', "\\\"")
}

fn merge_annotation_layout(existing: Option<&String>, layout: &HashMap<String, LayoutPoint>) -> String {
    let layout_value = layout_annotation_value(layout);
    let our_part = format!("__DiagramLayout=\"{}\"", layout_value);
    let existing = existing.map(|s| s.trim()).unwrap_or("");
    if existing.is_empty() {
        return format!("annotation({});", our_part);
    }
    let stripped = existing.strip_suffix(';').unwrap_or(existing).trim();
    if let Some(inner) = stripped
        .strip_prefix("annotation(")
        .and_then(|s| s.strip_suffix(')'))
    {
        let without_ours: Vec<&str> = inner
            .split(',')
            .filter(|p| !p.trim().starts_with("__DiagramLayout="))
            .collect();
        let mut parts: Vec<String> = without_ours.iter().map(|s| (*s).to_string()).collect();
        parts.push(our_part);
        format!("annotation({});", parts.join(", "))
    } else {
        format!("annotation({});", our_part)
    }
}

fn extract_diagram_from_model(m: &Model) -> DiagramModel {
    let components: Vec<ComponentInstance> = m
        .declarations
        .iter()
        .filter(|d| !d.is_parameter)
        .map(|d| ComponentInstance {
            name: d.name.clone(),
            type_name: d.type_name.clone(),
        })
        .collect();

    let mut connections = Vec::new();
    for eq in &m.equations {
        if let Equation::Connect(a, b) = eq {
            if let (Some(from), Some(to)) = (expr_to_connector_path(a), expr_to_connector_path(b)) {
                connections.push(Connection { from, to });
            }
        }
    }

    let layout = parse_layout_from_annotation(m.annotation.as_ref());

    DiagramModel {
        model_name: m.name.clone(),
        components,
        connections,
        layout,
    }
}

/// Parses source and returns diagram data for the top-level model.
pub fn get_diagram_data_from_source(source: &str) -> Result<DiagramModel, String> {
    let item = parser::parse(source).map_err(|e| e.to_string())?;
    let m = match item {
        ClassItem::Model(model) => model,
        ClassItem::Function(_) => return Err("File defines a function, not a model".to_string()),
    };
    Ok(extract_diagram_from_model(&m))
}

/// Reads file and returns diagram data.
pub fn get_diagram_data(project_dir: &str, relative_path: &str) -> Result<DiagramModel, String> {
    let path = std::path::Path::new(project_dir).join(relative_path);
    let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    get_diagram_data_from_source(&source)
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
/// layout: component name -> (x, y); if provided, written to model annotation for persistence.
pub fn apply_diagram_edits(
    source: &str,
    components: &[ComponentInstance],
    connections: &[Connection],
    layout: Option<&HashMap<String, LayoutPoint>>,
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

    if let Some(lay) = layout {
        m.annotation = Some(merge_annotation_layout(m.annotation.as_ref(), lay));
    }

    Ok(unparse::model_to_mo(&m))
}
