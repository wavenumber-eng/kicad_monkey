//! Bounded terminal hierarchical-sheet rendering from exact source spans.

use super::annotation_render::{
    apply_center_defaults, ki_round_i64, looks_like_bus_label, parse_at, schematic_text,
    styled_template_decoration, text_style,
};
use super::symbol_render::{
    Bbox, dim_operations, dnp_marker_operations_for_bboxes, operations_bbox, union_bbox,
};
use super::*;
use crate::plotter_text_cache::PlotterTextCacheSession;
use crate::plotter_types::{PlotterOperation, PlotterRect, PlotterTextVAlign};

const DEFAULT_SHEET_WIDTH_MM: f64 = 50.8;
const DEFAULT_SHEET_HEIGHT_MM: f64 = 35.56;
const SHEET_COLOR: &str = "#840000FF";
const SHEET_LABEL_COLOR: &str = "#006464FF";
const SHEET_NAME_COLOR: &str = "#006464FF";
const SHEET_FILE_COLOR: &str = "#725600FF";
const SHEET_FIELD_COLOR: &str = "#840084FF";
const BUS_COLOR: &str = "#000084FF";
const DNP_COLOR: &str = "#DC090DD9";

struct SheetGeometry {
    at_x_nm: i64,
    at_y_nm: i64,
    size_x_nm: i64,
    size_y_nm: i64,
    end_x_nm: i64,
    end_y_nm: i64,
}

#[derive(Default)]
struct RetainedBudget {
    text: usize,
    metadata: usize,
    operations: usize,
    points: usize,
}

impl RetainedBudget {
    fn text(&mut self, bytes: usize, maximum: usize) -> Result<(), Error> {
        self.text = checked_count(self.text, bytes, maximum)?;
        Ok(())
    }

    fn metadata(&mut self, bytes: usize, maximum: usize) -> Result<(), Error> {
        self.metadata = checked_count(self.metadata, bytes, maximum)?;
        Ok(())
    }

    fn operation(
        &mut self,
        operation: &SchematicPlotOperation,
        maximum_operations: usize,
        maximum_points: usize,
    ) -> Result<(), Error> {
        self.operations = checked_count(self.operations, 1, maximum_operations)?;
        self.points = checked_count(self.points, operation_points(operation), maximum_points)?;
        Ok(())
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "terminal sheets share the document parse, drawing, metric, and retained budgets"
)]
pub(super) fn append_sheet_records(
    source: &str,
    spans: &[FormSpan],
    drawing: SchematicDrawingSettings,
    metrics: Option<&PlotterTextCacheSession<'_>>,
    limits: SchematicPlotLimits,
    budget: &mut PlotBudget,
    records: &mut Vec<SchematicPlotRecord>,
) -> Result<(), Error> {
    let mut property_count = 0usize;
    let mut pin_count = 0usize;
    for span in spans {
        let form = parse_span(source, span, limits)?;
        let mut properties = Vec::new();
        for property in children(&form, "property") {
            property_count = checked_count(property_count, 1, limits.max_sheet_properties)?;
            properties.push(property);
        }
        let mut pins = Vec::new();
        for pin in children(&form, "pin") {
            pin_count = checked_count(pin_count, 1, limits.max_sheet_pins)?;
            pins.push(pin);
        }
        budget.charge_input_points(
            2usize
                .checked_add(properties.len())
                .and_then(|value| value.checked_add(pins.len()))
                .ok_or_else(limit_error)?,
        )?;
        let record = render_sheet(&form, &properties, &pins, drawing, metrics, budget)?;
        records.push(SchematicPlotRecord::Sheet(record));
    }
    Ok(())
}

