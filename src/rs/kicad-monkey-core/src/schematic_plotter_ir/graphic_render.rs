//! Bounded top-level schematic graphics, rule-area, and image projection.

use super::*;
use crate::plotter_types::{ArcThreePoint, BezierCurve, PlotterRect};

const NOTES_COLOR: &str = "#0000C2FF";
const BACKGROUND_COLOR: &str = "#F5F4EFFF";
const DEFAULT_GRAPHIC_WIDTH_MM: f64 = 0.1524;

#[derive(Clone)]
struct GraphicStyle {
    fill: PlotterFill,
    stroke: Stroke,
    fill_color: Option<String>,
}

#[derive(Default)]
struct ImageTotals {
    data_parts: usize,
    encoded: usize,
    decoded: usize,
    pixels: usize,
    work: usize,
}

pub(super) fn append_graphic_records(
    source: &str,
    spans: &GraphicSpans,
    limits: SchematicPlotLimits,
    budget: &mut PlotBudget,
    records: &mut Vec<SchematicPlotRecord>,
) -> Result<(), Error> {
    append_shape_family(
        source,
        &spans.polylines,
        SchematicGraphicRecordKind::GraphicPolyline,
        limits,
        budget,
        records,
    )?;
    append_shape_family(
        source,
        &spans.arcs,
        SchematicGraphicRecordKind::GraphicArc,
        limits,
        budget,
        records,
    )?;
    append_shape_family(
        source,
        &spans.circles,
        SchematicGraphicRecordKind::GraphicCircle,
        limits,
        budget,
        records,
    )?;
    append_shape_family(
        source,
        &spans.rectangles,
        SchematicGraphicRecordKind::GraphicRectangle,
        limits,
        budget,
        records,
    )?;
    append_shape_family(
        source,
        &spans.beziers,
        SchematicGraphicRecordKind::GraphicBezier,
        limits,
        budget,
        records,
    )
}

fn append_shape_family(
    source: &str,
    spans: &[FormSpan],
    kind: SchematicGraphicRecordKind,
    limits: SchematicPlotLimits,
    budget: &mut PlotBudget,
    records: &mut Vec<SchematicPlotRecord>,
) -> Result<(), Error> {
    for span in spans {
        let form = parse_span(source, span, limits)?;
        let (operations, input_points) = shape_operations(
            &form,
            kind,
            false,
            budget.remaining_input_points(),
            budget.remaining_operations(),
            budget.remaining_points(),
        )?;
        budget.charge_input_points(input_points)?;
        if operations.is_empty() {
            continue;
        }
        let uuid = child_string(&form, "uuid").unwrap_or_default();
        let points = operations.iter().map(operation_points).sum();
        charge_operation_metadata(budget, &operations)?;
        budget.charge_metadata(uuid.len().saturating_mul(2))?;
        budget.charge(1, operations.len(), points)?;
        records.push(SchematicPlotRecord::Graphic(SchematicGraphicRecord {
            uuid,
            kind,
            operations,
        }));
    }
    Ok(())
}

pub(super) fn append_rule_area_records(
    source: &str,
    spans: &GraphicSpans,
    limits: SchematicPlotLimits,
    budget: &mut PlotBudget,
    records: &mut Vec<SchematicPlotRecord>,
) -> Result<(), Error> {
    for span in &spans.rule_areas {
        let form = parse_span(source, span, limits)?;
        let Some((shape_form, shape, kind)) = first_rule_shape(&form) else {
            continue;
        };
        let (operations, input_points) = shape_operations(
            shape_form,
            kind,
            true,
            budget.remaining_input_points(),
            budget.remaining_operations(),
            budget.remaining_points(),
        )?;
        budget.charge_input_points(input_points)?;
        if operations.is_empty() {
            continue;
        }
        let uuid = child_string(shape_form, "uuid").unwrap_or_default();
        let points = operations.iter().map(operation_points).sum();
        charge_operation_metadata(budget, &operations)?;
        budget.charge_metadata(uuid.len().saturating_mul(2))?;
        budget.charge(1, operations.len(), points)?;
        records.push(SchematicPlotRecord::RuleArea(SchematicRuleAreaRecord {
            uuid,
            shape,
            locked: bool_child(&form, "locked", false),
            exclude_from_sim: bool_child(&form, "exclude_from_sim", false),
            in_bom: bool_child(&form, "in_bom", true),
            on_board: bool_child(&form, "on_board", true),
            dnp: bool_child(&form, "dnp", false),
            operations,
        }));
    }
    Ok(())
}

