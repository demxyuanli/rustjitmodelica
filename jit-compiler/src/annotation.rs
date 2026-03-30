// Structured Modelica annotation parser.
// Parses raw annotation strings into typed graphical data
// (Placement, Icon, Diagram, Line, Polygon, Ellipse, Rectangle, Text).

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
const MAX_GRAPHIC_GROUP_DEPTH: usize = 10;

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

// ---------------------------------------------------------------------------
// Generic annotation value representation for recursive descent parser
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum AVal {
    Num(f64),
    Str(String),
    Bool(bool),
    Ident(String),
    Array(Vec<AVal>),
    Record(String, Vec<(String, AVal)>),
}

impl AVal {
    fn as_num(&self) -> Option<f64> {
        match self {
            AVal::Num(n) => Some(*n),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            AVal::Str(s) => Some(s),
            _ => None,
        }
    }

    fn as_bool(&self) -> Option<bool> {
        match self {
            AVal::Bool(b) => Some(*b),
            _ => None,
        }
    }

    fn as_ident(&self) -> Option<&str> {
        match self {
            AVal::Ident(s) => Some(s),
            _ => None,
        }
    }

    fn as_array(&self) -> Option<&[AVal]> {
        match self {
            AVal::Array(a) => Some(a),
            _ => None,
        }
    }

    fn as_record(&self) -> Option<(&str, &[(String, AVal)])> {
        match self {
            AVal::Record(name, fields) => Some((name, fields)),
            _ => None,
        }
    }

    fn field(&self, name: &str) -> Option<&AVal> {
        match self {
            AVal::Record(_, fields) => fields.iter().find(|(n, _)| n == name).map(|(_, v)| v),
            _ => None,
        }
    }
}

// ---------------------------------------------------------------------------
// Recursive descent parser for annotation values
// ---------------------------------------------------------------------------

struct AParser<'a> {
    input: &'a str,
    pos: usize,
}

impl<'a> AParser<'a> {
    fn new(input: &'a str) -> Self {
        Self { input, pos: 0 }
    }

