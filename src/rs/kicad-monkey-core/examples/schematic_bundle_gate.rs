use kicad_monkey_contracts::generated::source_bundle_manifest::{
    SourceBundleManifestA0, SourceBundleSource, SourceKind,
};
use kicad_monkey_core::{
    SchematicBundleIndex, SchematicBundleLimits, SchematicDefinition, SchematicLabelScope,
    SchematicPlacedSymbol, SchematicPoint, SchematicSheet, SourceBundle, SourceBundleLimits,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::error::Error;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    bundle_root: PathBuf,
    project_path: Option<PathBuf>,
    root_schematic_path: PathBuf,
    schematic_paths: Vec<PathBuf>,
}

#[derive(Debug, Serialize)]
struct ResultSummary {
    definition_paths: Vec<String>,
    definitions: Vec<DefinitionSummary>,
    occurrences: Vec<OccurrenceSummary>,
    total_bytes: usize,
}

#[derive(Debug, Serialize)]
struct DefinitionSummary {
    source_path: String,
    sheets: Vec<SheetSummary>,
    symbols: Vec<SymbolSummary>,
    legacy_symbol_instances: Vec<LegacySymbolInstanceSummary>,
    wires: Vec<PolylineSummary>,
    buses: Vec<PolylineSummary>,
    bus_entries: Vec<BusEntrySummary>,
    junctions: Vec<MarkerSummary>,
    no_connects: Vec<MarkerSummary>,
    labels: Vec<LabelSummary>,
    connectivity_components: Vec<Vec<[i64; 2]>>,
}

#[derive(Debug, Serialize)]
struct SymbolSummary {
    lib_id: String,
    lib_name: String,
    at: [i64; 2],
    angle_degrees: f64,
    mirror: Option<String>,
    unit: i64,
    convert: i64,
    policy: [bool; 6],
    uuid: String,
    properties: Vec<[String; 2]>,
    pins: Vec<SymbolPinSummary>,
    instances: Vec<SymbolInstanceSummary>,
}

#[derive(Debug, Serialize)]
struct SymbolPinSummary {
    number: String,
    uuid: String,
    alternate: Option<String>,
}

#[derive(Debug, Serialize)]
struct SymbolInstanceSummary {
    project: String,
    path: String,
    reference: String,
    unit: i64,
    variants: Vec<SymbolVariantSummary>,
}

#[derive(Debug, Serialize)]
struct SymbolVariantSummary {
    name: String,
    policy: [Option<bool>; 5],
    fields: Vec<[String; 2]>,
}

#[derive(Debug, Serialize)]
struct LegacySymbolInstanceSummary {
    path: String,
    reference: String,
    unit: i64,
    value: String,
    footprint: String,
}

#[derive(Debug, Serialize)]
struct SheetSummary {
    uuid: String,
    pins: Vec<SheetPinSummary>,
}

#[derive(Debug, Serialize)]
struct SheetPinSummary {
    name: String,
    shape: String,
    uuid: String,
    at: [i64; 2],
}

#[derive(Debug, Serialize)]
struct PolylineSummary {
    uuid: String,
    points: Vec<[i64; 2]>,
}

#[derive(Debug, Serialize)]
struct BusEntrySummary {
    uuid: String,
    at: [i64; 2],
    size: [i64; 2],
}

#[derive(Debug, Serialize)]
struct MarkerSummary {
    uuid: String,
    at: [i64; 2],
}

#[derive(Debug, Serialize)]
struct LabelSummary {
    scope: &'static str,
    text: String,
    shape: String,
    uuid: String,
    at: [i64; 2],
}

#[derive(Debug, Serialize)]
struct OccurrenceSummary {
    source_path: String,
    parent_index: Option<usize>,
    occurrence_address: String,
    effective_in_bom: bool,
    effective_on_board: bool,
    effective_dnp: bool,
    effective_exclude_from_sim: bool,
}

