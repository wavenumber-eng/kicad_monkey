use kicad_cruncher_cli::design::{build_structured_design_facts, load_design_sources};
use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let input = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: schematic_instances_oracle <design.kicad_pro|design.kicad_sch>")?;
    let loaded = load_design_sources(&input)?;
    let facts = build_structured_design_facts(&loaded)?;
    serde_json::to_writer(std::io::stdout().lock(), &facts.schematic_instances)?;
    Ok(())
}
