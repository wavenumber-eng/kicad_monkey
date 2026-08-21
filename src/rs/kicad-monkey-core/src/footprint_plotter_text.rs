//! Standalone-footprint property, text, and text-box Plotter-IR emission.

use crate::board_plotter_ir::text_wrap::wrap_text_box;
use crate::footprint_text::{FootprintGraphicalProperty, FootprintText, FootprintTextBox};
use crate::plotter_ir::{FootprintPlotLimits, mm_to_nm};
use crate::plotter_types::{
    PlotterFill, PlotterOperation, PlotterRect, PlotterText, PlotterTextHAlign, PlotterTextVAlign,
};
use crate::sexpr::Error;
use crate::{BoardTextVariables, FootprintView, KiCadColor, KiCadTextEffects};

const MIN_PLOT_PEN_WIDTH_NM: i64 = 84_700;
const DEFAULT_TEXT_BOX_BORDER_WIDTH_NM: i64 = 200_000;
const DEFAULT_TEXT_SIZE_NM: i64 = 1_270_000;

#[derive(Clone, Copy)]
pub(crate) struct TextOperationInput<'a> {
    pub x: f64,
    pub y: f64,
    pub angle: f64,
    pub layer: &'a str,
    pub effects: &'a KiCadTextEffects,
    pub default_h: PlotterTextHAlign,
    pub default_v: PlotterTextVAlign,
    pub multiline: bool,
}

pub(crate) fn footprint_text_operations(
    view: &FootprintView<'_>,
    limits: FootprintPlotLimits,
) -> Result<Vec<PlotterOperation>, Error> {
    let properties = view.graphical_properties().collect::<Result<Vec<_>, _>>()?;
    let variables = BoardTextVariables::from_entries(
        properties
            .iter()
            .map(|property| (&property.name, &property.value)),
    );
    let mut operations = Vec::new();
    let mut retained_text_bytes = 0usize;

    append_properties(
        &mut operations,
        &mut retained_text_bytes,
        &properties,
        limits,
    )?;
    for text in view.texts() {
        let text = text?;
        if let Some(operation) = text_operation(
            &text,
            &variables,
            remaining_text_bytes(retained_text_bytes, limits)?,
        )? {
            append_text_operation(&mut operations, &mut retained_text_bytes, operation, limits)?;
        }
    }
    for text_box in view.text_boxes() {
        let text_box = text_box?;
        for operation in text_box_operations(
            &text_box,
            &variables,
            remaining_text_bytes(retained_text_bytes, limits)?,
        )? {
            append_text_operation(&mut operations, &mut retained_text_bytes, operation, limits)?;
        }
    }
    Ok(operations)
}

fn append_properties(
    operations: &mut Vec<PlotterOperation>,
    retained_text_bytes: &mut usize,
    properties: &[FootprintGraphicalProperty],
    limits: FootprintPlotLimits,
) -> Result<(), Error> {
    let reference = properties
        .iter()
        .position(|property| property.name == "Reference");
    let value = properties
        .iter()
        .position(|property| property.name == "Value");
    let ordered = reference.into_iter().chain(value).chain(
        properties
            .iter()
            .enumerate()
            .filter(|(_, property)| !matches!(property.name.as_str(), "Reference" | "Value"))
            .map(|(index, _)| index),
    );
    for index in ordered {
        let property = &properties[index];
        if property.hidden || property.value.is_empty() || !property.graphical {
            continue;
        }
        if property.value.len() > remaining_text_bytes(*retained_text_bytes, limits)? {
            return Err(super::plotter_ir::text_limit_error());
        }
        let operation = PlotterOperation::Text(operation_from_effects(
            property.value.clone(),
            TextOperationInput {
                x: property.at_x,
                y: property.at_y,
                angle: property.angle,
                layer: &property.layer,
                effects: &property.effects,
                default_h: PlotterTextHAlign::Left,
                default_v: PlotterTextVAlign::Bottom,
                multiline: false,
            },
        )?);
        append_text_operation(operations, retained_text_bytes, operation, limits)?;
    }
    Ok(())
}

