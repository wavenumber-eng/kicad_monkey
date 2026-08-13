//! Native all-corpus PCB owned read/write/reparse gate.

use kicad_monkey_core::{Error, PcbCounts, PcbDocument, PcbLimits, PcbView};
use serde::Serialize;
use std::io::{BufReader, Cursor};
use std::path::PathBuf;

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
    let paths = std::env::args_os().skip(1).map(PathBuf::from);
    let mut files = Vec::new();
    let mut source_bytes = 0usize;
    for path in paths {
        let limits = PcbLimits::default();
        let document =
            PcbDocument::from_reader(BufReader::new(std::fs::File::open(&path)?), limits)?;
        let file_source_bytes = document.source().len();
        let counts = validate_promoted_model(&document.view()?)?;

        let mut first_write = Vec::new();
        document.write_to(&mut first_write)?;
        if first_write != document.source().as_bytes() {
            return Err(format!("first owned write changed {}", path.display()).into());
        }
        drop(document);
        let reparsed = PcbDocument::from_reader(Cursor::new(&first_write), limits)?;
        drop(first_write);
        let second_counts = validate_promoted_model(&reparsed.view()?)?;
        if second_counts != counts {
            return Err(format!("semantic counts changed for {}", path.display()).into());
        }
        let mut second_write = Vec::new();
        reparsed.write_to(&mut second_write)?;
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
