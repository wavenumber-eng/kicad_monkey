//! Native JSON summary used by Rack to compare the Rust PCB view to Python.

use kicad_monkey_core::{PcbCounts, PcbGraphicKind, PcbLimits, PcbView};
use serde_json::{Value, json};
use std::env;
use std::fs;
use std::process::ExitCode;

fn summarize(path: &str) -> Result<Value, Box<dyn std::error::Error>> {
    let source = fs::read_to_string(path)?;
    let view = PcbView::parse(&source, PcbLimits::default())?;
    force_decode(&view)?;
    let mut summary = json!({
        "path": path,
        "source_bytes": source.len(),
        "counts": count_summary(view.counts()),
    });
    let object = summary.as_object_mut().expect("summary is an object");
    object.extend(
        native_summary(&view)?
            .as_object()
            .expect("native summary")
            .clone(),
    );
    object.extend(
        extended_summary(&view)?
            .as_object()
            .expect("extended summary")
            .clone(),
    );
    object.extend(
        zone_summary(&view)?
            .as_object()
            .expect("zone summary")
            .clone(),
    );
    Ok(summary)
}

fn force_decode(view: &PcbView<'_>) -> Result<(), kicad_monkey_core::Error> {
    view.layers().collect::<Result<Vec<_>, _>>()?;
    view.nets().collect::<Result<Vec<_>, _>>()?;
    view.properties().collect::<Result<Vec<_>, _>>()?;
    view.footprints().collect::<Result<Vec<_>, _>>()?;
    view.pads().collect::<Result<Vec<_>, _>>()?;
    view.models().collect::<Result<Vec<_>, _>>()?;
    view.segments().collect::<Result<Vec<_>, _>>()?;
    view.vias().collect::<Result<Vec<_>, _>>()?;
    view.zones().collect::<Result<Vec<_>, _>>()?;
    view.graphics().collect::<Result<Vec<_>, _>>()?;
    view.arcs().collect::<Result<Vec<_>, _>>()?;
    view.dimensions().collect::<Result<Vec<_>, _>>()?;
    view.groups().collect::<Result<Vec<_>, _>>()?;
    view.generated_items().collect::<Result<Vec<_>, _>>()?;
    view.embedded_files().collect::<Result<Vec<_>, _>>()?;
    view.variants().collect::<Result<Vec<_>, _>>()?;
    view.images().collect::<Result<Vec<_>, _>>()?;
    view.barcodes().collect::<Result<Vec<_>, _>>()?;
    view.tables().collect::<Result<Vec<_>, _>>()?;
    view.table_cells().collect::<Result<Vec<_>, _>>()?;
    Ok(())
}

fn native_summary(view: &PcbView<'_>) -> Result<Value, kicad_monkey_core::Error> {
    let graphics = view.graphics().collect::<Result<Vec<_>, _>>()?;
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
    Ok(json!({
        "first_footprint": footprint.map(|item| json!({
            "library_link": item.library_link, "reference": item.reference,
        })),
        "first_pad": pad.map(|item| json!({
            "number": item.number, "kind": item.kind, "shape": item.shape,
            "at_x": item.at_x, "at_y": item.at_y, "size_x": item.size_x,
            "size_y": item.size_y, "layers": item.layers,
            "net": {"ordinal": item.net.ordinal, "name": item.net.name},
        })),
        "first_model": model.map(|item| json!({
            "path": item.path, "offset": item.offset, "scale": item.scale, "rotate": item.rotate,
        })),
        "first_segment": segment.map(|item| json!({
            "start_x": item.start_x, "end_x": item.end_x,
            "net": {"ordinal": item.net.ordinal, "name": item.net.name},
        })),
        "first_via": via.map(|item| json!({
            "at_x": item.at_x, "at_y": item.at_y,
            "net": {"ordinal": item.net.ordinal, "name": item.net.name},
        })),
        "first_graphic": graphics.first().map(|item| json!({
            "kind": graphic_kind_name(item.kind), "text": item.text, "layer": item.layer,
        })),
        "first_arc": arc.map(|item| json!({
            "start_x": item.start.x, "mid_x": item.mid.x, "end_x": item.end.x,
            "net": {"ordinal": item.net.ordinal, "name": item.net.name},
        })),
        "first_dimension": dimension.map(|item| json!({
            "kind": item.kind, "layer": item.layer,
            "point_count": item.points.len(), "uuid": item.uuid,
        })),
        "first_group": group.map(|item| json!({
            "name": item.name, "uuid": item.uuid, "member_count": item.members.len(),
        })),
        "first_generated": generated.map(|item| json!({
            "kind": item.kind, "name": item.name, "uuid": item.uuid,
            "member_count": item.members.len(),
        })),
        "first_embedded_file": embedded_file.map(|item| json!({
            "name": item.name, "file_type": item.file_type, "checksum": item.checksum,
            "encoded_data_bytes": item.encoded_data_bytes,
        })),
    }))
}

