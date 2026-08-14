use super::{carrier_form_spans, child_scalar, direct_scalars, limit_error, source_error};
use crate::schematic_bundle::SchematicBundleLimits;
use crate::sexpr_projection::FormSpan;
use crate::source_bundle::SourceBundleError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicSymbolInstance {
    pub project: String,
    pub path: String,
    pub reference: String,
    pub unit: i64,
    pub variants: Vec<SchematicSymbolInstanceVariant>,
}

impl SchematicSymbolInstance {
    pub fn variant(&self, name: &str) -> Option<&SchematicSymbolInstanceVariant> {
        self.variants.iter().find(|variant| variant.name == name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicSymbolInstanceVariant {
    pub name: String,
    pub dnp: Option<bool>,
    pub exclude_from_sim: Option<bool>,
    pub in_bom: Option<bool>,
    pub on_board: Option<bool>,
    pub in_pos_files: Option<bool>,
    pub fields: Vec<SchematicSymbolVariantField>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicSymbolVariantField {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicLegacySymbolInstance {
    pub path: String,
    pub reference: String,
    pub unit: i64,
    pub value: String,
    pub footprint: String,
}

pub(crate) fn parse_symbol_instances(
    source: &str,
    instance_span: Option<&FormSpan>,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<Vec<SchematicSymbolInstance>, SourceBundleError> {
    let Some(instance_span) = instance_span else {
        return Ok(Vec::new());
    };
    let text = span_text(source, instance_span, source_path)?;
    let selected_limit = limits
        .max_symbol_instance_projects_per_symbol
        .saturating_add(limits.max_symbol_instances_per_symbol)
        .saturating_add(1);
    let spans = carrier_form_spans(
        text,
        &["instances", "project", "path"],
        source_path,
        limits,
        selected_limit,
    )?;
    let mut project = None;
    let mut project_count = 0_usize;
    let mut instances = Vec::new();
    for span in &spans {
        match (span.depth, span.head.as_deref()) {
            (1, Some("project")) => {
                if project_count >= limits.max_symbol_instance_projects_per_symbol {
                    return Err(limit_error(
                        source_path,
                        "symbol instance project count exceeds its limit",
                    ));
                }
                project_count += 1;
                project = Some((
                    direct_scalars(text, span, 1, source_path, limits)?
                        .into_iter()
                        .next()
                        .unwrap_or_default(),
                    span.range.clone(),
                ));
            }
            (2, Some("path")) => {
                if instances.len() >= limits.max_symbol_instances_per_symbol {
                    return Err(limit_error(
                        source_path,
                        "symbol instance path count exceeds its limit",
                    ));
                }
                let (owner, owner_range) = project.as_ref().ok_or_else(|| {
                    source_error(source_path, "symbol instance path has no owning project")
                })?;
                if span.range.start <= owner_range.start || span.range.end > owner_range.end {
                    return Err(source_error(
                        source_path,
                        "symbol instance path is outside its owning project",
                    ));
                }
                instances.push(parse_modern_path(text, span, owner, source_path, limits)?);
            }
            _ => {}
        }
    }
    Ok(instances)
}

pub(crate) fn parse_legacy_symbol_instances(
    source: &str,
    source_path: &str,
    spans: &[FormSpan],
    limits: SchematicBundleLimits,
) -> Result<Vec<SchematicLegacySymbolInstance>, SourceBundleError> {
    let Some(container) = spans
        .iter()
        .find(|span| span.depth == 1 && span.head.as_deref() == Some("symbol_instances"))
    else {
        return Ok(Vec::new());
    };
    let text = span_text(source, container, source_path)?;
    let nested = carrier_form_spans(
        text,
        &["symbol_instances", "path"],
        source_path,
        limits,
        limits
            .max_legacy_symbol_instances_per_source
            .saturating_add(2),
    )?;
    let mut instances = Vec::new();
    for span in nested
        .iter()
        .filter(|span| span.depth == 1 && span.head.as_deref() == Some("path"))
    {
        if instances.len() >= limits.max_legacy_symbol_instances_per_source {
            return Err(limit_error(
                source_path,
                "legacy symbol instance count exceeds its limit",
            ));
        }
        instances.push(parse_legacy_path(text, span, source_path, limits)?);
    }
    Ok(instances)
}

fn parse_modern_path(
    source: &str,
    span: &FormSpan,
    project: &str,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<SchematicSymbolInstance, SourceBundleError> {
    let text = span_text(source, span, source_path)?;
    let selected_limit = limits.max_symbol_variants_per_instance.saturating_add(4);
    let spans = carrier_form_spans(
        text,
        &["path", "reference", "unit", "variant"],
        source_path,
        limits,
        selected_limit,
    )?;
    let root = root(&spans, source_path, "symbol instance path")?;
    let path = direct_scalars(text, root, 1, source_path, limits)?
        .into_iter()
        .next()
        .unwrap_or_default();
    let mut variants = Vec::new();
    for variant in spans
        .iter()
        .filter(|span| span.depth == 1 && span.head.as_deref() == Some("variant"))
    {
        if variants.len() >= limits.max_symbol_variants_per_instance {
            return Err(limit_error(
                source_path,
                "symbol instance variant count exceeds its limit",
            ));
        }
        variants.push(parse_variant(text, variant, source_path, limits)?);
    }
    Ok(SchematicSymbolInstance {
        project: project.to_owned(),
        path,
        reference: child_scalar(text, &spans, "reference", source_path, limits)?
            .unwrap_or_default(),
        unit: child_integer(text, &spans, "unit", source_path, limits)?,
        variants,
    })
}

fn parse_variant(
    source: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<SchematicSymbolInstanceVariant, SourceBundleError> {
    let text = span_text(source, span, source_path)?;
    let selected_limit = limits
        .max_symbol_variant_fields_per_variant
        .saturating_mul(3)
        .saturating_add(8);
    let spans = carrier_form_spans(
        text,
        &[
            "variant",
            "name",
            "dnp",
            "exclude_from_sim",
            "in_bom",
            "on_board",
            "in_pos_files",
            "field",
            "value",
        ],
        source_path,
        limits,
        selected_limit,
    )?;
    let mut fields = Vec::new();
    for field in spans
        .iter()
        .filter(|span| span.depth == 1 && span.head.as_deref() == Some("field"))
    {
        if fields.len() >= limits.max_symbol_variant_fields_per_variant {
            return Err(limit_error(
                source_path,
                "symbol variant field count exceeds its limit",
            ));
        }
        fields.push(parse_variant_field(text, field, source_path, limits)?);
    }
    Ok(SchematicSymbolInstanceVariant {
        name: child_scalar(text, &spans, "name", source_path, limits)?.unwrap_or_default(),
        dnp: optional_boolean(text, &spans, "dnp", source_path, limits)?,
        exclude_from_sim: optional_boolean(text, &spans, "exclude_from_sim", source_path, limits)?,
        in_bom: optional_boolean(text, &spans, "in_bom", source_path, limits)?,
        on_board: optional_boolean(text, &spans, "on_board", source_path, limits)?,
        in_pos_files: optional_boolean(text, &spans, "in_pos_files", source_path, limits)?,
        fields,
    })
}

fn parse_variant_field(
    source: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<SchematicSymbolVariantField, SourceBundleError> {
    let text = span_text(source, span, source_path)?;
    let spans = carrier_form_spans(text, &["field", "name", "value"], source_path, limits, 3)?;
    Ok(SchematicSymbolVariantField {
        name: child_scalar(text, &spans, "name", source_path, limits)?.unwrap_or_default(),
        value: child_scalar(text, &spans, "value", source_path, limits)?.unwrap_or_default(),
    })
}

fn parse_legacy_path(
    source: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<SchematicLegacySymbolInstance, SourceBundleError> {
    let text = span_text(source, span, source_path)?;
    let spans = carrier_form_spans(
        text,
        &["path", "reference", "unit", "value", "footprint"],
        source_path,
        limits,
        5,
    )?;
    let root = root(&spans, source_path, "legacy symbol instance path")?;
    Ok(SchematicLegacySymbolInstance {
        path: direct_scalars(text, root, 1, source_path, limits)?
            .into_iter()
            .next()
            .unwrap_or_default(),
        reference: child_scalar(text, &spans, "reference", source_path, limits)?
            .unwrap_or_default(),
        unit: child_integer(text, &spans, "unit", source_path, limits)?,
        value: child_scalar(text, &spans, "value", source_path, limits)?.unwrap_or_default(),
        footprint: child_scalar(text, &spans, "footprint", source_path, limits)?
            .unwrap_or_default(),
    })
}

fn child_integer(
    source: &str,
    spans: &[FormSpan],
    head: &str,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<i64, SourceBundleError> {
    child_scalar(source, spans, head, source_path, limits)?.map_or(Ok(1), |value| {
        value
            .parse::<i64>()
            .map_err(|_| source_error(source_path, format!("invalid {head} integer")))
    })
}

fn optional_boolean(
    source: &str,
    spans: &[FormSpan],
    head: &str,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<Option<bool>, SourceBundleError> {
    let Some(span) = spans
        .iter()
        .find(|span| span.depth == 1 && span.head.as_deref() == Some(head))
    else {
        return Ok(None);
    };
    Ok(direct_scalars(source, span, 1, source_path, limits)?
        .first()
        .map(|value| value == "yes"))
}

fn root<'a>(
    spans: &'a [FormSpan],
    source_path: &str,
    label: &str,
) -> Result<&'a FormSpan, SourceBundleError> {
    spans
        .iter()
        .find(|span| span.depth == 0)
        .ok_or_else(|| source_error(source_path, format!("{label} root is missing")))
}

fn span_text<'a>(
    source: &'a str,
    span: &FormSpan,
    source_path: &str,
) -> Result<&'a str, SourceBundleError> {
    span.text(source)
        .map_err(|error| source_error(source_path, error.to_string()))
}
