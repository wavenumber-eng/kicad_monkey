//! Preserve authored config overrides: missing, empty, and null are distinct.
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub fn preserve_config_presence(schema: &Value, source: String) -> Result<String> {
    let mut objects = BTreeMap::new();
    objects.insert(
        schema["title"]
            .as_str()
            .context("config schema requires a title")?,
        schema,
    );
    if let Some(definitions) = schema["$defs"].as_object() {
        objects.extend(
            definitions
                .iter()
                .map(|(name, value)| (name.as_str(), value)),
        );
    }
    let mut syntax = syn::parse_file(&source)?;
    for item in &mut syntax.items {
        let syn::Item::Struct(structure) = item else {
            continue;
        };
        let name = structure.ident.to_string();
        let Some(schema) = objects.get(name.as_str()) else {
            continue;
        };
        let Some(properties) = schema["properties"].as_object() else {
            continue;
        };
        let required: BTreeSet<&str> = schema["required"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        let syn::Fields::Named(fields) = &mut structure.fields else {
            continue;
        };
        for field in &mut fields.named {
            let name = field
                .ident
                .as_ref()
                .context("unnamed config field")?
                .to_string();
            let wire = super::serde_field_name(field)?.unwrap_or(name);
            let Some(property) = properties.get(&wire) else {
                continue;
            };
            if !required.contains(wire.as_str()) {
                preserve_field(field, property, &wire)?;
            }
        }
    }
    super::rustfmt(&prettyplease::unparse(&syntax))
}

fn preserve_field(field: &mut syn::Field, schema: &Value, wire: &str) -> Result<()> {
    let mut inner = field.ty.clone();
    if !super::schema_allows_null(schema) && super::is_option_type(&inner) {
        let syn::Type::Path(path) = &inner else {
            bail!("expected Option path")
        };
        let Some(segment) = path.path.segments.last() else {
            bail!("empty Option path")
        };
        let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
            bail!("expected Option type argument");
        };
        let Some(syn::GenericArgument::Type(value)) = arguments.args.first() else {
            bail!("expected Option inner type");
        };
        inner = value.clone();
    }
    field.ty = syn::parse_quote!(crate::pcb_svg::presence::Field<#inner>);
    super::replace_serde_presence_attr(
        field,
        syn::parse_quote!(
            #[serde(rename = #wire, default, skip_serializing_if = "crate::pcb_svg::presence::Field::is_missing")]
        ),
    )
}
