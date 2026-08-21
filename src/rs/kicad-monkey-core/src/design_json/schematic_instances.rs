use super::{KiCadDesignFacts, KiCadDesignJsonError};
use crate::{SchematicBundleIndex, SchematicOccurrence};
use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct KiCadSchematicInstance {
    pub instance_index: usize,
    pub sheet_number: usize,
    pub sheet_count: usize,
    pub source_path: String,
    pub sheet_name: String,
    pub sheet_path: String,
    pub sheet_path_uuids: String,
    pub sheet_instance_path: String,
    pub sheet_symbol_uid: String,
    pub sheet_file: String,
    pub parent_sheet_path: Option<String>,
    pub parent_sheet_path_uuids: Option<String>,
    pub parent_sheet_instance_path: Option<String>,
    pub is_top_level: bool,
    pub document_id: String,
    pub page_occurrence_ref: String,
}

pub(super) fn schematic_instances(
    facts: &KiCadDesignFacts<'_>,
) -> Result<Vec<KiCadSchematicInstance>, KiCadDesignJsonError> {
    let index = facts.index;
    let graph = facts.graph();
    let page_by_instance_path = graph
        .page_occurrences
        .iter()
        .filter_map(|page| {
            page.source_identity
                .sch_source_key_source_record
                .as_deref()
                .and_then(|record| record.strip_prefix("instance-path:"))
                .map(|path| (path, page))
        })
        .collect::<HashMap<_, _>>();
    let sheet_count = index.occurrences().len();
    index
        .occurrences()
        .map(|occurrence| {
            let page = page_by_instance_path
                .get(occurrence.occurrence_address.as_str())
                .ok_or_else(|| {
                    KiCadDesignJsonError::context(
                        "could not resolve schematic review instance",
                        format!(
                            "compiled graph has no page for instance path {}",
                            occurrence.occurrence_address
                        ),
                    )
                })?;
            let parent = occurrence
                .parent_index
                .and_then(|parent_index| index.occurrence(parent_index));
            let definition = index.definition(&occurrence.source_path).ok_or_else(|| {
                KiCadDesignJsonError::context(
                    "could not resolve schematic review instance",
                    format!("missing definition {}", occurrence.source_path),
                )
            })?;
            Ok(KiCadSchematicInstance {
                instance_index: occurrence.index,
                sheet_number: sheet_number(index, occurrence),
                sheet_count,
                source_path: occurrence.source_path.clone(),
                sheet_name: sheet_name(occurrence),
                sheet_path: occurrence.human_address.clone(),
                sheet_path_uuids: occurrence.legacy_address.clone(),
                sheet_instance_path: occurrence.occurrence_address.clone(),
                sheet_symbol_uid: occurrence.sheet_uuid.clone().unwrap_or_default(),
                sheet_file: occurrence.sheet_file.clone(),
                parent_sheet_path: parent.map(|value| value.human_address.clone()),
                parent_sheet_path_uuids: parent.map(|value| value.legacy_address.clone()),
                parent_sheet_instance_path: parent.map(|value| value.occurrence_address.clone()),
                is_top_level: occurrence.parent_index.is_none(),
                document_id: definition
                    .uuid
                    .clone()
                    .filter(|value| !value.is_empty())
                    .unwrap_or_else(|| source_stem(&occurrence.source_path, occurrence.index)),
                page_occurrence_ref: page.id.clone(),
            })
        })
        .collect()
}

fn sheet_name(occurrence: &SchematicOccurrence) -> String {
    if occurrence.parent_index.is_none() {
        source_stem(&occurrence.source_path, occurrence.index)
    } else {
        occurrence.sheet_name.clone()
    }
}

fn source_stem(path: &str, index: usize) -> String {
    let filename = path.rsplit('/').next().unwrap_or(path);
    filename
        .strip_suffix(".kicad_sch")
        .filter(|value| !value.is_empty())
        .map_or_else(|| format!("sheet_{index}"), str::to_owned)
}

fn sheet_number(index: &SchematicBundleIndex, occurrence: &SchematicOccurrence) -> usize {
    if let (Some(parent_index), Some(sheet_index)) =
        (occurrence.parent_index, occurrence.parent_sheet_index)
        && let Some(parent) = index.occurrence(parent_index)
        && let Some(definition) = index.definition(&parent.source_path)
        && let Some(number) = definition
            .sheets
            .get(sheet_index)
            .and_then(|sheet| page_number(&sheet.page_instances, &occurrence.occurrence_address))
    {
        return if number == 0 {
            occurrence.index
        } else {
            number
        };
    }
    index
        .definition(&occurrence.source_path)
        .and_then(|definition| {
            page_number(
                &definition.root_page_instances,
                &occurrence.occurrence_address,
            )
        })
        .filter(|number| *number != 0)
        .unwrap_or(occurrence.index)
}

fn page_number(instances: &[crate::SchematicPageInstance], target_path: &str) -> Option<usize> {
    instances
        .iter()
        .find(|instance| normalized_instance_path(&instance.path) == target_path)
        .or_else(|| instances.first())
        .map(|instance| instance.page_number)
}

fn normalized_instance_path(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() { "/" } else { trimmed }
}