fn text_operation(
    text: &FootprintText,
    variables: &BoardTextVariables,
    max_text_bytes: usize,
) -> Result<Option<PlotterOperation>, Error> {
    if text.hidden {
        return Ok(None);
    }
    let raw = match text.kind.as_str() {
        "reference" => variables
            .get("Reference")
            .or_else(|| variables.get("REFERENCE"))
            .unwrap_or(&text.text),
        "value" => variables
            .get("Value")
            .or_else(|| variables.get("VALUE"))
            .unwrap_or(&text.text),
        _ => &text.text,
    };
    let resolved = variables.substitute_bounded(raw, max_text_bytes)?;
    if resolved.is_empty() {
        return Ok(None);
    }
    Ok(Some(PlotterOperation::Text(operation_from_effects(
        resolved,
        TextOperationInput {
            x: text.at_x,
            y: text.at_y,
            angle: text.angle,
            layer: &text.layer,
            effects: &text.effects,
            default_h: PlotterTextHAlign::Left,
            default_v: PlotterTextVAlign::Bottom,
            multiline: false,
        },
    )?)))
}

fn text_box_operations(
    text_box: &FootprintTextBox,
    variables: &BoardTextVariables,
    max_text_bytes: usize,
) -> Result<Vec<PlotterOperation>, Error> {
    let mut operations = Vec::with_capacity(2);
    if text_box.border.unwrap_or(false) {
        operations.push(PlotterOperation::Rect(PlotterRect {
            x1: mm_to_nm(text_box.start_x)?,
            y1: mm_to_nm(text_box.start_y)?,
            x2: mm_to_nm(text_box.end_x)?,
            y2: mm_to_nm(text_box.end_y)?,
            fill: PlotterFill::NoFill,
            width_nm: text_box_border_width(text_box.stroke_width)?,
            corner_radius_nm: 0,
            layer: Some(text_box.layer.clone()),
            stroke_color: None,
            fill_color: None,
            line_style: None,
        }));
    }
    if text_box.text.is_empty() {
        return Ok(operations);
    }

    let effects = text_box.effects.clone().unwrap_or_default();
    let (authored_h, authored_v) = alignments(&effects);
    let h_align = authored_h.unwrap_or(PlotterTextHAlign::Left);
    let v_align = authored_v.unwrap_or(PlotterTextVAlign::Top);
    let x1 = text_box.start_x.min(text_box.end_x);
    let y1 = text_box.start_y.min(text_box.end_y);
    let x2 = text_box.start_x.max(text_box.end_x);
    let y2 = text_box.start_y.max(text_box.end_y);
    let [margin_left, margin_top, margin_right, margin_bottom] = text_box.margins;
    let x = match h_align {
        PlotterTextHAlign::Right => x2 - margin_right,
        PlotterTextHAlign::Center => (x1 + x2) / 2.0,
        PlotterTextHAlign::Left => x1 + margin_left,
    };
    let y = match v_align {
        PlotterTextVAlign::Bottom => y2 - margin_bottom,
        PlotterTextVAlign::Center => (y1 + y2) / 2.0,
        PlotterTextVAlign::Top => y1 + margin_top,
    };
    let resolved = variables.substitute_bounded(&text_box.text, max_text_bytes)?;
    let size_x_nm = mm_to_nm(effects.font.size_x)?;
    let wrap_size_x_nm = if size_x_nm == 0 {
        DEFAULT_TEXT_SIZE_NM
    } else {
        size_x_nm
    };
    let wrapped = wrap_text_box(
        &resolved,
        ((x2 - x1) - margin_left - margin_right).max(0.0),
        wrap_size_x_nm,
    );
    let multiline = wrapped.contains('\n') || resolved.contains('\n');
    operations.push(PlotterOperation::Text(operation_from_effects(
        wrapped,
        TextOperationInput {
            x,
            y,
            angle: text_box.angle,
            layer: &text_box.layer,
            effects: &effects,
            default_h: h_align,
            default_v: v_align,
            multiline,
        },
    )?));
    Ok(operations)
}

