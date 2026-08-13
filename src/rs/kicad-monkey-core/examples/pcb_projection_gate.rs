//! Native JSON summary used by Rack to compare the Rust PCB view to Python.

use kicad_monkey_core::{PcbCounts, PcbGraphicKind, PcbLimits, PcbView};
use serde_json::{Value, json};
use std::env;
use std::fs;
use std::process::ExitCode;

fn summarize(path: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    let view = PcbView::parse(&source, PcbLimits::default())?;
    let counts = view.counts();
    let footprint = view.footprints().next().transpose()?;
    let pad = view.pads().next().transpose()?;
    let model = view.models().next().transpose()?;
    let segment = view.segments().next().transpose()?;
    let via = view.vias().next().transpose()?;
    let arc = view.arcs().next().transpose()?;
    let dimension = view.dimensions().next().transpose()?;
    let group = view.groups().next().transpose()?;
    let generated = view.generated_items().next().transpose()?;
    let embedded_file = view.embedded_files().next().transpose()?;
    // Force every promoted iterator to decode so lazy failures fail the gate.
    view.layers().collect::<Result<Vec<_>, _>>()?;
    view.nets().collect::<Result<Vec<_>, _>>()?;
    view.properties().collect::<Result<Vec<_>, _>>()?;
    view.footprints().collect::<Result<Vec<_>, _>>()?;
    view.pads().collect::<Result<Vec<_>, _>>()?;
    view.models().collect::<Result<Vec<_>, _>>()?;
    view.segments().collect::<Result<Vec<_>, _>>()?;
    view.vias().collect::<Result<Vec<_>, _>>()?;
    view.zones().collect::<Result<Vec<_>, _>>()?;
    let graphics = view.graphics().collect::<Result<Vec<_>, _>>()?;
    view.arcs().collect::<Result<Vec<_>, _>>()?;
    view.dimensions().collect::<Result<Vec<_>, _>>()?;
    view.groups().collect::<Result<Vec<_>, _>>()?;
    view.generated_items().collect::<Result<Vec<_>, _>>()?;
    view.embedded_files().collect::<Result<Vec<_>, _>>()?;

    Ok(json!({
        "path": path,
        "source_bytes": source.len(),
        "counts": count_summary(counts),
        "first_footprint": footprint.map(|item| json!({
            "library_link": item.library_link,
            "reference": item.reference,
        })),
        "first_pad": pad.map(|item| json!({
            "number": item.number,
            "kind": item.kind,
            "shape": item.shape,
            "at_x": item.at_x,
            "at_y": item.at_y,
            "size_x": item.size_x,
            "size_y": item.size_y,
            "layers": item.layers,
            "net": {"ordinal": item.net.ordinal, "name": item.net.name},
        })),
        "first_model": model.map(|item| json!({
            "path": item.path,
            "offset": item.offset,
            "scale": item.scale,
            "rotate": item.rotate,
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
        "first_graphic": graphics.first().map(|item| json!({
            "kind": graphic_kind_name(item.kind),
            "text": item.text,
            "layer": item.layer,
        })),
        "first_arc": arc.map(|item| json!({
            "start_x": item.start.x,
            "mid_x": item.mid.x,
            "end_x": item.end.x,
            "net": {"ordinal": item.net.ordinal, "name": item.net.name},
        })),
        "first_dimension": dimension.map(|item| json!({
            "kind": item.kind,
            "layer": item.layer,
            "point_count": item.points.len(),
            "uuid": item.uuid,
        })),
        "first_group": group.map(|item| json!({
            "name": item.name,
            "uuid": item.uuid,
            "member_count": item.members.len(),
        })),
        "first_generated": generated.map(|item| json!({
            "kind": item.kind,
            "name": item.name,
            "uuid": item.uuid,
            "member_count": item.members.len(),
        })),
        "first_embedded_file": embedded_file.map(|item| json!({
            "name": item.name,
            "file_type": item.file_type,
            "checksum": item.checksum,
            "encoded_data_bytes": item.encoded_data_bytes,
        })),
    }))
}

fn count_summary(counts: PcbCounts) -> Value {
    json!({
        "layers": counts.layers,
        "nets": counts.nets,
        "properties": counts.properties,
        "footprints": counts.footprints,
        "pads": counts.pads,
        "models": counts.models,
        "segments": counts.segments,
        "vias": counts.vias,
        "zones": counts.zones,
        "gr_texts": counts.gr_texts,
        "gr_lines": counts.gr_lines,
        "gr_rects": counts.gr_rects,
        "gr_arcs": counts.gr_arcs,
        "gr_circles": counts.gr_circles,
        "gr_polys": counts.gr_polys,
        "gr_curves": counts.gr_curves,
        "gr_text_boxes": counts.gr_text_boxes,
        "images": counts.images,
        "barcodes": counts.barcodes,
        "tables": counts.tables,
        "arcs": counts.arcs,
        "dimensions": counts.dimensions,
        "groups": counts.groups,
        "generated_items": counts.generated_items,
        "embedded_files": counts.embedded_files,
    })
}

fn graphic_kind_name(kind: PcbGraphicKind) -> &'static str {
    match kind {
        PcbGraphicKind::Text => "gr_text",
        PcbGraphicKind::Line => "gr_line",
        PcbGraphicKind::Rect => "gr_rect",
        PcbGraphicKind::Arc => "gr_arc",
        PcbGraphicKind::Circle => "gr_circle",
        PcbGraphicKind::Poly => "gr_poly",
        PcbGraphicKind::Curve => "gr_curve",
        PcbGraphicKind::TextBox => "gr_text_box",
    }
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
