//! Native JSON summary used by Rack to compare the Rust PCB view to Python.

use kicad_monkey_core::{PcbLimits, PcbView};
use serde_json::{Value, json};
use std::env;
use std::fs;
use std::process::ExitCode;

fn summarize(path: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    let view = PcbView::parse(&source, PcbLimits::default())?;
    let counts = view.counts();
    let footprint = view.footprints().next().transpose()?;
    let segment = view.segments().next().transpose()?;
    let via = view.vias().next().transpose()?;
    // Force every promoted iterator to decode so lazy failures fail the gate.
    view.layers().collect::<Result<Vec<_>, _>>()?;
    view.nets().collect::<Result<Vec<_>, _>>()?;
    view.properties().collect::<Result<Vec<_>, _>>()?;
    view.footprints().collect::<Result<Vec<_>, _>>()?;
    view.segments().collect::<Result<Vec<_>, _>>()?;
    view.vias().collect::<Result<Vec<_>, _>>()?;
    view.zones().collect::<Result<Vec<_>, _>>()?;

    Ok(json!({
        "path": path,
        "source_bytes": source.len(),
        "counts": {
            "layers": counts.layers,
            "nets": counts.nets,
            "properties": counts.properties,
            "footprints": counts.footprints,
            "pads": counts.pads,
            "models": counts.models,
            "segments": counts.segments,
            "vias": counts.vias,
            "zones": counts.zones,
        },
        "first_footprint": footprint.map(|item| json!({
            "library_link": item.library_link,
            "reference": item.reference,
        })),
        "first_segment": segment.map(|item| json!({
            "start_x": item.start_x,
            "end_x": item.end_x,
            "net": {"ordinal": item.net.ordinal, "name": item.net.name},
        })),
        "first_via": via.map(|item| json!({
            "at_x": item.at_x,
            "at_y": item.at_y,
            "net": {"ordinal": item.net.ordinal, "name": item.net.name},
        })),
    }))
}

fn main() -> ExitCode {
    let paths: Vec<_> = env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: pcb_projection_gate <board.kicad_pcb> [...]");
        return ExitCode::FAILURE;
    }
    let summaries = paths
        .iter()
        .map(|path| summarize(path))
        .collect::<Result<Vec<_>, _>>();
    match summaries {
        Ok(values) => match serde_json::to_string(&values) {
            Ok(output) => {
                println!("{output}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("failed to serialize summaries: {error}");
                ExitCode::FAILURE
            }
        },
        Err(error) => {
            eprintln!("PCB projection failed: {error}");
            ExitCode::FAILURE
        }
    }
}
