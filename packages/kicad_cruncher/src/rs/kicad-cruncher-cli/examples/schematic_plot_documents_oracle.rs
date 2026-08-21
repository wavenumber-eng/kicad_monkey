use kicad_cruncher_cli::design::{
    build_schematic_plot_documents, build_structured_design_facts, load_design_sources,
};
use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let input = arguments.next().map(PathBuf::from).ok_or(
        "usage: schematic_plot_documents_oracle <design.kicad_pro|design.kicad_sch> [--first]",
    )?;
    let first_only = match arguments.next() {
        None => false,
        Some(value) if value == "--first" => true,
        Some(_) => return Err("schematic plot oracle received an unknown argument".into()),
    };
    if arguments.next().is_some() {
        return Err("schematic plot oracle received too many arguments".into());
    }
    let loaded = load_design_sources(&input)?;
    let facts = build_structured_design_facts(&loaded)?;
    let instances = if first_only {
        &facts.schematic_instances[..facts.schematic_instances.len().min(1)]
    } else {
        &facts.schematic_instances
    };
    let documents = build_schematic_plot_documents(&loaded, instances)?;
    serde_json::to_writer(std::io::stdout().lock(), &documents)?;
    Ok(())
}
