use super::KiCadDesignJsonPaths;
use crate::{KiCadNetlist, SchematicBundleIndex, SchematicOccurrence, SchematicSheet};
use serde_json::{Value, json};
use std::collections::HashMap;

pub(super) fn sheets_json(
    index: &SchematicBundleIndex,
    netlist: &KiCadNetlist,
    paths: &KiCadDesignJsonPaths,
) -> Value {
    let sources = index
        .occurrences()
        .map(|occurrence| {
            (
                occurrence.human_address.as_str(),
                occurrence.source_path.as_str(),
            )
        })
        .collect::<HashMap<_, _>>();
    Value::Array(
        netlist
            .sheets
            .iter()
            .map(|sheet| {
                let source = sources
                    .get(sheet.name.as_str())
                    .and_then(|source| paths.schematic_paths.get(*source));
                json!({
                    "filename": source.map_or("", |source| source.filename.as_str()),
                    "path": source.map_or("", |source| source.path.as_str()),
                    "sheet_number": sheet.number,
                    "sheet_path": sheet.name,
                    "sheet_path_uuids": sheet.tstamps,
                    "title": sheet.title,
                    "company": sheet.company,
                    "revision": sheet.revision,
                    "date": sheet.date,
                })
            })
            .collect(),
    )
}

pub(super) fn schematic_hierarchy_json(
    index: &SchematicBundleIndex,
    paths: &KiCadDesignJsonPaths,
) -> Value {
    let occurrences = index.occurrences().collect::<Vec<_>>();
    let children = occurrences
        .iter()
        .filter_map(|occurrence| {
            Some((
                (occurrence.parent_index?, occurrence.parent_sheet_index?),
                *occurrence,
            ))
        })
        .collect::<HashMap<_, _>>();
    let documents = occurrences
        .iter()
        .enumerate()
        .map(|(position, occurrence)| document_json(index, occurrence, position + 1, paths))
        .collect::<Vec<_>>();
    let mut sheet_symbols = Vec::new();
    let mut links = Vec::new();
    let mut unresolved = Vec::new();
    for occurrence in occurrences {
        let Some(definition) = index.definition(&occurrence.source_path) else {
            continue;
        };
        for (sheet_index, sheet) in definition.sheets.iter().enumerate() {
            let child = children.get(&(occurrence.index, sheet_index));
            let child_path = child.map_or_else(
                || join_sheet_path(&occurrence.human_address, sheet_name(sheet)),
                |child| child.human_address.clone(),
            );
            let child_uuid_path = child.map_or_else(
                || join_sheet_path(&occurrence.legacy_address, sheet_uuid(sheet)),
                |child| child.legacy_address.clone(),
            );
            let row = sheet_symbol_json(
                sheet,
                &occurrence.human_address,
                &child_path,
                &child_uuid_path,
            );
            sheet_symbols.push(row.clone());
            if child.is_some() {
                links.push(json!({
                    "parent_sheet_path": occurrence.human_address,
                    "sheet_symbol_uid": sheet.uuid,
                    "child_sheet_path": child_path,
                    "child_filename": sheet.sheet_file,
                }));
            } else {
                unresolved.push(row);
            }
        }
    }
    json!({
        "schema": "kicad_monkey.schematic_hierarchy.a0",
        "requested_scope": "KICAD_PROJECT",
        "effective_scope": if sheet_symbols.is_empty() { "GLOBAL" } else { "HIERARCHICAL" },
        "documents": documents,
        "sheet_symbols": sheet_symbols,
        "hierarchy_paths": [],
        "channels": [],
        "links": links,
        "unresolved": unresolved,
    })
}

fn document_json(
    index: &SchematicBundleIndex,
    occurrence: &SchematicOccurrence,
    sheet_index: usize,
    paths: &KiCadDesignJsonPaths,
) -> Value {
    let source = paths.schematic_paths.get(&occurrence.source_path);
    let definition = index.definition(&occurrence.source_path);
    json!({
        "sheet_index": sheet_index,
        "filename": source.map_or("", |source| source.filename.as_str()),
        "path": source.map_or("", |source| source.path.as_str()),
        "is_top_level": occurrence.parent_index.is_none(),
        "sheet_path": occurrence.human_address,
        "sheet_path_uuids": occurrence.legacy_address,
        "metadata": {
            "uuid": definition.and_then(|definition| definition.uuid.as_deref()).unwrap_or(""),
            "version": definition.and_then(|definition| definition.version.as_deref())
                .and_then(|version| version.parse::<i64>().ok()).unwrap_or(0),
            "generator": definition.and_then(|definition| definition.generator.as_deref()).unwrap_or(""),
            "generator_version": definition.and_then(|definition| definition.generator_version.as_deref()).unwrap_or(""),
        },
    })
}

fn sheet_symbol_json(
    sheet: &SchematicSheet,
    source_sheet_path: &str,
    child_sheet_path: &str,
    child_sheet_path_uuids: &str,
) -> Value {
    json!({
        "uid": sheet.uuid,
        "name": sheet.sheet_name,
        "child_filename": sheet.sheet_file,
        "source_sheet_path": source_sheet_path,
        "child_sheet_path": child_sheet_path,
        "child_sheet_path_uuids": child_sheet_path_uuids,
        "entries": sheet.pins.iter().map(|pin| json!({
            "name": pin.name,
            "uid": pin.uuid,
            "shape": pin.shape.as_str(),
        })).collect::<Vec<_>>(),
    })
}

fn sheet_name(sheet: &SchematicSheet) -> &str {
    if sheet.sheet_name.is_empty() {
        &sheet.sheet_file
    } else {
        &sheet.sheet_name
    }
}

fn sheet_uuid(sheet: &SchematicSheet) -> &str {
    if sheet.uuid.is_empty() {
        &sheet.sheet_file
    } else {
        &sheet.uuid
    }
}

fn join_sheet_path(parent: &str, child: &str) -> String {
    let mut result = if parent.is_empty() {
        "/".to_owned()
    } else if parent.ends_with('/') {
        parent.to_owned()
    } else {
        format!("{parent}/")
    };
    let child = child.trim_matches('/');
    if !child.is_empty() {
        result.push_str(child);
        result.push('/');
    }
    result
}
