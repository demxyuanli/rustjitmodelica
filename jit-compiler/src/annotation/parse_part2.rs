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