fn render_sheet(
    form: &Sexp,
    properties: &[&Sexp],
    pins: &[&Sexp],
    drawing: SchematicDrawingSettings,
    metrics: Option<&PlotterTextCacheSession<'_>>,
    budget: &mut PlotBudget,
) -> Result<SchematicSheetRecord, Error> {
    let geometry = sheet_geometry(form)?;
    let uuid = child_string(form, "uuid").unwrap_or_default();
    let sheet_name = sheet_property_value(properties, &["Sheetname", "Sheet name"]);
    let sheet_file = sheet_property_value(properties, &["Sheetfile", "Sheet file"]);
    let dnp = yes_child(form, "dnp", false);
    let remaining_text = budget.remaining_text_bytes();
    let remaining_metadata = budget.remaining_metadata_bytes();
    let remaining_operations = budget.remaining_operations();
    let remaining_points = budget.remaining_points();
    let mut retained = RetainedBudget::default();
    retained.metadata(
        uuid.len()
            .checked_add(sheet_name.len().checked_mul(2).ok_or_else(limit_error)?)
            .and_then(|value| value.checked_add(sheet_file.len()))
            .ok_or_else(limit_error)?,
        remaining_metadata,
    )?;

    let outline_stroke = resolve_stroke(form, DEFAULT_WIRE_WIDTH_MM, SHEET_COLOR)?;
    let outline = SchematicPlotOperation::Plotter(PlotterOperation::Rect(PlotterRect {
        x1: geometry.at_x_nm,
        y1: geometry.at_y_nm,
        x2: geometry.end_x_nm,
        y2: geometry.end_y_nm,
        fill: PlotterFill::NoFill,
        width_nm: outline_stroke.width_nm,
        corner_radius_nm: 0,
        layer: None,
        stroke_color: Some(outline_stroke.color),
        fill_color: None,
        line_style: Some(outline_stroke.style),
    }));
    let background_color = child(form, "fill")
        .and_then(|fill| child(fill, "color"))
        .map(parse_color)
        .transpose()?
        .flatten();
    let mut operations = Vec::new();
    let background_present = if let Some(color) = background_color {
        let background = SchematicPlotOperation::Plotter(PlotterOperation::Rect(PlotterRect {
            x1: geometry.at_x_nm,
            y1: geometry.at_y_nm,
            x2: geometry.end_x_nm,
            y2: geometry.end_y_nm,
            fill: PlotterFill::FilledShape,
            width_nm: 0,
            corner_radius_nm: 0,
            layer: None,
            stroke_color: Some(color.clone()),
            fill_color: Some(color),
            line_style: None,
        }));
        push_operation(
            &mut operations,
            background,
            &mut retained,
            remaining_operations,
            remaining_points,
            remaining_metadata,
        )?;
        true
    } else {
        false
    };
    push_operation(
        &mut operations,
        outline.clone(),
        &mut retained,
        remaining_operations,
        remaining_points,
        remaining_metadata,
    )?;
    if !background_present {
        push_operation(
            &mut operations,
            outline,
            &mut retained,
            remaining_operations,
            remaining_points,
            remaining_metadata,
        )?;
    }
    let sheet_body_operations = operations.len();

    for pin in pins {
        append_pin(
            pin,
            &uuid,
            &sheet_name,
            &sheet_file,
            drawing,
            &mut operations,
            &mut retained,
            remaining_text,
            remaining_metadata,
            remaining_operations,
            remaining_points,
        )?;
    }
    for property in properties {
        append_property(
            property,
            &mut operations,
            &mut retained,
            remaining_text,
            remaining_metadata,
            remaining_operations,
            remaining_points,
        )?;
    }

    if dnp {
        let body_bbox = Bbox::new(
            geometry.at_x_nm,
            geometry.at_y_nm,
            geometry.end_x_nm,
            geometry.end_y_nm,
        );
        let full_bbox = union_bbox(
            Some(body_bbox),
            operations_bbox(&operations[sheet_body_operations..], metrics, true)?,
        )
        .unwrap_or(body_bbox);
        if background_present {
            dim_operations(&mut operations[1..]);
        } else {
            dim_operations(&mut operations);
        }
        for marker in dnp_marker_operations_for_bboxes(body_bbox, full_bbox)? {
            push_operation(
                &mut operations,
                marker,
                &mut retained,
                remaining_operations,
                remaining_points,
                remaining_metadata,
            )?;
        }
    }

    budget.charge_text(retained.text)?;
    budget.charge_metadata(retained.metadata)?;
    budget.charge(1, retained.operations, retained.points)?;
    Ok(SchematicSheetRecord {
        uuid,
        sheet_name,
        sheet_file,
        at_x_nm: geometry.at_x_nm,
        at_y_nm: geometry.at_y_nm,
        size_x_nm: geometry.size_x_nm,
        size_y_nm: geometry.size_y_nm,
        dnp,
        operations,
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "pin construction is preflighted against one retained sheet budget"
)]
fn append_pin(
    pin: &Sexp,
    sheet_uuid: &str,
    sheet_name: &str,
    sheet_file: &str,
    drawing: SchematicDrawingSettings,
    operations: &mut Vec<SchematicPlotOperation>,
    retained: &mut RetainedBudget,
    maximum_text: usize,
    maximum_metadata: usize,
    maximum_operations: usize,
    maximum_points: usize,
) -> Result<(), Error> {
    let name = value_at(pin, 1).unwrap_or_default();
    let shape = normalized_shape(value_at(pin, 2).as_deref());
    let source_uuid = child_string(pin, "uuid").unwrap_or_default();
    let group_id = sheet_pin_group_id(sheet_uuid, &name, &source_uuid, maximum_metadata)?;
    let has_group = !group_id.is_empty();
    let at = parse_at(pin)?;
    let role_color = if looks_like_bus_label(&name) {
        BUS_COLOR
    } else {
        SHEET_LABEL_COLOR
    };
    let mut style = text_style(pin, role_color, Some(drawing.default_line_width_nm))?;
    style.v_align = PlotterTextVAlign::Center;
    let display = name.replace("{slash}", "/");
    retained.text(display.len(), maximum_text)?;
    let distance = checked_add(
        ki_round_i64(drawing.text_offset_ratio * style.size_y_nm as f64)?,
        style.size_x_nm,
    )?;
    let spin = sheet_pin_spin(at.angle)?;
    let (x, y) = match spin {
        0 => (checked_sub(at.x, distance)?, at.y),
        1 => (at.x, checked_sub(at.y, distance)?),
        3 => (at.x, checked_add(at.y, distance)?),
        _ => (checked_add(at.x, distance)?, at.y),
    };
    let text_op = schematic_text(
        x,
        y,
        display,
        if matches!(spin, 1 | 3) { 90.0 } else { 0.0 },
        style.clone(),
        false,
    );
    let decoration_shape = match shape {
        "input" => Some("output"),
        "output" => Some("input"),
        "bidirectional" | "tri_state" | "passive" => Some(shape),
        _ => None,
    };
    let decoration = decoration_shape
        .map(|shape| {
            styled_template_decoration(
                shape,
                at,
                spin,
                style.size_y_nm,
                style.pen_width_nm,
                SHEET_LABEL_COLOR,
            )
        })
        .transpose()?;
    let required = 1usize
        .checked_add(usize::from(decoration.is_some()))
        .and_then(|value| value.checked_add(usize::from(has_group) * 2))
        .ok_or_else(limit_error)?;
    checked_count(retained.operations, required, maximum_operations)?;
    if has_group {
        let block = SchematicSheetPinBlock {
            label: group_id.clone(),
            data_uuid: group_id.clone(),
            object_id: group_id,
            extra_attrs: SchematicSheetPinAttrs {
                primitive: "sheet-entry".to_owned(),
                object_type: "sheet-pin".to_owned(),
                sheet_uuid: sheet_uuid.to_owned(),
                sheet_name: sheet_name.to_owned(),
                sheet_file: sheet_file.to_owned(),
                pin: name.clone(),
                pin_name: name.clone(),
                shape: shape.to_owned(),
            },
        };
        push_operation(
            operations,
            SchematicPlotOperation::StartSheetPinBlock(block),
            retained,
            maximum_operations,
            maximum_points,
            maximum_metadata,
        )?;
    }
    push_operation(
        operations,
        text_op,
        retained,
        maximum_operations,
        maximum_points,
        maximum_metadata,
    )?;
    if let Some(decoration) = decoration {
        push_operation(
            operations,
            decoration,
            retained,
            maximum_operations,
            maximum_points,
            maximum_metadata,
        )?;
    }
    if has_group {
        push_operation(
            operations,
            SchematicPlotOperation::EndBlock,
            retained,
            maximum_operations,
            maximum_points,
            maximum_metadata,
        )?;
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "property construction is preflighted against one retained sheet budget"
)]
fn append_property(
    property: &Sexp,
    operations: &mut Vec<SchematicPlotOperation>,
    retained: &mut RetainedBudget,
    maximum_text: usize,
    maximum_metadata: usize,
    maximum_operations: usize,
    maximum_points: usize,
) -> Result<(), Error> {
    if named_flag(property, "hide")
        || child(property, "effects").is_some_and(|effects| named_flag(effects, "hide"))
    {
        return Ok(());
    }
    let key = value_at(property, 1).unwrap_or_default();
    let value = value_at(property, 2).unwrap_or_default();
    let show_name = named_flag(property, "show_name");
    if value.is_empty() && !show_name {
        return Ok(());
    }
    let prefix = if show_name {
        Some((key.as_str(), ": "))
    } else if matches!(key.as_str(), "Sheetfile" | "Sheet file") {
        Some(("File", ": "))
    } else {
        None
    };
    let output_len = prefix.map_or(value.len(), |(name, delimiter)| {
        name.len()
            .saturating_add(delimiter.len())
            .saturating_add(value.len())
    });
    retained.text(output_len, maximum_text)?;
    let body = if let Some((name, delimiter)) = prefix {
        let mut body = String::with_capacity(output_len);
        body.push_str(name);
        body.push_str(delimiter);
        body.push_str(&value);
        body
    } else {
        value
    };
    if body.is_empty() {
        return Ok(());
    }
    let at = parse_at(property)?;
    let role_color = match key.as_str() {
        "Sheetname" | "Sheet name" => SHEET_NAME_COLOR,
        "Sheetfile" | "Sheet file" => SHEET_FILE_COLOR,
        _ => SHEET_FIELD_COLOR,
    };
    let mut style = text_style(property, role_color, None)?;
    apply_center_defaults(property, &mut style);
    push_operation(
        operations,
        schematic_text(at.x, at.y, body, at.angle, style, false),
        retained,
        maximum_operations,
        maximum_points,
        maximum_metadata,
    )
}

fn sheet_geometry(form: &Sexp) -> Result<SheetGeometry, Error> {
    let (at_x_mm, at_y_mm) = pair_or_default(child(form, "at"), 0.0, 0.0)?;
    let (size_x_mm, size_y_mm) = pair_or_default(
        child(form, "size"),
        DEFAULT_SHEET_WIDTH_MM,
        DEFAULT_SHEET_HEIGHT_MM,
    )?;
    if size_x_mm <= 0.0 || size_y_mm <= 0.0 {
        return Err(model_error("Schematic sheet size must be positive"));
    }
    let end_x_mm = at_x_mm + size_x_mm;
    let end_y_mm = at_y_mm + size_y_mm;
    if !end_x_mm.is_finite() || !end_y_mm.is_finite() {
        return Err(model_error("Derived schematic sheet corner is not finite"));
    }
    let at_x_nm = mm_to_nm(at_x_mm)?;
    let at_y_nm = mm_to_nm(at_y_mm)?;
    let size_x_nm = mm_to_nm(size_x_mm)?;
    let size_y_nm = mm_to_nm(size_y_mm)?;
    if size_x_nm <= 0 || size_y_nm <= 0 {
        return Err(model_error(
            "Schematic sheet size must remain positive after nm rounding",
        ));
    }
    let coherent_end_x_nm = checked_add(at_x_nm, size_x_nm)?;
    let coherent_end_y_nm = checked_add(at_y_nm, size_y_nm)?;
    if mm_to_nm(end_x_mm)? != coherent_end_x_nm || mm_to_nm(end_y_mm)? != coherent_end_y_nm {
        return Err(model_error(
            "Schematic sheet corner is incoherent after independent nm rounding",
        ));
    }
    Ok(SheetGeometry {
        at_x_nm,
        at_y_nm,
        size_x_nm,
        size_y_nm,
        end_x_nm: coherent_end_x_nm,
        end_y_nm: coherent_end_y_nm,
    })
}

fn pair_or_default(
    form: Option<&Sexp>,
    default_x: f64,
    default_y: f64,
) -> Result<(f64, f64), Error> {
    let Some(form) = form else {
        return Ok((default_x, default_y));
    };
    Ok((number_at(form, 1)?, number_at(form, 2)?))
}

fn sheet_property_value(properties: &[&Sexp], names: &[&str]) -> String {
    properties
        .iter()
        .find(|property| value_at(property, 1).is_some_and(|key| names.contains(&key.as_str())))
        .and_then(|property| value_at(property, 2))
        .unwrap_or_default()
}

fn normalized_shape(value: Option<&str>) -> &'static str {
    match value {
        Some("output") => "output",
        Some("bidirectional") => "bidirectional",
        Some("tri_state") => "tri_state",
        Some("passive") => "passive",
        Some("dot") => "dot",
        Some("round") => "round",
        Some("diamond") => "diamond",
        Some("rectangle") => "rectangle",
        _ => "input",
    }
}

