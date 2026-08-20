use super::KiCadDesignJsonError;
use crate::{ProjectView, SchematicBundleIndex};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

type VariantOverrides = BTreeMap<String, BTreeMap<String, String>>;
type VariantEffects = (BTreeSet<String>, VariantOverrides);

pub(super) fn variants_json(
    index: &SchematicBundleIndex,
    project: Option<ProjectView<'_>>,
) -> Result<Value, KiCadDesignJsonError> {
    let variants = project
        .map(|view| view.variants())
        .transpose()
        .map_err(|error| KiCadDesignJsonError::context("could not read variants", error))?
        .unwrap_or_default();
    let mut rows = Vec::with_capacity(variants.len());
    for variant in variants {
        let (dnp, overrides) = variant_effects(index, &variant.name)?;
        let mut row = Map::new();
        row.insert("name".to_owned(), json!(variant.name));
        row.insert("dnp".to_owned(), json!(dnp));
        if !overrides.is_empty() {
            row.insert("parameter_overrides".to_owned(), json!(overrides));
        }
        row.insert(
            "kicad_project_variant".to_owned(),
            json!({"name": variant.name, "description": variant.description}),
        );
        if let Some(description) = variant.description {
            row.insert("description".to_owned(), json!(description));
        }
        rows.push(Value::Object(row));
    }
    Ok(Value::Array(rows))
}

fn variant_effects(
    index: &SchematicBundleIndex,
    variant_name: &str,
) -> Result<VariantEffects, KiCadDesignJsonError> {
    let mut dnp = BTreeSet::new();
    let mut overrides = BTreeMap::new();
    for occurrence in index.occurrences() {
        let base = index.effective_symbols(occurrence.index, None)?;
        let effective = index.effective_symbols(occurrence.index, Some(variant_name))?;
        for (base, effective) in base.into_iter().zip(effective) {
            let reference = if effective.reference.is_empty() {
                base.reference.as_str()
            } else {
                effective.reference.as_str()
            };
            if reference.is_empty() {
                continue;
            }
            if effective.dnp {
                dnp.insert(reference.to_owned());
            }
            let mut changed = effective
                .fields
                .iter()
                .filter(|(key, value)| base.fields.get(*key) != Some(*value))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<BTreeMap<_, _>>();
            if effective.value != base.value {
                changed.entry("Value".to_owned()).or_insert(effective.value);
            }
            if !changed.is_empty() {
                overrides.insert(reference.to_owned(), changed);
            }
        }
    }
    Ok((dnp, overrides))
}
