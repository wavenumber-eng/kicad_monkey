//! Bounded schematic annotation-family projection from raw source forms.

use super::*;
use crate::plotter_text_cache::{PlotterTextCacheSession, PlotterTextLayout};
use crate::plotter_types::{PlotterRect, PlotterTextHAlign, PlotterTextVAlign};
use crate::text_metadata::parse_text_effects;
use crate::{TextHorizontalAlignment, TextVerticalAlignment};

const DEFAULT_TEXT_SIZE_NM: i64 = 1_270_000;
const DEFAULT_TEXT_PEN_WIDTH_NM: i64 = 152_400;
const DEFAULT_FONT_FACE: &str = "Arial";
const LOCAL_COLOR: &str = "#0F0F0FFF";
const GLOBAL_COLOR: &str = "#840000FF";
const HIER_COLOR: &str = "#725600FF";
const NOTES_COLOR: &str = "#0000C2FF";
const NETCLASS_COLOR: &str = "#484848FF";
const FIELDS_COLOR: &str = "#840084FF";
const REFERENCE_COLOR: &str = "#006464FF";
const VALUE_COLOR: &str = "#006464FF";
const LABEL_SIZE_RATIO: f64 = 0.375;
const TEXTBOX_INTERLINE_FACTOR: f64 = 1.68;
const SCH_TEXT_PLOT_OFFSET_NM: i64 = 250_000;
const DIRECTIVE_SYMBOL_SIZE_NM: i64 = 508_000;
const JAVASCRIPT_SAFE_INTEGER_MAX: i64 = 9_007_199_254_740_991;
const JAVASCRIPT_SAFE_INTEGER_MIN: i64 = -JAVASCRIPT_SAFE_INTEGER_MAX;

struct AnnotationContext<'a, 'font> {
    source: &'a str,
    settings: SchematicDrawingSettings,
    variables: &'a BTreeMap<String, String>,
    session: Option<&'a PlotterTextCacheSession<'font>>,
    limits: SchematicPlotLimits,
}

pub(super) fn append_annotation_records(
    source: &str,
    spans: AnnotationSpans,
    settings: SchematicDrawingSettings,
    variables: &BTreeMap<String, String>,
    session: Option<&PlotterTextCacheSession<'_>>,
    limits: SchematicPlotLimits,
    output: (&mut PlotBudget, &mut Vec<SchematicPlotRecord>),
) -> Result<(), Error> {
    let (budget, records) = output;
    let context = AnnotationContext {
        source,
        settings,
        variables,
        session,
        limits,
    };
    append_labels(
        &context,
        spans.labels,
        SchematicAnnotationRecordKind::Label,
        LOCAL_COLOR,
        budget,
        records,
    )?;
    append_labels(
        &context,
        spans.global_labels,
        SchematicAnnotationRecordKind::GlobalLabel,
        GLOBAL_COLOR,
        budget,
        records,
    )?;
    append_labels(
        &context,
        spans.hierarchical_labels,
        SchematicAnnotationRecordKind::HierarchicalLabel,
        HIER_COLOR,
        budget,
        records,
    )?;
    append_netclass_flags(&context, spans.netclass_flags, budget, records)?;
    append_texts(&context, spans.texts, budget, records)?;
    append_text_boxes(&context, spans.text_boxes, budget, records)
}

fn append_labels(
    context: &AnnotationContext<'_, '_>,
    spans: Vec<FormSpan>,
    kind: SchematicAnnotationRecordKind,
    role_color: &str,
    budget: &mut PlotBudget,
    records: &mut Vec<SchematicPlotRecord>,
) -> Result<(), Error> {
    for span in spans {
        let form = parse_span(context.source, &span, context.limits)?;
        let text = value_at(&form, 1).unwrap_or_default();
        let uuid = child_string(&form, "uuid").unwrap_or_default();
        let shape = if kind == SchematicAnnotationRecordKind::Label {
            None
        } else {
            Some(
                child_string(&form, "shape")
                    .filter(|shape| valid_label_shape(shape))
                    .unwrap_or_else(|| "input".to_owned()),
            )
        };
        let at = parse_at(&form)?;
        budget.charge_input_points(1)?;
        let text_color = if looks_like_bus_label(&text) {
            BUS_COLOR
        } else {
            role_color
        };
        let mut style = text_style(
            &form,
            text_color,
            Some(context.settings.default_line_width_nm),
        )?;
        if matches!(
            kind,
            SchematicAnnotationRecordKind::GlobalLabel
                | SchematicAnnotationRecordKind::HierarchicalLabel
        ) {
            style.v_align = PlotterTextVAlign::Center;
        }
        let spin = label_spin(&form, &style, at.angle);
        let (dx, dy) = label_offset(
            kind,
            shape.as_deref(),
            spin,
            &style,
            context.settings.text_offset_ratio,
        )?;
        let display = text.replace("{slash}", "/");
        let text_op = schematic_text(
            checked_add(at.x, dx)?,
            checked_add(at.y, dy)?,
            display.clone(),
            if matches!(spin, 1 | 3) { 90.0 } else { 0.0 },
            style.clone(),
            false,
        );
        let mut operations = vec![text_op];
        if let Some(shape) = shape.as_deref().filter(|shape| decoration_shape(shape)) {
            let decoration = match kind {
                SchematicAnnotationRecordKind::GlobalLabel => {
                    global_decoration(&form, shape, at, spin, &style, context.session)?
                }
                SchematicAnnotationRecordKind::HierarchicalLabel => Some(template_decoration(
                    shape,
                    at,
                    angle_spin(at.angle),
                    style.size_y_nm,
                )?),
                _ => None,
            };
            if let Some(op) = decoration {
                operations.push(op);
            }
        }
        let points = operations.iter().map(operation_points).sum();
        charge_text_operations(budget, &operations)?;
        budget.charge_metadata(
            uuid.len()
                .saturating_add(text.len().saturating_mul(2))
                .saturating_add(shape.as_deref().map_or(0, str::len)),
        )?;
        budget.charge(1, operations.len(), points)?;
        records.push(SchematicPlotRecord::Annotation(SchematicAnnotationRecord {
            uuid,
            kind,
            object_id: text.clone(),
            text: Some(text),
            shape,
            at_x_nm: None,
            at_y_nm: None,
            length_nm: None,
            operations,
        }));
    }
    Ok(())
}