fn sheet_pin_spin(angle: f64) -> Result<usize, Error> {
    const MAX_EXACT_ANGLE: f64 = 9_007_199_254_740_991.0;
    if !angle.is_finite() || angle.abs() > MAX_EXACT_ANGLE {
        return Err(model_error(
            "Schematic sheet pin angle exceeds the exact conversion range",
        ));
    }
    Ok(match (angle.round_ties_even() as i64).rem_euclid(360) {
        0 => 0,
        90 => 3,
        180 => 2,
        270 => 1,
        _ => 2,
    })
}

fn sheet_pin_group_id(
    sheet_uuid: &str,
    pin_name: &str,
    source_uuid: &str,
    maximum_metadata: usize,
) -> Result<String, Error> {
    if !source_uuid.is_empty() {
        if source_uuid.len() > maximum_metadata {
            return Err(limit_error());
        }
        return Ok(source_uuid.to_owned());
    }
    if sheet_uuid.is_empty() || pin_name.is_empty() {
        return Ok(String::new());
    }
    let maximum = sheet_uuid
        .len()
        .checked_add("__sheet_pin__".len())
        .and_then(|value| value.checked_add(pin_name.len()))
        .ok_or_else(limit_error)?;
    if maximum > maximum_metadata {
        return Err(limit_error());
    }
    let mut token = String::with_capacity(pin_name.len());
    let mut replacing = false;
    for character in pin_name.trim().chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '_' | '.' | ':' | '-') {
            token.push(character);
            replacing = false;
        } else if !replacing {
            token.push('_');
            replacing = true;
        }
    }
    let token = token.trim_matches('_');
    let token = if token.is_empty() { "pin" } else { token };
    Ok(format!("{sheet_uuid}__sheet_pin__{token}"))
}