pub(super) fn append_image_records(
    source: &str,
    spans: &GraphicSpans,
    limits: SchematicPlotLimits,
    budget: &mut PlotBudget,
    records: &mut Vec<SchematicPlotRecord>,
) -> Result<(), Error> {
    let mut totals = ImageTotals::default();
    for span in &spans.images {
        let form = parse_span(source, span, limits)?;
        let [x, y] = child(&form, "at").map_or(Ok([0, 0]), parse_point)?;
        budget.charge_input_points(1)?;
        let scale = child(&form, "scale").map_or(Ok(1.0), |value| number_at(value, 1))?;
        if !scale.is_finite() || scale <= 0.0 {
            return Err(model_error(
                "Schematic image scale must be finite and positive",
            ));
        }
        let data_values = child(&form, "data").and_then(list).unwrap_or(&[]);
        let parts = || data_values.iter().skip(1).filter_map(text);
        totals.data_parts = checked_limit(
            totals.data_parts,
            parts().count(),
            limits.max_image_data_parts,
        )?;
        let encoded_len = parts().try_fold(0usize, |total, part| {
            total.checked_add(part.trim().len()).ok_or_else(limit_error)
        })?;
        totals.encoded =
            checked_limit(totals.encoded, encoded_len, limits.max_image_encoded_bytes)?;
        let mut encoded = String::with_capacity(encoded_len);
        for part in parts() {
            encoded.push_str(part.trim());
        }
        let decoded = image_decode::decode_base64(
            &encoded,
            limits
                .max_image_decoded_bytes
                .saturating_sub(totals.decoded),
        )?;
        totals.decoded = checked_limit(
            totals.decoded,
            decoded.len(),
            limits.max_image_decoded_bytes,
        )?;
        totals.work = checked_limit(
            totals.work,
            encoded_len.saturating_add(decoded.len()),
            limits.max_image_decode_work,
        )?;
        let metadata = image_decode::image_metadata(
            &decoded,
            limits.max_image_decode_work.saturating_sub(totals.work),
        )?;
        totals.work = checked_limit(totals.work, metadata.work, limits.max_image_decode_work)?;
        if metadata.width as usize > limits.max_image_width_px
            || metadata.height as usize > limits.max_image_height_px
        {
            return Err(limit_error());
        }
        let pixels = (metadata.width as usize)
            .checked_mul(metadata.height as usize)
            .ok_or_else(limit_error)?;
        totals.pixels = checked_limit(totals.pixels, pixels, limits.max_image_pixels)?;
        let width_nm = image_decode::extent_nm(metadata.width, scale, metadata.ppi_x)?;
        let height_nm = image_decode::extent_nm(metadata.height, scale, metadata.ppi_y)?;
        if width_nm < 0 || height_nm < 0 {
            return Err(model_error("Schematic image extents must be non-negative"));
        }
        let uuid = child_string(&form, "uuid").unwrap_or_default();
        let image_format = metadata.format.as_str().to_owned();
        budget.charge_metadata(
            uuid.len()
                .saturating_mul(2)
                .saturating_add(image_format.len().saturating_mul(2))
                .saturating_add(encoded.len())
                .saturating_add(NOTES_COLOR.len()),
        )?;
        budget.charge(1, 1, 0)?;
        records.push(SchematicPlotRecord::Image(SchematicImageRecord {
            uuid,
            scale,
            image_format: image_format.clone(),
            width_nm,
            height_nm,
            operations: vec![SchematicPlotOperation::PlotImage(PlotterImage {
                x,
                y,
                width_nm,
                height_nm,
                scale,
                image_data_b64: encoded,
                image_format,
                stroke_color: Some(NOTES_COLOR.to_owned()),
            })],
        }));
    }
    Ok(())
}