fn append_netclass_flags(
    context: &AnnotationContext<'_, '_>,
    spans: Vec<FormSpan>,
    budget: &mut PlotBudget,
    records: &mut Vec<SchematicPlotRecord>,
) -> Result<(), Error> {
    let mut property_count = 0usize;
    for span in spans {
        let form = parse_span(context.source, &span, context.limits)?;
        let text = value_at(&form, 1).unwrap_or_default();
        let uuid = child_string(&form, "uuid").unwrap_or_default();
        let at = parse_at(&form)?;
        let length = child(&form, "length")
            .map_or_else(|| mm_to_nm(2.54), |value| mm_to_nm(number_at(value, 1)?))?;
        let shape = child_string(&form, "shape").unwrap_or_else(|| "round".to_owned());
        if !matches!(shape.as_str(), "round" | "dot" | "diamond" | "rectangle") {
            return Err(model_error("Unsupported schematic netclass flag shape"));
        }
        budget.charge_input_points(1)?;
        let style = text_style(
            &form,
            NETCLASS_COLOR,
            Some(context.settings.default_line_width_nm),
        )?;
        let mut operations = directive_marker_ops(at, length, &shape, style.pen_width_nm)?;
        for property in children(&form, "property") {
            property_count = checked_limit(
                property_count,
                1,
                context.limits.max_netclass_flag_properties,
            )?;
            budget.charge_input_points(1)?;
            if let Some(op) = property_text(property, context.settings.default_line_width_nm)? {
                operations.push(op);
            }
        }
        let points = operations.iter().map(operation_points).sum();
        charge_text_operations(budget, &operations)?;
        let object_id = if text.is_empty() {
            uuid.clone()
        } else {
            text.clone()
        };
        budget.charge_metadata(
            uuid.len()
                .saturating_add(object_id.len())
                .saturating_add(shape.len()),
        )?;
        budget.charge(1, operations.len(), points)?;
        records.push(SchematicPlotRecord::Annotation(SchematicAnnotationRecord {
            uuid: uuid.clone(),
            kind: SchematicAnnotationRecordKind::NetclassFlag,
            object_id,
            text: None,
            shape: Some(shape),
            at_x_nm: Some(at.x),
            at_y_nm: Some(at.y),
            length_nm: Some(length),
            operations,
        }));
    }
    Ok(())
}

fn append_texts(
    context: &AnnotationContext<'_, '_>,
    spans: Vec<FormSpan>,
    budget: &mut PlotBudget,
    records: &mut Vec<SchematicPlotRecord>,
) -> Result<(), Error> {
    for span in spans {
        let form = parse_span(context.source, &span, context.limits)?;
        let raw_text = value_at(&form, 1).unwrap_or_default();
        let expanded = expand_once_bounded(
            &raw_text,
            context.variables,
            context
                .limits
                .max_text_bytes
                .min(context.limits.max_metadata_bytes),
        )?;
        let plotted = expanded.strip_suffix('\n').unwrap_or(&expanded).to_owned();
        let uuid = child_string(&form, "uuid").unwrap_or_default();
        let at = parse_at(&form)?;
        budget.charge_input_points(1)?;
        let mut style = text_style(
            &form,
            NOTES_COLOR,
            Some(context.settings.default_line_width_nm),
        )?;
        apply_center_defaults(&form, &mut style);
        let adjustment = outline_adjust(&plotted, &style, context.session)?;
        let (adjust_x, adjust_y) = rotate(0.0, -(adjustment as f64), -at.angle);
        let op = schematic_text(
            checked_add(at.x, ki_round_i64(adjust_x)?)?,
            checked_add(
                checked_add(at.y, -SCH_TEXT_PLOT_OFFSET_NM)?,
                ki_round_i64(adjust_y)?,
            )?,
            plotted.clone(),
            at.angle,
            style,
            plotted.contains('\n'),
        );
        charge_text_operations(budget, std::slice::from_ref(&op))?;
        budget.charge_metadata(uuid.len().saturating_mul(2).saturating_add(expanded.len()))?;
        budget.charge(1, 1, 0)?;
        records.push(SchematicPlotRecord::Annotation(SchematicAnnotationRecord {
            uuid: uuid.clone(),
            kind: SchematicAnnotationRecordKind::Text,
            object_id: uuid,
            text: Some(expanded),
            shape: None,
            at_x_nm: None,
            at_y_nm: None,
            length_nm: None,
            operations: vec![op],
        }));
    }
    Ok(())
}

fn append_text_boxes(
    context: &AnnotationContext<'_, '_>,
    spans: Vec<FormSpan>,
    budget: &mut PlotBudget,
    records: &mut Vec<SchematicPlotRecord>,
) -> Result<(), Error> {
    let mut total_lines = 0usize;
    for span in spans {
        let form = parse_span(context.source, &span, context.limits)?;
        let uuid = child_string(&form, "uuid").unwrap_or_default();
        let remaining_lines = context
            .limits
            .max_text_box_lines
            .checked_sub(total_lines)
            .ok_or_else(limit_error)?;
        let rendered = render_text_box(
            &form,
            context.variables,
            context.session,
            context.limits,
            remaining_lines,
            budget.remaining_operations(),
            budget,
        )?;
        total_lines = total_lines
            .checked_add(rendered.line_count)
            .ok_or_else(limit_error)?;
        budget.charge_metadata(
            uuid.len()
                .saturating_mul(2)
                .saturating_add(rendered.expanded.len()),
        )?;
        budget.charge(1, rendered.operations.len(), rendered.points)?;
        records.push(SchematicPlotRecord::Annotation(SchematicAnnotationRecord {
            uuid: uuid.clone(),
            kind: SchematicAnnotationRecordKind::TextBox,
            object_id: uuid,
            text: Some(rendered.expanded),
            shape: None,
            at_x_nm: None,
            at_y_nm: None,
            length_nm: None,
            operations: rendered.operations,
        }));
    }
    Ok(())
}

pub(super) struct RenderedTextBox {
    pub expanded: String,
    pub operations: Vec<SchematicPlotOperation>,
    pub line_count: usize,
    pub points: usize,
}

