//! Native all-corpus schematic owned read/write/reparse gate.

use kicad_monkey_core::{SchematicDocument, SchematicDocumentLimits};
use serde::Serialize;
use std::io::{BufReader, Cursor};
use std::path::PathBuf;

#[derive(Serialize)]
struct FileEvidence {
    path: String,
    source_bytes: usize,
    symbols: usize,
    sheets: usize,
    connectivity_objects: usize,
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
    let mut files = Vec::new();
    let mut source_bytes = 0usize;
    for path in std::env::args_os().skip(1).map(PathBuf::from) {
        let limits = SchematicDocumentLimits::default();
        let source_path = path.to_string_lossy();
        let file = std::fs::File::open(&path).map_err(|error| stage_error(&path, "open", error))?;
        let document = SchematicDocument::from_named_reader(
            source_path.as_ref(),
            BufReader::new(file),
            limits,
        )
        .map_err(|error| stage_error(&path, "owned read", error))?;
        let definition = document
            .definition()
            .map_err(|error| stage_error(&path, "first semantic decode", error))?;
        let file_source_bytes = document.source().len();

        let mut first_write = Vec::new();
        document
            .write_to(&mut first_write)
            .map_err(|error| stage_error(&path, "first write", error))?;
        if first_write != document.source().as_bytes() {
            return Err(format!("first owned write changed {}", path.display()).into());
        }
        drop(document);

        let reparsed = SchematicDocument::from_named_reader(
            source_path.as_ref(),
            Cursor::new(&first_write),
            limits,
        )
        .map_err(|error| stage_error(&path, "reparse", error))?;
        drop(first_write);
        let second_definition = reparsed
            .definition()
            .map_err(|error| stage_error(&path, "second semantic decode", error))?;
        if second_definition != definition {
            return Err(format!(
                "promoted schematic semantics changed for {}",
                path.display()
            )
            .into());
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
            symbols: definition.symbols.len(),
            sheets: definition.sheets.len(),
            connectivity_objects: definition.connectivity.points().len(),
        });
    }
    if files.is_empty() {
        return Err("no schematic inputs supplied".into());
    }
    let evidence = GateEvidence {
        schema: "kicad_monkey.schematic_roundtrip_evidence.a0",
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

fn stage_error(
    path: &std::path::Path,
    stage: &str,
    error: impl std::fmt::Display,
) -> Box<dyn std::error::Error> {
    format!("{}: {stage}: {error}", path.display()).into()
}
