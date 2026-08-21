use crate::SvgError;
use crate::sink::SvgSink;
use serde_json::{Map, Value};

type Point = (i64, i64);
type MultilinePositions = (Point, Point);

pub(crate) fn render_operation(
    operation: &Value,
    sink: &mut SvgSink,
    block_depth: &mut usize,
    maximum_block_depth: &mut usize,
    max_block_depth: usize,
) -> Result<(), SvgError> {
    let object = operation
        .as_object()
        .ok_or_else(|| SvgError("plot operation must be an object".to_owned()))?;
    let kind = string(object, "kind")?;
    if kind == "StartBlock" {
        return render_start_block(
            object,
            sink,
            block_depth,
            maximum_block_depth,
            max_block_depth,
        );
    }
    if kind == "EndBlock" {
        return render_end_block(sink, block_depth);
    }
    let owned = has_operation_ownership(object);
    if owned {
        open_ownership_group(object, sink)?;
    }
    let result = match kind {
        "ThickSegment" => render_segment(object, sink),
        "ArcThreePoint" => render_arc(object, sink),
        "Circle" => render_circle(object, sink),
        "Rect" => render_rect(object, sink),
        "PlotPoly" => render_poly(object, sink),
        "BezierCurve" => render_bezier(object, sink),
        "Text" => render_text(object, sink),
        "PlotImage" => render_image(object, sink),
        "FlashPadCircle" => render_pad_circle(object, sink),
        "FlashPadOval" => render_pad_oval(object, sink),
        "FlashPadRect" => render_pad_rect(object, sink, false),
        "FlashPadRoundRect" => render_pad_rect(object, sink, true),
        "FlashPadCustom" => render_pad_custom(object, sink),
        "FlashPadTrapez" => render_pad_trapez(object, sink),
        kind => Err(SvgError(format!(
            "unsupported frozen plot operation {kind}"
        ))),
    };
    result?;
    if owned {
        sink.raw("</g>\n")?;
    }
    Ok(())
}

fn has_operation_ownership(object: &Map<String, Value>) -> bool {
    ["label", "data_uuid", "data_ref", "object_id"]
        .iter()
        .any(|field| {
            object
                .get(*field)
                .and_then(Value::as_str)
                .is_some_and(|value| !value.is_empty())
        })
        || object
            .get("extra_attrs")
            .and_then(Value::as_object)
            .is_some_and(|extra| !extra.is_empty())
}