pub(super) fn render_text_box(
    form: &Sexp,
    variables: &BTreeMap<String, String>,
    metrics: Option<&PlotterTextCacheSession<'_>>,
    limits: SchematicPlotLimits,
    remaining_lines: usize,
    remaining_operations: usize,
    budget: &mut PlotBudget,
) -> Result<RenderedTextBox, Error> {
    let raw_text = value_at(form, 1).unwrap_or_default();
    let expanded = expand_once_bounded(
        &raw_text,
        variables,
        limits.max_text_bytes.min(limits.max_metadata_bytes),
    )?;
    let at = parse_at(form)?;
    let [size_x, size_y] = child(form, "size").map_or(Ok([0, 0]), parse_point)?;
    budget.charge_input_points(2)?;
    let mut style = text_style(form, NOTES_COLOR, None)?;
    apply_center_defaults(form, &mut style);
    let stroke = resolve_stroke(form, DEFAULT_WIRE_WIDTH_MM, NOTES_COLOR)?;
    let fill_form = child(form, "fill");
    let fill_name = fill_form
        .and_then(|fill| child_string(fill, "type"))
        .unwrap_or_else(|| "none".to_owned());
    let fill = plot_fill(&fill_name);
    let fill_color = fill_form
        .and_then(|value| child(value, "color"))
        .map(parse_color)
        .transpose()?
        .flatten();
    let x2 = checked_add(at.x, size_x)?;
    let y2 = checked_add(at.y, size_y)?;
    let outline = PlotterRect {
        x1: at.x,
        y1: at.y,
        x2,
        y2,
        fill,
        width_nm: stroke.width_nm,
        corner_radius_nm: 0,
        layer: None,
        stroke_color: Some(stroke.color.clone()),
        fill_color: fill_color.clone(),
        line_style: Some(stroke.style),
    };
    let mut operations = Vec::new();
    if !matches!(fill, PlotterFill::NoFill | PlotterFill::FilledShape) {
        let color = fill_color.unwrap_or_else(|| stroke.color.clone());
        let mut fill_pass = outline.clone();
        fill_pass.width_nm = 0;
        fill_pass.stroke_color = Some(color.clone());
        fill_pass.fill_color = Some(color);
        operations.push(PlotterOperation::Rect(fill_pass).into());
        let mut outline_pass = outline;
        outline_pass.fill = PlotterFill::NoFill;
        outline_pass.fill_color = None;
        operations.push(PlotterOperation::Rect(outline_pass).into());
    } else {
        operations.push(PlotterOperation::Rect(outline).into());
    }
    let margins = text_box_margins(form, style.size_y_nm)?;
    let mut lines = wrap_text_box(
        &expanded,
        TextBoxWrapInput {
            angle: at.angle,
            size_x,
            size_y,
            margins,
            style: &style,
            metrics,
            maximum_lines: remaining_lines,
        },
    )?;
    let line_count = lines.len();
    let retained_lines = lines.iter().filter(|line| !line.is_empty()).count();
    if operations
        .len()
        .checked_add(retained_lines)
        .is_none_or(|count| count > remaining_operations)
    {
        return Err(limit_error());
    }
    let positions = text_box_positions(at, size_x, size_y, margins, &style, line_count)?;
    for (line, (x, y)) in lines.drain(..).zip(positions) {
        if line.is_empty() {
            continue;
        }
        operations.push(schematic_text(x, y, line, at.angle, style.clone(), false));
    }
    let points = operations.iter().map(operation_points).sum();
    charge_text_operations(budget, &operations)?;
    Ok(RenderedTextBox {
        expanded,
        operations,
        line_count,
        points,
    })
}

#[derive(Clone, Copy)]
pub(super) struct At {
    pub(super) x: i64,
    pub(super) y: i64,
    pub(super) angle: f64,
}

#[derive(Clone)]
pub(super) struct TextStyle {
    pub(super) color: String,
    pub(super) size_x_nm: i64,
    pub(super) size_y_nm: i64,
    pub(super) h_align: PlotterTextHAlign,
    pub(super) v_align: PlotterTextVAlign,
    pub(super) pen_width_nm: i64,
    pub(super) italic: bool,
    pub(super) bold: bool,
    pub(super) mirror: bool,
    pub(super) font_face: String,
    pub(super) hyperlink_href: Option<String>,
}

pub(super) fn parse_at(form: &Sexp) -> Result<At, Error> {
    let Some(at) = child(form, "at") else {
        return Ok(At {
            x: 0,
            y: 0,
            angle: 0.0,
        });
    };
    Ok(At {
        x: mm_to_nm(number_at(at, 1)?)?,
        y: mm_to_nm(number_at(at, 2)?)?,
        angle: list(at)
            .is_some_and(|values| values.len() > 3)
            .then(|| number_at(at, 3))
            .transpose()?
            .unwrap_or(0.0),
    })
}

pub(super) fn text_style(
    form: &Sexp,
    role_color: &str,
    drawing_width: Option<i64>,
) -> Result<TextStyle, Error> {
    let effects = parse_text_effects(form)?.unwrap_or_default();
    let size_x_nm = positive_text_size(effects.font.size_x)?;
    let size_y_nm = positive_text_size(effects.font.size_y)?;
    let color = effects
        .font
        .color
        .and_then(|color| color_hex(color.red, color.green, color.blue, color.alpha))
        .unwrap_or_else(|| role_color.to_owned());
    let explicit_thickness = effects.font.thickness;
    let mut pen_width_nm = match explicit_thickness {
        Some(value) if value > 0.0 => mm_to_nm(value)?,
        _ if effects.font.bold => round_to_100(size_x_nm as f64 / 5.0)?,
        _ => DEFAULT_TEXT_PEN_WIDTH_NM,
    };
    if explicit_thickness.is_none()
        && !effects.font.bold
        && let Some(width) = drawing_width
    {
        pen_width_nm = width.max(MIN_PLOT_PEN_WIDTH_NM);
    }
    pen_width_nm =
        pen_width_nm.min(((size_x_nm.abs().min(size_y_nm.abs()) as f64) * 0.25 + 0.5) as i64);
    let mut h_align = PlotterTextHAlign::Left;
    let mut v_align = PlotterTextVAlign::Bottom;
    for token in &effects.justify {
        match token.as_str() {
            "left" => h_align = PlotterTextHAlign::Left,
            "center" => h_align = PlotterTextHAlign::Center,
            "right" => h_align = PlotterTextHAlign::Right,
            "top" => v_align = PlotterTextVAlign::Top,
            "bottom" => v_align = PlotterTextVAlign::Bottom,
            _ => {}
        }
    }
    Ok(TextStyle {
        color,
        size_x_nm,
        size_y_nm,
        h_align,
        v_align,
        pen_width_nm,
        italic: effects.font.italic,
        bold: effects.font.bold,
        mirror: effects.justify.iter().any(|token| token == "mirror"),
        font_face: effects
            .font
            .face
            .unwrap_or_else(|| DEFAULT_FONT_FACE.to_owned()),
        hyperlink_href: effects
            .href
            .map(|href| href.trim().to_owned())
            .filter(|href| !href.is_empty()),
    })
}

