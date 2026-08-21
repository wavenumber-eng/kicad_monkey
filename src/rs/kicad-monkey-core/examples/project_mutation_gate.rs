use kicad_monkey_core::{ProjectDocument, ProjectLimits};
use serde_json::json;
use std::fs::File;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let input = PathBuf::from(arguments.next().ok_or("missing input project path")?);
    let output = PathBuf::from(arguments.next().ok_or("missing output project path")?);
    if arguments.next().is_some() {
        return Err("usage: project_mutation_gate <input.kicad_pro> <output.kicad_pro>".into());
    }
    let source = std::fs::read(&input)?;
    let mut document = ProjectDocument::from_reader(source.as_slice(), ProjectLimits::default())?;
    let changed_text = document.set_text_variable("RUST_GATE", "enabled")?;
    document.add_variant("Rust Gate", Some("native parity"))?;
    let renamed = document.rename_variant("Rust Gate", "Rust Gate Renamed")?;
    let changed_path = document.set_path("meta.rust_gate", json!(true))?;
    document.write_to(File::create(output)?)?;
    println!(
        "{}",
        json!({
            "schema": "kicad_monkey.project_mutation_gate.a0",
            "changed_text": changed_text,
            "renamed": renamed,
            "changed_path": changed_path,
        })
    );
    Ok(())
}
