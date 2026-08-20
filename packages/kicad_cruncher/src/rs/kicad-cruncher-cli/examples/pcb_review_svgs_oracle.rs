use std::path::PathBuf;

use kicad_cruncher_cli::design::{build_board_plot_document, load_design_sources};
use kicad_cruncher_cli::pcb_review_svg::build_pcb_review_svgs;
use serde::Serialize;

#[derive(Serialize)]
struct Output<'a> {
    layer: &'a str,
    included_layers: &'a [String],
    drill_slot_record_count: usize,
    svg: &'a str,
    viewport_bounds_nm: [i64; 4],
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("expected one KiCad project path")?;
    let loaded = load_design_sources(&input)?;
    let Some(document) = build_board_plot_document(&loaded)? else {
        println!("[]");
        return Ok(());
    };
    let artifacts = build_pcb_review_svgs(&loaded, &document)?;
    let output = artifacts
        .iter()
        .map(|artifact| Output {
            layer: &artifact.layer,
            included_layers: &artifact.included_layers,
            drill_slot_record_count: artifact.drill_slot_record_count,
            svg: &artifact.svg,
            viewport_bounds_nm: artifact.viewport_bounds_nm,
        })
        .collect::<Vec<_>>();
    serde_json::to_writer(std::io::stdout().lock(), &output)?;
    Ok(())
}