fn positive_text_size(value: f64) -> Result<i64, Error> {
    if value == 0.0 {
        Ok(DEFAULT_TEXT_SIZE_NM)
    } else if value < 0.0 {
        Err(model_error("Schematic text size must be non-negative"))
    } else {
        mm_to_nm(value)
    }
}

pub(super) fn schematic_text(
    x: i64,
    y: i64,
    text: String,
    orient_deg: f64,
    style: TextStyle,
    multiline: bool,
) -> SchematicPlotOperation {
    SchematicPlotOperation::Text(SchematicTextOperation {
        text: PlotterText {
            x,
            y,
            text,
            color: style.color,
            orient_deg,
            size_x_nm: style.size_x_nm,
            size_y_nm: style.size_y_nm,
            h_align: style.h_align,
            v_align: style.v_align,
            pen_width_nm: style.pen_width_nm,
            italic: style.italic,
            bold: style.bold,
            mirror: style.mirror,
            multiline,
            font_face: style.font_face,
            layer: None,
        },
        hyperlink_href: style.hyperlink_href,
    })
}

fn label_spin(form: &Sexp, style: &TextStyle, angle: f64) -> usize {
    let has_horizontal = child(form, "effects")
        .and_then(|effects| child(effects, "justify"))
        .and_then(list)
        .is_some_and(|values| {
            values
                .iter()
                .skip(1)
                .filter_map(text)
                .any(|value| matches!(value, "left" | "center" | "right"))
        });
    if has_horizontal {
        let right = style.h_align == PlotterTextHAlign::Right;
        if rounded_angle(angle) % 180 == 90 {
            if right { 3 } else { 1 }
        } else if right {
            0
        } else {
            2
        }
    } else {
        match rounded_angle(angle) {
            0 => 2,
            90 => 1,
            180 => 0,
            270 => 3,
            _ => 2,
        }
    }
}

fn label_offset(
    kind: SchematicAnnotationRecordKind,
    shape: Option<&str>,
    spin: usize,
    style: &TextStyle,
    ratio: f64,
) -> Result<(i64, i64), Error> {
    if kind == SchematicAnnotationRecordKind::GlobalLabel {
        let mut horizontal = ki_round_i64(LABEL_SIZE_RATIO * style.size_y_nm as f64)?;
        if matches!(shape, Some("input" | "bidirectional" | "tri_state")) {
            horizontal = checked_add(horizontal, checked_mul(style.size_y_nm, 3)? / 4)?;
        }
        let vertical = (style.size_y_nm as f64 * 0.0715) as i64;
        return Ok(match spin {
            0 => (checked_neg(horizontal)?, vertical),
            1 => (vertical, checked_neg(horizontal)?),
            3 => (vertical, horizontal),
            _ => (horizontal, vertical),
        });
    }
    let offset = ki_round_i64(ratio * style.size_y_nm as f64)?;
    if kind == SchematicAnnotationRecordKind::HierarchicalLabel {
        let distance = checked_add(offset, style.size_x_nm)?;
        return Ok(match spin {
            0 => (checked_neg(distance)?, 0),
            1 => (0, checked_neg(distance)?),
            3 => (0, distance),
            _ => (distance, 0),
        });
    }
    let auto_pen = if style.bold {
        ki_round_i64(style.size_x_nm as f64 / 5.0)?
    } else {
        ki_round_i64(style.size_x_nm as f64 / 8.0)?
    }
    .min(ki_round_i64(
        style.size_x_nm.abs().min(style.size_y_nm.abs()) as f64 * 0.25,
    )?);
    let distance = checked_add(offset, auto_pen)?;
    if matches!(spin, 1 | 3) {
        Ok((checked_neg(distance)?, 0))
    } else {
        Ok((0, checked_neg(distance)?))
    }
}

fn global_decoration(
    form: &Sexp,
    shape: &str,
    at: At,
    spin: usize,
    style: &TextStyle,
    metrics: Option<&PlotterTextCacheSession<'_>>,
) -> Result<Option<SchematicPlotOperation>, Error> {
    let text = value_at(form, 1)
        .unwrap_or_default()
        .replace("{slash}", "/");
    let text_width = if text.is_empty() {
        0
    } else {
        measure_width(metrics, &text, style)?
    };
    let margin = round_i64(LABEL_SIZE_RATIO * style.size_y_nm as f64)?;
    let half = checked_add(style.size_y_nm / 2, margin)?;
    let line_width = mm_to_nm(DEFAULT_WIRE_WIDTH_MM)?;
    let x = checked_add(
        checked_add(text_width, checked_mul(2, margin)?)?,
        checked_add(line_width, 3)?,
    )?;
    let y = checked_add(half, checked_add(line_width, 3)?)?;
    let neg_x = checked_neg(x)?;
    let neg_y = checked_neg(y)?;
    let mut points = vec![
        [0, 0],
        [0, neg_y],
        [neg_x, neg_y],
        [neg_x, 0],
        [neg_x, y],
        [0, y],
    ];
    let mut x_offset = 0;
    match shape {
        "input" => {
            x_offset = checked_neg(half)?;
            points[0][0] = checked_add(points[0][0], half)?;
        }
        "output" => points[3][0] = checked_sub(points[3][0], half)?,
        "bidirectional" | "tri_state" => {
            x_offset = checked_neg(half)?;
            points[0][0] = checked_add(points[0][0], half)?;
            points[3][0] = checked_sub(points[3][0], half)?;
        }
        "passive" => {}
        _ => return Ok(None),
    }
    let mut rotated = Vec::with_capacity(7);
    for [px, py] in points {
        let (rx, ry) = rotate_spin(checked_add(px, x_offset)?, py, spin);
        rotated.push([checked_add(at.x, rx)?, checked_add(at.y, ry)?]);
    }
    rotated.push(rotated[0]);
    Ok(Some(
        PlotterOperation::PlotPoly(PlotterPoly {
            points: rotated,
            fill: PlotterFill::NoFill,
            width_nm: line_width,
            layer: None,
            stroke_color: Some(GLOBAL_COLOR.to_owned()),
            fill_color: None,
            line_style: None,
        })
        .into(),
    ))
}

