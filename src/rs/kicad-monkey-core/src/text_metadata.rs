//! Close-to-format text metadata shared by KiCad document families.

use std::ops::Range;

use crate::sexpr::{Error, ErrorKind, ErrorPhase, Position, Sexp};

const DEFAULT_TEXT_SIZE_MM: f64 = 1.27;

/// An optional RGBA color authored in a KiCad font or stroke block.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KiCadColor {
    pub red: i64,
    pub green: i64,
    pub blue: i64,
    pub alpha: f64,
}

/// Font fields common to PCB, footprint, schematic, and symbol text.
#[derive(Clone, Debug, PartialEq)]
pub struct KiCadFont {
    pub face: Option<String>,
    pub size_x: f64,
    pub size_y: f64,
    pub thickness: Option<f64>,
    pub bold: bool,
    pub italic: bool,
    pub line_spacing: Option<f64>,
    pub color: Option<KiCadColor>,
}

impl Default for KiCadFont {
    fn default() -> Self {
        Self {
            face: None,
            size_x: 1.27,
            size_y: 1.27,
            thickness: None,
            bold: false,
            italic: false,
            line_spacing: None,
            color: None,
        }
    }
}

/// Text effects shared across KiCad document families.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct KiCadTextEffects {
    pub font: KiCadFont,
    pub justify: Vec<String>,
    pub hidden: bool,
    pub href: Option<String>,
    pub source_range: Option<Range<usize>>,
}

/// Decode the close-to-format `(effects ...)` block shared by footprint and
/// library-symbol text carriers.
pub(crate) fn parse_text_effects(form: &Sexp) -> Result<Option<KiCadTextEffects>, Error> {
    let Some(effects) = child(form, "effects") else {
        return Ok(None);
    };
    let font_form = child(effects, "font");
    let size = font_form.and_then(|font| child(font, "size"));
    let font = KiCadFont {
        face: font_form
            .and_then(|font| child(font, "face"))
            .and_then(|value| value_at(value, 1))
            .filter(|value| !value.is_empty())
            .map(str::to_owned),
        size_y: numeric_or(size, 1, DEFAULT_TEXT_SIZE_MM)?,
        size_x: numeric_or(size, 2, DEFAULT_TEXT_SIZE_MM)?,
        thickness: optional_numeric_child(font_form, "thickness")?,
        bold: font_form.is_some_and(|font| named_bool(font, "bold").unwrap_or(false)),
        italic: font_form.is_some_and(|font| named_bool(font, "italic").unwrap_or(false)),
        line_spacing: optional_numeric_child(font_form, "line_spacing")?,
        color: font_form
            .and_then(|font| child(font, "color"))
            .filter(|value| list_values(value).is_some_and(|values| values.len() >= 5))
            .map(parse_color)
            .transpose()?,
    };
    let justify = child(effects, "justify")
        .and_then(list_values)
        .map(|values| {
            values[1..]
                .iter()
                .filter_map(text_value)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    Ok(Some(KiCadTextEffects {
        font,
        justify,
        hidden: named_bool(effects, "hide").unwrap_or(false),
        href: child(effects, "href")
            .and_then(|value| value_at(value, 1))
            .map(str::to_owned),
        source_range: None,
    }))
}

fn parse_color(form: &Sexp) -> Result<KiCadColor, Error> {
    Ok(KiCadColor {
        red: finite_numeric_at(form, 1)? as i64,
        green: finite_numeric_at(form, 2)? as i64,
        blue: finite_numeric_at(form, 3)? as i64,
        alpha: finite_numeric_at(form, 4)?,
    })
}

fn numeric_or(form: Option<&Sexp>, index: usize, default: f64) -> Result<f64, Error> {
    match form {
        Some(value) if list_values(value).is_some_and(|values| values.len() > index) => {
            finite_numeric_at(value, index)
        }
        _ => Ok(default),
    }
}

fn optional_numeric_child(form: Option<&Sexp>, name: &str) -> Result<Option<f64>, Error> {
    form.and_then(|value| child(value, name))
        .filter(|value| list_values(value).is_some_and(|values| values.len() > 1))
        .map(|value| finite_numeric_at(value, 1))
        .transpose()
}

fn finite_numeric_at(form: &Sexp, index: usize) -> Result<f64, Error> {
    let value = list_values(form)
        .and_then(|values| values.get(index))
        .ok_or_else(|| text_metadata_error("Expected text-effects numeric value"))?;
    let value = match value {
        Sexp::Integer(value) => *value as f64,
        Sexp::Float(value) => *value,
        Sexp::Atom(value) | Sexp::Quoted(value) => value
            .parse()
            .map_err(|_| text_metadata_error("Expected text-effects numeric value"))?,
        _ => return Err(text_metadata_error("Expected text-effects numeric value")),
    };
    if value.is_finite() {
        Ok(value)
    } else {
        Err(text_metadata_error(
            "Expected finite text-effects numeric value",
        ))
    }
}

fn named_bool(form: &Sexp, name: &str) -> Option<bool> {
    if has_flag(form, name) {
        return Some(true);
    }
    let value = child(form, name)?;
    if list_values(value).is_none_or(|values| values.len() <= 1) {
        return Some(false);
    }
    Some(value_at(value, 1) == Some("yes"))
}

fn has_flag(form: &Sexp, name: &str) -> bool {
    list_values(form)
        .into_iter()
        .flatten()
        .any(|value| text_value(value) == Some(name))
}

fn list_values(form: &Sexp) -> Option<&[Sexp]> {
    match form {
        Sexp::List(values) => Some(values),
        _ => None,
    }
}

fn text_value(value: &Sexp) -> Option<&str> {
    match value {
        Sexp::Atom(value) | Sexp::Quoted(value) => Some(value),
        _ => None,
    }
}

fn child<'a>(form: &'a Sexp, head: &str) -> Option<&'a Sexp> {
    list_values(form)?
        .iter()
        .find(|candidate| value_at(candidate, 0) == Some(head))
}

fn value_at(form: &Sexp, index: usize) -> Option<&str> {
    list_values(form)?.get(index).and_then(text_value)
}

fn text_metadata_error(message: &'static str) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::UnexpectedToken,
        message,
        Position::START,
    )
}
