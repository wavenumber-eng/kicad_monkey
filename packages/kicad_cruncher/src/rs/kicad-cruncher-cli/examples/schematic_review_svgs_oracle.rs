use std::env;
use std::path::PathBuf;

use kicad_cruncher_cli::design::{
    build_schematic_base_svgs_for_plot_documents, build_schematic_plot_document_artifacts,
    build_structured_design_facts, load_design_sources,
};
use kicad_cruncher_cli::schematic_review_svg::build_schematic_review_svgs;
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let input = args.next().map(PathBuf::from).ok_or(
        "usage: schematic_review_svgs_oracle <design.kicad_pro|design.kicad_sch> [--first]",
    )?;
    let first = match args.next() {
        None => false,
        Some(value) if value == "--first" => true,
        Some(_) => return Err("only --first may follow the input path".into()),
    };
    if args.next().is_some() {
        return Err("too many arguments".into());
    }
    let loaded = load_design_sources(&input)?;
    let facts = build_structured_design_facts(&loaded)?;
    let instances = if first {
        &facts.schematic_instances[..1]
    } else {
        &facts.schematic_instances
    };
    let documents = build_schematic_plot_document_artifacts(&loaded, instances)?;
    let base = build_schematic_base_svgs_for_plot_documents(&documents)?;
    let review = build_schematic_review_svgs(
        &documents,
        &base,
        &facts.compiled_schematic_graph,
        &facts.design_json,
        "../compiled_schematic_graph.json",
    )?;
    let payload = review
        .iter()
        .map(|artifact| {
            json!({
                "document_id": artifact.document_id,
                "page_occurrence_ref": artifact.page_occurrence_ref,
                "artifact_key": artifact.artifact_key,
                "graph_link_count": artifact.graph_link_count,
                "resolved_svg_identity_count": artifact.resolved_svg_identity_count,
                "svg": artifact.svg,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_writer(std::io::stdout().lock(), &payload)?;
    Ok(())
}