fn template_decoration(
    shape: &str,
    at: At,
    spin: usize,
    size_y_nm: i64,
) -> Result<SchematicPlotOperation, Error> {
    styled_template_decoration(
        shape,
        at,
        spin,
        size_y_nm,
        mm_to_nm(DEFAULT_WIRE_WIDTH_MM)?,
        HIER_COLOR,
    )
}

pub(super) fn styled_template_decoration(
    shape: &str,
    at: At,
    spin: usize,
    size_y_nm: i64,
    pen_width_nm: i64,
    stroke_color: &str,
) -> Result<SchematicPlotOperation, Error> {
    const INPUT: [[(i64, i64); 6]; 4] = [
        [(0, 0), (-1, -1), (-2, -1), (-2, 1), (-1, 1), (0, 0)],
        [(0, 0), (1, -1), (1, -2), (-1, -2), (-1, -1), (0, 0)],
        [(0, 0), (1, 1), (2, 1), (2, -1), (1, -1), (0, 0)],
        [(0, 0), (1, 1), (1, 2), (-1, 2), (-1, 1), (0, 0)],
    ];
    const OUTPUT: [[(i64, i64); 6]; 4] = [
        [(-2, 0), (-1, 1), (0, 1), (0, -1), (-1, -1), (-2, 0)],
        [(0, -2), (1, -1), (1, 0), (-1, 0), (-1, -1), (0, -2)],
        [(2, 0), (1, -1), (0, -1), (0, 1), (1, 1), (2, 0)],
        [(0, 2), (1, 1), (1, 0), (-1, 0), (-1, 1), (0, 2)],
    ];
    const BIDI: [[(i64, i64); 5]; 4] = [
        [(0, 0), (-1, -1), (-2, 0), (-1, 1), (0, 0)],
        [(0, 0), (-1, -1), (0, -2), (1, -1), (0, 0)],
        [(0, 0), (1, -1), (2, 0), (1, 1), (0, 0)],
        [(0, 0), (-1, 1), (0, 2), (1, 1), (0, 0)],
    ];
    const PASSIVE: [[(i64, i64); 5]; 4] = [
        [(0, -1), (-2, -1), (-2, 1), (0, 1), (0, -1)],
        [(1, 0), (1, -2), (-1, -2), (-1, 0), (1, 0)],
        [(0, -1), (2, -1), (2, 1), (0, 1), (0, -1)],
        [(1, 0), (1, 2), (-1, 2), (-1, 0), (1, 0)],
    ];
    let half = size_y_nm / 2;
    let tuples: Vec<(i64, i64)> = match shape {
        "input" => INPUT[spin].to_vec(),
        "output" => OUTPUT[spin].to_vec(),
        "bidirectional" | "tri_state" => BIDI[spin].to_vec(),
        "passive" => PASSIVE[spin].to_vec(),
        _ => Vec::new(),
    };
    let points = tuples
        .into_iter()
        .map(|(x, y)| {
            Ok([
                checked_add(at.x, checked_mul(half, x)?)?,
                checked_add(at.y, checked_mul(half, y)?)?,
            ])
        })
        .collect::<Result<Vec<_>, Error>>()?;
    Ok(PlotterOperation::PlotPoly(PlotterPoly {
        points,
        fill: PlotterFill::NoFill,
        width_nm: pen_width_nm,
        layer: None,
        stroke_color: Some(stroke_color.to_owned()),
        fill_color: None,
        line_style: None,
    })
    .into())
}

fn directive_marker_ops(
    at: At,
    length: i64,
    shape: &str,
    pen: i64,
) -> Result<Vec<SchematicPlotOperation>, Error> {
    let mut symbol = DIRECTIVE_SYMBOL_SIZE_NM;
    if shape == "dot" {
        symbol = ki_round_i64(symbol as f64 * 0.7)?;
    }
    if shape == "rectangle" {
        symbol = ki_round_i64(symbol as f64 * 0.8)?;
    }
    let spin = match rounded_angle(at.angle) {
        0 => 2,
        90 => 1,
        180 => 0,
        270 => 3,
        _ => 2,
    };
    let point = |x, y| -> Result<[i64; 2], Error> {
        let (x, y) = rotate_spin(x, y, spin);
        Ok([checked_add(at.x, x)?, checked_add(at.y, y)?])
    };
    if matches!(shape, "round" | "dot") {
        let end = point(0, checked_sub(length, symbol)?)?;
        let center = point(0, length)?;
        return Ok(vec![
            SchematicPlotOperation::StyledThickSegment(SchematicStyledThickSegment {
                segment: ThickSegment {
                    start_x: at.x,
                    start_y: at.y,
                    end_x: end[0],
                    end_y: end[1],
                    width_nm: pen,
                    layer: None,
                    role: None,
                    layers: Vec::new(),
                    mask_margin_nm: None,
                    pad_size_x_nm: None,
                    pad_size_y_nm: None,
                },
                stroke_color: NETCLASS_COLOR.to_owned(),
            }),
            PlotterOperation::Circle(PlotterCircle {
                cx: center[0],
                cy: center[1],
                diameter_nm: checked_mul(2, symbol)?,
                fill: if shape == "dot" {
                    PlotterFill::FilledShape
                } else {
                    PlotterFill::NoFill
                },
                width_nm: if shape == "dot" { 0 } else { pen },
                layer: None,
                role: None,
                layers: Vec::new(),
                mask_margin_nm: None,
                pad_size_x_nm: None,
                pad_size_y_nm: None,
                stroke_color: Some(NETCLASS_COLOR.to_owned()),
                fill_color: (shape == "dot").then(|| NETCLASS_COLOR.to_owned()),
                line_style: None,
            })
            .into(),
        ]);
    }
    let relative = if shape == "diamond" {
        vec![
            (0, 0),
            (0, checked_sub(length, symbol)?),
            (checked_neg(checked_mul(2, symbol)?)?, length),
            (0, checked_add(length, symbol)?),
            (checked_mul(2, symbol)?, length),
            (0, checked_sub(length, symbol)?),
            (0, 0),
        ]
    } else {
        let twice_symbol = checked_mul(2, symbol)?;
        vec![
            (0, 0),
            (0, checked_sub(length, symbol)?),
            (checked_neg(twice_symbol)?, checked_sub(length, symbol)?),
            (checked_neg(twice_symbol)?, checked_add(length, symbol)?),
            (twice_symbol, checked_add(length, symbol)?),
            (twice_symbol, checked_sub(length, symbol)?),
            (0, checked_sub(length, symbol)?),
            (0, 0),
        ]
    };
    let points = relative
        .into_iter()
        .map(|(x, y)| point(x, y))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(vec![
        PlotterOperation::PlotPoly(PlotterPoly {
            points,
            fill: PlotterFill::NoFill,
            width_nm: pen,
            layer: None,
            stroke_color: Some(NETCLASS_COLOR.to_owned()),
            fill_color: None,
            line_style: None,
        })
        .into(),
    ])
}

