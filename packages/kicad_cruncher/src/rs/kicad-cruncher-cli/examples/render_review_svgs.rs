//! Render the actual Cruncher PCB and schematic review artifacts for browser tests.

use std::path::PathBuf;

use kicad_cruncher_cli::design::{
    build_board_plot_document, build_schematic_base_svgs_for_plot_documents,
    build_schematic_plot_document_artifacts, build_structured_design_facts, load_design_sources,
};
use kicad_cruncher_cli::pcb_review_svg::build_pcb_review_svgs;
use kicad_cruncher_cli::schematic_review_svg::build_schematic_review_svgs;

fn run() -> Result<(), String> {
    let project = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or_else(|| "usage: render_review_svgs <project.kicad_pro> <output-dir>".to_owned())?;
    let output = std::env::args()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "usage: render_review_svgs <project.kicad_pro> <output-dir>".to_owned())?;
    std::fs::create_dir_all(&output)
        .map_err(|error| format!("create {}: {error}", output.display()))?;

    let loaded = load_design_sources(&project).map_err(|error| error.to_string())?;
    let facts = build_structured_design_facts(&loaded).map_err(|error| error.to_string())?;
    let board = build_board_plot_document(&loaded)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "test project has no board".to_owned())?;
    let pcb = build_pcb_review_svgs(&loaded, &board).map_err(|error| error.to_string())?;
    let pcb = pcb
        .first()
        .ok_or_else(|| "test project produced no PCB review SVG".to_owned())?;
    std::fs::write(output.join("pcb-review.svg"), pcb.svg.as_bytes())
        .map_err(|error| format!("write PCB review SVG: {error}"))?;

    let documents = build_schematic_plot_document_artifacts(&loaded, &facts.schematic_instances)
        .map_err(|error| error.to_string())?;
    let base = build_schematic_base_svgs_for_plot_documents(&documents)
        .map_err(|error| error.to_string())?;
    let schematic = build_schematic_review_svgs(
        &documents,
        &base,
        &facts.compiled_schematic_graph,
        &facts.design_json,
        "../compiled_schematic_graph.json",
    )
    .map_err(|error| error.to_string())?;
    let schematic = schematic
        .first()
        .ok_or_else(|| "test project produced no schematic review SVG".to_owned())?;
    std::fs::write(
        output.join("schematic-review.svg"),
        schematic.svg.as_bytes(),
    )
    .map_err(|error| format!("write schematic review SVG: {error}"))?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
