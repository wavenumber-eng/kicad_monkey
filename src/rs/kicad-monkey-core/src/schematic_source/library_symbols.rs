use super::{
    SchematicPoint, carrier_form_spans, child_point, child_scalar, direct_scalars, limit_error,
    parse_iu, source_error,
};
use crate::schematic_bundle::SchematicBundleLimits;
use crate::sexpr::{Lexer, Token, TokenKind, decode_quoted_with_limit};
use crate::sexpr_projection::{FormSpan, ProjectionLimits, Selector, scan_form_spans_with_limits};
use crate::source_bundle::SourceBundleError;
use std::collections::BTreeSet;

use super::SchematicSymbolProperty;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicLibrarySymbol {
    pub name: String,
    pub extends: Option<String>,
    pub properties: Vec<SchematicSymbolProperty>,
    pub power: bool,
    pub power_kind: Option<String>,
    pub duplicate_pin_numbers_are_jumpers: bool,
    pub jumper_pin_groups: Vec<Vec<String>>,
    pub pin_names_hide: bool,
    pub pin_numbers_hide: bool,
    pub subsymbols: Vec<SchematicLibrarySubsymbol>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicLibrarySubsymbol {
    pub name: String,
    pub unit: i64,
    pub style: i64,
    pub pins: Vec<SchematicLibraryPin>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicLibraryPin {
    pub electrical_type: String,
    pub graphic_style: String,
    pub at: SchematicPoint,
    pub angle_degrees: i64,
    pub length_is_zero: bool,
    pub name: String,
    pub name_has_visible_size: bool,
    pub number: String,
    pub number_has_visible_size: bool,
    pub hidden: bool,
    pub uuid: Option<String>,
}

pub(crate) fn parse_embedded_library_symbols(
    source: &str,
    source_path: &str,
    spans: &[FormSpan],
    limits: SchematicBundleLimits,
) -> Result<Vec<SchematicLibrarySymbol>, SourceBundleError> {
    let Some(container) = spans
        .iter()
        .find(|span| span.depth == 1 && span.head.as_deref() == Some("lib_symbols"))
    else {
        return Ok(Vec::new());
    };
    let text = container
        .text(source)
        .map_err(|error| source_error(source_path, error.to_string()))?;
    let selected_limit = limits
        .max_library_symbols_per_source
        .saturating_add(limits.max_library_subsymbols_per_source)
        .saturating_add(limits.max_library_pins_per_source)
        .saturating_add(1);
    let selected = scan_form_spans_with_limits(
        text,
        &Selector {
            heads: Some(
                ["lib_symbols", "symbol", "pin"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect::<BTreeSet<_>>(),
            ),
            min_depth: Some(0),
            max_depth: Some(3),
            ..Selector::default()
        },
        ProjectionLimits {
            max_source_bytes: limits.max_source_bytes,
            max_depth: limits.max_depth,
            max_selected_forms: selected_limit,
            ..ProjectionLimits::default()
        },
    )
    .map_err(|error| source_error(source_path, error.to_string()))?;
    let mut symbols = Vec::new();
    let mut counts = LibraryParseCounts::default();
    for symbol_span in selected
        .iter()
        .filter(|span| span.depth == 1 && span.head.as_deref() == Some("symbol"))
    {
        if symbols.len() >= limits.max_library_symbols_per_source {
            return Err(limit_error(
                source_path,
                "embedded library symbol count exceeds its limit",
            ));
        }
        symbols.push(parse_library_symbol(
            text,
            symbol_span,
            source_path,
            limits,
            &mut counts,
        )?);
    }
    Ok(symbols)
}

fn parse_library_symbol(
    source: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
    counts: &mut LibraryParseCounts,
) -> Result<SchematicLibrarySymbol, SourceBundleError> {
    let text = span
        .text(source)
        .map_err(|error| source_error(source_path, error.to_string()))?;
    let selected = carrier_form_spans(
        text,
        &[
            "symbol",
            "extends",
            "power",
            "pin_names",
            "pin_numbers",
            "hide",
            "pin",
            "property",
            "duplicate_pin_numbers_are_jumpers",
            "jumper_pin_groups",
        ],
        source_path,
        limits,
        limits
            .max_library_subsymbols_per_source
            .saturating_add(limits.max_library_pins_per_source)
            .saturating_add(limits.max_library_properties_per_symbol)
            .saturating_add(10),
    )?;
    let root = selected
        .iter()
        .find(|selected| selected.depth == 0)
        .ok_or_else(|| source_error(source_path, "embedded library symbol root is missing"))?;
    let name = direct_scalars(text, root, 1, source_path, limits)?
        .into_iter()
        .next()
        .unwrap_or_default();
    let extends = child_scalar(text, &selected, "extends", source_path, limits)?;
    let properties = parse_library_properties(text, &selected, source_path, limits)?;
    let power = selected
        .iter()
        .find(|selected| selected.depth == 1 && selected.head.as_deref() == Some("power"));
    let power_kind = power
        .map(|power| direct_scalars(text, power, 1, source_path, limits))
        .transpose()?
        .and_then(|values| values.into_iter().next())
        .filter(|value| matches!(value.as_str(), "global" | "local"));
    let duplicate_pin_numbers_are_jumpers = child_scalar(
        text,
        &selected,
        "duplicate_pin_numbers_are_jumpers",
        source_path,
        limits,
    )?
    .is_some_and(|value| matches!(value.as_str(), "yes" | "true" | "1"));
    let jumper_pin_groups = selected
        .iter()
        .find(|selected| {
            selected.depth == 1 && selected.head.as_deref() == Some("jumper_pin_groups")
        })
        .map(|span| parse_jumper_pin_groups(text, span, source_path, limits, counts))
        .transpose()?
        .unwrap_or_default();
    let pin_names_hide = symbol_pin_text_hidden(text, &selected, "pin_names", source_path, limits)?;
    let pin_numbers_hide =
        symbol_pin_text_hidden(text, &selected, "pin_numbers", source_path, limits)?;
    let mut subsymbols = Vec::new();
    for subsymbol_span in selected
        .iter()
        .filter(|selected| selected.depth == 1 && selected.head.as_deref() == Some("symbol"))
    {
        if counts.subsymbols >= limits.max_library_subsymbols_per_source {
            return Err(limit_error(
                source_path,
                "embedded library subsymbol count exceeds its limit",
            ));
        }
        counts.subsymbols += 1;
        subsymbols.push(parse_library_subsymbol(
            text,
            subsymbol_span,
            source_path,
            limits,
            &mut counts.pins,
        )?);
    }
    Ok(SchematicLibrarySymbol {
        name,
        extends,
        properties,
        power: power.is_some(),
        power_kind,
        duplicate_pin_numbers_are_jumpers,
        jumper_pin_groups,
        pin_names_hide,
        pin_numbers_hide,
        subsymbols,
    })
}

fn symbol_pin_text_hidden(
    source: &str,
    selected: &[FormSpan],
    head: &str,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<bool, SourceBundleError> {
    let Some(carrier) = selected
        .iter()
        .find(|span| span.depth == 1 && span.head.as_deref() == Some(head))
    else {
        return Ok(false);
    };
    if direct_scalars(source, carrier, 2, source_path, limits)?
        .iter()
        .any(|value| value == "hide")
    {
        return Ok(true);
    }
    selected
        .iter()
        .find(|span| {
            span.depth == 2
                && span.head.as_deref() == Some("hide")
                && span.range.start >= carrier.range.start
                && span.range.end <= carrier.range.end
        })
        .map(|span| direct_scalars(source, span, 1, source_path, limits))
        .transpose()
        .map(|values| {
            values.is_some_and(|values| {
                values
                    .first()
                    .is_some_and(|value| matches!(value.as_str(), "yes" | "true" | "1"))
            })
        })
}

fn parse_library_properties(
    source: &str,
    selected: &[FormSpan],
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<Vec<SchematicSymbolProperty>, SourceBundleError> {
    let mut properties = Vec::new();
    for property in selected
        .iter()
        .filter(|selected| selected.depth == 1 && selected.head.as_deref() == Some("property"))
    {
        if properties.len() >= limits.max_library_properties_per_symbol {
            return Err(limit_error(
                source_path,
                "embedded library symbol property count exceeds its limit",
            ));
        }
        let values = direct_scalars(source, property, 2, source_path, limits)?;
        properties.push(SchematicSymbolProperty {
            key: values.first().cloned().unwrap_or_default(),
            value: values.get(1).cloned().unwrap_or_default(),
        });
    }
    Ok(properties)
}

#[derive(Default)]
struct LibraryParseCounts {
    subsymbols: usize,
    pins: usize,
    jumper_groups: usize,
    jumper_members: usize,
    jumper_member_bytes: usize,
}

fn parse_jumper_pin_groups(
    source: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
    counts: &mut LibraryParseCounts,
) -> Result<Vec<Vec<String>>, SourceBundleError> {
    let text = span
        .text(source)
        .map_err(|error| source_error(source_path, error.to_string()))?;
    let mut lexer = Lexer::new(text);
    let mut parser = JumperGroupParser::new(source_path, limits, counts);
    while let Some(token) = lexer
        .next()
        .transpose()
        .map_err(|error| source_error(source_path, error.to_string()))?
    {
        match token.kind {
            TokenKind::Left => parser.open(),
            TokenKind::Right => parser.close()?,
            TokenKind::Atom | TokenKind::QuotedString => parser.scalar(token)?,
            _ => {}
        }
    }
    Ok(parser.groups)
}

struct JumperGroupParser<'a, 'b> {
    source_path: &'a str,
    limits: SchematicBundleLimits,
    counts: &'b mut LibraryParseCounts,
    depth: usize,
    groups: Vec<Vec<String>>,
    current: Vec<String>,
}

impl<'a, 'b> JumperGroupParser<'a, 'b> {
    fn new(
        source_path: &'a str,
        limits: SchematicBundleLimits,
        counts: &'b mut LibraryParseCounts,
    ) -> Self {
        Self {
            source_path,
            limits,
            counts,
            depth: 0,
            groups: Vec::new(),
            current: Vec::new(),
        }
    }

    fn open(&mut self) {
        self.depth = self.depth.saturating_add(1);
        if self.depth == 2 {
            self.current.clear();
        }
    }

    fn close(&mut self) -> Result<(), SourceBundleError> {
        if self.depth == 2 && !self.current.is_empty() {
            self.counts.jumper_groups += 1;
            self.groups.push(std::mem::take(&mut self.current));
        }
        self.depth = self.depth.saturating_sub(1);
        Ok(())
    }

    fn scalar(&mut self, token: Token<'_>) -> Result<(), SourceBundleError> {
        if self.depth != 2 {
            return Ok(());
        }
        if self.current.is_empty()
            && self.counts.jumper_groups >= self.limits.max_jumper_groups_per_source
        {
            return Err(limit_error(
                self.source_path,
                "embedded jumper group count exceeds its limit",
            ));
        }
        if self.counts.jumper_members >= self.limits.max_jumper_members_per_source {
            return Err(limit_error(
                self.source_path,
                "embedded jumper member count exceeds its limit",
            ));
        }
        let remaining_bytes = self
            .limits
            .max_jumper_member_bytes_per_source
            .saturating_sub(self.counts.jumper_member_bytes);
        let value = self.decode_member(token, remaining_bytes)?;
        self.counts.jumper_members += 1;
        self.counts.jumper_member_bytes += value.len();
        self.current.push(value);
        Ok(())
    }

    fn decode_member(
        &self,
        token: Token<'_>,
        remaining_bytes: usize,
    ) -> Result<String, SourceBundleError> {
        if token.kind == TokenKind::QuotedString {
            return decode_quoted_with_limit(token.lexeme, remaining_bytes).ok_or_else(|| {
                limit_error(
                    self.source_path,
                    "embedded jumper member bytes exceed their limit",
                )
            });
        }
        if token.lexeme.len() > remaining_bytes {
            return Err(limit_error(
                self.source_path,
                "embedded jumper member bytes exceed their limit",
            ));
        }
        Ok(token.lexeme.to_owned())
    }
}

fn parse_library_subsymbol(
    source: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
    pin_count: &mut usize,
) -> Result<SchematicLibrarySubsymbol, SourceBundleError> {
    let text = span
        .text(source)
        .map_err(|error| source_error(source_path, error.to_string()))?;
    let selected = carrier_form_spans(
        text,
        &["symbol", "pin"],
        source_path,
        limits,
        limits.max_library_pins_per_source.saturating_add(2),
    )?;
    let root = selected
        .iter()
        .find(|selected| selected.depth == 0)
        .ok_or_else(|| source_error(source_path, "embedded subsymbol root is missing"))?;
    let name = direct_scalars(text, root, 1, source_path, limits)?
        .into_iter()
        .next()
        .unwrap_or_default();
    let (unit, style) = unit_and_style(&name);
    let mut pins = Vec::new();
    for pin_span in selected
        .iter()
        .filter(|selected| selected.depth == 1 && selected.head.as_deref() == Some("pin"))
    {
        if *pin_count >= limits.max_library_pins_per_source {
            return Err(limit_error(
                source_path,
                "embedded library pin count exceeds its limit",
            ));
        }
        *pin_count += 1;
        pins.push(parse_library_pin(text, pin_span, source_path, limits)?);
    }
    Ok(SchematicLibrarySubsymbol {
        name,
        unit,
        style,
        pins,
    })
}

fn parse_library_pin(
    source: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<SchematicLibraryPin, SourceBundleError> {
    let text = span
        .text(source)
        .map_err(|error| source_error(source_path, error.to_string()))?;
    let selected = carrier_form_spans(
        text,
        &["pin", "at", "length", "name", "number", "hide", "uuid"],
        source_path,
        limits,
        8,
    )?;
    let root = selected
        .iter()
        .find(|selected| selected.depth == 0)
        .ok_or_else(|| source_error(source_path, "embedded library pin root is missing"))?;
    let header = direct_scalars(text, root, 3, source_path, limits)?;
    let at = child_point(
        text,
        &selected,
        "at",
        SchematicPoint { x_iu: 0, y_iu: 0 },
        source_path,
        limits,
    )?;
    let angle_degrees = selected
        .iter()
        .find(|selected| selected.depth == 1 && selected.head.as_deref() == Some("at"))
        .map(|at_span| direct_scalars(text, at_span, 3, source_path, limits))
        .transpose()?
        .and_then(|values| values.get(2).cloned())
        .map_or(Ok(0), |value| {
            value
                .parse::<i64>()
                .map_err(|_| source_error(source_path, "library pin angle is not an integer"))
        })?;
    let hidden = header.iter().any(|value| value == "hide")
        || child_scalar(text, &selected, "hide", source_path, limits)?.as_deref() == Some("yes");
    let length = child_scalar(text, &selected, "length", source_path, limits)?;
    if let Some(value) = &length {
        parse_iu(value, source_path)?;
    }
    let length_is_zero = length.as_deref().is_some_and(decimal_scalar_is_zero);
    let name_has_visible_size =
        pin_text_has_visible_size(text, &selected, "name", source_path, limits)?;
    let number_has_visible_size =
        pin_text_has_visible_size(text, &selected, "number", source_path, limits)?;
    Ok(SchematicLibraryPin {
        electrical_type: header
            .first()
            .cloned()
            .unwrap_or_else(|| "unspecified".to_owned()),
        graphic_style: header.get(1).cloned().unwrap_or_else(|| "line".to_owned()),
        at,
        angle_degrees,
        length_is_zero,
        name: child_scalar(text, &selected, "name", source_path, limits)?.unwrap_or_default(),
        name_has_visible_size,
        number: child_scalar(text, &selected, "number", source_path, limits)?.unwrap_or_default(),
        number_has_visible_size,
        hidden,
        uuid: child_scalar(text, &selected, "uuid", source_path, limits)?,
    })
}

fn pin_text_has_visible_size(
    source: &str,
    selected: &[FormSpan],
    carrier_head: &str,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<bool, SourceBundleError> {
    let Some(carrier) = selected
        .iter()
        .find(|span| span.depth == 1 && span.head.as_deref() == Some(carrier_head))
    else {
        return Ok(true);
    };
    let carrier_source = source
        .get(carrier.range.clone())
        .ok_or_else(|| source_error(source_path, "schematic pin text carrier range is invalid"))?;
    let size_spans = scan_form_spans_with_limits(
        carrier_source,
        &Selector {
            heads: Some(BTreeSet::from(["size".to_owned()])),
            min_depth: Some(3),
            max_depth: Some(3),
            ..Selector::default()
        },
        ProjectionLimits {
            max_source_bytes: limits.max_source_bytes,
            max_depth: limits.max_depth,
            max_selected_forms: 2,
            ..ProjectionLimits::default()
        },
    )
    .map_err(|error| source_error(source_path, error.to_string()))?;
    let Some(size) = size_spans.first() else {
        return Ok(true);
    };
    let values = direct_scalars(carrier_source, size, 2, source_path, limits)?;
    let parse_dimension = |value: Option<&String>| -> Result<f64, SourceBundleError> {
        let value = value.map_or(1.27, |value| value.parse::<f64>().unwrap_or(f64::NAN));
        let scaled = value * 1_000_000.0;
        if !scaled.is_finite() {
            return Err(source_error(
                source_path,
                "schematic pin text size is not finite",
            ));
        }
        Ok(scaled.round_ties_even())
    };
    Ok(parse_dimension(values.first())?.abs() > 0.0 && parse_dimension(values.get(1))?.abs() > 0.0)
}

fn decimal_scalar_is_zero(value: &str) -> bool {
    let unsigned = value
        .strip_prefix('+')
        .or_else(|| value.strip_prefix('-'))
        .unwrap_or(value);
    let mantissa = unsigned.split(['e', 'E']).next().unwrap_or_default();
    let mut saw_digit = false;
    for byte in mantissa.bytes() {
        if byte.is_ascii_digit() {
            saw_digit = true;
            if byte != b'0' {
                return false;
            }
        } else if byte != b'.' {
            return false;
        }
    }
    saw_digit
}

fn unit_and_style(name: &str) -> (i64, i64) {
    let mut parts = name.rsplitn(3, '_');
    let style = parts.next().and_then(|value| value.parse().ok());
    let unit = parts.next().and_then(|value| value.parse().ok());
    match (unit, style, parts.next()) {
        (Some(unit), Some(style), Some(_)) => (unit, style),
        _ => (1, 0),
    }
}