struct LoadedRequest {
    manifest: SourceBundleManifestA0,
    buffers: Vec<Vec<u8>>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let input = io::stdin();
    let mut request_count = 0_usize;
    for line in input.lock().lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let request: Request = serde_json::from_str(&line)?;
        let loaded = load_request(request)?;
        let bundle = SourceBundle::from_manifest(
            loaded.manifest,
            loaded.buffers,
            SourceBundleLimits::default(),
        )?;
        let index = SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default())?;
        println!(
            "{}",
            serde_json::to_string(&ResultSummary {
                definition_paths: index
                    .definitions()
                    .map(|definition| definition.source_path.clone())
                    .collect(),
                definitions: index.definitions().map(definition_summary).collect(),
                occurrences: index
                    .occurrences()
                    .map(|occurrence| OccurrenceSummary {
                        source_path: occurrence.source_path.clone(),
                        parent_index: occurrence.parent_index,
                        occurrence_address: occurrence.occurrence_address.clone(),
                        effective_in_bom: occurrence.effective_in_bom,
                        effective_on_board: occurrence.effective_on_board,
                        effective_dnp: occurrence.effective_dnp,
                        effective_exclude_from_sim: occurrence.effective_exclude_from_sim,
                    })
                    .collect(),
                total_bytes: bundle.total_bytes(),
            })?
        );
        request_count += 1;
    }
    if request_count == 0 {
        return Err("no source bundle requests supplied".into());
    }
    Ok(())
}

fn definition_summary(definition: &SchematicDefinition) -> DefinitionSummary {
    DefinitionSummary {
        source_path: definition.source_path.clone(),
        sheets: definition.sheets.iter().map(sheet_summary).collect(),
        symbols: definition.symbols.iter().map(symbol_summary).collect(),
        legacy_symbol_instances: definition
            .legacy_symbol_instances
            .iter()
            .map(|instance| LegacySymbolInstanceSummary {
                path: instance.path.clone(),
                reference: instance.reference.clone(),
                unit: instance.unit,
                value: instance.value.clone(),
                footprint: instance.footprint.clone(),
            })
            .collect(),
        wires: definition
            .wires
            .iter()
            .map(|value| PolylineSummary {
                uuid: value.uuid.clone(),
                points: value.points.iter().copied().map(point_pair).collect(),
            })
            .collect(),
        buses: definition
            .buses
            .iter()
            .map(|value| PolylineSummary {
                uuid: value.uuid.clone(),
                points: value.points.iter().copied().map(point_pair).collect(),
            })
            .collect(),
        bus_entries: definition
            .bus_entries
            .iter()
            .map(|value| BusEntrySummary {
                uuid: value.uuid.clone(),
                at: point_pair(value.at),
                size: point_pair(value.size),
            })
            .collect(),
        junctions: definition
            .junctions
            .iter()
            .map(|value| MarkerSummary {
                uuid: value.uuid.clone(),
                at: point_pair(value.at),
            })
            .collect(),
        no_connects: definition
            .no_connects
            .iter()
            .map(|value| MarkerSummary {
                uuid: value.uuid.clone(),
                at: point_pair(value.at),
            })
            .collect(),
        labels: definition
            .labels
            .iter()
            .map(|value| LabelSummary {
                scope: match value.scope {
                    SchematicLabelScope::Local => "local",
                    SchematicLabelScope::Global => "global",
                    SchematicLabelScope::Hierarchical => "hierarchical",
                },
                text: value.text.clone(),
                shape: value.shape.clone(),
                uuid: value.uuid.clone(),
                at: point_pair(value.at),
            })
            .collect(),
        connectivity_components: definition
            .connectivity
            .components()
            .map(|component| component.iter().copied().map(point_pair).collect())
            .collect(),
    }
}

fn sheet_summary(sheet: &SchematicSheet) -> SheetSummary {
    SheetSummary {
        uuid: sheet.uuid.clone(),
        pins: sheet
            .pins
            .iter()
            .map(|pin| SheetPinSummary {
                name: pin.name.clone(),
                shape: pin.shape.as_str().to_owned(),
                uuid: pin.uuid.clone(),
                at: point_pair(pin.at),
            })
            .collect(),
    }
}

