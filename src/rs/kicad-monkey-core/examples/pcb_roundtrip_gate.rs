//! Native all-corpus PCB owned read/write/reparse gate.

use kicad_monkey_core::{Error, PcbCounts, PcbDocument, PcbLimits, PcbView};
use serde::{Deserialize, Serialize};
use std::io::{BufReader, Cursor, Read};
use std::path::PathBuf;

const PATH_MANIFEST_SCHEMA: &str = "kicad_monkey.pcb_roundtrip_paths.v1";

#[derive(Clone, Copy)]
struct PathManifestLimits {
    max_manifest_bytes: usize,
    max_paths: usize,
    max_path_bytes: usize,
    max_total_path_bytes: usize,
}

impl Default for PathManifestLimits {
    fn default() -> Self {
        Self {
            max_manifest_bytes: 1024 * 1024,
            max_paths: 4_096,
            max_path_bytes: 32 * 1024,
            max_total_path_bytes: 16 * 1024 * 1024,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PathManifest {
    schema: String,
    paths: Vec<String>,
}

#[derive(Serialize)]
struct FileEvidence {
    path: String,
    source_bytes: usize,
    counts: CountsEvidence,
}

#[derive(Serialize)]
struct CountsEvidence {
    footprints: usize,
    pads: usize,
    vias: usize,
    zones: usize,
    graphics: usize,
}

impl From<PcbCounts> for CountsEvidence {
    fn from(value: PcbCounts) -> Self {
        Self {
            footprints: value.footprints,
            pads: value.pads,
            vias: value.vias,
            zones: value.zones,
            graphics: value.graphics,
        }
    }
}

#[derive(Serialize)]
struct GateEvidence {
    schema: &'static str,
    file_count: usize,
    source_bytes: usize,
    semantic_decode_passes_per_file: usize,
    exact_first_writes: usize,
    stable_second_writes: usize,
    files: Vec<FileEvidence>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = path_manifest_arguments()?;
    let mut files = Vec::new();
    let mut source_bytes = 0usize;
    for path in paths {
        let limits = PcbLimits::default();
        let file = std::fs::File::open(&path).map_err(|error| stage_error(&path, "open", error))?;
        let document = PcbDocument::from_reader(BufReader::new(file), limits)
            .map_err(|error| stage_error(&path, "owned read", error))?;
        let file_source_bytes = document.source().len();
        let view = document
            .view()
            .map_err(|error| stage_error(&path, "first view", error))?;
        let counts = validate_promoted_model(&view)
            .map_err(|error| stage_error(&path, "first semantic decode", error))?;

        let mut first_write = Vec::new();
        document
            .write_to(&mut first_write)
            .map_err(|error| stage_error(&path, "first write", error))?;
        if first_write != document.source().as_bytes() {
            return Err(format!("first owned write changed {}", path.display()).into());
        }
        drop(document);
        let reparsed = PcbDocument::from_reader(Cursor::new(&first_write), limits)
            .map_err(|error| stage_error(&path, "reparse", error))?;
        drop(first_write);
        let second_view = reparsed
            .view()
            .map_err(|error| stage_error(&path, "second view", error))?;
        let second_counts = validate_promoted_model(&second_view)
            .map_err(|error| stage_error(&path, "second semantic decode", error))?;
        if second_counts != counts {
            return Err(format!("semantic counts changed for {}", path.display()).into());
        }
        let mut second_write = Vec::new();
        reparsed
            .write_to(&mut second_write)
            .map_err(|error| stage_error(&path, "second write", error))?;
        if second_write != reparsed.source().as_bytes() {
            return Err(format!("second owned write changed {}", path.display()).into());
        }

        source_bytes = source_bytes
            .checked_add(file_source_bytes)
            .ok_or("aggregate source byte count overflow")?;
        files.push(FileEvidence {
            path: path.to_string_lossy().into_owned(),
            source_bytes: file_source_bytes,
            counts: counts.into(),
        });
    }
    if files.is_empty() {
        return Err("no PCB inputs supplied".into());
    }
    let evidence = GateEvidence {
        schema: "kicad_monkey.pcb_roundtrip_evidence.a0",
        file_count: files.len(),
        source_bytes,
        semantic_decode_passes_per_file: 2,
        exact_first_writes: files.len(),
        stable_second_writes: files.len(),
        files,
    };
    println!("{}", serde_json::to_string(&evidence)?);
    Ok(())
}

fn path_manifest_arguments() -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    if arguments.next().as_deref() != Some(std::ffi::OsStr::new("--path-manifest")) {
        return Err("usage: pcb_roundtrip_gate --path-manifest <manifest.json>".into());
    }
    let manifest_path = PathBuf::from(
        arguments
            .next()
            .ok_or("missing path manifest after --path-manifest")?,
    );
    if arguments.next().is_some() {
        return Err("unexpected argument after path manifest".into());
    }
    let limits = PathManifestLimits::default();
    let file = std::fs::File::open(&manifest_path)
        .map_err(|error| stage_error(&manifest_path, "manifest open", error))?;
    let read_limit = u64::try_from(limits.max_manifest_bytes)?.saturating_add(1);
    let mut source = Vec::new();
    file.take(read_limit)
        .read_to_end(&mut source)
        .map_err(|error| stage_error(&manifest_path, "manifest read", error))?;
    decode_path_manifest(&source, limits)
        .map_err(|error| stage_error(&manifest_path, "manifest decode", error))
}

fn decode_path_manifest(
    source: &[u8],
    limits: PathManifestLimits,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    if source.len() > limits.max_manifest_bytes {
        return Err("path manifest exceeds max_manifest_bytes".into());
    }
    let manifest: PathManifest = serde_json::from_slice(source)?;
    if manifest.schema != PATH_MANIFEST_SCHEMA {
        return Err("path manifest has the wrong schema".into());
    }
    if manifest.paths.is_empty() {
        return Err("path manifest contains no paths".into());
    }
    if manifest.paths.len() > limits.max_paths {
        return Err("path manifest exceeds max_paths".into());
    }
    let mut total_path_bytes = 0usize;
    let mut paths = Vec::with_capacity(manifest.paths.len());
    for path in manifest.paths {
        if path.is_empty() || path.contains('\0') || path.len() > limits.max_path_bytes {
            return Err("path manifest contains an invalid or over-limit path".into());
        }
        total_path_bytes = total_path_bytes
            .checked_add(path.len())
            .ok_or("aggregate path byte count overflow")?;
        if total_path_bytes > limits.max_total_path_bytes {
            return Err("path manifest exceeds max_total_path_bytes".into());
        }
        paths.push(PathBuf::from(path));
    }
    Ok(paths)
}

fn stage_error(
    path: &std::path::Path,
    stage: &str,
    error: impl std::fmt::Display,
) -> Box<dyn std::error::Error> {
    format!("{}: {stage}: {error}", path.display()).into()
}

fn validate_promoted_model(view: &PcbView<'_>) -> Result<PcbCounts, Error> {
    let _ = view.paper()?;
    let _ = view.title_block()?;
    let _ = view.metadata()?;
    let _ = view.setup()?;
    exhaust(view.layers())?;
    exhaust(view.nets())?;
    exhaust(view.properties())?;
    exhaust(view.variants())?;
    exhaust(view.footprints())?;
    exhaust(view.footprint_properties())?;
    exhaust(view.footprint_graphics())?;
    exhaust(view.footprint_texts())?;
    exhaust(view.footprint_text_boxes())?;
    exhaust(view.pads())?;
    exhaust(view.models())?;
    exhaust(view.segments())?;
    exhaust(view.vias())?;
    exhaust(view.zones())?;
    exhaust(view.graphics())?;
    exhaust(view.arcs())?;
    exhaust(view.dimensions())?;
    exhaust(view.groups())?;
    exhaust(view.generated_items())?;
    exhaust(view.embedded_files())?;
    exhaust(view.images())?;
    exhaust(view.barcodes())?;
    exhaust(view.tables())?;
    exhaust(view.table_cells())?;
    exhaust(view.holes())?;
    exhaust(view.footprint_transforms())?;
    exhaust(view.profile_primitives())?;
    Ok(view.counts())
}

fn exhaust<T>(items: impl Iterator<Item = Result<T, Error>>) -> Result<(), Error> {
    for item in items {
        let _ = item?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{PathManifestLimits, decode_path_manifest};

    const VALID: &[u8] =
        br#"{"schema":"kicad_monkey.pcb_roundtrip_paths.v1","paths":["a.kicad_pcb","folder/b.kicad_pcb"]}"#;

    fn exact_limits() -> PathManifestLimits {
        PathManifestLimits {
            max_manifest_bytes: VALID.len(),
            max_paths: 2,
            max_path_bytes: "folder/b.kicad_pcb".len(),
            max_total_path_bytes: "a.kicad_pcb".len() + "folder/b.kicad_pcb".len(),
        }
    }

    #[test]
    fn path_manifest_limits_are_inclusive_and_one_under_rejects() {
        let paths = decode_path_manifest(VALID, exact_limits()).expect("exact ceilings");
        assert_eq!(paths.len(), 2);

        for limits in [
            PathManifestLimits {
                max_manifest_bytes: VALID.len() - 1,
                ..exact_limits()
            },
            PathManifestLimits {
                max_paths: 1,
                ..exact_limits()
            },
            PathManifestLimits {
                max_path_bytes: "folder/b.kicad_pcb".len() - 1,
                ..exact_limits()
            },
            PathManifestLimits {
                max_total_path_bytes: exact_limits().max_total_path_bytes - 1,
                ..exact_limits()
            },
        ] {
            assert!(decode_path_manifest(VALID, limits).is_err());
        }
    }

    #[test]
    fn path_manifest_rejects_malformed_or_ambiguous_inputs() {
        for source in [
            br#"{"schema":"wrong","paths":["a.kicad_pcb"]}"#.as_slice(),
            br#"{"schema":"kicad_monkey.pcb_roundtrip_paths.v1","paths":[]}"#,
            br#"{"schema":"kicad_monkey.pcb_roundtrip_paths.v1","paths":[""]}"#,
            br#"{"schema":"kicad_monkey.pcb_roundtrip_paths.v1","paths":["a\u0000b"]}"#,
            br#"{"schema":"kicad_monkey.pcb_roundtrip_paths.v1","paths":["a"],"extra":true}"#,
            br#"not-json"#,
        ] {
            assert!(decode_path_manifest(source, PathManifestLimits::default()).is_err());
        }
    }
}