pub(crate) fn operation_from_effects(
    text: String,
    input: TextOperationInput<'_>,
) -> Result<PlotterText, Error> {
    let (h_align, v_align) = alignments(input.effects);
    Ok(PlotterText {
        x: mm_to_nm(input.x)?,
        y: mm_to_nm(input.y)?,
        text,
        color: input.effects.font.color.map_or_else(
            || "#000000".to_owned(),
            |color| rgba_to_hex(color).unwrap_or_else(|| "#000000".to_owned()),
        ),
        orient_deg: input.angle,
        size_x_nm: mm_to_nm(input.effects.font.size_x)?,
        size_y_nm: mm_to_nm(input.effects.font.size_y)?,
        h_align: h_align.unwrap_or(input.default_h),
        v_align: v_align.unwrap_or(input.default_v),
        pen_width_nm: input
            .effects
            .font
            .thickness
            .map(mm_to_nm)
            .transpose()?
            .unwrap_or(0),
        italic: input.effects.font.italic,
        bold: input.effects.font.bold,
        multiline: input.multiline,
        font_face: input.effects.font.face.clone().unwrap_or_default(),
        layer: Some(input.layer.to_owned()),
    })
}

pub(crate) fn alignments(
    effects: &KiCadTextEffects,
) -> (Option<PlotterTextHAlign>, Option<PlotterTextVAlign>) {
    let mut horizontal = None;
    let mut vertical = None;
    for token in &effects.justify {
        match token.as_str() {
            "left" => horizontal = Some(PlotterTextHAlign::Left),
            "right" => horizontal = Some(PlotterTextHAlign::Right),
            "center" => horizontal = Some(PlotterTextHAlign::Center),
            "top" => vertical = Some(PlotterTextVAlign::Top),
            "bottom" => vertical = Some(PlotterTextVAlign::Bottom),
            _ => {}
        }
    }
    (horizontal, vertical)
}

fn rgba_to_hex(color: KiCadColor) -> Option<String> {
    if color.alpha <= 0.0 {
        return None;
    }
    let alpha = if color.alpha <= 1.0 {
        (color.alpha * 255.0).round_ties_even()
    } else {
        color.alpha.round_ties_even()
    };
    Some(format!(
        "#{:02X}{:02X}{:02X}{:02X}",
        color.red.clamp(0, 255),
        color.green.clamp(0, 255),
        color.blue.clamp(0, 255),
        (alpha as i64).clamp(0, 255)
    ))
}

pub(crate) fn text_box_border_width(width: Option<f64>) -> Result<i64, Error> {
    let width = width.unwrap_or(0.0);
    if width < 0.0 {
        return Ok(0);
    }
    if width == 0.0 {
        return Ok(DEFAULT_TEXT_BOX_BORDER_WIDTH_NM.max(MIN_PLOT_PEN_WIDTH_NM));
    }
    Ok(mm_to_nm(width)?.max(MIN_PLOT_PEN_WIDTH_NM))
}

fn append_text_operation(
    operations: &mut Vec<PlotterOperation>,
    retained_text_bytes: &mut usize,
    operation: PlotterOperation,
    limits: FootprintPlotLimits,
) -> Result<(), Error> {
    if operations.len() >= limits.max_operations {
        return Err(super::plotter_ir::limit_error());
    }
    if let PlotterOperation::Text(text) = &operation {
        *retained_text_bytes = retained_text_bytes
            .checked_add(text.text.len())
            .filter(|bytes| *bytes <= limits.max_text_bytes)
            .ok_or_else(super::plotter_ir::text_limit_error)?;
    }
    operations.push(operation);
    Ok(())
}

fn remaining_text_bytes(
    retained_text_bytes: usize,
    limits: FootprintPlotLimits,
) -> Result<usize, Error> {
    limits
        .max_text_bytes
        .checked_sub(retained_text_bytes)
        .ok_or_else(super::plotter_ir::text_limit_error)
}