fn first_rule_shape(
    form: &Sexp,
) -> Option<(&Sexp, SchematicRuleAreaShape, SchematicGraphicRecordKind)> {
    list(form)?.iter().skip(1).find_map(|value| {
        let head = list(value)?.first().and_then(text)?;
        match head {
            "polyline" => Some((
                value,
                SchematicRuleAreaShape::Polyline,
                SchematicGraphicRecordKind::GraphicPolyline,
            )),
            "rectangle" => Some((
                value,
                SchematicRuleAreaShape::Rectangle,
                SchematicGraphicRecordKind::GraphicRectangle,
            )),
            "arc" => Some((
                value,
                SchematicRuleAreaShape::Arc,
                SchematicGraphicRecordKind::GraphicArc,
            )),
            "circle" => Some((
                value,
                SchematicRuleAreaShape::Circle,
                SchematicGraphicRecordKind::GraphicCircle,
            )),
            "bezier" => Some((
                value,
                SchematicRuleAreaShape::Bezier,
                SchematicGraphicRecordKind::GraphicBezier,
            )),
            _ => None,
        }
    })
}

fn shape_operations(
    form: &Sexp,
    kind: SchematicGraphicRecordKind,
    close_rule_polyline: bool,
    max_input_points: usize,
    max_operations: usize,
    max_output_points: usize,
) -> Result<(Vec<SchematicPlotOperation>, usize), Error> {
    let style = graphic_style(form)?;
    let (operation, input_points) = match kind {
        SchematicGraphicRecordKind::GraphicPolyline => {
            let mut points = child(form, "pts").map_or(Ok(Vec::new()), |points| {
                parse_points_limited(points, max_input_points)
            })?;
            let input_points = points.len();
            if points.len() < 2 {
                return Ok((Vec::new(), input_points));
            }
            if close_rule_polyline && points.last() != points.first() {
                points.push(points[0]);
            }
            (
                PlotterOperation::PlotPoly(PlotterPoly {
                    points,
                    fill: style.fill,
                    width_nm: style.stroke.width_nm,
                    layer: None,
                    stroke_color: Some(style.stroke.color.clone()),
                    fill_color: style.fill_color.clone(),
                    line_style: Some(style.stroke.style),
                }),
                input_points,
            )
        }
        SchematicGraphicRecordKind::GraphicArc => {
            let [start_x, start_y] = child(form, "start").map_or(Ok([0, 0]), parse_point)?;
            let [mid_x, mid_y] = child(form, "mid").map_or(Ok([0, 0]), parse_point)?;
            let [end_x, end_y] = child(form, "end").map_or(Ok([0, 0]), parse_point)?;
            (
                PlotterOperation::ArcThreePoint(ArcThreePoint {
                    start_x,
                    start_y,
                    mid_x,
                    mid_y,
                    end_x,
                    end_y,
                    fill: style.fill,
                    width_nm: style.stroke.width_nm,
                    layer: None,
                    stroke_color: Some(style.stroke.color.clone()),
                    fill_color: style.fill_color.clone(),
                    line_style: Some(style.stroke.style),
                }),
                3,
            )
        }
        SchematicGraphicRecordKind::GraphicCircle => {
            let [cx, cy] = child(form, "center").map_or(Ok([0, 0]), parse_point)?;
            let radius = child(form, "radius").map_or(Ok(0.0), |value| number_at(value, 1))?;
            if radius < 0.0 {
                return Err(model_error("Schematic circle radius must be non-negative"));
            }
            (
                PlotterOperation::Circle(PlotterCircle {
                    cx,
                    cy,
                    diameter_nm: mm_to_nm(radius * 2.0)?,
                    fill: style.fill,
                    width_nm: style.stroke.width_nm,
                    layer: None,
                    role: None,
                    layers: Vec::new(),
                    mask_margin_nm: None,
                    pad_size_x_nm: None,
                    pad_size_y_nm: None,
                    stroke_color: Some(style.stroke.color.clone()),
                    fill_color: style.fill_color.clone(),
                    line_style: Some(style.stroke.style),
                }),
                1,
            )
        }
        SchematicGraphicRecordKind::GraphicRectangle => {
            let [x1, y1] = child(form, "start").map_or(Ok([0, 0]), parse_point)?;
            let [x2, y2] = child(form, "end").map_or(Ok([0, 0]), parse_point)?;
            let radius = child(form, "radius").map_or(Ok(0.0), |value| number_at(value, 1))?;
            if radius < 0.0 {
                return Err(model_error(
                    "Schematic rectangle radius must be non-negative",
                ));
            }
            (
                PlotterOperation::Rect(PlotterRect {
                    x1,
                    y1,
                    x2,
                    y2,
                    fill: style.fill,
                    width_nm: style.stroke.width_nm,
                    corner_radius_nm: mm_to_nm(radius)?,
                    layer: None,
                    stroke_color: Some(style.stroke.color.clone()),
                    fill_color: style.fill_color.clone(),
                    line_style: Some(style.stroke.style),
                }),
                2,
            )
        }
        SchematicGraphicRecordKind::GraphicBezier => {
            let points = child(form, "pts").map_or(Ok(Vec::new()), |points| {
                parse_points_limited(points, max_input_points)
            })?;
            let input_points = points.len();
            if points.len() < 2 {
                return Ok((Vec::new(), input_points));
            }
            if points.len() == 4 {
                (
                    PlotterOperation::BezierCurve(BezierCurve {
                        start_x: points[0][0],
                        start_y: points[0][1],
                        ctrl1_x: points[1][0],
                        ctrl1_y: points[1][1],
                        ctrl2_x: points[2][0],
                        ctrl2_y: points[2][1],
                        end_x: points[3][0],
                        end_y: points[3][1],
                        width_nm: style.stroke.width_nm,
                        tolerance_nm: 0,
                        layer: None,
                        stroke_color: Some(style.stroke.color.clone()),
                        line_style: Some(style.stroke.style),
                    }),
                    input_points,
                )
            } else {
                (
                    PlotterOperation::PlotPoly(PlotterPoly {
                        points,
                        fill: style.fill,
                        width_nm: style.stroke.width_nm,
                        layer: None,
                        stroke_color: Some(style.stroke.color.clone()),
                        fill_color: style.fill_color.clone(),
                        line_style: Some(style.stroke.style),
                    }),
                    input_points,
                )
            }
        }
    };
    let multiplier = if matches!(
        &operation,
        PlotterOperation::ArcThreePoint(value) if !matches!(value.fill, PlotterFill::NoFill | PlotterFill::FilledShape)
    ) || matches!(
        &operation,
        PlotterOperation::Circle(value) if !matches!(value.fill, PlotterFill::NoFill | PlotterFill::FilledShape)
    ) || matches!(
        &operation,
        PlotterOperation::Rect(value) if !matches!(value.fill, PlotterFill::NoFill | PlotterFill::FilledShape)
    ) || matches!(
        &operation,
        PlotterOperation::PlotPoly(value) if !matches!(value.fill, PlotterFill::NoFill | PlotterFill::FilledShape)
    ) {
        2
    } else {
        1
    };
    let base_points = match &operation {
        PlotterOperation::ArcThreePoint(_) => 3,
        PlotterOperation::PlotPoly(value) => value.points.len(),
        PlotterOperation::BezierCurve(_) => 4,
        _ => 0,
    };
    if multiplier > max_operations
        || base_points
            .checked_mul(multiplier)
            .is_none_or(|points| points > max_output_points)
    {
        return Err(limit_error());
    }
    Ok((
        split_fill_outline(operation)
            .into_iter()
            .map(Into::into)
            .collect(),
        input_points,
    ))
}