fn symbol_summary(symbol: &SchematicPlacedSymbol) -> SymbolSummary {
    SymbolSummary {
        lib_id: symbol.lib_id.clone(),
        lib_name: symbol.lib_name.clone(),
        at: point_pair(symbol.at),
        angle_degrees: symbol.angle_degrees,
        mirror: symbol.mirror.clone(),
        unit: symbol.unit,
        convert: symbol.convert,
        policy: [
            symbol.exclude_from_sim,
            symbol.in_bom,
            symbol.on_board,
            symbol.in_pos_files,
            symbol.dnp,
            symbol.fields_autoplaced,
        ],
        uuid: symbol.uuid.clone(),
        properties: symbol
            .properties
            .iter()
            .map(|property| [property.key.clone(), property.value.clone()])
            .collect(),
        pins: symbol
            .pins
            .iter()
            .map(|pin| SymbolPinSummary {
                number: pin.number.clone(),
                uuid: pin.uuid.clone(),
                alternate: pin.alternate.clone(),
            })
            .collect(),
        instances: symbol
            .instances
            .iter()
            .map(|instance| SymbolInstanceSummary {
                project: instance.project.to_string(),
                path: instance.path.clone(),
                reference: instance.reference.clone(),
                unit: instance.unit,
                variants: instance
                    .variants
                    .iter()
                    .map(|variant| SymbolVariantSummary {
                        name: variant.name.clone(),
                        policy: [
                            variant.dnp,
                            variant.exclude_from_sim,
                            variant.in_bom,
                            variant.on_board,
                            variant.in_pos_files,
                        ],
                        fields: variant
                            .fields
                            .iter()
                            .map(|field| [field.name.clone(), field.value.clone()])
                            .collect(),
                    })
                    .collect(),
            })
            .collect(),
    }
}

fn point_pair(point: SchematicPoint) -> [i64; 2] {
    [point.x_iu, point.y_iu]
}

fn load_request(request: Request) -> Result<LoadedRequest, Box<dyn Error>> {
    let mut paths = BTreeSet::new();
    paths.insert(request.root_schematic_path.clone());
    paths.extend(request.schematic_paths);
    if let Some(project_path) = request.project_path.as_ref() {
        paths.insert(project_path.clone());
    }
    let mut descriptors = Vec::with_capacity(paths.len());
    let mut buffers = Vec::with_capacity(paths.len());
    let mut project_path = None;
    let mut root_schematic_path = None;
    for path in paths {
        let relative = portable_relative(&request.bundle_root, &path)?;
        let bytes = std::fs::read(&path)?;
        let kind = if Some(&path) == request.project_path.as_ref() {
            project_path = Some(relative.clone());
            SourceKind::Project
        } else {
            if path == request.root_schematic_path {
                root_schematic_path = Some(relative.clone());
            }
            SourceKind::Schematic
        };
        let slot = u32::try_from(buffers.len())?;
        descriptors.push(SourceBundleSource {
            kind,
            path: relative,
            slot: slot.into(),
            source_bytes: bytes.len().to_string().into(),
        });
        buffers.push(bytes);
    }
    Ok(LoadedRequest {
        manifest: SourceBundleManifestA0 {
            project_path,
            root_schematic_path: root_schematic_path.ok_or("root schematic was not loaded")?,
            schema: "kicad_monkey.source_bundle_manifest.a0".to_owned(),
            sources: descriptors,
            type_: "kicad_monkey.source_bundle_manifest".to_owned(),
            version: "a0".to_owned(),
        },
        buffers,
    })
}

fn portable_relative(root: &Path, path: &Path) -> Result<String, Box<dyn Error>> {
    let relative = path.strip_prefix(root)?;
    let value = relative.to_string_lossy().replace('\\', "/");
    if value.is_empty() {
        Err("source path equals the bundle root".into())
    } else {
        Ok(value)
    }
}
