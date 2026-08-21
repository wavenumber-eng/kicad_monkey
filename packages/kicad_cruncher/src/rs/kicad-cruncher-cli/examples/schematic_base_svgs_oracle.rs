use std::env;
use std::path::PathBuf;

use kicad_cruncher_cli::design::{
    build_schematic_base_svgs, build_schematic_plot_documents, build_structured_design_facts,
    load_design_sources,
};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args_os().skip(1);
    let input = args
        .next()
        .map(PathBuf::from)
        .ok_or("usage: schematic_base_svgs_oracle <design.kicad_pro|design.kicad_sch> [--first]")?;
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
    let documents = build_schematic_plot_documents(&loaded, instances)?;
    let artifacts = build_schematic_base_svgs(&documents)?;
    let payload = artifacts
        .iter()
        .map(|artifact| {
            json!({
                "document_id": artifact.document_id,
                "metrics": {
                    "records": artifact.metrics.records,
                    "operations": artifact.metrics.operations,
                    "points": artifact.metrics.points,
                    "svg_elements": artifact.metrics.svg_elements,
                    "svg_bytes": artifact.metrics.svg_bytes,
                },
                "svg": artifact.svg,
            })
        })
        .collect::<Vec<_>>();
    serde_json::to_writer(std::io::stdout().lock(), &payload)?;
    Ok(())
}