    fn skip_ws(&mut self) {
        while self.pos < self.input.len() {
            let b = self.input.as_bytes()[self.pos];
            if b == b' ' || b == b'\t' || b == b'\r' || b == b'\n' {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn expect(&mut self, ch: u8) -> bool {
        self.skip_ws();
        if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == ch {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn parse_number(&mut self) -> Option<f64> {
        self.skip_ws();
        let start = self.pos;
        let bytes = self.input.as_bytes();
        if self.pos < bytes.len() && (bytes[self.pos] == b'-' || bytes[self.pos] == b'+') {
            self.pos += 1;
        }
        let digit_start = self.pos;
        while self.pos < bytes.len() && bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos == digit_start {
            self.pos = start;
            return None;
        }
        if self.pos < bytes.len() && bytes[self.pos] == b'.' {
            self.pos += 1;
            while self.pos < bytes.len() && bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        if self.pos < bytes.len() && (bytes[self.pos] == b'e' || bytes[self.pos] == b'E') {
            self.pos += 1;
            if self.pos < bytes.len() && (bytes[self.pos] == b'+' || bytes[self.pos] == b'-') {
                self.pos += 1;
            }
            while self.pos < bytes.len() && bytes[self.pos].is_ascii_digit() {
                self.pos += 1;
            }
        }
        self.input[start..self.pos].parse::<f64>().ok().or_else(|| {
            self.pos = start;
            None
        })
    }

    fn parse_string(&mut self) -> Option<String> {
        self.skip_ws();
        if self.pos >= self.input.len() || self.input.as_bytes()[self.pos] != b'"' {
            return None;
        }
        self.pos += 1;
        let mut s = String::new();
        let bytes = self.input.as_bytes();
        while self.pos < bytes.len() {
            if bytes[self.pos] == b'\\' && self.pos + 1 < bytes.len() {
                self.pos += 1;
                s.push(bytes[self.pos] as char);
                self.pos += 1;
            } else if bytes[self.pos] == b'"' {
                self.pos += 1;
                return Some(s);
            } else {
                s.push(bytes[self.pos] as char);
                self.pos += 1;
            }
        }
        Some(s)
    }

    fn parse_ident(&mut self) -> Option<String> {
        self.skip_ws();
        let start = self.pos;
        let bytes = self.input.as_bytes();
        if self.pos < bytes.len()
            && (bytes[self.pos].is_ascii_alphabetic() || bytes[self.pos] == b'_')
        {
            self.pos += 1;
            while self.pos < bytes.len()
                && (bytes[self.pos].is_ascii_alphanumeric()
                    || bytes[self.pos] == b'_'
                    || bytes[self.pos] == b'.')
            {
                self.pos += 1;
            }
            Some(self.input[start..self.pos].to_string())
        } else {
            None
        }
    }

    fn parse_value(&mut self) -> Option<AVal> {
        self.skip_ws();
        if self.pos >= self.input.len() {
            return None;
        }
        let b = self.input.as_bytes()[self.pos];
        if b == b'"' {
            return self.parse_string().map(AVal::Str);
        }
        if b == b'{' {
            return self.parse_array().map(AVal::Array);
        }
        if b == b'-' || b == b'+' || b.is_ascii_digit() {
            return self.parse_number().map(AVal::Num);
        }
        let saved = self.pos;
        if let Some(ident) = self.parse_ident() {
            if ident == "true" {
                return Some(AVal::Bool(true));
            }
            if ident == "false" {
                return Some(AVal::Bool(false));
            }
            self.skip_ws();
            if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'(' {
                self.pos += 1;
                let fields = self.parse_named_args();
                self.expect(b')');
                return Some(AVal::Record(ident, fields));
            }
            return Some(AVal::Ident(ident));
        }
        self.pos = saved;
        None
    }

    fn parse_array(&mut self) -> Option<Vec<AVal>> {
        if !self.expect(b'{') {
            return None;
        }
        let mut items = Vec::new();
        loop {
            self.skip_ws();
            if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'}' {
                self.pos += 1;
                return Some(items);
            }
            if let Some(v) = self.parse_value() {
                items.push(v);
            } else {
                self.pos += 1;
                continue;
            }
            self.skip_ws();
            if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b',' {
                self.pos += 1;
            }
        }
    }

    fn parse_named_args(&mut self) -> Vec<(String, AVal)> {
        let mut fields = Vec::new();
        loop {
            self.skip_ws();
            if self.pos >= self.input.len() {
                break;
            }
            if self.input.as_bytes()[self.pos] == b')' {
                break;
            }
            let saved = self.pos;
            if let Some(name) = self.parse_ident() {
                self.skip_ws();
                if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b'=' {
                    self.pos += 1;
                    if let Some(val) = self.parse_value() {
                        fields.push((name, val));
                    }
                } else {
                    self.pos = saved;
                    if let Some(val) = self.parse_value() {
                        fields.push((String::new(), val));
                    }
                }
            } else if let Some(val) = self.parse_value() {
                fields.push((String::new(), val));
            }
            self.skip_ws();
            if self.pos < self.input.len() && self.input.as_bytes()[self.pos] == b',' {
                self.pos += 1;
            }
        }
        fields
    }

    fn parse_top_level(&mut self) -> Vec<(String, AVal)> {
        self.parse_named_args()
    }
}

// ---------------------------------------------------------------------------
// Conversion from generic AVal to typed structures
// ---------------------------------------------------------------------------

fn extract_point(v: &AVal) -> Option<Point> {
    let arr = v.as_array()?;
    if arr.len() >= 2 {
        Some(Point {
            x: arr[0].as_num()?,
            y: arr[1].as_num()?,
        })
    } else {
        None
    }
}

fn extract_extent(v: &AVal) -> Option<Extent> {
    let arr = v.as_array()?;
    if arr.len() >= 2 {
        Some(Extent {
            p1: extract_point(&arr[0])?,
            p2: extract_point(&arr[1])?,
        })
    } else {
        None
    }
}

fn extract_color(v: &AVal) -> Option<Color> {
    let arr = v.as_array()?;
    if arr.len() >= 3 {
        Some(Color {
            r: arr[0].as_num()? as u8,
            g: arr[1].as_num()? as u8,
            b: arr[2].as_num()? as u8,
        })
    } else {
        None
    }
}

fn extract_points(v: &AVal) -> Vec<Point> {
    let arr = match v.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };
    arr.iter().filter_map(extract_point).collect()
}

fn extract_string_array(v: &AVal) -> Option<Vec<String>> {
    let arr = v.as_array()?;
    Some(
        arr.iter()
            .filter_map(|x| {
                x.as_ident()
                    .map(|s| s.to_string())
                    .or_else(|| x.as_str().map(|s| s.to_string()))
            })
            .collect(),
    )
}

fn ident_or_str(v: &AVal) -> Option<String> {
    v.as_ident()
        .map(|s| s.to_string())
        .or_else(|| v.as_str().map(|s| s.to_string()))
}

fn extract_one_gradient_stop(v: &AVal) -> Option<GradientStop> {
    if let Some(arr) = v.as_array() {
        if arr.len() >= 2 {
            return Some(GradientStop {
                offset: arr[0].as_num()?,
                color: extract_color(arr.get(1)?)?,
                opacity: arr.get(2).and_then(|x| x.as_num()),
            });
        }
        return None;
    }
    if let Some((rname, _)) = v.as_record() {
        if rname == "stop" {
            return Some(GradientStop {
                offset: v.field("offset").and_then(|x| x.as_num())?,
                color: v.field("color").and_then(extract_color)?,
                opacity: v.field("opacity").and_then(|x| x.as_num()),
            });
        }
    }
    None
}

fn extract_gradient_stops_list(v: &AVal) -> Vec<GradientStop> {
    v.as_array()
        .map(|a| a.iter().filter_map(extract_one_gradient_stop).collect())
        .unwrap_or_default()
}

fn extract_linear_gradient_from_val(v: &AVal) -> Option<LinearGradient> {
    let _ = v.as_record()?;
    Some(LinearGradient {
        x1: v.field("x1").and_then(|x| x.as_num())?,
        y1: v.field("y1").and_then(|x| x.as_num())?,
        x2: v.field("x2").and_then(|x| x.as_num())?,
        y2: v.field("y2").and_then(|x| x.as_num())?,
        stops: v
            .field("stops")
            .map(extract_gradient_stops_list)
            .unwrap_or_default(),
    })
}

fn extract_radial_gradient_from_val(v: &AVal) -> Option<RadialGradient> {
    let _ = v.as_record()?;
    Some(RadialGradient {
        cx: v.field("cx").and_then(|x| x.as_num())?,
        cy: v.field("cy").and_then(|x| x.as_num())?,
        r: v.field("r").and_then(|x| x.as_num())?,
        stops: v
            .field("stops")
            .map(extract_gradient_stops_list)
            .unwrap_or_default(),
    })
}

fn extract_fill_gradient(v: &AVal) -> Option<FillGradient> {
    let (name, _) = v.as_record()?;
    match name {
        "linearGradient" => extract_linear_gradient_from_val(v)
            .map(|gradient| FillGradient::LinearGradient { gradient }),
        "radialGradient" => extract_radial_gradient_from_val(v)
            .map(|gradient| FillGradient::RadialGradient { gradient }),
        _ => None,
    }
}

fn extract_transformation(v: &AVal) -> Option<Transformation> {
    let (_, _fields) = v.as_record()?;
    Some(Transformation {
        origin: v.field("origin").and_then(extract_point),
        extent: v.field("extent").and_then(extract_extent),
        rotation: v.field("rotation").and_then(|v| v.as_num()),
    })
}

fn extract_placement(v: &AVal) -> Option<Placement> {
    Some(Placement {
        transformation: v.field("transformation").and_then(extract_transformation),
        icon_transformation: v
            .field("iconTransformation")
            .and_then(extract_transformation),
        visible: v.field("visible").and_then(|v| v.as_bool()),
    })
}

fn extract_coordinate_system(v: &AVal) -> Option<CoordinateSystem> {
    Some(CoordinateSystem {
        extent: v.field("extent").and_then(extract_extent),
        preserve_aspect_ratio: v.field("preserveAspectRatio").and_then(|v| v.as_bool()),
        initial_scale: v.field("initialScale").and_then(|v| v.as_num()),
    })
}

fn extract_graphic_item(v: &AVal, depth: usize) -> Option<GraphicItem> {
    let (name, _fields) = v.as_record()?;
    match name {
        "Line" => Some(GraphicItem::Line(GraphicLine {
            points: v
                .field("points")
                .map(|v| extract_points(v))
                .unwrap_or_default(),
            color: v.field("color").and_then(extract_color),
            thickness: v.field("thickness").and_then(|v| v.as_num()),
            pattern: v.field("pattern").and_then(ident_or_str),
            smooth: v.field("smooth").and_then(ident_or_str),
            arrow: v.field("arrow").and_then(extract_string_array),
            arrow_size: v.field("arrowSize").and_then(|v| v.as_num()),
            rotation: v.field("rotation").and_then(|v| v.as_num()),
            origin: v.field("origin").and_then(extract_point),
            layer_hidden: v.field("layerHidden").and_then(|v| v.as_bool()),
            layer_locked: v.field("layerLocked").and_then(|v| v.as_bool()),
        })),
        "Rectangle" => Some(GraphicItem::Rectangle(GraphicRectangle {
            extent: v.field("extent").and_then(extract_extent),
            line_color: v.field("lineColor").and_then(extract_color),
            fill_color: v.field("fillColor").and_then(extract_color),
            fill_pattern: v.field("fillPattern").and_then(ident_or_str),
            fill_gradient: v.field("fillGradient").and_then(extract_fill_gradient),
            border_pattern: v.field("borderPattern").and_then(ident_or_str),
            line_thickness: v.field("lineThickness").and_then(|v| v.as_num()),
            radius: v.field("radius").and_then(|v| v.as_num()),
            rotation: v.field("rotation").and_then(|v| v.as_num()),
            origin: v.field("origin").and_then(extract_point),
            layer_hidden: v.field("layerHidden").and_then(|v| v.as_bool()),
            layer_locked: v.field("layerLocked").and_then(|v| v.as_bool()),
        })),
        "Ellipse" => Some(GraphicItem::Ellipse(GraphicEllipse {
            extent: v.field("extent").and_then(extract_extent),
            line_color: v.field("lineColor").and_then(extract_color),
            fill_color: v.field("fillColor").and_then(extract_color),
            fill_pattern: v.field("fillPattern").and_then(ident_or_str),
            fill_gradient: v.field("fillGradient").and_then(extract_fill_gradient),
            start_angle: v.field("startAngle").and_then(|v| v.as_num()),
            end_angle: v.field("endAngle").and_then(|v| v.as_num()),
            line_thickness: v.field("lineThickness").and_then(|v| v.as_num()),
            rotation: v.field("rotation").and_then(|v| v.as_num()),
            origin: v.field("origin").and_then(extract_point),
            layer_hidden: v.field("layerHidden").and_then(|v| v.as_bool()),
            layer_locked: v.field("layerLocked").and_then(|v| v.as_bool()),
        })),
        "Polygon" => Some(GraphicItem::Polygon(GraphicPolygon {
            points: v
                .field("points")
                .map(|v| extract_points(v))
                .unwrap_or_default(),
            line_color: v.field("lineColor").and_then(extract_color),
            fill_color: v.field("fillColor").and_then(extract_color),
            fill_pattern: v.field("fillPattern").and_then(ident_or_str),
            fill_gradient: v.field("fillGradient").and_then(extract_fill_gradient),
            line_thickness: v.field("lineThickness").and_then(|v| v.as_num()),
            smooth: v.field("smooth").and_then(ident_or_str),
            rotation: v.field("rotation").and_then(|v| v.as_num()),
            origin: v.field("origin").and_then(extract_point),
            layer_hidden: v.field("layerHidden").and_then(|v| v.as_bool()),
            layer_locked: v.field("layerLocked").and_then(|v| v.as_bool()),
        })),
        "Text" => Some(GraphicItem::Text(GraphicText {
            extent: v.field("extent").and_then(extract_extent),
            text_string: v.field("textString").and_then(|v| {
                v.as_str()
                    .map(|s| s.to_string())
                    .or_else(|| v.as_ident().map(|s| s.to_string()))
            }),
            font_size: v.field("fontSize").and_then(|v| v.as_num()),
            font_name: v
                .field("fontName")
                .and_then(|v| v.as_str().map(|s| s.to_string())),
            text_color: v.field("textColor").and_then(extract_color),
            line_color: v.field("lineColor").and_then(extract_color),
            fill_color: v.field("fillColor").and_then(extract_color),
            horizontal_alignment: v.field("horizontalAlignment").and_then(ident_or_str),
            fill_pattern: v.field("fillPattern").and_then(ident_or_str),
            rotation: v.field("rotation").and_then(|v| v.as_num()),
            origin: v.field("origin").and_then(extract_point),
            layer_hidden: v.field("layerHidden").and_then(|v| v.as_bool()),
            layer_locked: v.field("layerLocked").and_then(|v| v.as_bool()),
        })),
        "Bitmap" => Some(GraphicItem::Bitmap(GraphicBitmap {
            extent: v.field("extent").and_then(extract_extent),
            file_name: v
                .field("fileName")
                .and_then(|v| v.as_str().map(|s| s.to_string())),
            image_source: v
                .field("imageSource")
                .and_then(|v| v.as_str().map(|s| s.to_string())),
            rotation: v.field("rotation").and_then(|v| v.as_num()),
            origin: v.field("origin").and_then(extract_point),
            layer_hidden: v.field("layerHidden").and_then(|v| v.as_bool()),
            layer_locked: v.field("layerLocked").and_then(|v| v.as_bool()),
        })),
        "BSpline" => Some(GraphicItem::BSpline(GraphicBSpline {
            points: v
                .field("points")
                .map(|v| extract_points(v))
                .unwrap_or_default(),
            color: v.field("color").and_then(extract_color),
            thickness: v.field("thickness").and_then(|v| v.as_num()),
            pattern: v.field("pattern").and_then(ident_or_str),
            smooth: v.field("smooth").and_then(ident_or_str),
            arrow: v.field("arrow").and_then(extract_string_array),
            arrow_size: v.field("arrowSize").and_then(|v| v.as_num()),
            rotation: v.field("rotation").and_then(|v| v.as_num()),
            origin: v.field("origin").and_then(extract_point),
            layer_hidden: v.field("layerHidden").and_then(|v| v.as_bool()),
            layer_locked: v.field("layerLocked").and_then(|v| v.as_bool()),
        })),
        "Group" => {
            if depth >= MAX_GRAPHIC_GROUP_DEPTH {
                return None;
            }
            let mut children = Vec::new();
            if let Some(arr) = v.field("children").and_then(|c| c.as_array()) {
                for item in arr {
                    if let Some(gi) = extract_graphic_item(item, depth + 1) {
                        children.push(gi);
                    }
                }
            }
            Some(GraphicItem::Group(GraphicGroup {
                children,
                rotation: v.field("rotation").and_then(|v| v.as_num()),
                origin: v.field("origin").and_then(extract_point),
                opacity: v.field("opacity").and_then(|v| v.as_num()),
                mirror_x: v.field("mirrorX").and_then(|v| v.as_bool()),
                mirror_y: v.field("mirrorY").and_then(|v| v.as_bool()),
                layer_hidden: v.field("layerHidden").and_then(|v| v.as_bool()),
                layer_locked: v.field("layerLocked").and_then(|v| v.as_bool()),
            }))
        },
        _ => None,
    }
}

fn extract_icon_diagram(v: &AVal) -> Option<IconDiagramAnnotation> {
    let (_, _fields) = v.as_record()?;
    let coord = v
        .field("coordinateSystem")
        .and_then(extract_coordinate_system);
    let mut graphics = Vec::new();
    if let Some(g) = v.field("graphics") {
        if let Some(arr) = g.as_array() {
            for item in arr {
                if let Some(gi) = extract_graphic_item(item, 0) {
                    graphics.push(gi);
                }
            }
        }
    }
    Some(IconDiagramAnnotation {
        coordinate_system: coord,
        graphics,
    })
}

fn extract_line_annotation(fields: &[(String, AVal)]) -> Option<LineAnnotation> {
    let mut points = Vec::new();
    let mut color = None;
    let mut thickness = None;
    let mut pattern = None;
    let mut smooth = None;
    for (name, val) in fields {
        match name.as_str() {
            "points" => points = extract_points(val),
            "color" => color = extract_color(val),
            "thickness" => thickness = val.as_num(),
            "pattern" => pattern = ident_or_str(val),
            "smooth" => smooth = ident_or_str(val),
            _ => {}
        }
    }
    if points.is_empty() {
        return None;
    }
    Some(LineAnnotation {
        points,
        color,
        thickness,
        pattern,
        smooth,
    })
}

fn extract_selector_annotation(v: &AVal) -> Option<SelectorAnnotation> {
    let (_, _) = v.as_record()?;
    Some(SelectorAnnotation {
        filter: v
            .field("filter")
            .and_then(|x| x.as_str().map(|s| s.to_string())),
        caption: v
            .field("caption")
            .and_then(|x| x.as_str().map(|s| s.to_string())),
    })
}

fn extract_dialog_annotation(v: &AVal) -> Option<DialogAnnotation> {
    let (_, _) = v.as_record()?;
    Some(DialogAnnotation {
        tab: v
            .field("tab")
            .and_then(|x| x.as_str().map(|s| s.to_string())),
        group: v
            .field("group")
            .and_then(|x| x.as_str().map(|s| s.to_string())),
        group_image: v
            .field("groupImage")
            .and_then(|x| x.as_str().map(|s| s.to_string())),
        enable: v.field("enable").and_then(|x| x.as_bool()),
        show_start_attribute: v.field("showStartAttribute").and_then(|x| x.as_bool()),
        connector_sizing: v.field("connectorSizing").and_then(|x| x.as_bool()),
        color_selector: v.field("colorSelector").and_then(|x| x.as_bool()),
        load_selector: v
            .field("loadSelector")
            .and_then(extract_selector_annotation),
        save_selector: v
            .field("saveSelector")
            .and_then(extract_selector_annotation),
    })
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse a raw annotation string (e.g. "annotation(Placement(...))") into structured data.
pub fn parse_annotation(raw: &str) -> Option<AnnotationData> {
    let trimmed = raw.trim();
    let trimmed = trimmed.strip_suffix(';').unwrap_or(trimmed).trim();
    let inner = trimmed
        .strip_prefix("annotation(")
        .and_then(|s| s.strip_suffix(')'))?;

    let mut parser = AParser::new(inner);
    let fields = parser.parse_top_level();

    let mut placement = None;
    let mut icon = None;
    let mut diagram = None;
    let mut line = None;
    let mut dialog = None;

    for (name, val) in &fields {
        match name.as_str() {
            "Placement" | "" => {
                if let Some((_rname, _)) = val.as_record() {
                    if _rname == "Placement" || (name.is_empty() && _rname == "Placement") {
                        placement = extract_placement(val);
                    }
                }
            }
            _ => {}
        }
    }
    if placement.is_none() {
        for (_name, val) in &fields {
            if let Some((rname, _)) = val.as_record() {
                if rname == "Placement" {
                    placement = extract_placement(val);
                }
            }
        }
    }

    for (_name, val) in &fields {
        if let Some((rname, _)) = val.as_record() {
            match rname {
                "Icon" => icon = extract_icon_diagram(val),
                "Diagram" => diagram = extract_icon_diagram(val),
                "Line" => {
                    if let AVal::Record(_, ref lf) = val {
                        line = extract_line_annotation(lf);
                    }
                }
                "Dialog" => dialog = extract_dialog_annotation(val),
                _ => {}
            }
        }
    }

    Some(AnnotationData {
        placement,
        icon,
        diagram,
        line,
        dialog,
    })
}

/// Parse only Placement from a declaration annotation string.
pub fn parse_placement(raw: &str) -> Option<Placement> {
    parse_annotation(raw).and_then(|a| a.placement)
}

/// Parse Icon annotation from a class annotation string.
pub fn parse_icon(raw: &str) -> Option<IconDiagramAnnotation> {
    parse_annotation(raw).and_then(|a| a.icon)
}

pub fn parse_dialog(raw: &str) -> Option<DialogAnnotation> {
    parse_annotation(raw).and_then(|a| a.dialog)
}

// ---------------------------------------------------------------------------
// Serialize Icon / Diagram (including Group) into class annotation text
// ---------------------------------------------------------------------------

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.is_finite() && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{}", n)
    }
}

fn escape_mo_string(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn format_mo_string_literal(s: &str) -> String {
    format!("\"{}\"", escape_mo_string(s))
}

fn format_color_mo(c: &Color) -> String {
    format!("{{{},{},{}}}", c.r, c.g, c.b)
}

fn format_point_mo(p: &Point) -> String {
    format!("{{{},{}}}", fmt_num(p.x), fmt_num(p.y))
}

fn format_extent_mo(e: &Extent) -> String {
    format!("{{{},{}}}", format_point_mo(&e.p1), format_point_mo(&e.p2))
}

fn format_points_mo(pts: &[Point]) -> String {
    let inner: Vec<String> = pts.iter().map(format_point_mo).collect();
    format!("{{{}}}", inner.join(","))
}

fn push_opt_color(parts: &mut Vec<String>, name: &str, c: Option<&Color>) {
    if let Some(v) = c {
        parts.push(format!("{}={}", name, format_color_mo(v)));
    }
}

fn push_opt_extent(parts: &mut Vec<String>, name: &str, e: Option<&Extent>) {
    if let Some(v) = e {
        parts.push(format!("{}={}", name, format_extent_mo(v)));
    }
}

fn push_opt_point(parts: &mut Vec<String>, name: &str, p: Option<&Point>) {
    if let Some(v) = p {
        parts.push(format!("{}={}", name, format_point_mo(v)));
    }
}

fn push_opt_f64(parts: &mut Vec<String>, name: &str, v: Option<f64>) {
    if let Some(n) = v {
        parts.push(format!("{}={}", name, fmt_num(n)));
    }
}

fn push_opt_str(parts: &mut Vec<String>, name: &str, v: Option<&str>) {
    if let Some(s) = v {
        if !s.is_empty() {
            parts.push(format!("{}={}", name, format_mo_string_literal(s)));
        }
    }
}

fn push_opt_bool(parts: &mut Vec<String>, name: &str, v: Option<bool>) {
    if let Some(b) = v {
        parts.push(format!("{}={}", name, b));
    }
}

fn format_arrow_mo(arrows: &[String]) -> String {
    let inner: Vec<String> = arrows
        .iter()
        .map(|s| {
            if s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                s.clone()
            } else {
                format_mo_string_literal(s)
            }
        })
        .collect();
    format!("{{{}}}", inner.join(","))
}

fn format_coordinate_system_mo(cs: &CoordinateSystem) -> String {
    let mut p = Vec::new();
    if let Some(ext) = &cs.extent {
        p.push(format!("extent={}", format_extent_mo(ext)));
    }
    if let Some(b) = cs.preserve_aspect_ratio {
        p.push(format!("preserveAspectRatio={}", b));
    }
    if let Some(s) = cs.initial_scale {
        p.push(format!("initialScale={}", fmt_num(s)));
    }
    if p.is_empty() {
        "coordinateSystem(extent={{-100,-100},{100,100}})".to_string()
    } else {
        format!("coordinateSystem({})", p.join(", "))
    }
}

fn format_gradient_stop_tuple_mo(s: &GradientStop) -> String {
    let mut parts = vec![fmt_num(s.offset), format_color_mo(&s.color)];
    if let Some(o) = s.opacity {
        parts.push(fmt_num(o));
    }
    format!("{{{}}}", parts.join(","))
}

fn format_linear_gradient_record_mo(g: &LinearGradient) -> String {
    let stops: Vec<String> = g.stops.iter().map(format_gradient_stop_tuple_mo).collect();
    format!(
        "linearGradient(x1={}, y1={}, x2={}, y2={}, stops={{{}}})",
        fmt_num(g.x1),
        fmt_num(g.y1),
        fmt_num(g.x2),
        fmt_num(g.y2),
        stops.join(",")
    )
}

fn format_radial_gradient_record_mo(g: &RadialGradient) -> String {
    let stops: Vec<String> = g.stops.iter().map(format_gradient_stop_tuple_mo).collect();
    format!(
        "radialGradient(cx={}, cy={}, r={}, stops={{{}}})",
        fmt_num(g.cx),
        fmt_num(g.cy),
        fmt_num(g.r),
        stops.join(",")
    )
}

fn push_opt_fill_gradient(parts: &mut Vec<String>, fg: Option<&FillGradient>) {
    if let Some(f) = fg {
        match f {
            FillGradient::LinearGradient { gradient } => {
                parts.push(format!(
                    "fillGradient={}",
                    format_linear_gradient_record_mo(gradient)
                ));
            }
            FillGradient::RadialGradient { gradient } => {
                parts.push(format!(
                    "fillGradient={}",
                    format_radial_gradient_record_mo(gradient)
                ));
            }
        }
    }
}

fn format_graphic_item_mo(item: &GraphicItem) -> String {
    match item {
        GraphicItem::Line(line) => {
            let mut p = vec![format!("points={}", format_points_mo(&line.points))];
            push_opt_color(&mut p, "color", line.color.as_ref());
            push_opt_f64(&mut p, "thickness", line.thickness);
            if let Some(ref pat) = line.pattern {
                p.push(format!("pattern={}", pat));
            }
            if let Some(ref sm) = line.smooth {
                p.push(format!("smooth={}", sm));
            }
            if let Some(ref ar) = line.arrow {
                if !ar.is_empty() {
                    p.push(format!("arrow={}", format_arrow_mo(ar)));
                }
            }
            push_opt_f64(&mut p, "arrowSize", line.arrow_size);
            push_opt_f64(&mut p, "rotation", line.rotation);
            push_opt_point(&mut p, "origin", line.origin.as_ref());
            push_opt_bool(&mut p, "layerHidden", line.layer_hidden);
            push_opt_bool(&mut p, "layerLocked", line.layer_locked);
            format!("Line({})", p.join(", "))
        }
        GraphicItem::Rectangle(r) => {
            let mut p = Vec::new();
            push_opt_extent(&mut p, "extent", r.extent.as_ref());
            push_opt_color(&mut p, "lineColor", r.line_color.as_ref());
            push_opt_color(&mut p, "fillColor", r.fill_color.as_ref());
            if let Some(ref fp) = r.fill_pattern {
                p.push(format!("fillPattern={}", fp));
            }
            push_opt_fill_gradient(&mut p, r.fill_gradient.as_ref());
            if let Some(ref bp) = r.border_pattern {
                p.push(format!("borderPattern={}", bp));
            }
            push_opt_f64(&mut p, "lineThickness", r.line_thickness);
            push_opt_f64(&mut p, "radius", r.radius);
            push_opt_f64(&mut p, "rotation", r.rotation);
            push_opt_point(&mut p, "origin", r.origin.as_ref());
            push_opt_bool(&mut p, "layerHidden", r.layer_hidden);
            push_opt_bool(&mut p, "layerLocked", r.layer_locked);
            format!("Rectangle({})", p.join(", "))
        }
        GraphicItem::Ellipse(e) => {
            let mut p = Vec::new();
            push_opt_extent(&mut p, "extent", e.extent.as_ref());
            push_opt_color(&mut p, "lineColor", e.line_color.as_ref());
            push_opt_color(&mut p, "fillColor", e.fill_color.as_ref());
            if let Some(ref fp) = e.fill_pattern {
                p.push(format!("fillPattern={}", fp));
            }
            push_opt_fill_gradient(&mut p, e.fill_gradient.as_ref());
            push_opt_f64(&mut p, "startAngle", e.start_angle);
            push_opt_f64(&mut p, "endAngle", e.end_angle);
            push_opt_f64(&mut p, "lineThickness", e.line_thickness);
            push_opt_f64(&mut p, "rotation", e.rotation);
            push_opt_point(&mut p, "origin", e.origin.as_ref());
            push_opt_bool(&mut p, "layerHidden", e.layer_hidden);
            push_opt_bool(&mut p, "layerLocked", e.layer_locked);
            format!("Ellipse({})", p.join(", "))
        }
        GraphicItem::Polygon(pg) => {
            let mut p = vec![format!("points={}", format_points_mo(&pg.points))];
            push_opt_color(&mut p, "lineColor", pg.line_color.as_ref());
            push_opt_color(&mut p, "fillColor", pg.fill_color.as_ref());
            if let Some(ref fp) = pg.fill_pattern {
                p.push(format!("fillPattern={}", fp));
            }
            push_opt_fill_gradient(&mut p, pg.fill_gradient.as_ref());
            push_opt_f64(&mut p, "lineThickness", pg.line_thickness);
            if let Some(ref sm) = pg.smooth {
                p.push(format!("smooth={}", sm));
            }
            push_opt_f64(&mut p, "rotation", pg.rotation);
            push_opt_point(&mut p, "origin", pg.origin.as_ref());
            push_opt_bool(&mut p, "layerHidden", pg.layer_hidden);
            push_opt_bool(&mut p, "layerLocked", pg.layer_locked);
            format!("Polygon({})", p.join(", "))
        }
        GraphicItem::Text(t) => {
            let mut p = Vec::new();
            push_opt_extent(&mut p, "extent", t.extent.as_ref());
            push_opt_str(&mut p, "textString", t.text_string.as_deref());
            push_opt_f64(&mut p, "fontSize", t.font_size);
            push_opt_str(&mut p, "fontName", t.font_name.as_deref());
            push_opt_color(&mut p, "textColor", t.text_color.as_ref());
            push_opt_color(&mut p, "lineColor", t.line_color.as_ref());
            push_opt_color(&mut p, "fillColor", t.fill_color.as_ref());
            if let Some(ref ha) = t.horizontal_alignment {
                p.push(format!("horizontalAlignment={}", ha));
            }
            if let Some(ref fp) = t.fill_pattern {
                p.push(format!("fillPattern={}", fp));
            }
            push_opt_f64(&mut p, "rotation", t.rotation);
            push_opt_point(&mut p, "origin", t.origin.as_ref());
            push_opt_bool(&mut p, "layerHidden", t.layer_hidden);
            push_opt_bool(&mut p, "layerLocked", t.layer_locked);
            format!("Text({})", p.join(", "))
        }
        GraphicItem::Bitmap(b) => {
            let mut p = Vec::new();
            push_opt_extent(&mut p, "extent", b.extent.as_ref());
            push_opt_str(&mut p, "fileName", b.file_name.as_deref());
            push_opt_str(&mut p, "imageSource", b.image_source.as_deref());
            push_opt_f64(&mut p, "rotation", b.rotation);
            push_opt_point(&mut p, "origin", b.origin.as_ref());
            push_opt_bool(&mut p, "layerHidden", b.layer_hidden);
            push_opt_bool(&mut p, "layerLocked", b.layer_locked);
            format!("Bitmap({})", p.join(", "))
        }
        GraphicItem::BSpline(bs) => {
            let mut p = vec![format!("points={}", format_points_mo(&bs.points))];
            push_opt_color(&mut p, "color", bs.color.as_ref());
            push_opt_f64(&mut p, "thickness", bs.thickness);
            if let Some(ref pat) = bs.pattern {
                p.push(format!("pattern={}", pat));
            }
            if let Some(ref sm) = bs.smooth {
                p.push(format!("smooth={}", sm));
            }
            if let Some(ref ar) = bs.arrow {
                if !ar.is_empty() {
                    p.push(format!("arrow={}", format_arrow_mo(ar)));
                }
            }
            push_opt_f64(&mut p, "arrowSize", bs.arrow_size);
            push_opt_f64(&mut p, "rotation", bs.rotation);
            push_opt_point(&mut p, "origin", bs.origin.as_ref());
            push_opt_bool(&mut p, "layerHidden", bs.layer_hidden);
            push_opt_bool(&mut p, "layerLocked", bs.layer_locked);
            format!("BSpline({})", p.join(", "))
        }
        GraphicItem::Group(g) => {
            let children: Vec<String> = g.children.iter().map(format_graphic_item_mo).collect();
            let mut p = vec![format!("children={{{}}}", children.join(","))];
            push_opt_f64(&mut p, "rotation", g.rotation);
            push_opt_point(&mut p, "origin", g.origin.as_ref());
            push_opt_f64(&mut p, "opacity", g.opacity);
            push_opt_bool(&mut p, "mirrorX", g.mirror_x);
            push_opt_bool(&mut p, "mirrorY", g.mirror_y);
            push_opt_bool(&mut p, "layerHidden", g.layer_hidden);
            push_opt_bool(&mut p, "layerLocked", g.layer_locked);
            format!("Group({})", p.join(", "))
        }
    }
}

/// Build `Icon(...)` or `Diagram(...)` text for embedding in `annotation(...)`.
pub fn format_icon_diagram_record(record_name: &str, ann: &IconDiagramAnnotation) -> String {
    let cs = ann
        .coordinate_system
        .as_ref()
        .map(format_coordinate_system_mo)
        .unwrap_or_else(|| "coordinateSystem(extent={{-100,-100},{100,100}})".to_string());
    let gs: Vec<String> = ann.graphics.iter().map(format_graphic_item_mo).collect();
    let graphics = format!("graphics={{{}}}", gs.join(","));
    format!("{}({}, {})", record_name, cs, graphics)
}