fn open_ownership_group(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<(), SvgError> {
    sink.element()?;
    sink.raw("<g")?;
    if let Some(label) = optional_string(object, "label")? {
        sink.id_attribute(label)?;
    }
    for (attribute, field) in [
        ("data-uuid", "data_uuid"),
        ("data-ref", "data_ref"),
        ("data-object-id", "object_id"),
    ] {
        if let Some(value) = optional_string(object, field)?
            && !value.is_empty()
        {
            sink.attribute(attribute, value)?;
        }
    }
    emit_extra_attrs(object, sink)?;
    sink.raw(">\n")
}

fn render_segment(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<(), SvgError> {
    sink.element()?;
    sink.raw("<line")?;
    number_attrs(
        sink,
        object,
        &[
            ("x1", "start_x"),
            ("y1", "start_y"),
            ("x2", "end_x"),
            ("y2", "end_y"),
        ],
    )?;
    style(sink, object, false)?;
    sink.raw("/>\n")
}

fn render_circle(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<(), SvgError> {
    let radius = half_number(i128::from(nonnegative_integer(object, "diameter_nm")?));
    sink.element()?;
    sink.raw("<circle")?;
    number_attrs(sink, object, &[("cx", "cx"), ("cy", "cy")])?;
    sink.attribute("r", &radius)?;
    style(sink, object, fill_enabled(object))?;
    sink.raw("/>\n")
}

fn render_rect(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<(), SvgError> {
    let x1 = integer(object, "x1")?;
    let y1 = integer(object, "y1")?;
    let x2 = integer(object, "x2")?;
    let y2 = integer(object, "y2")?;
    let x = x1.min(x2);
    let y = y1.min(y2);
    let width = x1.abs_diff(x2);
    let height = y1.abs_diff(y2);
    sink.element()?;
    sink.raw("<rect")?;
    for (name, value) in [("x", x), ("y", y)] {
        sink.attribute(name, &value.to_string())?;
    }
    sink.attribute("width", &width.to_string())?;
    sink.attribute("height", &height.to_string())?;
    if let Some(radius) = optional_integer(object, "corner_radius_nm")?
        && radius > 0
    {
        sink.attribute("rx", &radius.to_string())?;
        sink.attribute("ry", &radius.to_string())?;
    }
    style(sink, object, fill_enabled(object))?;
    sink.raw("/>\n")
}

fn render_poly(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<(), SvgError> {
    let points = point_list(object, "points")?;
    let closed = fill_enabled(object) || points.first() == points.last();
    sink.element()?;
    sink.raw(if closed { "<polygon" } else { "<polyline" })?;
    sink.raw(" points=\"")?;
    for (index, (x, y)) in points.iter().enumerate() {
        if index > 0 {
            sink.raw(" ")?;
        }
        sink.raw(&format!("{x},{y}"))?;
    }
    sink.raw("\"")?;
    style(sink, object, fill_enabled(object))?;
    sink.raw("/>\n")
}

fn render_bezier(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<(), SvgError> {
    let values = [
        integer(object, "start_x")?,
        integer(object, "start_y")?,
        integer(object, "ctrl1_x")?,
        integer(object, "ctrl1_y")?,
        integer(object, "ctrl2_x")?,
        integer(object, "ctrl2_y")?,
        integer(object, "end_x")?,
        integer(object, "end_y")?,
    ];
    sink.element()?;
    sink.raw(&format!(
        "<path d=\"M {} {} C {} {}, {} {}, {} {}\"",
        values[0], values[1], values[2], values[3], values[4], values[5], values[6], values[7]
    ))?;
    style(sink, object, false)?;
    sink.raw("/>\n")
}

fn render_arc(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<(), SvgError> {
    let start = (integer(object, "start_x")?, integer(object, "start_y")?);
    let mid = (integer(object, "mid_x")?, integer(object, "mid_y")?);
    let end = (integer(object, "end_x")?, integer(object, "end_y")?);
    sink.element()?;
    if let Some((radius, large, sweep)) = arc_parameters(start, mid, end) {
        sink.raw(&format!(
            "<path d=\"M {} {} A {} {} 0 {} {} {} {}\"",
            start.0,
            start.1,
            number(radius),
            number(radius),
            u8::from(large),
            u8::from(sweep),
            end.0,
            end.1,
        ))?;
    } else {
        sink.raw("<line")?;
        for (name, value) in [
            ("x1", start.0),
            ("y1", start.1),
            ("x2", end.0),
            ("y2", end.1),
        ] {
            sink.attribute(name, &value.to_string())?;
        }
    }
    style(sink, object, fill_enabled(object))?;
    sink.raw("/>\n")
}

fn arc_parameters(
    start: (i64, i64),
    mid: (i64, i64),
    end: (i64, i64),
) -> Option<(f64, bool, bool)> {
    let (ax, ay) = (start.0 as f64, start.1 as f64);
    let (bx, by) = (mid.0 as f64, mid.1 as f64);
    let (cx, cy) = (end.0 as f64, end.1 as f64);
    let determinant = 2.0 * (ax * (by - cy) + bx * (cy - ay) + cx * (ay - by));
    if determinant.abs() < f64::EPSILON {
        return None;
    }
    let a2 = ax * ax + ay * ay;
    let b2 = bx * bx + by * by;
    let c2 = cx * cx + cy * cy;
    let ux = (a2 * (by - cy) + b2 * (cy - ay) + c2 * (ay - by)) / determinant;
    let uy = (a2 * (cx - bx) + b2 * (ax - cx) + c2 * (bx - ax)) / determinant;
    let radius = ((ax - ux).powi(2) + (ay - uy).powi(2)).sqrt();
    let start_angle = (ay - uy).atan2(ax - ux);
    let mid_angle = (by - uy).atan2(bx - ux);
    let end_angle = (cy - uy).atan2(cx - ux);
    let ccw_total = positive_angle(end_angle - start_angle);
    let ccw_mid = positive_angle(mid_angle - start_angle);
    let sweep = ccw_mid <= ccw_total;
    let span = if sweep {
        ccw_total
    } else {
        positive_angle(start_angle - end_angle)
    };
    Some((radius, span > std::f64::consts::PI, sweep))
}

fn positive_angle(value: f64) -> f64 {
    value.rem_euclid(std::f64::consts::TAU)
}

fn render_text(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<(), SvgError> {
    if render_typed_cache(object, sink)? {
        return Ok(());
    }
    if render_legacy_cache(object, sink)? {
        return Ok(());
    }
    let text = string(object, "text")?;
    if !boolean(object, "multiline").unwrap_or(false) || !text.contains('\n') {
        return render_text_line(
            object,
            sink,
            text,
            integer(object, "x")?,
            integer(object, "y")?,
        );
    }
    let line_count = text.split('\n').count();
    let (first, step) = multiline_positions(object, line_count)?;
    for (line_index, line) in text.split('\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        let index = i64::try_from(line_index)
            .map_err(|_| SvgError("text line index does not fit i64".to_owned()))?;
        let line_x = checked_line_coordinate(first.0, step.0, index)?;
        let line_y = checked_line_coordinate(first.1, step.1, index)?;
        render_text_line(object, sink, line, line_x, line_y)?;
    }
    Ok(())
}

fn render_text_line(
    object: &Map<String, Value>,
    sink: &mut SvgSink,
    text: &str,
    x: i64,
    y: i64,
) -> Result<(), SvgError> {
    sink.element()?;
    sink.raw("<text")?;
    sink.attribute("x", &x.to_string())?;
    sink.attribute("y", &y.to_string())?;
    sink.attribute("font-size", &integer(object, "size_y_nm")?.to_string())?;
    if let Some(face) = optional_string(object, "font_face")?
        && !face.is_empty()
    {
        sink.attribute("font-family", face)?;
    }
    if boolean(object, "bold").unwrap_or(false) {
        sink.attribute("font-weight", "bold")?;
    }
    if boolean(object, "italic").unwrap_or(false) {
        sink.attribute("font-style", "italic")?;
    }
    text_alignment(object, sink)?;
    text_color(object, sink)?;
    let angle = float(object, "orient_deg").unwrap_or(0.0);
    if angle != 0.0 {
        sink.attribute("transform", &format!("rotate({} {x} {y})", number(-angle)))?;
    }
    sink.raw(">")?;
    sink.escaped(text)?;
    sink.raw("</text>\n")
}

fn multiline_positions(
    object: &Map<String, Value>,
    line_count: usize,
) -> Result<MultilinePositions, SvgError> {
    let x = integer(object, "x")?;
    let y = integer(object, "y")?;
    let size_y = integer(object, "size_y_nm")?;
    let size_iu = ki_round_i64((size_y as f64) / 100.0)?;
    let line_step_iu = ki_round_i64((size_iu as f64) * 1.68)?;
    let line_step = line_step_iu
        .checked_mul(100)
        .ok_or_else(|| SvgError("text line step overflowed".to_owned()))?;
    let gaps = i64::try_from(line_count.saturating_sub(1))
        .map_err(|_| SvgError("text line count does not fit i64".to_owned()))?;
    let total_step = gaps
        .checked_mul(line_step)
        .ok_or_else(|| SvgError("text block height overflowed".to_owned()))?;
    let vertical_offset = match optional_string(object, "v_align")?.unwrap_or("") {
        value if value.ends_with("CENTER") => {
            let half_steps_iu = gaps
                .checked_mul(line_step_iu)
                .ok_or_else(|| SvgError("text centered offset overflowed".to_owned()))?
                / 2;
            half_steps_iu
                .checked_mul(-100)
                .ok_or_else(|| SvgError("text centered offset overflowed".to_owned()))?
        }
        value if value.ends_with("BOTTOM") => total_step
            .checked_neg()
            .ok_or_else(|| SvgError("text bottom offset overflowed".to_owned()))?,
        _ => 0,
    };
    let angle = -float(object, "orient_deg").unwrap_or(0.0);
    let first_offset = rotate_offset(0.0, vertical_offset as f64, angle);
    let step_offset = rotate_offset(0.0, line_step as f64, angle);
    let first_x = round_ties_even_i64((x as f64) + first_offset.0)?;
    let first_y = round_ties_even_i64((y as f64) + first_offset.1)?;
    let step_x = round_ties_even_i64(step_offset.0)?;
    let step_y = round_ties_even_i64(step_offset.1)?;
    Ok(((first_x, first_y), (step_x, step_y)))
}

fn checked_line_coordinate(first: i64, step: i64, index: i64) -> Result<i64, SvgError> {
    let offset = step
        .checked_mul(index)
        .ok_or_else(|| SvgError("text line offset overflowed".to_owned()))?;
    first
        .checked_add(offset)
        .ok_or_else(|| SvgError("text line position overflowed".to_owned()))
}

fn rotate_offset(x: f64, y: f64, angle_degrees: f64) -> (f64, f64) {
    let angle = angle_degrees.rem_euclid(360.0);
    if angle == 0.0 {
        return (x, y);
    }
    if angle == 90.0 {
        return (-y, x);
    }
    if angle == 180.0 {
        return (-x, -y);
    }
    if angle == 270.0 {
        return (y, -x);
    }
    let radians = angle.to_radians();
    let (sine, cosine) = radians.sin_cos();
    (x * cosine - y * sine, x * sine + y * cosine)
}

fn ki_round_i64(value: f64) -> Result<i64, SvgError> {
    rounded_i64(value.round(), "KiCad text rounding")
}

fn round_ties_even_i64(value: f64) -> Result<i64, SvgError> {
    rounded_i64(value.round_ties_even(), "text coordinate rounding")
}

fn rounded_i64(value: f64, context: &str) -> Result<i64, SvgError> {
    if !value.is_finite() || value < i64::MIN as f64 || value > i64::MAX as f64 {
        return Err(SvgError(format!("{context} exceeds i64")));
    }
    Ok(value as i64)
}

fn render_typed_cache(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<bool, SvgError> {
    let Some(polygons) = object
        .get("render_cache")
        .and_then(Value::as_object)
        .and_then(|cache| cache.get("polygons"))
        .and_then(Value::as_array)
    else {
        return Ok(false);
    };
    for polygon in polygons {
        let contours = polygon
            .get("contours")
            .and_then(Value::as_array)
            .ok_or_else(|| SvgError("text cache contours are missing".to_owned()))?;
        sink.element()?;
        sink.raw("<path d=\"")?;
        let mut emitted = 0usize;
        for contour in contours {
            let points = parse_points(contour)?;
            if points.len() < 3 {
                continue;
            }
            write_path_points(sink, &points)?;
            emitted += 1;
        }
        sink.raw("\"")?;
        let color = optional_string(object, "color")?.unwrap_or("#000000FF");
        color_attr(sink, "fill", color)?;
        sink.attribute("stroke", "none")?;
        if emitted > 1 {
            sink.attribute("fill-rule", "evenodd")?;
        }
        sink.raw("/>\n")?;
    }
    Ok(true)
}

fn render_legacy_cache(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<bool, SvgError> {
    let Some(polygons) = object
        .get("render_cache_polygons")
        .and_then(Value::as_array)
        .filter(|polygons| !polygons.is_empty())
    else {
        return Ok(false);
    };
    let color = optional_string(object, "color")?.unwrap_or("#000000FF");
    for polygon in polygons {
        let points = parse_points(polygon)?;
        if points.len() < 3 {
            continue;
        }
        sink.element()?;
        sink.raw("<polygon points=\"")?;
        for (index, (x, y)) in points.iter().enumerate() {
            if index > 0 {
                sink.raw(" ")?;
            }
            sink.raw(&format!("{x},{y}"))?;
        }
        sink.raw("\"")?;
        color_attr(sink, "fill", color)?;
        sink.attribute("stroke", "none")?;
        sink.raw("/>\n")?;
    }
    Ok(true)
}

fn render_image(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<(), SvgError> {
    let width = integer(object, "width_nm")?;
    let height = integer(object, "height_nm")?;
    let x = centered_start(integer(object, "x")?, width);
    let y = centered_start(integer(object, "y")?, height);
    let data = string(object, "image_data_b64")?;
    let format = string(object, "image_format")?;
    let mime = match format {
        "jpeg" | "jpg" => "image/jpeg",
        "bmp" => "image/bmp",
        _ => "image/png",
    };
    sink.element()?;
    sink.raw("<image")?;
    for (name, value) in [
        ("x", x),
        ("y", y),
        ("width", width.to_string()),
        ("height", height.to_string()),
    ] {
        sink.attribute(name, &value)?;
    }
    sink.attribute("preserveAspectRatio", "none")?;
    sink.raw(" href=\"data:")?;
    sink.raw(mime)?;
    sink.raw(";base64,")?;
    sink.escaped(data)?;
    sink.raw("\"/>\n")
}

fn render_pad_circle(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<(), SvgError> {
    let radius = half_number(i128::from(nonnegative_integer(object, "diameter_nm")?));
    sink.element()?;
    sink.raw("<circle")?;
    number_attrs(sink, object, &[("cx", "x"), ("cy", "y")])?;
    sink.attribute("r", &radius)?;
    pad_style(sink)?;
    sink.raw("/>\n")
}

fn render_pad_oval(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<(), SvgError> {
    let x = integer(object, "x")?;
    let y = integer(object, "y")?;
    let size_x = nonnegative_integer(object, "size_x_nm")?;
    let size_y = nonnegative_integer(object, "size_y_nm")?;
    sink.element()?;
    if size_x == size_y {
        sink.raw("<circle")?;
        sink.attribute("cx", &x.to_string())?;
        sink.attribute("cy", &y.to_string())?;
        sink.attribute("r", &half_number(i128::from(size_x)))?;
        pad_style(sink)?;
        return sink.raw("/>\n");
    }

    let (x1, y1, x2, y2, width) = if size_x > size_y {
        let straight = i128::from(size_x - size_y);
        (
            half_number(i128::from(x) * 2 - straight),
            y.to_string(),
            half_number(i128::from(x) * 2 + straight),
            y.to_string(),
            size_y,
        )
    } else {
        let straight = i128::from(size_y - size_x);
        (
            x.to_string(),
            half_number(i128::from(y) * 2 - straight),
            x.to_string(),
            half_number(i128::from(y) * 2 + straight),
            size_x,
        )
    };
    sink.raw("<line")?;
    for (name, value) in [("x1", x1), ("y1", y1), ("x2", x2), ("y2", y2)] {
        sink.attribute(name, &value)?;
    }
    rotation(object, sink)?;
    sink.attribute("fill", "none")?;
    sink.attribute("stroke", "#000000")?;
    sink.attribute("stroke-width", &width.to_string())?;
    sink.attribute("stroke-linecap", "round")?;
    sink.attribute("stroke-linejoin", "round")?;
    sink.raw("/>\n")
}

fn render_pad_rect(
    object: &Map<String, Value>,
    sink: &mut SvgSink,
    rounded: bool,
) -> Result<(), SvgError> {
    let size_x = nonnegative_integer(object, "size_x_nm")?;
    let size_y = nonnegative_integer(object, "size_y_nm")?;
    let x = integer(object, "x")?;
    let y = integer(object, "y")?;
    sink.element()?;
    sink.raw("<rect")?;
    sink.attribute("x", &centered_start(x, size_x))?;
    sink.attribute("y", &centered_start(y, size_y))?;
    sink.attribute("width", &size_x.to_string())?;
    sink.attribute("height", &size_y.to_string())?;
    if rounded {
        let radius = nonnegative_integer(object, "corner_radius_nm")?;
        sink.attribute("rx", &radius.to_string())?;
        sink.attribute("ry", &radius.to_string())?;
    }
    rotation(object, sink)?;
    pad_style(sink)?;
    sink.raw("/>\n")
}

fn render_pad_trapez(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<(), SvgError> {
    render_local_polygons(object, "corners", sink)
}

fn render_pad_custom(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<(), SvgError> {
    let polygons = object
        .get("polygons")
        .and_then(Value::as_array)
        .ok_or_else(|| SvgError("custom pad polygons are missing".to_owned()))?;
    for polygon in polygons {
        render_local_point_array(object, polygon, sink)?;
    }
    Ok(())
}

fn render_local_polygons(
    object: &Map<String, Value>,
    field: &str,
    sink: &mut SvgSink,
) -> Result<(), SvgError> {
    let points = object
        .get(field)
        .ok_or_else(|| SvgError(format!("missing {field}")))?;
    render_local_point_array(object, points, sink)
}

fn render_local_point_array(
    object: &Map<String, Value>,
    value: &Value,
    sink: &mut SvgSink,
) -> Result<(), SvgError> {
    let points = parse_points(value)?;
    sink.element()?;
    sink.raw("<polygon points=\"")?;
    for (index, (x, y)) in points.iter().enumerate() {
        if index > 0 {
            sink.raw(" ")?;
        }
        sink.raw(&format!("{x},{y}"))?;
    }
    sink.raw("\"")?;
    let x = integer(object, "x")?;
    let y = integer(object, "y")?;
    let angle = float(object, "orient_deg").unwrap_or(0.0);
    sink.attribute(
        "transform",
        &format!("translate({x} {y}) rotate({})", number(-angle)),
    )?;
    pad_style(sink)?;
    sink.raw("/>\n")
}

fn render_start_block(
    object: &Map<String, Value>,
    sink: &mut SvgSink,
    depth: &mut usize,
    maximum_depth: &mut usize,
    maximum: usize,
) -> Result<(), SvgError> {
    *depth = depth
        .checked_add(1)
        .ok_or_else(|| SvgError("block depth overflowed".to_owned()))?;
    if *depth > maximum {
        return Err(SvgError(
            "block depth exceeds the configured limit".to_owned(),
        ));
    }
    *maximum_depth = (*maximum_depth).max(*depth);
    sink.element()?;
    sink.raw("<g")?;
    sink.id_attribute(string(object, "label")?)?;
    for (attribute, field) in [
        ("data-uuid", "data_uuid"),
        ("data-ref", "data_ref"),
        ("data-object-id", "object_id"),
    ] {
        let value = string(object, field)?;
        if !value.is_empty() {
            sink.attribute(attribute, value)?;
        }
    }
    emit_extra_attrs(object, sink)?;
    sink.raw(">\n")
}

fn emit_extra_attrs(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<(), SvgError> {
    if let Some(extra) = object.get("extra_attrs").and_then(Value::as_object) {
        let mut entries = extra.iter().collect::<Vec<_>>();
        entries.sort_by_key(|(key, _)| *key);
        for (key, value) in entries {
            if valid_data_name(key) {
                let Some(text) = scalar_attribute(value) else {
                    return Err(SvgError(
                        "operation data attribute must be a scalar".to_owned(),
                    ));
                };
                if !text.is_empty() {
                    sink.attribute(&format!("data-{}", key.replace('_', "-")), &text)?;
                }
            }
        }
    }
    Ok(())
}

fn scalar_attribute(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        Value::Null => Some(String::new()),
        Value::Array(_) | Value::Object(_) => None,
    }
}

fn render_end_block(sink: &mut SvgSink, depth: &mut usize) -> Result<(), SvgError> {
    if *depth == 0 {
        return Err(SvgError(
            "plot document contains an orphan EndBlock".to_owned(),
        ));
    }
    *depth -= 1;
    sink.raw("</g>\n")
}

fn style(sink: &mut SvgSink, object: &Map<String, Value>, filled: bool) -> Result<(), SvgError> {
    let width = optional_integer(object, "width_nm")?.unwrap_or(0);
    if width < 0 {
        return Err(SvgError("field width_nm must be nonnegative".to_owned()));
    }
    let stroke = optional_string(object, "stroke_color")?.unwrap_or("#000000FF");
    if width == 0 && filled {
        sink.attribute("stroke", "none")?;
    } else {
        let effective_width = if width == 0 { 152_400 } else { width };
        color_attr(sink, "stroke", stroke)?;
        sink.attribute("stroke-width", &effective_width.to_string())?;
        sink.attribute("stroke-linecap", "round")?;
        sink.attribute("stroke-linejoin", "round")?;
        line_style(sink, object, effective_width)?;
    }
    if filled {
        let fill = optional_string(object, "fill_color")?.unwrap_or(stroke);
        color_attr(sink, "fill", fill)?;
    } else {
        sink.attribute("fill", "none")?;
    }
    Ok(())
}

fn text_color(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<(), SvgError> {
    let color = optional_string(object, "color")?.unwrap_or("#000000FF");
    color_attr(sink, "fill", color)
}

fn pad_style(sink: &mut SvgSink) -> Result<(), SvgError> {
    sink.attribute("fill", "#000000")?;
    sink.attribute("stroke", "none")
}

fn color_attr(sink: &mut SvgSink, name: &str, rgba: &str) -> Result<(), SvgError> {
    let bytes = rgba.as_bytes();
    if !matches!(bytes.len(), 7 | 9)
        || bytes[0] != b'#'
        || !bytes[1..].iter().all(u8::is_ascii_hexdigit)
    {
        return Err(SvgError(format!("invalid RGBA color {rgba}")));
    }
    sink.attribute(name, &rgba[..7])?;
    let alpha = if bytes.len() == 9 {
        u8::from_str_radix(&rgba[7..9], 16)
            .map_err(|_| SvgError(format!("invalid RGBA color {rgba}")))?
    } else {
        u8::MAX
    };
    if alpha != u8::MAX {
        sink.attribute(
            &format!("{name}-opacity"),
            &number(f64::from(alpha) / 255.0),
        )?;
    }
    Ok(())
}

fn line_style(sink: &mut SvgSink, object: &Map<String, Value>, width: i64) -> Result<(), SvgError> {
    let Some(style) = optional_string(object, "line_style")? else {
        return Ok(());
    };
    let pattern = match style {
        "DASH" => Some(format!("{} {}", width * 4, width * 2)),
        "DOT" => Some(format!("{} {}", width, width * 2)),
        "DASH_DOT" => Some(format!(
            "{} {} {} {}",
            width * 4,
            width * 2,
            width,
            width * 2
        )),
        "DASH_DOT_DOT" => Some(format!(
            "{} {} {} {} {} {}",
            width * 4,
            width * 2,
            width,
            width * 2,
            width,
            width * 2
        )),
        _ => None,
    };
    if let Some(pattern) = pattern {
        sink.attribute("stroke-dasharray", &pattern)?;
    }
    Ok(())
}

fn text_alignment(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<(), SvgError> {
    let anchor = match optional_string(object, "h_align")?.unwrap_or("") {
        value if value.ends_with("CENTER") => "middle",
        value if value.ends_with("RIGHT") => "end",
        _ => "start",
    };
    sink.attribute("text-anchor", anchor)?;
    let baseline = match optional_string(object, "v_align")?.unwrap_or("") {
        value if value.ends_with("TOP") => "hanging",
        value if value.ends_with("CENTER") => "central",
        _ => "alphabetic",
    };
    sink.attribute("dominant-baseline", baseline)
}

fn rotation(object: &Map<String, Value>, sink: &mut SvgSink) -> Result<(), SvgError> {
    let angle = float(object, "orient_deg").unwrap_or(0.0);
    if angle != 0.0 {
        let x = integer(object, "x")?;
        let y = integer(object, "y")?;
        sink.attribute("transform", &format!("rotate({} {x} {y})", number(-angle)))?;
    }
    Ok(())
}

fn fill_enabled(object: &Map<String, Value>) -> bool {
    object
        .get("fill")
        .and_then(Value::as_str)
        .is_some_and(|fill| fill != "NO_FILL")
}

fn point_list(object: &Map<String, Value>, field: &str) -> Result<Vec<(i64, i64)>, SvgError> {
    parse_points(
        object
            .get(field)
            .ok_or_else(|| SvgError(format!("missing point list {field}")))?,
    )
}

fn parse_points(value: &Value) -> Result<Vec<(i64, i64)>, SvgError> {
    value
        .as_array()
        .ok_or_else(|| SvgError("points must be an array".to_owned()))?
        .iter()
        .map(|point| {
            let pair = point
                .as_array()
                .filter(|pair| pair.len() == 2)
                .ok_or_else(|| SvgError("point must contain two coordinates".to_owned()))?;
            Ok((
                pair[0]
                    .as_i64()
                    .ok_or_else(|| SvgError("point X must be an integer".to_owned()))?,
                pair[1]
                    .as_i64()
                    .ok_or_else(|| SvgError("point Y must be an integer".to_owned()))?,
            ))
        })
        .collect()
}

fn write_path_points(sink: &mut SvgSink, points: &[(i64, i64)]) -> Result<(), SvgError> {
    for (index, (x, y)) in points.iter().enumerate() {
        sink.raw(if index == 0 { "M " } else { " L " })?;
        sink.raw(&format!("{x} {y}"))?;
    }
    sink.raw(" Z ")
}

fn number_attrs(
    sink: &mut SvgSink,
    object: &Map<String, Value>,
    fields: &[(&str, &str)],
) -> Result<(), SvgError> {
    for (attribute, field) in fields {
        sink.attribute(attribute, &integer(object, field)?.to_string())?;
    }
    Ok(())
}

fn integer(object: &Map<String, Value>, field: &str) -> Result<i64, SvgError> {
    object
        .get(field)
        .and_then(Value::as_i64)
        .ok_or_else(|| SvgError(format!("missing integer field {field}")))
}

fn optional_integer(object: &Map<String, Value>, field: &str) -> Result<Option<i64>, SvgError> {
    object
        .get(field)
        .map(|value| {
            value
                .as_i64()
                .ok_or_else(|| SvgError(format!("field {field} must be an integer")))
        })
        .transpose()
}

fn nonnegative_integer(object: &Map<String, Value>, field: &str) -> Result<i64, SvgError> {
    let value = integer(object, field)?;
    if value < 0 {
        Err(SvgError(format!("field {field} must be nonnegative")))
    } else {
        Ok(value)
    }
}

fn float(object: &Map<String, Value>, field: &str) -> Option<f64> {
    object.get(field).and_then(Value::as_f64)
}

fn boolean(object: &Map<String, Value>, field: &str) -> Option<bool> {
    object.get(field).and_then(Value::as_bool)
}

fn string<'a>(object: &'a Map<String, Value>, field: &str) -> Result<&'a str, SvgError> {
    optional_string(object, field)?.ok_or_else(|| SvgError(format!("missing string field {field}")))
}

fn optional_string<'a>(
    object: &'a Map<String, Value>,
    field: &str,
) -> Result<Option<&'a str>, SvgError> {
    object
        .get(field)
        .map(|value| {
            value
                .as_str()
                .ok_or_else(|| SvgError(format!("field {field} must be a string")))
        })
        .transpose()
}

fn valid_data_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

fn centered_start(center: i64, size: i64) -> String {
    half_number(i128::from(center) * 2 - i128::from(size))
}

fn half_number(numerator: i128) -> String {
    let whole = numerator / 2;
    if numerator % 2 == 0 {
        return whole.to_string();
    }
    if numerator == -1 {
        "-0.5".to_owned()
    } else {
        format!("{whole}.5")
    }
}

fn number(value: f64) -> String {
    let normalized = if value.abs() < 0.000_000_5 {
        0.0
    } else {
        value
    };
    format!("{normalized:.6}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::{half_number, render_operation};
    use crate::sink::SvgSink;
    use serde_json::json;

    #[test]
    fn odd_nanometres_and_legacy_cache_polygons_remain_exact() {
        assert_eq!(half_number(3), "1.5");
        assert_eq!(half_number(-1), "-0.5");
        let operation = json!({
            "kind": "Text",
            "index": 0,
            "x": 0,
            "y": 0,
            "text": "legacy",
            "color": "#102030FF",
            "orient_deg": 0.0,
            "size_x_nm": 1,
            "size_y_nm": 1,
            "h_align": "GR_TEXT_H_ALIGN_LEFT",
            "v_align": "GR_TEXT_V_ALIGN_BOTTOM",
            "pen_width_nm": 0,
            "italic": false,
            "bold": false,
            "multiline": false,
            "font_face": "",
            "render_cache_polygons": [[[0, 0], [3, 0], [3, 3]]]
        });
        let mut sink = SvgSink::new(4096, 10, 4096);
        let mut depth = 0;
        let mut maximum_depth = 0;
        render_operation(&operation, &mut sink, &mut depth, &mut maximum_depth, 10)
            .expect("legacy cache polygon");
        let (svg, elements, _) = sink.finish().expect("SVG fragment");
        assert_eq!(elements, 1);
        assert!(svg.contains("<polygon points=\"0,0 3,0 3,3\""));
        assert!(!svg.contains("<text"));
    }

    #[test]
    fn invalid_unicode_color_and_orphan_block_fail_without_panicking() {
        let operation = json!({
            "kind": "Circle",
            "index": 0,
            "cx": 0,
            "cy": 0,
            "diameter_nm": 3,
            "fill": "FILLED_SHAPE",
            "width_nm": 0,
            "fill_color": "#aaaaaé?"
        });
        let mut sink = SvgSink::new(4096, 10, 4096);
        let mut depth = 0;
        let mut maximum_depth = 0;
        assert!(
            render_operation(&operation, &mut sink, &mut depth, &mut maximum_depth, 10,).is_err()
        );

        let mut sink = SvgSink::new(4096, 10, 4096);
        let mut depth = 0;
        let mut maximum_depth = 0;
        assert!(
            render_operation(
                &json!({"kind": "EndBlock", "index": 0}),
                &mut sink,
                &mut depth,
                &mut maximum_depth,
                10,
            )
            .is_err()
        );
    }

    #[test]
    fn oval_pad_is_an_exact_rotated_stadium_and_negative_geometry_fails_closed() {
        let oval = json!({
            "kind": "FlashPadOval",
            "index": 0,
            "x": -1_000_000,
            "y": 0,
            "size_x_nm": 2_000_001,
            "size_y_nm": 1_000_000,
            "orient_deg": 30.0
        });
        let mut sink = SvgSink::new(4096, 10, 4096);
        let mut depth = 0;
        let mut maximum_depth = 0;
        render_operation(&oval, &mut sink, &mut depth, &mut maximum_depth, 10)
            .expect("oval stadium");
        let (svg, elements, _) = sink.finish().expect("SVG fragment");
        assert_eq!(elements, 1);
        assert!(svg.starts_with("<line x1=\"-1500000.5\" y1=\"0\" x2=\"-499999.5\" y2=\"0\""));
        assert!(svg.contains("transform=\"rotate(-30 -1000000 0)\""));
        assert!(svg.contains("stroke-width=\"1000000\" stroke-linecap=\"round\""));
        assert!(!svg.contains("<ellipse"));

        for invalid in [
            json!({
                "kind": "ThickSegment", "index": 0,
                "start_x": 0, "start_y": 0, "end_x": 1, "end_y": 1,
                "width_nm": -1
            }),
            json!({
                "kind": "Circle", "index": 0, "cx": 0, "cy": 0,
                "diameter_nm": -1, "width_nm": 0, "fill": "NO_FILL"
            }),
            json!({
                "kind": "FlashPadCircle", "index": 0, "x": 0, "y": 0,
                "diameter_nm": -1
            }),
            json!({
                "kind": "FlashPadOval", "index": 0, "x": 0, "y": 0,
                "size_x_nm": -1, "size_y_nm": 1, "orient_deg": 0.0
            }),
            json!({
                "kind": "FlashPadRect", "index": 0, "x": 0, "y": 0,
                "size_x_nm": 1, "size_y_nm": -1, "orient_deg": 0.0
            }),
            json!({
                "kind": "FlashPadRoundRect", "index": 0, "x": 0, "y": 0,
                "size_x_nm": 1, "size_y_nm": 1, "corner_radius_nm": -1,
                "orient_deg": 0.0
            }),
        ] {
            let mut sink = SvgSink::new(4096, 10, 4096);
            let mut depth = 0;
            let mut maximum_depth = 0;
            assert!(
                render_operation(&invalid, &mut sink, &mut depth, &mut maximum_depth, 10).is_err()
            );
        }
    }
}
