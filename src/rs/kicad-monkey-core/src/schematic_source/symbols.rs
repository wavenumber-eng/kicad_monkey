use super::{
    SchematicPoint, carrier_form_spans, child_point, child_scalar, direct_scalars, limit_error,
    parse_symbol_instances, source_error,
};
use crate::schematic_bundle::SchematicBundleLimits;
use crate::sexpr_projection::FormSpan;
use crate::source_bundle::SourceBundleError;
use std::collections::HashMap;

use super::SchematicSymbolInstance;

#[derive(Clone, Debug, PartialEq)]
pub struct SchematicPlacedSymbol {
    pub lib_id: String,
    pub lib_name: String,
    pub at: SchematicPoint,
    pub angle_degrees: f64,
    pub mirror: Option<String>,
    pub unit: i64,
    pub convert: i64,
    pub exclude_from_sim: bool,
    pub in_bom: bool,
    pub on_board: bool,
    pub in_pos_files: bool,
    pub dnp: bool,
    pub fields_autoplaced: bool,
    pub uuid: String,
    pub properties: Vec<SchematicSymbolProperty>,
    pub pins: Vec<SchematicSymbolPin>,
    pub instances: Vec<SchematicSymbolInstance>,
    instance_by_path: HashMap<String, usize>,
}

