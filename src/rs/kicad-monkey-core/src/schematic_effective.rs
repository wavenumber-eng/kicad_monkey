use crate::{
    SchematicDefinition, SchematicOccurrence, SchematicPlacedSymbol, SchematicSymbolInstance,
    SourceBundleError, SourceBundleErrorKind,
};
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicEffectiveSymbol {
    pub symbol_index: usize,
    pub uuid: String,
    pub lib_id: String,
    pub reference: String,
    pub value: String,
    pub unit: i64,
    pub convert: i64,
    pub dnp: bool,
    pub exclude_from_sim: bool,
    pub in_bom: bool,
    pub on_board: bool,
    pub in_pos_files: bool,
    pub fields: BTreeMap<String, String>,
    pub instance_project: Option<String>,
    pub instance_path: Option<String>,
}

pub(crate) fn resolve_effective_symbols(
    definition: &SchematicDefinition,
    occurrence: &SchematicOccurrence,
    project_name: &str,
    variant_name: Option<&str>,
    definitions: &[SchematicDefinition],
    legacy_index: &HashMap<String, (usize, usize)>,
) -> Result<Vec<SchematicEffectiveSymbol>, SourceBundleError> {
    definition
        .symbols
        .iter()
        .enumerate()
        .map(|(index, symbol)| {
            resolve_effective_symbol(
                occurrence,
                project_name,
                variant_name,
                index,
                symbol,
                definitions,
                legacy_index,
            )
        })
        .collect()
}

fn resolve_effective_symbol(
    occurrence: &SchematicOccurrence,
    project_name: &str,
    variant_name: Option<&str>,
    symbol_index: usize,
    symbol: &SchematicPlacedSymbol,
    definitions: &[SchematicDefinition],
    legacy_index: &HashMap<String, (usize, usize)>,
) -> Result<SchematicEffectiveSymbol, SourceBundleError> {
    let modern = resolve_modern_instance(symbol, project_name, occurrence)?;
    let legacy_path = legacy_symbol_path(&occurrence.legacy_address, &symbol.uuid);
    let legacy = modern
        .is_none()
        .then(|| {
            legacy_index
                .get(legacy_path.trim_end_matches('/'))
                .and_then(|(definition_index, instance_index)| {
                    definitions.get(*definition_index).and_then(|definition| {
                        definition.legacy_symbol_instances.get(*instance_index)
                    })
                })
        })
        .flatten();
    let mut fields = symbol
        .properties
        .iter()
        .map(|property| (property.key.clone(), property.value.clone()))
        .collect::<BTreeMap<_, _>>();
    let variant = variant_name.and_then(|name| modern.and_then(|instance| instance.variant(name)));
    if let Some(variant) = variant {
        for field in &variant.fields {
            fields.insert(field.name.clone(), field.value.clone());
        }
    }
    let reference = modern
        .map(|instance| instance.reference.as_str())
        .filter(|reference| !reference.is_empty())
        .or_else(|| {
            legacy
                .map(|instance| instance.reference.as_str())
                .filter(|reference| !reference.is_empty())
        })
        .or_else(|| fields.get("Reference").map(String::as_str))
        .unwrap_or_default()
        .to_owned();
    let unit = modern.map_or_else(
        || legacy.map_or(symbol.unit, |instance| instance.unit),
        |instance| instance.unit,
    );
    let dnp = variant.and_then(|value| value.dnp).unwrap_or(symbol.dnp);
    let exclude_from_sim = variant
        .and_then(|value| value.exclude_from_sim)
        .unwrap_or(symbol.exclude_from_sim);
    let in_bom = variant
        .and_then(|value| value.in_bom)
        .unwrap_or(symbol.in_bom);
    let on_board = variant
        .and_then(|value| value.on_board)
        .unwrap_or(symbol.on_board);
    let in_pos_files = variant
        .and_then(|value| value.in_pos_files)
        .unwrap_or(symbol.in_pos_files);
    Ok(SchematicEffectiveSymbol {
        symbol_index,
        uuid: symbol.uuid.clone(),
        lib_id: symbol.lib_id.clone(),
        reference,
        value: fields.get("Value").cloned().unwrap_or_default(),
        unit,
        convert: symbol.convert,
        dnp: occurrence.effective_dnp || dnp,
        exclude_from_sim: occurrence.effective_exclude_from_sim || exclude_from_sim,
        in_bom: occurrence.effective_in_bom && in_bom,
        on_board: occurrence.effective_on_board && on_board,
        in_pos_files,
        fields,
        instance_project: modern.map(|instance| instance.project.to_string()),
        instance_path: modern.map(|instance| instance.path.clone()),
    })
}

fn resolve_modern_instance<'a>(
    symbol: &'a SchematicPlacedSymbol,
    project_name: &str,
    occurrence: &SchematicOccurrence,
) -> Result<Option<&'a SchematicSymbolInstance>, SourceBundleError> {
    for path in [&occurrence.occurrence_address, &occurrence.legacy_address] {
        if let Some(instance) = exact_instance(symbol, project_name, path)? {
            return Ok(Some(instance));
        }
    }
    let target = occurrence.legacy_address.trim_end_matches('/');
    if !target.is_empty() {
        match symbol.compatible_instance_for_project(project_name, target) {
            Ok(Some(instance)) => return Ok(Some(instance)),
            Ok(None) => {}
            Err(error) => return Err(ambiguity_error(occurrence, error.matches)),
        }
    }
    Ok(symbol.first_instance_for_project(project_name))
}

fn exact_instance<'a>(
    symbol: &'a SchematicPlacedSymbol,
    project_name: &str,
    path: &str,
) -> Result<Option<&'a SchematicSymbolInstance>, SourceBundleError> {
    match symbol.instance_for_project(project_name, path) {
        Ok(Some(instance)) => Ok(Some(instance)),
        Ok(None) => symbol
            .unique_instance_for_path(path)
            .map_err(|error| lookup_error(path, error.matches)),
        Err(error) => Err(lookup_error(path, error.matches)),
    }
}

fn legacy_symbol_path(sheet_path: &str, symbol_uuid: &str) -> String {
    let sheet = sheet_path.trim_end_matches('/');
    if symbol_uuid.is_empty() {
        sheet.to_owned()
    } else if sheet.is_empty() {
        format!("/{symbol_uuid}")
    } else {
        format!("{sheet}/{symbol_uuid}")
    }
}

fn lookup_error(path: &str, matches: usize) -> SourceBundleError {
    SourceBundleError::new(
        SourceBundleErrorKind::Schematic,
        None,
        format!("symbol instance path {path:?} is ambiguous across {matches} records"),
    )
}

fn ambiguity_error(occurrence: &SchematicOccurrence, matches: usize) -> SourceBundleError {
    SourceBundleError::new(
        SourceBundleErrorKind::Schematic,
        Some(&occurrence.source_path),
        format!("symbol instance suffix match is ambiguous across {matches} records"),
    )
}
