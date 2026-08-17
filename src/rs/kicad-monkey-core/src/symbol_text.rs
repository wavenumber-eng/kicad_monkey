//! Library-symbol body text and pin name/number Plotter-IR emission.

use crate::plotter_types::{PlotterOperation, PlotterText, PlotterTextHAlign, PlotterTextVAlign};
use crate::sexpr::{Error, ErrorKind, ErrorPhase, Position, Sexp};
use crate::text_metadata::parse_text_effects;
use crate::{KiCadColor, KiCadTextEffects, SymbolPlotLimits};
use std::collections::BTreeMap;

const DEFAULT_PIN_NAME_OFFSET_MM: f64 = 0.508;
const DEFAULT_TEXT_PEN_WIDTH_NM: i64 = 152_400;
const PIN_TEXT_MARGIN_NM: i64 = 101_600;
const DEVICE_COLOR: &str = "#840000FF";
const PIN_NAME_COLOR: &str = "#006464FF";
const PIN_NUMBER_COLOR: &str = "#A90000FF";
const DEFAULT_FONT_FACE: &str = "Arial";
const JS_SAFE_MAX: f64 = 9_007_199_254_740_991.0;
const JS_SAFE_MAX_I64: i64 = 9_007_199_254_740_991;

/// Exact-name `${NAME}` values supplied by the caller for symbol body text.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SymbolTextVariables {
    by_name: BTreeMap<String, String>,
}

impl SymbolTextVariables {
    pub fn from_entries<N, V>(entries: impl IntoIterator<Item = (N, V)>) -> Self
    where
        N: Into<String>,
        V: Into<String>,
    {
        Self {
            by_name: entries
                .into_iter()
                .map(|(name, value)| (name.into(), value.into()))
                .collect(),
        }
    }

