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
            if depth >= super::types::MAX_GRAPHIC_GROUP_DEPTH {
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