fn parse_points_limited(form: &Sexp, maximum: usize) -> Result<Vec<[i64; 2]>, Error> {
    let values = list(form).unwrap_or(&[]);
    let count = values
        .iter()
        .skip(1)
        .filter(|value| {
            list(value).and_then(|items| items.first()).and_then(text) == Some("xy")
                && list(value).is_some_and(|items| items.len() >= 3)
        })
        .count();
    if count > maximum {
        return Err(limit_error());
    }
    let mut points = Vec::with_capacity(count);
    for value in values.iter().skip(1).filter(|value| {
        list(value).and_then(|items| items.first()).and_then(text) == Some("xy")
            && list(value).is_some_and(|items| items.len() >= 3)
    }) {
        points.push(parse_point(value)?);
    }
    Ok(points)
}

fn graphic_style(form: &Sexp) -> Result<GraphicStyle, Error> {
    let stroke = resolve_stroke(form, DEFAULT_GRAPHIC_WIDTH_MM, NOTES_COLOR)?;
    let fill_form = child(form, "fill");
    let fill_name = fill_form
        .and_then(|fill| child_string(fill, "type"))
        .unwrap_or_else(|| "none".to_owned());
    let fill = match fill_name.as_str() {
        "outline" => PlotterFill::FilledShape,
        "background" => PlotterFill::FilledWithBackgroundBodyColor,
        "color" => PlotterFill::FilledWithColor,
        "hatch" => PlotterFill::Hatch,
        "reverse_hatch" => PlotterFill::ReverseHatch,
        "cross_hatch" => PlotterFill::CrossHatch,
        _ => PlotterFill::NoFill,
    };
    let explicit = fill_form
        .and_then(|value| child(value, "color"))
        .map(parse_color)
        .transpose()?
        .flatten();
    let fill_color = explicit.or_else(|| match fill_name.as_str() {
        "background" => Some(BACKGROUND_COLOR.to_owned()),
        "outline" => Some(stroke.color.clone()),
        "color" | "hatch" | "reverse_hatch" | "cross_hatch" => Some(NOTES_COLOR.to_owned()),
        _ => None,
    });
    Ok(GraphicStyle {
        fill,
        stroke,
        fill_color,
    })
}