fn property_text(
    property: &Sexp,
    default_width: i64,
) -> Result<Option<SchematicPlotOperation>, Error> {
    if named_yes(property, "hide") {
        return Ok(None);
    }
    let key = value_at(property, 1).unwrap_or_default();
    let mut value = value_at(property, 2).unwrap_or_default();
    if value == "~" {
        return Ok(None);
    }
    if named_yes(property, "show_name") {
        value = format!("{key}: {value}");
    }
    if value.is_empty() {
        return Ok(None);
    }
    let at = parse_at(property)?;
    let color = match key.as_str() {
        "Reference" => REFERENCE_COLOR,
        "Value" => VALUE_COLOR,
        _ => FIELDS_COLOR,
    };
    let mut style = text_style(property, color, Some(default_width))?;
    apply_center_defaults(property, &mut style);
    Ok(Some(schematic_text(
        at.x, at.y, value, at.angle, style, false,
    )))
}

pub(super) fn annotation_variables(
    context: &SchematicPlotContext,
    title_block: Option<&SchematicTitleBlock>,
    limits: SchematicPlotLimits,
) -> Result<BTreeMap<String, String>, Error> {
    let mut result = context.project_variables.values.clone();
    let expansion_limit = limits.max_text_bytes.max(limits.max_metadata_bytes);
    let mut expanded_bytes = 0usize;
    result.insert("#".to_owned(), context.sheet_index.to_string());
    result.insert("##".to_owned(), context.sheet_count.to_string());
    result.insert("VARIANT".to_owned(), String::new());
    if let Some(title) = title_block {
        for (name, value) in [
            ("TITLE", title.title.as_str()),
            ("ISSUE_DATE", title.date.as_str()),
            ("REVISION", title.revision.as_str()),
            ("COMPANY", title.company.as_str()),
        ] {
            let expanded = expand_once_bounded(
                value,
                &context.project_variables.values,
                expansion_limit
                    .checked_sub(expanded_bytes)
                    .ok_or_else(limit_error)?,
            )?;
            expanded_bytes = expanded_bytes
                .checked_add(expanded.len())
                .ok_or_else(limit_error)?;
            result.insert(name.to_owned(), expanded);
        }
        for (index, value) in &title.comments {
            let expanded = expand_once_bounded(
                value,
                &context.project_variables.values,
                expansion_limit
                    .checked_sub(expanded_bytes)
                    .ok_or_else(limit_error)?,
            )?;
            expanded_bytes = expanded_bytes
                .checked_add(expanded.len())
                .ok_or_else(limit_error)?;
            result.insert(format!("COMMENT{index}"), expanded);
        }
    }
    Ok(result)
}

fn expand_once_bounded(
    source: &str,
    variables: &BTreeMap<String, String>,
    maximum_bytes: usize,
) -> Result<String, Error> {
    let mut output_len = 0usize;
    visit_expansion(source, variables, |part| {
        output_len = output_len
            .checked_add(part.len())
            .filter(|length| *length <= maximum_bytes)
            .ok_or_else(limit_error)?;
        Ok(())
    })?;
    let mut output = String::with_capacity(output_len);
    visit_expansion(source, variables, |part| {
        output.push_str(part);
        Ok(())
    })?;
    Ok(output)
}

fn visit_expansion(
    source: &str,
    variables: &BTreeMap<String, String>,
    mut visit: impl FnMut(&str) -> Result<(), Error>,
) -> Result<(), Error> {
    let mut rest = source;
    while let Some(start) = rest.find("${") {
        visit(&rest[..start])?;
        let tail = &rest[start + 2..];
        let Some(end) = tail.find('}') else {
            visit(&rest[start..])?;
            return Ok(());
        };
        let name = &tail[..end];
        if name.is_empty() {
            // Annotation authority replaces an empty name with empty text.
        } else if let Some(value) = variables.get(name) {
            visit(value)?;
        } else {
            visit(&rest[start..start + 3 + end])?;
        }
        rest = &tail[end + 1..];
    }
    visit(rest)
}

fn text_box_margins(form: &Sexp, size_y: i64) -> Result<[i64; 4], Error> {
    if let Some(margins) = child(form, "margins") {
        return Ok([
            mm_to_nm(number_at(margins, 1)?)?,
            mm_to_nm(number_at(margins, 2)?)?,
            mm_to_nm(number_at(margins, 3)?)?,
            mm_to_nm(number_at(margins, 4)?)?,
        ]);
    }
    let authored_stroke = child(form, "stroke")
        .and_then(|stroke| child(stroke, "width"))
        .map(|value| number_at(value, 1))
        .transpose()?
        .filter(|value| *value > 0.0)
        .unwrap_or(0.0);
    // Authority combines the raw authored-mm half-stroke with the text
    // height before the single ties-even mm-to-nm conversion.
    let margin = mm_to_nm(authored_stroke / 2.0 + size_y as f64 / 1_000_000.0 * 0.75)?;
    Ok([margin; 4])
}

struct TextBoxWrapInput<'a, 'font> {
    angle: f64,
    size_x: i64,
    size_y: i64,
    margins: [i64; 4],
    style: &'a TextStyle,
    metrics: Option<&'a PlotterTextCacheSession<'font>>,
    maximum_lines: usize,
}