fn push_operation(
    operations: &mut Vec<SchematicPlotOperation>,
    operation: SchematicPlotOperation,
    retained: &mut RetainedBudget,
    maximum_operations: usize,
    maximum_points: usize,
    maximum_metadata: usize,
) -> Result<(), Error> {
    retained.operation(&operation, maximum_operations, maximum_points)?;
    retained.metadata(operation_metadata(&operation)?, maximum_metadata)?;
    operations.push(operation);
    Ok(())
}

fn operation_points(operation: &SchematicPlotOperation) -> usize {
    match operation {
        SchematicPlotOperation::Plotter(PlotterOperation::Rect(_))
        | SchematicPlotOperation::StyledThickSegment(_) => 2,
        SchematicPlotOperation::Plotter(PlotterOperation::PlotPoly(value)) => value.points.len(),
        SchematicPlotOperation::Text(_) => 1,
        _ => 0,
    }
}

fn operation_metadata(operation: &SchematicPlotOperation) -> Result<usize, Error> {
    let values: Vec<usize> = match operation {
        SchematicPlotOperation::Plotter(PlotterOperation::Rect(value)) => vec![
            value.stroke_color.as_deref().map_or(0, str::len),
            value.fill_color.as_deref().map_or(0, str::len),
        ],
        SchematicPlotOperation::Plotter(PlotterOperation::PlotPoly(value)) => vec![
            value.stroke_color.as_deref().map_or(0, str::len),
            value.fill_color.as_deref().map_or(0, str::len),
        ],
        SchematicPlotOperation::Text(value) => vec![
            value.text.color.len(),
            value.text.font_face.len(),
            value.hyperlink_href.as_deref().map_or(0, str::len),
        ],
        SchematicPlotOperation::StyledThickSegment(_) => vec![DNP_COLOR.len()],
        SchematicPlotOperation::StartSheetPinBlock(value) => vec![
            value.label.len(),
            value.data_uuid.len(),
            "sheet_pin".len(),
            value.object_id.len(),
            value.extra_attrs.primitive.len(),
            value.extra_attrs.object_type.len(),
            value.extra_attrs.sheet_uuid.len(),
            value.extra_attrs.sheet_name.len(),
            value.extra_attrs.sheet_file.len(),
            value.extra_attrs.pin.len(),
            value.extra_attrs.pin_name.len(),
            value.extra_attrs.shape.len(),
        ],
        _ => Vec::new(),
    };
    values.into_iter().try_fold(0usize, |total, value| {
        total.checked_add(value).ok_or_else(limit_error)
    })
}

