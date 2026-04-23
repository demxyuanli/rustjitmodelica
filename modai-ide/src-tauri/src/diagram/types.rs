use rustmodlica::annotation::{IconDiagramAnnotation, LineAnnotation, Placement, Point};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    /// MSL replaceable component flag (loaded from declaration).
    #[serde(default)]
    pub replaceable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constrainedby_type: Option<String>,
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
    pub(crate) fn from_diagram_model(diagram: DiagramModel) -> Self {
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

    pub(crate) fn into_diagram_model(self) -> DiagramModel {
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