fn wrap_text_box(text: &str, input: TextBoxWrapInput<'_, '_>) -> Result<Vec<String>, Error> {
    let TextBoxWrapInput {
        angle,
        size_x,
        size_y,
        margins,
        style,
        metrics,
        maximum_lines,
    } = input;
    let box_width = if rounded_angle(angle) % 180 == 90 {
        size_y
            .abs()
            .saturating_sub(margins[1])
            .saturating_sub(margins[3])
    } else {
        size_x
            .abs()
            .saturating_sub(margins[0])
            .saturating_sub(margins[2])
    };
    let width = box_width.saturating_sub(style.pen_width_nm).max(0);
    let mut output = Vec::new();
    let text = text.trim_end_matches('\n');
    if text.is_empty() {
        return Ok(output);
    }
    let raw_line_count = text.split('\n').count();
    if raw_line_count > maximum_lines {
        return Err(limit_error());
    }
    for line in text.split('\n') {
        let line = line.trim_end();
        if line.is_empty() || width == 0 {
            push_bounded_line(&mut output, line, maximum_lines)?;
            continue;
        }
        let Some(metrics) = metrics else {
            return Err(model_error(
                "Schematic text-box wrapping requires explicit font resources",
            ));
        };
        if output.len() >= maximum_lines {
            return Err(limit_error());
        }
        // The linebreak engine's max_output_bytes limit bounds this
        // temporary String; max_text_box_lines bounds retained line Strings.
        let broken = metrics.linebreak(metric_layout(line, style), width as f64 / 1_000_000.0)?;
        for line in broken.split('\n') {
            push_bounded_line(&mut output, line, maximum_lines)?;
        }
    }
    Ok(output)
}

fn push_bounded_line(
    output: &mut Vec<String>,
    line: &str,
    maximum_lines: usize,
) -> Result<(), Error> {
    if output.len() >= maximum_lines {
        return Err(limit_error());
    }
    output.push(line.to_owned());
    Ok(())
}

fn text_box_positions(
    at: At,
    size_x: i64,
    size_y: i64,
    margins: [i64; 4],
    style: &TextStyle,
    count: usize,
) -> Result<Vec<(i64, i64)>, Error> {
    let x2 = checked_add(at.x, size_x)?;
    let y2 = checked_add(at.y, size_y)?;
    let (left, right) = (at.x.min(x2), at.x.max(x2));
    let (top, bottom) = (at.y.min(y2), at.y.max(y2));
    let vertical = rounded_angle(at.angle) % 180 == 90;
    let (draw_x, draw_y) = if vertical {
        let y = match style.h_align {
            PlotterTextHAlign::Center => checked_midpoint(top, bottom)?,
            PlotterTextHAlign::Right => checked_add(top, margins[1])?,
            PlotterTextHAlign::Left => checked_sub(bottom, margins[3])?,
        };
        let x = match style.v_align {
            PlotterTextVAlign::Center => checked_midpoint(left, right)?,
            PlotterTextVAlign::Bottom => checked_sub(right, margins[2])?,
            PlotterTextVAlign::Top => checked_add(left, margins[0])?,
        };
        (x, y)
    } else {
        let x = match style.h_align {
            PlotterTextHAlign::Center => checked_midpoint(left, right)?,
            PlotterTextHAlign::Right => checked_sub(right, margins[2])?,
            PlotterTextHAlign::Left => checked_add(left, margins[0])?,
        };
        let y = match style.v_align {
            PlotterTextVAlign::Center => checked_midpoint(top, bottom)?,
            PlotterTextVAlign::Bottom => checked_sub(bottom, margins[3])?,
            PlotterTextVAlign::Top => checked_add(top, margins[1])?,
        };
        (x, y)
    };
    let step = round_i64(style.size_y_nm as f64 * TEXTBOX_INTERLINE_FACTOR)?;
    let mut pos_x = draw_x;
    let mut pos_y = draw_y;
    if count > 1 {
        let line_gaps = i64::try_from(count - 1).map_err(|_| limit_error())?;
        let distance = checked_mul(line_gaps, step)?;
        if style.v_align == PlotterTextVAlign::Center {
            pos_y = checked_sub(pos_y, distance.div_euclid(2))?;
        }
        if style.v_align == PlotterTextVAlign::Bottom {
            pos_y = checked_sub(pos_y, distance)?;
        }
    }
    let (rx, ry) = rotate(
        checked_sub(pos_x, draw_x)? as f64,
        checked_sub(pos_y, draw_y)? as f64,
        -at.angle,
    );
    pos_x = checked_add(draw_x, round_i64(rx)?)?;
    pos_y = checked_add(draw_y, round_i64(ry)?)?;
    let (sx, sy) = rotate(0.0, step as f64, -at.angle);
    let sx = round_i64(sx)?;
    let sy = round_i64(sy)?;
    (0..count)
        .map(|index| {
            let result = (pos_x, pos_y);
            if index + 1 < count {
                pos_x = checked_add(pos_x, sx)?;
                pos_y = checked_add(pos_y, sy)?;
            }
            Ok(result)
        })
        .collect()
}

fn outline_adjust(
    text: &str,
    style: &TextStyle,
    metrics: Option<&PlotterTextCacheSession<'_>>,
) -> Result<i64, Error> {
    if text.split('\n').next().unwrap_or_default().is_empty() {
        let size_y_iu = ki_round_i64(style.size_y_nm as f64 / 100.0)?;
        return checked_mul(ki_round_i64(-(size_y_iu as f64) * 0.4)?, 100);
    }
    let Some(metrics) = metrics else {
        return Ok(0);
    };
    let measured = metrics.measure(metric_layout("Ag", style))?;
    let height_iu = ki_round_i64(measured.line_height * 10_000.0)?;
    let size_y_iu = ki_round_i64(style.size_y_nm as f64 / 100.0)?;
    checked_mul(
        ki_round_i64(checked_sub(height_iu, size_y_iu)? as f64 * 0.4)?,
        100,
    )
}

fn measure_width(
    metrics: Option<&PlotterTextCacheSession<'_>>,
    text: &str,
    style: &TextStyle,
) -> Result<i64, Error> {
    let metrics = metrics
        .ok_or_else(|| model_error("Schematic text metrics require explicit font resources"))?;
    round_to_100(metrics.measure(metric_layout(text, style))?.width * 1_000_000.0)
}

fn metric_layout<'a>(text: &'a str, style: &'a TextStyle) -> PlotterTextLayout<'a> {
    PlotterTextLayout {
        text,
        face: &style.font_face,
        bold: style.bold,
        italic: style.italic,
        size_x: style.size_x_nm as f64 / 1_000_000.0,
        size_y: style.size_y_nm as f64 / 1_000_000.0,
        position_x: 0.0,
        position_y: 0.0,
        angle_degrees: 0.0,
        mirrored: false,
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Bottom,
        line_spacing: 1.0,
        stroke_width: style.pen_width_nm as f64 / 1_000_000.0,
    }
}