impl SchematicPlacedSymbol {
    pub fn instance(&self, path: &str) -> Option<&SchematicSymbolInstance> {
        self.instance_by_path
            .get(path.trim_end_matches('/'))
            .map(|index| &self.instances[*index])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicSymbolProperty {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicSymbolPin {
    pub number: String,
    pub uuid: String,
    pub alternate: Option<String>,
}

pub(crate) fn parse_placed_symbols(
    source: &str,
    source_path: &str,
    spans: &[FormSpan],
    limits: SchematicBundleLimits,
) -> Result<Vec<SchematicPlacedSymbol>, SourceBundleError> {
    let mut symbols = Vec::new();
    for span in spans
        .iter()
        .filter(|span| span.depth == 1 && span.head.as_deref() == Some("symbol"))
    {
        if symbols.len() >= limits.max_symbols_per_source {
            return Err(limit_error(
                source_path,
                "placed symbol count exceeds its limit",
            ));
        }
        let text = span
            .text(source)
            .map_err(|error| source_error(source_path, error.to_string()))?;
        symbols.push(parse_symbol(text, source_path, limits)?);
    }
    Ok(symbols)
}

fn parse_symbol(
    source: &str,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<SchematicPlacedSymbol, SourceBundleError> {
    let selected_limit = limits
        .max_symbol_properties_per_symbol
        .saturating_mul(2)
        .saturating_add(limits.max_symbol_pins_per_symbol.saturating_mul(3))
        .saturating_add(20);
    let spans = carrier_form_spans(
        source,
        &[
            "symbol",
            "lib_id",
            "lib_name",
            "at",
            "mirror",
            "unit",
            "convert",
            "exclude_from_sim",
            "in_bom",
            "on_board",
            "in_pos_files",
            "dnp",
            "fields_autoplaced",
            "uuid",
            "property",
            "pin",
            "instances",
        ],
        source_path,
        limits,
        selected_limit,
    )?;
    let at_span = child(&spans, "at");
    let at = at_span.map_or(Ok(SchematicPoint { x_iu: 0, y_iu: 0 }), |span| {
        child_point_from_span(source, span, source_path, limits)
    })?;
    let angle_degrees = at_span.map_or(Ok(0.0), |span| {
        let values = direct_scalars(source, span, 3, source_path, limits)?;
        values
            .get(2)
            .map_or(Ok(0.0), |value| finite_f64(value, source_path))
    })?;
    let instances =
        parse_symbol_instances(source, child(&spans, "instances"), source_path, limits)?;
    let mut instance_by_path = HashMap::with_capacity(instances.len());
    for (index, instance) in instances.iter().enumerate() {
        let path = instance.path.trim_end_matches('/');
        if !path.is_empty() {
            instance_by_path.entry(path.to_owned()).or_insert(index);
        }
    }
    Ok(SchematicPlacedSymbol {
        lib_id: scalar(source, &spans, "lib_id", source_path, limits)?.unwrap_or_default(),
        lib_name: scalar(source, &spans, "lib_name", source_path, limits)?.unwrap_or_default(),
        at,
        angle_degrees,
        mirror: scalar(source, &spans, "mirror", source_path, limits)?,
        unit: integer(source, &spans, "unit", 1, source_path, limits)?,
        convert: integer(source, &spans, "convert", 1, source_path, limits)?,
        exclude_from_sim: boolean(
            source,
            &spans,
            "exclude_from_sim",
            false,
            source_path,
            limits,
        )?,
        in_bom: boolean(source, &spans, "in_bom", true, source_path, limits)?,
        on_board: boolean(source, &spans, "on_board", true, source_path, limits)?,
        in_pos_files: boolean(source, &spans, "in_pos_files", true, source_path, limits)?,
        dnp: boolean(source, &spans, "dnp", false, source_path, limits)?,
        fields_autoplaced: maybe_absent_boolean(
            source,
            &spans,
            "fields_autoplaced",
            source_path,
            limits,
        )?,
        uuid: scalar(source, &spans, "uuid", source_path, limits)?.unwrap_or_default(),
        properties: parse_properties(source, &spans, source_path, limits)?,
        pins: parse_pins(source, &spans, source_path, limits)?,
        instances,
        instance_by_path,
    })
}

fn parse_properties(
    source: &str,
    spans: &[FormSpan],
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<Vec<SchematicSymbolProperty>, SourceBundleError> {
    let mut properties = Vec::new();
    for span in direct_children(spans, "property") {
        if properties.len() >= limits.max_symbol_properties_per_symbol {
            return Err(limit_error(
                source_path,
                "symbol property count exceeds its limit",
            ));
        }
        let values = direct_scalars(source, span, 2, source_path, limits)?;
        properties.push(SchematicSymbolProperty {
            key: values.first().cloned().unwrap_or_default(),
            value: values.get(1).cloned().unwrap_or_default(),
        });
    }
    Ok(properties)
}

fn parse_pins(
    source: &str,
    spans: &[FormSpan],
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<Vec<SchematicSymbolPin>, SourceBundleError> {
    let mut pins = Vec::new();
    for span in direct_children(spans, "pin") {
        if pins.len() >= limits.max_symbol_pins_per_symbol {
            return Err(limit_error(
                source_path,
                "symbol pin count exceeds its limit",
            ));
        }
        let text = span
            .text(source)
            .map_err(|error| source_error(source_path, error.to_string()))?;
        let nested =
            carrier_form_spans(text, &["pin", "uuid", "alternate"], source_path, limits, 4)?;
        let root = nested
            .iter()
            .find(|selected| selected.depth == 0)
            .ok_or_else(|| source_error(source_path, "symbol pin root is missing"))?;
        let number = direct_scalars(text, root, 1, source_path, limits)?
            .into_iter()
            .next()
            .unwrap_or_default();
        pins.push(SchematicSymbolPin {
            number,
            uuid: child_scalar(text, &nested, "uuid", source_path, limits)?.unwrap_or_default(),
            alternate: child_scalar(text, &nested, "alternate", source_path, limits)?,
        });
    }
    Ok(pins)
}

fn child_point_from_span(
    source: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<SchematicPoint, SourceBundleError> {
    let local = [span.clone()];
    child_point(
        source,
        &local,
        span.head.as_deref().unwrap_or("at"),
        SchematicPoint { x_iu: 0, y_iu: 0 },
        source_path,
        limits,
    )
}

fn direct_children<'a>(spans: &'a [FormSpan], head: &'a str) -> impl Iterator<Item = &'a FormSpan> {
    spans
        .iter()
        .filter(move |span| span.depth == 1 && span.head.as_deref() == Some(head))
}

fn child<'a>(spans: &'a [FormSpan], head: &str) -> Option<&'a FormSpan> {
    spans
        .iter()
        .find(|span| span.depth == 1 && span.head.as_deref() == Some(head))
}

fn scalar(
    source: &str,
    spans: &[FormSpan],
    head: &str,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<Option<String>, SourceBundleError> {
    child_scalar(source, spans, head, source_path, limits)
}

fn integer(
    source: &str,
    spans: &[FormSpan],
    head: &str,
    default: i64,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<i64, SourceBundleError> {
    scalar(source, spans, head, source_path, limits)?.map_or(Ok(default), |value| {
        value
            .parse::<i64>()
            .map_err(|_| source_error(source_path, format!("invalid {head} integer")))
    })
}

fn boolean(
    source: &str,
    spans: &[FormSpan],
    head: &str,
    default: bool,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<bool, SourceBundleError> {
    Ok(scalar(source, spans, head, source_path, limits)?.map_or(default, |value| value == "yes"))
}

fn maybe_absent_boolean(
    source: &str,
    spans: &[FormSpan],
    head: &str,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<bool, SourceBundleError> {
    let Some(span) = child(spans, head) else {
        return Ok(false);
    };
    Ok(direct_scalars(source, span, 1, source_path, limits)?
        .first()
        .is_none_or(|value| value == "yes"))
}

fn finite_f64(value: &str, source_path: &str) -> Result<f64, SourceBundleError> {
    let value = value
        .parse::<f64>()
        .map_err(|_| source_error(source_path, "invalid symbol angle"))?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(source_error(source_path, "symbol angle must be finite"))
    }
}
