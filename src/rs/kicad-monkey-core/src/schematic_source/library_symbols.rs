use super::{
    SchematicPoint, carrier_form_spans, child_point, child_scalar, direct_scalars, limit_error,
    source_error,
};
use crate::schematic_bundle::SchematicBundleLimits;
use crate::sexpr_projection::{FormSpan, ProjectionLimits, Selector, scan_form_spans_with_limits};
use crate::source_bundle::SourceBundleError;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicLibrarySymbol {
    pub name: String,
    pub extends: Option<String>,
    pub power: bool,
    pub power_kind: Option<String>,
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
    pub name: String,
    pub number: String,
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
    let mut subsymbol_count = 0_usize;
    let mut pin_count = 0_usize;
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
            &mut subsymbol_count,
            &mut pin_count,
        )?);
    }
    Ok(symbols)
}

fn parse_library_symbol(
    source: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
    subsymbol_count: &mut usize,
    pin_count: &mut usize,
) -> Result<SchematicLibrarySymbol, SourceBundleError> {
    let text = span
        .text(source)
        .map_err(|error| source_error(source_path, error.to_string()))?;
    let selected = carrier_form_spans(
        text,
        &["symbol", "extends", "power", "pin"],
        source_path,
        limits,
        limits
            .max_library_subsymbols_per_source
            .saturating_add(limits.max_library_pins_per_source)
            .saturating_add(4),
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
    let power = selected
        .iter()
        .find(|selected| selected.depth == 1 && selected.head.as_deref() == Some("power"));
    let power_kind = power
        .map(|power| direct_scalars(text, power, 1, source_path, limits))
        .transpose()?
        .and_then(|values| values.into_iter().next())
        .filter(|value| matches!(value.as_str(), "global" | "local"));
    let mut subsymbols = Vec::new();
    for subsymbol_span in selected
        .iter()
        .filter(|selected| selected.depth == 1 && selected.head.as_deref() == Some("symbol"))
    {
        if *subsymbol_count >= limits.max_library_subsymbols_per_source {
            return Err(limit_error(
                source_path,
                "embedded library subsymbol count exceeds its limit",
            ));
        }
        *subsymbol_count += 1;
        subsymbols.push(parse_library_subsymbol(
            text,
            subsymbol_span,
            source_path,
            limits,
            pin_count,
        )?);
    }
    Ok(SchematicLibrarySymbol {
        name,
        extends,
        power: power.is_some(),
        power_kind,
        subsymbols,
    })
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
        &["pin", "at", "name", "number", "hide", "uuid"],
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
    Ok(SchematicLibraryPin {
        electrical_type: header
            .first()
            .cloned()
            .unwrap_or_else(|| "unspecified".to_owned()),
        graphic_style: header.get(1).cloned().unwrap_or_else(|| "line".to_owned()),
        at,
        angle_degrees,
        name: child_scalar(text, &selected, "name", source_path, limits)?.unwrap_or_default(),
        number: child_scalar(text, &selected, "number", source_path, limits)?.unwrap_or_default(),
        hidden,
        uuid: child_scalar(text, &selected, "uuid", source_path, limits)?,
    })
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