fn named_flag(form: &Sexp, name: &str) -> bool {
    list(form).into_iter().flatten().skip(1).any(|value| {
        text(value) == Some(name)
            || list(value)
                .and_then(|values| values.first())
                .and_then(text)
                .is_some_and(|head| {
                    head == name
                        && (list(value).is_some_and(|values| values.len() == 1)
                            || scalar_at(value, 1).as_deref() == Some("yes"))
                })
    })
}

fn yes_child(form: &Sexp, name: &str, default: bool) -> bool {
    child(form, name)
        .and_then(|value| scalar_at(value, 1))
        .map_or(default, |value| value == "yes")
}

fn children<'a>(form: &'a Sexp, head: &str) -> impl Iterator<Item = &'a Sexp> {
    list(form).into_iter().flatten().filter(move |value| {
        list(value).and_then(|values| values.first()).and_then(text) == Some(head)
    })
}

fn checked_count(current: usize, additional: usize, maximum: usize) -> Result<usize, Error> {
    current
        .checked_add(additional)
        .filter(|value| *value <= maximum)
        .ok_or_else(limit_error)
}

fn checked_add(left: i64, right: i64) -> Result<i64, Error> {
    let value = left.checked_add(right).ok_or_else(limit_error)?;
    ensure_javascript_safe_integer(value)?;
    Ok(value)
}

fn checked_sub(left: i64, right: i64) -> Result<i64, Error> {
    let value = left.checked_sub(right).ok_or_else(limit_error)?;
    ensure_javascript_safe_integer(value)?;
    Ok(value)
}