fn split_fill_outline(operation: PlotterOperation) -> Vec<PlotterOperation> {
    let fill = match &operation {
        PlotterOperation::ArcThreePoint(value) => value.fill,
        PlotterOperation::Circle(value) => value.fill,
        PlotterOperation::Rect(value) => value.fill,
        PlotterOperation::PlotPoly(value) => value.fill,
        _ => return vec![operation],
    };
    if matches!(fill, PlotterFill::NoFill | PlotterFill::FilledShape) {
        return vec![operation];
    }
    macro_rules! split {
        ($value:ident, $variant:ident) => {{
            let color = $value
                .fill_color
                .clone()
                .or_else(|| $value.stroke_color.clone());
            let mut fill_pass = $value.clone();
            fill_pass.width_nm = 0;
            fill_pass.stroke_color = color.clone();
            fill_pass.fill_color = color;
            let mut outline = $value;
            outline.fill = PlotterFill::NoFill;
            outline.fill_color = None;
            vec![
                PlotterOperation::$variant(fill_pass),
                PlotterOperation::$variant(outline),
            ]
        }};
    }
    match operation {
        PlotterOperation::ArcThreePoint(value) => split!(value, ArcThreePoint),
        PlotterOperation::Circle(value) => split!(value, Circle),
        PlotterOperation::Rect(value) => split!(value, Rect),
        PlotterOperation::PlotPoly(value) => split!(value, PlotPoly),
        _ => unreachable!(),
    }
}

fn bool_child(form: &Sexp, head: &str, default: bool) -> bool {
    child(form, head)
        .and_then(|value| scalar_at(value, 1))
        .map_or(default, |value| value == "yes")
}

fn charge_operation_metadata(
    budget: &mut PlotBudget,
    operations: &[SchematicPlotOperation],
) -> Result<(), Error> {
    for operation in operations {
        let (stroke, fill) = match operation {
            SchematicPlotOperation::Plotter(PlotterOperation::ArcThreePoint(value)) => {
                (value.stroke_color.as_deref(), value.fill_color.as_deref())
            }
            SchematicPlotOperation::Plotter(PlotterOperation::Circle(value)) => {
                (value.stroke_color.as_deref(), value.fill_color.as_deref())
            }
            SchematicPlotOperation::Plotter(PlotterOperation::Rect(value)) => {
                (value.stroke_color.as_deref(), value.fill_color.as_deref())
            }
            SchematicPlotOperation::Plotter(PlotterOperation::PlotPoly(value)) => {
                (value.stroke_color.as_deref(), value.fill_color.as_deref())
            }
            SchematicPlotOperation::Plotter(PlotterOperation::BezierCurve(value)) => {
                (value.stroke_color.as_deref(), None)
            }
            _ => (None, None),
        };
        budget.charge_metadata(
            stroke
                .map_or(0, str::len)
                .saturating_add(fill.map_or(0, str::len)),
        )?;
    }
    Ok(())
}

fn operation_points(operation: &SchematicPlotOperation) -> usize {
    match operation {
        SchematicPlotOperation::Plotter(PlotterOperation::ArcThreePoint(_)) => 3,
        SchematicPlotOperation::Plotter(PlotterOperation::Circle(_)) => 0,
        SchematicPlotOperation::Plotter(PlotterOperation::Rect(_)) => 0,
        SchematicPlotOperation::Plotter(PlotterOperation::PlotPoly(value)) => value.points.len(),
        SchematicPlotOperation::Plotter(PlotterOperation::BezierCurve(_)) => 4,
        _ => 0,
    }
}