fn extended_summary(view: &PcbView<'_>) -> Result<Value, kicad_monkey_core::Error> {
    let metadata = view.metadata()?;
    let variant = view.variants().next().transpose()?;
    let image = view.images().next().transpose()?;
    let barcode = view.barcodes().next().transpose()?;
    let table = view.tables().next().transpose()?;
    let table_cell = view.table_cells().next().transpose()?;
    Ok(json!({
        "metadata": {
            "version": metadata.version,
            "generator": metadata.generator,
            "generator_version": metadata.generator_version,
            "paper": metadata.paper,
            "thickness": metadata.thickness,
            "legacy_teardrops": metadata.legacy_teardrops,
            "embedded_fonts": metadata.embedded_fonts,
            "pad_to_mask_clearance": metadata.pad_to_mask_clearance,
            "pad_to_paste_clearance": metadata.pad_to_paste_clearance,
            "pad_to_paste_clearance_ratio": metadata.pad_to_paste_clearance_ratio,
        },
        "first_variant": variant.map(|item| json!({
            "name": item.name,
            "description": item.description,
        })),
        "first_image": image.map(|item| json!({
            "at_x": item.at.x,
            "at_y": item.at.y,
            "scale": item.scale,
            "layer": item.layer,
            "locked": item.locked,
            "encoded_data_bytes": item.encoded_data_bytes,
            "uuid": item.uuid,
        })),
        "first_barcode": barcode.map(|item| json!({
            "at_x": item.at.x,
            "at_y": item.at.y,
            "angle": item.angle,
            "layer": item.layer,
            "width": item.width,
            "height": item.height,
            "text": item.text,
            "text_height": item.text_height,
            "kind": item.kind,
            "ecc_level": item.ecc_level,
            "locked": item.locked,
            "show_text": item.show_text,
            "knockout": item.knockout,
            "margin_x": item.margins.x,
            "margin_y": item.margins.y,
            "uuid": item.uuid,
        })),
        "first_table": table.map(|item| json!({
            "column_count": item.column_count,
            "layer": item.layer,
            "border_external": item.border_external,
            "border_header": item.border_header,
            "separator_rows": item.separator_rows,
            "separator_columns": item.separator_columns,
            "column_widths": item.column_widths,
            "row_heights": item.row_heights,
            "cell_count": item.cell_count,
            "uuid": item.uuid,
        })),
        "first_table_cell": table_cell.map(|item| json!({
            "table_index": item.table_index,
            "text": item.text,
            "start_x": item.start.x,
            "start_y": item.start.y,
            "end_x": item.end.x,
            "end_y": item.end.y,
            "margins": item.margins,
            "column_span": item.column_span,
            "row_span": item.row_span,
            "angle": item.angle,
            "layer": item.layer,
            "locked": item.locked,
            "uuid": item.uuid,
        })),
    }))
}

fn zone_summary(view: &PcbView<'_>) -> Result<Value, kicad_monkey_core::Error> {
    let zones = view.zones().collect::<Result<Vec<_>, _>>()?;
    let authored_polygons = zones.iter().map(|zone| zone.polygons.len()).sum::<usize>();
    let filled_polygons = zones
        .iter()
        .map(|zone| zone.filled_polygons.len())
        .sum::<usize>();
    let authored_points = zones
        .iter()
        .flat_map(|zone| &zone.polygons)
        .map(|polygon| polygon.points.len())
        .sum::<usize>();
    let filled_points = zones
        .iter()
        .flat_map(|zone| &zone.filled_polygons)
        .map(|polygon| polygon.points.len())
        .sum::<usize>();
    Ok(json!({
        "zone_metrics": {
            "authored_polygons": authored_polygons,
            "filled_polygons": filled_polygons,
            "authored_points": authored_points,
            "filled_points": filled_points,
            "keepouts": zones.iter().filter(|zone| zone.keepout.is_some()).count(),
            "placements": zones.iter().filter(|zone| zone.placement.is_some()).count(),
            "layer_properties": zones.iter().map(|zone| zone.layer_properties.len()).sum::<usize>(),
        },
        "first_zone": zones.first().map(|zone| json!({
            "net": {"ordinal": zone.net.ordinal, "name": zone.net.name},
            "has_explicit_net_name": zone.has_explicit_net_name,
            "layers": zone.layers,
            "layers_plural": zone.layers_plural,
            "locked": zone.locked,
            "uuid": zone.uuid,
            "name": zone.name,
            "hatch_style": zone.hatch_style,
            "hatch_pitch": zone.hatch_pitch,
            "priority": zone.priority,
            "connect_pads_clearance": zone.connect_pads_clearance,
            "min_thickness": zone.min_thickness,
            "filled_areas_thickness": zone.filled_areas_thickness,
            "fill_enabled": zone.fill_enabled,
            "thermal_gap": zone.thermal_gap,
            "thermal_bridge_width": zone.thermal_bridge_width,
            "island_removal_mode": zone.island_removal_mode,
            "island_area_min": zone.island_area_min,
            "keepout": zone.keepout.as_ref().map(|keepout| json!({
                "tracks": keepout.tracks, "vias": keepout.vias, "pads": keepout.pads,
                "copperpour": keepout.copperpour, "footprints": keepout.footprints,
            })),
            "placement": zone.placement.as_ref().map(|placement| json!({
                "enabled": placement.enabled,
                "source_type": placement.source_type.as_str(),
                "source": placement.source,
            })),
            "first_layer_property": zone.layer_properties.first().map(|property| json!({
                "layer": property.layer,
                "hatch_offset": [property.hatch_offset.x, property.hatch_offset.y],
            })),
            "first_authored_points": zone.polygons.first().map(|polygon| polygon.points
                .iter().map(|point| [point.x, point.y]).collect::<Vec<_>>()),
            "first_filled": zone.filled_polygons.first().map(|polygon| json!({
                "layer": polygon.layer,
                "island": polygon.island,
                "points": polygon.points.iter().map(|point| [point.x, point.y]).collect::<Vec<_>>(),
            })),
        })),
    }))
}

fn count_summary(counts: PcbCounts) -> Value {
    json!({
        "layers": counts.layers,
        "nets": counts.nets,
        "properties": counts.properties,
        "variants": counts.variants,
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
        "table_cells": counts.table_cells,
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
