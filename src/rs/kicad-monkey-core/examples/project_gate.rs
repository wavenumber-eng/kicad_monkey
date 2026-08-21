use kicad_monkey_core::{ProjectDocument, ProjectLimits};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Serialize)]
struct Evidence {
    schema: &'static str,
    file_count: usize,
    files: Vec<FileEvidence>,
}

#[derive(Serialize)]
struct FileEvidence {
    path: String,
    source_bytes: usize,
    source_sha256: String,
    canonical_sha256: String,
    exact_write: bool,
    stable_canonical_write: bool,
    text_variables: Vec<(String, String)>,
    variants: Vec<kicad_monkey_core::ProjectVariant>,
    net_settings: kicad_monkey_core::ProjectNetSettings,
    board_design_settings: kicad_monkey_core::ProjectBoardDesignSettings,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return Err("usage: project_gate <project.kicad_pro> [...]".into());
    }
    let files = paths
        .iter()
        .map(project_evidence)
        .collect::<Result<Vec<_>, _>>()?;
    println!(
        "{}",
        serde_json::to_string(&Evidence {
            schema: "kicad_monkey.project_gate_evidence.a0",
            file_count: files.len(),
            files,
        })?
    );
    Ok(())
}

fn project_evidence(path: &PathBuf) -> Result<FileEvidence, Box<dyn std::error::Error>> {
    let source = std::fs::read(path)?;
    let document = ProjectDocument::from_reader(source.as_slice(), ProjectLimits::default())?;
    let view = document.view();
    let mut exact = Vec::new();
    document.write_to(&mut exact)?;
    let canonical = document.canonical_text()?;
    let second =
        ProjectDocument::parse(canonical.clone(), ProjectLimits::default())?.canonical_text()?;
    Ok(FileEvidence {
        path: path.display().to_string(),
        source_bytes: source.len(),
        source_sha256: sha256(&source),
        canonical_sha256: sha256(canonical.as_bytes()),
        exact_write: exact == source,
        stable_canonical_write: canonical == second,
        text_variables: view.text_variables()?,
        variants: view.variants()?,
        net_settings: view.net_settings()?,
        board_design_settings: view.board_design_settings()?,
    })
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
