use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Extent {
    pub p1: Point,
    pub p2: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transformation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Point>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extent: Option<Extent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Placement {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transformation: Option<Transformation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_transformation: Option<Transformation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoordinateSystem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extent: Option<Extent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preserve_aspect_ratio: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_scale: Option<f64>,
}

/// Arrow type for line endpoints
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ArrowType {
    None,
    Arrow,
    Filled,
    Open,
    TShape,
    Circle,
}

/// Fill pattern for shapes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FillPattern {
    Solid,
    Horizontal,
    Vertical,
    Cross,
    DiagCross,
    Forward,
    Backward,
    None,
}

/// Border pattern for rectangles
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum BorderPattern {
    Solid,
    Dashed,
    Dotted,
    DotDashed,
}

/// Line pattern for strokes
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LinePattern {
    Solid,
    Dashed,
    Dotted,
    DotDashed,
}

/// Gradient stop for gradient fills
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GradientStop {
    pub offset: f64,
    pub color: Color,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
}

/// Linear gradient specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinearGradient {
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub stops: Vec<GradientStop>,
}

/// Radial gradient specification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RadialGradient {
    pub cx: f64,
    pub cy: f64,
    pub r: f64,
    pub stops: Vec<GradientStop>,
}

/// Fill definition - solid color or gradient
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FillDefinition {
    Solid(Color),
    LinearGradient(LinearGradient),
    RadialGradient(RadialGradient),
}

/// Matches ModAI TS `fillGradient` and `annotation::format_*` / AVal `fillGradient=linearGradient(...)`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FillGradient {
    #[serde(rename = "linearGradient")]
    LinearGradient {
        gradient: LinearGradient,
    },
    #[serde(rename = "radialGradient")]
    RadialGradient {
        gradient: RadialGradient,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum GraphicItem {
    Line(GraphicLine),
    Rectangle(GraphicRectangle),
    Ellipse(GraphicEllipse),
    Polygon(GraphicPolygon),
    Text(GraphicText),
    Bitmap(GraphicBitmap),
    BSpline(GraphicBSpline),
    Group(GraphicGroup),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphicLine {
    pub points: Vec<Point>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thickness: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smooth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrow: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrow_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Point>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "layerHidden")]
    pub layer_hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "layerLocked")]
    pub layer_locked: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphicRectangle {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extent: Option<Extent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "fillGradient")]
    pub fill_gradient: Option<FillGradient>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub border_pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_thickness: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub radius: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Point>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "layerHidden")]
    pub layer_hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "layerLocked")]
    pub layer_locked: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphicEllipse {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extent: Option<Extent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "fillGradient")]
    pub fill_gradient: Option<FillGradient>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_angle: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_angle: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_thickness: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Point>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "layerHidden")]
    pub layer_hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "layerLocked")]
    pub layer_locked: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphicPolygon {
    pub points: Vec<Point>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "fillGradient")]
    pub fill_gradient: Option<FillGradient>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_thickness: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smooth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Point>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "layerHidden")]
    pub layer_hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "layerLocked")]
    pub layer_locked: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphicText {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extent: Option<Extent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_string: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub font_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line_color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub horizontal_alignment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fill_pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Point>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "layerHidden")]
    pub layer_hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "layerLocked")]
    pub layer_locked: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphicBitmap {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extent: Option<Extent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Point>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "layerHidden")]
    pub layer_hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "layerLocked")]
    pub layer_locked: Option<bool>,
}

/// Bezier spline curve for smooth connections
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphicBSpline {
    pub points: Vec<Point>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thickness: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smooth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrow: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arrow_size: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Point>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "layerHidden")]
    pub layer_hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "layerLocked")]
    pub layer_locked: Option<bool>,
}

/// Maximum nesting depth for `Group` children (avoids stack overflow on pathological input).
pub(crate) const MAX_GRAPHIC_GROUP_DEPTH: usize = 10;

/// Logical group of graphics (editor / persistence; not standard Modelica Icon primitive).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphicGroup {
    pub children: Vec<GraphicItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rotation: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<Point>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "mirrorX")]
    pub mirror_x: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "mirrorY")]
    pub mirror_y: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "layerHidden")]
    pub layer_hidden: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "layerLocked")]
    pub layer_locked: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IconDiagramAnnotation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub coordinate_system: Option<CoordinateSystem>,
    pub graphics: Vec<GraphicItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineAnnotation {
    pub points: Vec<Point>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thickness: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub smooth: Option<String>,
    /// Editor-only routing hint (e.g. manhattan, orthogonal); persisted in .modai JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationData {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placement: Option<Placement>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<IconDiagramAnnotation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagram: Option<IconDiagramAnnotation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<LineAnnotation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dialog: Option<DialogAnnotation>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SelectorAnnotation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caption: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DialogAnnotation {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tab: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_image: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_start_attribute: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector_sizing: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color_selector: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load_selector: Option<SelectorAnnotation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_selector: Option<SelectorAnnotation>,
}
