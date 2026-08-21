//! Native schematic mutation acceptance gate.

use kicad_monkey_core::{SchematicDocument, SchematicDocumentLimits};
use serde::Serialize;
use std::io::BufReader;
use std::path::PathBuf;

const PROPERTY_NAME: &str = "Rust Native Property";
const PROPERTY_VALUE: &str = "source-preserving";

#[derive(Serialize)]
struct GateEvidence {
    schema: &'static str,
    output: String,
    symbol_uuid: String,
    changed: bool,
    stable_second_write: bool,
    unrelated_semantics_preserved: bool,
    inserted_property_has_complete_placement: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (input, output) = arguments()?;
    let evidence = mutate(&input, &output)?;
    println!("{}", serde_json::to_string(&evidence)?);
    Ok(())
}

fn arguments() -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let input = args
        .next()
        .map(PathBuf::from)
        .ok_or("expected input schematic path")?;
    let output = args
        .next()
        .map(PathBuf::from)
        .ok_or("expected output schematic path")?;
    if args.next().is_some() {
        return Err("expected exactly input and output schematic paths".into());
    }
    Ok((input, output))
}

fn mutate(
    input: &std::path::Path,
    output: &std::path::Path,
) -> Result<GateEvidence, Box<dyn std::error::Error>> {
    let limits = SchematicDocumentLimits::default();
    let file = std::fs::File::open(input)?;
    let mut document = SchematicDocument::from_named_reader(
        input.to_string_lossy(),
        BufReader::new(file),
        limits,
    )?;
    let before = document.definition()?;
    let symbol_uuid = before
        .symbols
        .iter()
        .find(|symbol| !symbol.uuid.is_empty())
        .map(|symbol| symbol.uuid.clone())
        .ok_or("input has no identified placed symbol")?;

    let changed = document.upsert_symbol_property(&symbol_uuid, PROPERTY_NAME, PROPERTY_VALUE)?;
    let stable_second_write =
        !document.upsert_symbol_property(&symbol_uuid, PROPERTY_NAME, PROPERTY_VALUE)?;
    let after = document.definition()?;
    let edited = after
        .symbols
        .iter()
        .find(|symbol| symbol.uuid == symbol_uuid)
        .ok_or("edited symbol disappeared")?;
    let inserted = edited
        .properties
        .iter()
        .find(|property| property.key == PROPERTY_NAME && property.value == PROPERTY_VALUE)
        .ok_or("inserted property is absent after semantic reparse")?;
    let unrelated_semantics_preserved = before.uuid == after.uuid
        && before.sheets == after.sheets
        && before.wires == after.wires
        && before.buses == after.buses
        && before.symbols.len() == after.symbols.len()
        && !inserted.key.is_empty();
    let inserted_property_has_complete_placement = document.source().contains(&format!(
        "(property \"{PROPERTY_NAME}\" \"{PROPERTY_VALUE}\" (at 0 0 0))"
    ));
    let evidence = GateEvidence {
        schema: "kicad_monkey.schematic_mutation_cli_evidence.a0",
        output: output.to_string_lossy().into_owned(),
        symbol_uuid,
        changed,
        stable_second_write,
        unrelated_semantics_preserved,
        inserted_property_has_complete_placement,
    };
    if !acceptance_invariants(&evidence) {
        return Err("schematic mutation acceptance invariant failed".into());
    }
    let mut file = std::fs::File::create(output)?;
    document.write_to(&mut file)?;
    Ok(evidence)
}

fn acceptance_invariants(evidence: &GateEvidence) -> bool {
    evidence.changed
        && evidence.stable_second_write
        && evidence.unrelated_semantics_preserved
        && evidence.inserted_property_has_complete_placement
}
