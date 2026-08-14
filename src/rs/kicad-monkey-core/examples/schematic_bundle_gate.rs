use kicad_monkey_contracts::generated::source_bundle_manifest::{
    SourceBundleManifestA0, SourceBundleSource, SourceKind,
};
use kicad_monkey_core::{
    SchematicBundleIndex, SchematicBundleLimits, SourceBundle, SourceBundleLimits,
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
    occurrences: Vec<OccurrenceSummary>,
    total_bytes: usize,
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