fn plot_fill(value: &str) -> PlotterFill {
    match value {
        "outline" => PlotterFill::FilledShape,
        "background" => PlotterFill::FilledWithBackgroundBodyColor,
        "color" => PlotterFill::FilledWithColor,
        "hatch" => PlotterFill::Hatch,
        "reverse_hatch" => PlotterFill::ReverseHatch,
        "cross_hatch" => PlotterFill::CrossHatch,
        _ => PlotterFill::NoFill,
    }
}

fn charge_text_operations(
    budget: &mut PlotBudget,
    operations: &[SchematicPlotOperation],
) -> Result<(), Error> {
    for operation in operations {
        if let SchematicPlotOperation::Text(operation) = operation {
            budget.charge_text(operation.text.text.len())?;
            budget.charge_metadata(
                operation
                    .text
                    .color
                    .len()
                    .saturating_add(operation.text.font_face.len())
                    .saturating_add(operation.hyperlink_href.as_deref().map_or(0, str::len)),
            )?;
        }
    }
    Ok(())
}

fn operation_points(operation: &SchematicPlotOperation) -> usize {
    match operation {
        SchematicPlotOperation::Plotter(PlotterOperation::PlotPoly(poly)) => poly.points.len(),
        SchematicPlotOperation::StyledThickSegment(_) => 2,
        _ => 0,
    }
}

fn valid_label_shape(shape: &str) -> bool {
    matches!(
        shape,
        "input"
            | "output"
            | "bidirectional"
            | "tri_state"
            | "passive"
            | "dot"
            | "round"
            | "diamond"
            | "rectangle"
    )
}
fn decoration_shape(shape: &str) -> bool {
    matches!(
        shape,
        "input" | "output" | "bidirectional" | "tri_state" | "passive"
    )
}
pub(super) fn apply_center_defaults(form: &Sexp, style: &mut TextStyle) {
    let tokens = child(form, "effects")
        .and_then(|effects| child(effects, "justify"))
        .and_then(list)
        .map(|values| values.iter().skip(1).filter_map(text).collect::<Vec<_>>())
        .unwrap_or_default();
    if !tokens
        .iter()
        .any(|value| matches!(*value, "left" | "center" | "right"))
    {
        style.h_align = PlotterTextHAlign::Center;
    }
    if !tokens
        .iter()
        .any(|value| matches!(*value, "top" | "bottom"))
    {
        style.v_align = PlotterTextVAlign::Center;
    }
}
pub(super) fn looks_like_bus_label(source: &str) -> bool {
    let value = source.replace("{slash}", "");
    value.char_indices().any(|(index, ch)| {
        ch == '{'
            && (index == 0 || !value[..index].ends_with('~'))
            && value[index + ch.len_utf8()..].contains('}')
    })
}
fn children<'a>(form: &'a Sexp, head: &str) -> impl Iterator<Item = &'a Sexp> {
    list(form).into_iter().flatten().filter(move |value| {
        list(value).and_then(|values| values.first()).and_then(text) == Some(head)
    })
}
fn named_yes(form: &Sexp, head: &str) -> bool {
    child(form, head)
        .and_then(|value| scalar_at(value, 1))
        .as_deref()
        == Some("yes")
        || list(form)
            .into_iter()
            .flatten()
            .any(|value| text(value) == Some(head))
}
fn rounded_angle(value: f64) -> i64 {
    round_ties_even_i64(value).rem_euclid(360)
}
fn angle_spin(angle: f64) -> usize {
    match rounded_angle(angle) {
        0 => 2,
        90 => 1,
        180 => 0,
        270 => 3,
        _ => 2,
    }
}
fn rotate_spin(x: i64, y: i64, spin: usize) -> (i64, i64) {
    match spin {
        0 => (x, y),
        1 => (-y, x),
        2 => (-x, -y),
        3 => (y, -x),
        _ => (x, y),
    }
}
fn rotate(x: f64, y: f64, angle: f64) -> (f64, f64) {
    match round_ties_even_i64(angle).rem_euclid(360) {
        0 => (x, y),
        90 => (-y, x),
        180 => (-x, -y),
        270 => (y, -x),
        _ => {
            let r = angle.to_radians();
            (x * r.cos() - y * r.sin(), x * r.sin() + y * r.cos())
        }
    }
}
fn checked_add(a: i64, b: i64) -> Result<i64, Error> {
    let value = a.checked_add(b).ok_or_else(limit_error)?;
    ensure_javascript_safe_integer(value)?;
    Ok(value)
}
fn checked_sub(a: i64, b: i64) -> Result<i64, Error> {
    let value = a.checked_sub(b).ok_or_else(limit_error)?;
    ensure_javascript_safe_integer(value)?;
    Ok(value)
}
fn checked_mul(a: i64, b: i64) -> Result<i64, Error> {
    let value = a.checked_mul(b).ok_or_else(limit_error)?;
    ensure_javascript_safe_integer(value)?;
    Ok(value)
}
fn checked_neg(value: i64) -> Result<i64, Error> {
    let value = value.checked_neg().ok_or_else(limit_error)?;
    ensure_javascript_safe_integer(value)?;
    Ok(value)
}
fn checked_midpoint(a: i64, b: i64) -> Result<i64, Error> {
    Ok(a.checked_add(b).ok_or_else(limit_error)?.div_euclid(2))
}
fn round_i64(value: f64) -> Result<i64, Error> {
    if !value.is_finite()
        || value < JAVASCRIPT_SAFE_INTEGER_MIN as f64
        || value > JAVASCRIPT_SAFE_INTEGER_MAX as f64
    {
        Err(model_error("Derived schematic coordinate is not finite"))
    } else {
        Ok(round_ties_even_i64(value))
    }
}
pub(super) fn ki_round_i64(value: f64) -> Result<i64, Error> {
    if !value.is_finite()
        || value < JAVASCRIPT_SAFE_INTEGER_MIN as f64
        || value > JAVASCRIPT_SAFE_INTEGER_MAX as f64
    {
        return Err(model_error(
            "Derived schematic coordinate is outside the safe range",
        ));
    }
    Ok(if value >= 0.0 {
        value.floor() as i64 + i64::from(value.fract() >= 0.5)
    } else {
        value.ceil() as i64 - i64::from(value.fract() <= -0.5)
    })
}
fn round_ties_even_i64(value: f64) -> i64 {
    value.round_ties_even() as i64
}
fn round_to_100(value: f64) -> Result<i64, Error> {
    checked_mul(ki_round_i64(value / 100.0)?, 100)
}