    fn substitute_bounded(&self, text: &str, max_bytes: usize) -> Result<String, Error> {
        let mut output = String::with_capacity(text.len().min(max_bytes));
        let mut rest = text;
        while let Some(start) = rest.find("${") {
            push_bounded(&mut output, &rest[..start], max_bytes)?;
            let after = &rest[start + 2..];
            let Some(end) = after.find('}') else {
                push_bounded(&mut output, &rest[start..], max_bytes)?;
                rest = "";
                break;
            };
            let placeholder = &rest[start..start + end + 3];
            let name = &after[..end];
            let replacement = if name.is_empty() {
                placeholder
            } else {
                self.by_name.get(name).map_or(placeholder, String::as_str)
            };
            push_bounded(&mut output, replacement, max_bytes)?;
            rest = &after[end + 1..];
        }
        push_bounded(&mut output, rest, max_bytes)?;
        Ok(output)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct SymbolTextSettings {
    pin_names_offset: f64,
    pin_names_hide: bool,
    pin_numbers_hide: bool,
}

impl SymbolTextSettings {
    pub(crate) fn from_header(form: &Sexp, position: Position) -> Result<Self, Error> {
        let pin_names = child(form, "pin_names");
        Ok(Self {
            pin_names_offset: pin_names
                .and_then(|value| child(value, "offset"))
                .map(|value| {
                    numeric_or_missing(Some(value), 1, DEFAULT_PIN_NAME_OFFSET_MM, position)
                })
                .transpose()?
                .unwrap_or(DEFAULT_PIN_NAME_OFFSET_MM),
            pin_names_hide: pin_names.is_some_and(hidden),
            pin_numbers_hide: child(form, "pin_numbers").is_some_and(hidden),
        })
    }
}

pub(crate) struct SymbolTextBudget {
    max_carriers: usize,
    max_bytes: usize,
    carriers: usize,
    bytes: usize,
}

impl SymbolTextBudget {
    pub(crate) fn new(limits: SymbolPlotLimits) -> Self {
        Self {
            max_carriers: limits.max_text_carriers,
            max_bytes: limits.max_text_bytes,
            carriers: 0,
            bytes: 0,
        }
    }

    fn remaining_bytes(&self) -> Result<usize, Error> {
        self.max_bytes
            .checked_sub(self.bytes)
            .ok_or_else(text_limit)
    }

    fn charge_carrier(&mut self) -> Result<(), Error> {
        self.carriers = self
            .carriers
            .checked_add(1)
            .filter(|count| *count <= self.max_carriers)
            .ok_or_else(text_limit)?;
        Ok(())
    }

    fn retain_bytes(&mut self, text: &str) -> Result<(), Error> {
        self.bytes = self
            .bytes
            .checked_add(text.len())
            .filter(|bytes| *bytes <= self.max_bytes)
            .ok_or_else(text_limit)?;
        Ok(())
    }
}

pub(crate) fn body_text_operation(
    form: &Sexp,
    variables: &SymbolTextVariables,
    budget: &mut SymbolTextBudget,
    position: Position,
) -> Result<Option<PlotterOperation>, Error> {
    budget.charge_carrier()?;
    let effects = parse_text_effects(form)?.unwrap_or_default();
    if hidden(form) || effects.hidden {
        return Ok(None);
    }
    let raw = value_at(form, 1).unwrap_or_default();
    let substitution_limit = budget.remaining_bytes()?.saturating_add(1);
    let mut resolved = variables.substitute_bounded(raw, substitution_limit)?;
    if resolved.ends_with('\n') {
        resolved.pop();
    }
    budget.retain_bytes(&resolved)?;
    let at = child(form, "at");
    let multiline = resolved.contains('\n');
    Ok(Some(PlotterOperation::Text(styled_text(
        resolved,
        TextStyleInput {
            x: numeric_or_missing(at, 1, 0.0, position)?,
            y: -numeric_or_missing(at, 2, 0.0, position)?,
            orient_deg: angle_or_default(at, 3, position)? / 10.0,
            effects: &effects,
            color: DEVICE_COLOR,
            default_h: PlotterTextHAlign::Center,
            default_v: PlotterTextVAlign::Center,
            multiline,
            clamp_pen_width: true,
            pin_number_width: false,
        },
        position,
    )?)))
}

pub(crate) fn pin_text_operations(
    form: &Sexp,
    settings: SymbolTextSettings,
    default_line_width_nm: Option<i64>,
    budget: &mut SymbolTextBudget,
    position: Position,
) -> Result<Vec<PlotterOperation>, Error> {
    let name_form = child(form, "name");
    for _ in 0..child_count(form, "name") {
        budget.charge_carrier()?;
    }
    for _ in 0..child_count(form, "number") {
        budget.charge_carrier()?;
    }
    if hidden(form) {
        return Ok(Vec::new());
    }
    let geometry = pin_geometry(form, position)?;
    let name = name_form
        .and_then(|value| value_at(value, 1))
        .unwrap_or_default();
    let name_effects = name_form.map(parse_text_effects).transpose()?.flatten();
    let draws_name = !name.is_empty()
        && name != "~"
        && !settings.pin_names_hide
        && positive_text_size(name_effects.as_ref());
    let mut operations = Vec::with_capacity(2);
    append_pin_number(
        &mut operations,
        form,
        geometry,
        draws_name,
        settings,
        default_line_width_nm,
        budget,
        position,
    )?;
    if draws_name {
        append_pin_name(
            &mut operations,
            name,
            name_effects.unwrap_or_default(),
            geometry,
            settings.pin_names_offset,
            budget,
            position,
        )?;
    }
    Ok(operations)
}

#[derive(Clone, Copy)]
struct PinGeometry {
    root_x: i64,
    root_y: i64,
    pos_x: i64,
    pos_y: i64,
    horizontal: bool,
    pin_right: bool,
    pin_down: bool,
    orient_deg: f64,
}

fn pin_geometry(form: &Sexp, position: Position) -> Result<PinGeometry, Error> {
    let at = child(form, "at");
    let x = numeric_or_missing(at, 1, 0.0, position)?;
    let y = numeric_or_missing(at, 2, 0.0, position)?;
    let angle = angle_or_default(at, 3, position)?;
    let length = numeric_or_missing(child(form, "length"), 1, 2.54, position)?;
    let pos_x = mm_to_nm(x, position)?;
    let pos_y = mm_to_nm(-y, position)?;
    let length_nm = mm_to_nm(length, position)?;
    let (root_x, root_y) = pin_root(
        PinRootInput {
            x,
            y,
            angle,
            length,
            pos_x,
            pos_y,
            length_nm,
        },
        position,
    )?;
    let (direction_x, direction_y) = pin_direction(angle);
    let horizontal = direction_x.abs() >= direction_y.abs();
    Ok(PinGeometry {
        root_x,
        root_y,
        pos_x,
        pos_y,
        horizontal,
        pin_right: horizontal && direction_x > 0,
        pin_down: !horizontal && direction_y > 0,
        orient_deg: if horizontal { 0.0 } else { 90.0 },
    })
}

fn append_pin_number(
    operations: &mut Vec<PlotterOperation>,
    form: &Sexp,
    geometry: PinGeometry,
    draws_name: bool,
    settings: SymbolTextSettings,
    default_line_width_nm: Option<i64>,
    budget: &mut SymbolTextBudget,
    position: Position,
) -> Result<(), Error> {
    let number_form = child(form, "number");
    let number = number_form
        .and_then(|value| value_at(value, 1))
        .unwrap_or_default();
    let effects = number_form.map(parse_text_effects).transpose()?.flatten();
    if number.is_empty() || settings.pin_numbers_hide || !positive_text_size(effects.as_ref()) {
        return Ok(());
    }
    let effects = effects.unwrap_or_default();
    let mut style = resolved_style(&effects, false, true, PIN_NUMBER_COLOR, position)?;
    if effects.font.thickness.unwrap_or(0.0) <= 0.0
        && let Some(default_line_width_nm) = default_line_width_nm
    {
        style.pen_width_nm = default_line_width_nm;
    }
    let clearance = coordinate_add(PIN_TEXT_MARGIN_NM, style.pen_width_nm, position)?;
    let midpoint_x = midpoint(geometry.root_x, geometry.pos_x, position)?;
    let midpoint_y = midpoint(geometry.root_y, geometry.pos_y, position)?;
    let (x, y, v_align) = if settings.pin_names_offset > 0.0 || !draws_name {
        if geometry.horizontal {
            (
                midpoint_x,
                coordinate_add(geometry.root_y, -clearance, position)?,
                PlotterTextVAlign::Bottom,
            )
        } else {
            (
                coordinate_add(geometry.root_x, -clearance, position)?,
                midpoint_y,
                PlotterTextVAlign::Bottom,
            )
        }
    } else if geometry.horizontal {
        (
            midpoint_x,
            coordinate_add(geometry.root_y, clearance, position)?,
            PlotterTextVAlign::Top,
        )
    } else {
        (
            coordinate_add(geometry.root_x, clearance, position)?,
            midpoint_y,
            PlotterTextVAlign::Top,
        )
    };
    budget.retain_bytes(number)?;
    operations.push(PlotterOperation::Text(PlotterText {
        x,
        y,
        text: number.to_owned(),
        color: style.color,
        orient_deg: geometry.orient_deg,
        size_x_nm: style.size_x_nm,
        size_y_nm: style.size_y_nm,
        h_align: PlotterTextHAlign::Center,
        v_align,
        pen_width_nm: style.pen_width_nm,
        italic: effects.font.italic,
        bold: effects.font.bold,
        multiline: false,
        font_face: style.font_face,
        layer: None,
    }));
    Ok(())
}

fn append_pin_name(
    operations: &mut Vec<PlotterOperation>,
    name: &str,
    effects: KiCadTextEffects,
    geometry: PinGeometry,
    offset_mm: f64,
    budget: &mut SymbolTextBudget,
    position: Position,
) -> Result<(), Error> {
    let style = resolved_style(&effects, false, false, PIN_NAME_COLOR, position)?;
    let clearance = coordinate_add(PIN_TEXT_MARGIN_NM, style.pen_width_nm, position)?;
    let midpoint_x = midpoint(geometry.root_x, geometry.pos_x, position)?;
    let midpoint_y = midpoint(geometry.root_y, geometry.pos_y, position)?;
    let (x, y, h_align, v_align) = if offset_mm > 0.0 {
        let offset = mm_to_nm(offset_mm, position)?;
        if geometry.horizontal {
            (
                if geometry.pin_right {
                    coordinate_add(geometry.root_x, offset, position)?
                } else {
                    coordinate_add(geometry.root_x, -offset, position)?
                },
                geometry.root_y,
                if geometry.pin_right {
                    PlotterTextHAlign::Left
                } else {
                    PlotterTextHAlign::Right
                },
                PlotterTextVAlign::Center,
            )
        } else {
            (
                geometry.root_x,
                if geometry.pin_down {
                    coordinate_add(geometry.root_y, offset, position)?
                } else {
                    coordinate_add(geometry.root_y, -offset, position)?
                },
                if geometry.pin_down {
                    PlotterTextHAlign::Right
                } else {
                    PlotterTextHAlign::Left
                },
                PlotterTextVAlign::Center,
            )
        }
    } else if geometry.horizontal {
        (
            midpoint_x,
            coordinate_add(geometry.root_y, -clearance, position)?,
            PlotterTextHAlign::Center,
            PlotterTextVAlign::Bottom,
        )
    } else {
        (
            coordinate_add(geometry.root_x, -clearance, position)?,
            midpoint_y,
            PlotterTextHAlign::Center,
            PlotterTextVAlign::Bottom,
        )
    };
    budget.retain_bytes(name)?;
    operations.push(PlotterOperation::Text(PlotterText {
        x,
        y,
        text: name.to_owned(),
        color: style.color,
        orient_deg: geometry.orient_deg,
        size_x_nm: style.size_x_nm,
        size_y_nm: style.size_y_nm,
        h_align,
        v_align,
        pen_width_nm: style.pen_width_nm,
        italic: effects.font.italic,
        bold: effects.font.bold,
        multiline: false,
        font_face: style.font_face,
        layer: None,
    }));
    Ok(())
}

struct TextStyleInput<'a> {
    x: f64,
    y: f64,
    orient_deg: f64,
    effects: &'a KiCadTextEffects,
    color: &'a str,
    default_h: PlotterTextHAlign,
    default_v: PlotterTextVAlign,
    multiline: bool,
    clamp_pen_width: bool,
    pin_number_width: bool,
}

fn styled_text(
    text: String,
    input: TextStyleInput<'_>,
    position: Position,
) -> Result<PlotterText, Error> {
    let style = resolved_style(
        input.effects,
        input.clamp_pen_width,
        input.pin_number_width,
        input.color,
        position,
    )?;
    let (h_align, v_align) = alignments(input.effects);
    Ok(PlotterText {
        x: mm_to_nm(input.x, position)?,
        y: mm_to_nm(input.y, position)?,
        text,
        color: style.color,
        orient_deg: input.orient_deg,
        size_x_nm: style.size_x_nm,
        size_y_nm: style.size_y_nm,
        h_align: h_align.unwrap_or(input.default_h),
        v_align: v_align.unwrap_or(input.default_v),
        pen_width_nm: style.pen_width_nm,
        italic: input.effects.font.italic,
        bold: input.effects.font.bold,
        multiline: input.multiline,
        font_face: style.font_face,
        layer: None,
    })
}

struct ResolvedStyle {
    color: String,
    font_face: String,
    size_x_nm: i64,
    size_y_nm: i64,
    pen_width_nm: i64,
}

fn resolved_style(
    effects: &KiCadTextEffects,
    clamp_pen_width: bool,
    pin_number_width: bool,
    default_color: &str,
    position: Position,
) -> Result<ResolvedStyle, Error> {
    let size_x_nm = mm_to_nm(effects.font.size_x, position)?;
    let size_y_nm = mm_to_nm(effects.font.size_y, position)?;
    let text_size = size_x_nm.abs().min(size_y_nm.abs());
    let explicit = effects
        .font
        .thickness
        .map(|value| mm_to_nm(value, position))
        .transpose()?;
    let mut pen_width_nm = explicit.filter(|width| *width > 0).unwrap_or_else(|| {
        if pin_number_width {
            if text_size > 0 {
                (text_size as f64 / 5.0 + 0.5) as i64
            } else {
                DEFAULT_TEXT_PEN_WIDTH_NM
            }
        } else if effects.font.bold {
            if text_size > 0 {
                round_pen_width(text_size as f64 / 5.0)
            } else {
                DEFAULT_TEXT_PEN_WIDTH_NM
            }
        } else {
            DEFAULT_TEXT_PEN_WIDTH_NM
        }
    });
    if clamp_pen_width && text_size > 0 {
        pen_width_nm = pen_width_nm.min((text_size as f64 * 0.25 + 0.5) as i64);
    }
    Ok(ResolvedStyle {
        color: effects
            .font
            .color
            .and_then(rgba_to_hex)
            .unwrap_or_else(|| default_color.to_owned()),
        font_face: effects
            .font
            .face
            .clone()
            .filter(|face| !face.is_empty())
            .unwrap_or_else(|| DEFAULT_FONT_FACE.to_owned()),
        size_x_nm,
        size_y_nm,
        pen_width_nm,
    })
}

fn alignments(
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

fn positive_text_size(effects: Option<&KiCadTextEffects>) -> bool {
    effects.is_none_or(|value| value.font.size_x.abs() > 0.0 && value.font.size_y.abs() > 0.0)
}

#[derive(Clone, Copy)]
struct PinRootInput {
    x: f64,
    y: f64,
    angle: f64,
    length: f64,
    pos_x: i64,
    pos_y: i64,
    length_nm: i64,
}

fn pin_root(input: PinRootInput, position: Position) -> Result<(i64, i64), Error> {
    let PinRootInput {
        x,
        y,
        angle,
        length,
        pos_x,
        pos_y,
        length_nm,
    } = input;
    match (angle.round_ties_even() as i64).rem_euclid(360) {
        0 => Ok((coordinate_add(pos_x, length_nm, position)?, pos_y)),
        90 => Ok((pos_x, coordinate_add(pos_y, -length_nm, position)?)),
        180 => Ok((coordinate_add(pos_x, -length_nm, position)?, pos_y)),
        270 => Ok((pos_x, coordinate_add(pos_y, length_nm, position)?)),
        _ => {
            let radians = angle.to_radians();
            Ok((
                mm_to_nm(x + length * radians.cos(), position)?,
                mm_to_nm(-(y + length * radians.sin()), position)?,
            ))
        }
    }
}

fn pin_direction(angle: f64) -> (i64, i64) {
    match (angle.round_ties_even() as i64).rem_euclid(360) {
        0 => (1_000_000, 0),
        180 => (-1_000_000, 0),
        90 => (0, -1_000_000),
        270 => (0, 1_000_000),
        _ => {
            let radians = angle.to_radians();
            (
                (radians.cos() * 1_000_000.0).round_ties_even() as i64,
                (-radians.sin() * 1_000_000.0).round_ties_even() as i64,
            )
        }
    }
}

fn mm_to_nm(value: f64, position: Position) -> Result<i64, Error> {
    let scaled = value * 1_000_000.0;
    if !scaled.is_finite() || !(-JS_SAFE_MAX..=JS_SAFE_MAX).contains(&scaled) {
        return Err(model(
            "Symbol text exceeds JavaScript safe-integer range",
            position,
        ));
    }
    Ok(scaled.round_ties_even() as i64)
}

fn coordinate_add(left: i64, right: i64, position: Position) -> Result<i64, Error> {
    derived_coordinate(left as i128 + right as i128, position)
}

fn midpoint(left: i64, right: i64, position: Position) -> Result<i64, Error> {
    derived_coordinate((left as i128 + right as i128).div_euclid(2), position)
}

fn derived_coordinate(value: i128, position: Position) -> Result<i64, Error> {
    if !(-(JS_SAFE_MAX_I64 as i128)..=JS_SAFE_MAX_I64 as i128).contains(&value) {
        return Err(model(
            "Derived symbol text coordinate exceeds JavaScript safe-integer range",
            position,
        ));
    }
    Ok(value as i64)
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

fn round_pen_width(value: f64) -> i64 {
    (value / 100.0 + 0.5) as i64 * 100
}

fn hidden(form: &Sexp) -> bool {
    has_atom(form, "hide")
        || child(form, "hide").and_then(|value| value_at(value, 1)) == Some("yes")
}

fn push_bounded(output: &mut String, value: &str, max_bytes: usize) -> Result<(), Error> {
    output
        .len()
        .checked_add(value.len())
        .filter(|bytes| *bytes <= max_bytes)
        .ok_or_else(text_limit)?;
    output.push_str(value);
    Ok(())
}

fn child<'a>(form: &'a Sexp, head: &str) -> Option<&'a Sexp> {
    list(form)?
        .iter()
        .find(|candidate| value_at(candidate, 0) == Some(head))
}

fn child_count(form: &Sexp, head: &str) -> usize {
    list(form)
        .into_iter()
        .flatten()
        .filter(|candidate| value_at(candidate, 0) == Some(head))
        .count()
}

fn list(form: &Sexp) -> Option<&[Sexp]> {
    match form {
        Sexp::List(values) => Some(values),
        _ => None,
    }
}

fn text(value: &Sexp) -> Option<&str> {
    match value {
        Sexp::Atom(value) | Sexp::Quoted(value) => Some(value),
        _ => None,
    }
}

fn value_at(form: &Sexp, index: usize) -> Option<&str> {
    list(form)?.get(index).and_then(text)
}

fn has_atom(form: &Sexp, expected: &str) -> bool {
    list(form).is_some_and(|values| values.iter().any(|value| text(value) == Some(expected)))
}

fn numeric_or_missing(
    form: Option<&Sexp>,
    index: usize,
    default: f64,
    position: Position,
) -> Result<f64, Error> {
    let Some(value) = form.and_then(list).and_then(|values| values.get(index)) else {
        return Ok(default);
    };
    finite_numeric(value, position)
}

fn angle_or_default(form: Option<&Sexp>, index: usize, position: Position) -> Result<f64, Error> {
    let Some(value) = form.and_then(list).and_then(|values| values.get(index)) else {
        return Ok(0.0);
    };
    match parse_numeric(value) {
        Ok(number) if number.is_finite() => Ok(number),
        Ok(_) => Err(model("Symbol text numeric value must be finite", position)),
        Err(()) => Ok(0.0),
    }
}

fn finite_numeric(value: &Sexp, position: Position) -> Result<f64, Error> {
    let number =
        parse_numeric(value).map_err(|()| model("Expected numeric symbol text value", position))?;
    if number.is_finite() {
        Ok(number)
    } else {
        Err(model("Symbol text numeric value must be finite", position))
    }
}

fn parse_numeric(value: &Sexp) -> Result<f64, ()> {
    let number = match value {
        Sexp::Integer(value) => *value as f64,
        Sexp::Float(value) => *value,
        Sexp::Atom(value) | Sexp::Quoted(value) => value.parse().map_err(|_| ())?,
        _ => return Err(()),
    };
    Ok(number)
}

fn model(message: &'static str, position: Position) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::UnexpectedToken,
        message,
        position,
    )
}

fn text_limit() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "Symbol text resource limit exceeded",
        Position::START,
    )
}
