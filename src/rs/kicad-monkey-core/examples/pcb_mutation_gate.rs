//! Native PCB mutation acceptance gate used by Rack and `kicad-cli`.

use kicad_monkey_core::{Error, PcbDocument, PcbGraphic, PcbLimits, PcbProperty, PcbView};
use serde::Serialize;
use std::path::{Path, PathBuf};

const EXISTING_PROPERTY: &str = "Existing";
const INSERTED_PROPERTY: &str = "RustPhase3Gate";
const LAYER_EDIT_ID: &str = "a1111111-1111-4111-8111-111111111111";
const REMOVE_ID: &str = "b2222222-2222-4222-8222-222222222222";
const EDITED_LAYER: &str = "B.SilkS";

#[derive(Serialize)]
struct GateEvidence {
    schema: &'static str,
    cases: Vec<CaseEvidence>,
}

#[derive(Serialize)]
struct CaseEvidence {
    operation: &'static str,
    output: String,
    changed: bool,
    reparsed: bool,
    stable_second_write: bool,
    unrelated_semantics_preserved: bool,
}

#[derive(Debug, Eq, PartialEq)]
struct SemanticSnapshot {
    properties: Vec<(String, String)>,
    graphics: Vec<(String, Option<String>)>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let input = PathBuf::from(args.next().ok_or("expected input PCB path")?);
    let output_dir = PathBuf::from(args.next().ok_or("expected output directory")?);
    if args.next().is_some() {
        return Err("expected exactly two arguments".into());
    }
    std::fs::create_dir_all(&output_dir)?;
    let source = std::fs::read_to_string(&input)?;
    let limits = PcbLimits::default();
    let baseline = PcbDocument::parse(source.clone(), limits)?;
    let baseline_snapshot = snapshot(&baseline.view()?)?;
    require_property(&baseline.view()?, EXISTING_PROPERTY, Some("original"))?;
    require_graphic_layer(&baseline.view()?, LAYER_EDIT_ID, Some("F.SilkS"))?;
    require_graphic_layer(&baseline.view()?, REMOVE_ID, Some("F.SilkS"))?;

    let cases = run_cases(source, limits, &baseline_snapshot, &output_dir)?;
    println!(
        "{}",
        serde_json::to_string(&GateEvidence {
            schema: "kicad_monkey.pcb_mutation_cli_evidence.a0",
            cases,
        })?
    );
    Ok(())
}

fn run_cases(
    source: String,
    limits: PcbLimits,
    baseline_snapshot: &SemanticSnapshot,
    output_dir: &Path,
) -> Result<Vec<CaseEvidence>, Box<dyn std::error::Error>> {
    let mut cases = Vec::new();

    let mut update = PcbDocument::parse(source.clone(), limits)?;
    require(
        update.set_property(EXISTING_PROPERTY, "updated")?,
        "property update changed",
    )?;
    require_property(&update.view()?, EXISTING_PROPERTY, Some("updated"))?;
    require_graphics_equal(baseline_snapshot, &snapshot(&update.view()?)?)?;
    cases.push(finish_case(
        "property_update",
        update,
        output_dir,
        "property-update.kicad_pcb",
        |document| document.set_property(EXISTING_PROPERTY, "updated"),
    )?);

    let mut insert = PcbDocument::parse(source.clone(), limits)?;
    require(
        insert.upsert_property(INSERTED_PROPERTY, "inserted")?,
        "property insertion changed",
    )?;
    require_property(&insert.view()?, INSERTED_PROPERTY, Some("inserted"))?;
    require_graphics_equal(baseline_snapshot, &snapshot(&insert.view()?)?)?;
    cases.push(finish_case(
        "property_insert",
        insert,
        output_dir,
        "property-insert.kicad_pcb",
        |document| document.upsert_property(INSERTED_PROPERTY, "inserted"),
    )?);

    let mut remove_property = PcbDocument::parse(source.clone(), limits)?;
    require(
        remove_property.remove_property(EXISTING_PROPERTY)?,
        "property removal changed",
    )?;
    require_property(&remove_property.view()?, EXISTING_PROPERTY, None)?;
    require_graphics_equal(baseline_snapshot, &snapshot(&remove_property.view()?)?)?;
    cases.push(finish_case(
        "property_remove",
        remove_property,
        output_dir,
        "property-remove.kicad_pcb",
        |document| document.remove_property(EXISTING_PROPERTY),
    )?);

    let mut layer = PcbDocument::parse(source.clone(), limits)?;
    require(
        layer.set_top_level_layer_by_id(LAYER_EDIT_ID, EDITED_LAYER)?,
        "layer edit changed",
    )?;
    require_graphic_layer(&layer.view()?, LAYER_EDIT_ID, Some(EDITED_LAYER))?;
    require_graphic_layer(&layer.view()?, REMOVE_ID, Some("F.SilkS"))?;
    require_properties_equal(baseline_snapshot, &snapshot(&layer.view()?)?)?;
    cases.push(finish_case(
        "stable_layer_edit",
        layer,
        output_dir,
        "layer-edit.kicad_pcb",
        |document| document.set_top_level_layer_by_id(LAYER_EDIT_ID, EDITED_LAYER),
    )?);

    let mut remove_top_level = PcbDocument::parse(source, limits)?;
    require(
        remove_top_level.remove_top_level_by_id(REMOVE_ID)?,
        "top-level removal changed",
    )?;
    require_graphic_absent(&remove_top_level.view()?, REMOVE_ID)?;
    require_graphic_layer(&remove_top_level.view()?, LAYER_EDIT_ID, Some("F.SilkS"))?;
    require_properties_equal(baseline_snapshot, &snapshot(&remove_top_level.view()?)?)?;
    require(
        snapshot(&remove_top_level.view()?)?.graphics.len() + 1 == baseline_snapshot.graphics.len(),
        "top-level removal changed an unexpected number of graphics",
    )?;
    cases.push(finish_case(
        "top_level_remove",
        remove_top_level,
        output_dir,
        "top-level-remove.kicad_pcb",
        |document| document.remove_top_level_by_id(REMOVE_ID),
    )?);

    Ok(cases)
}

fn finish_case(
    operation: &'static str,
    document: PcbDocument,
    output_dir: &Path,
    filename: &str,
    repeat: impl FnOnce(&mut PcbDocument) -> Result<bool, Error>,
) -> Result<CaseEvidence, Box<dyn std::error::Error>> {
    let output = output_dir.join(filename);
    let mut bytes = Vec::new();
    document.write_to(&mut bytes)?;
    std::fs::write(&output, &bytes)?;
    let mut reparsed = PcbDocument::parse(String::from_utf8(bytes)?, document.limits())?;
    let before_repeat = reparsed.source().to_owned();
    require(!repeat(&mut reparsed)?, "repeated mutation was not a no-op")?;
    require(
        reparsed.source() == before_repeat,
        "repeated mutation changed source bytes",
    )?;
    let mut second_bytes = Vec::new();
    reparsed.write_to(&mut second_bytes)?;
    require(
        second_bytes == before_repeat.as_bytes(),
        "second write changed emitted bytes",
    )?;
    Ok(CaseEvidence {
        operation,
        output: output.to_string_lossy().into_owned(),
        changed: true,
        reparsed: true,
        stable_second_write: true,
        unrelated_semantics_preserved: true,
    })
}

fn snapshot(view: &PcbView<'_>) -> Result<SemanticSnapshot, Error> {
    let properties = view
        .properties()
        .map(|item| item.map(|property| (property.name, property.value)))
        .collect::<Result<Vec<_>, _>>()?;
    let graphics = view
        .graphics()
        .map(|item| item.map(|graphic| (graphic.uuid.unwrap_or_default(), graphic.layer)))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(SemanticSnapshot {
        properties,
        graphics,
    })
}

fn require_property(
    view: &PcbView<'_>,
    name: &str,
    expected: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let value = view
        .properties()
        .collect::<Result<Vec<PcbProperty>, _>>()?
        .into_iter()
        .find(|property| property.name == name)
        .map(|property| property.value);
    require(value.as_deref() == expected, "property value did not match")
}

fn require_graphic_layer(
    view: &PcbView<'_>,
    identifier: &str,
    expected: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let layer = view
        .graphics()
        .collect::<Result<Vec<PcbGraphic>, _>>()?
        .into_iter()
        .find(|graphic| graphic.uuid.as_deref() == Some(identifier))
        .and_then(|graphic| graphic.layer);
    require(layer.as_deref() == expected, "graphic layer did not match")
}

fn require_graphic_absent(
    view: &PcbView<'_>,
    identifier: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let found = view
        .graphics()
        .collect::<Result<Vec<PcbGraphic>, _>>()?
        .into_iter()
        .any(|graphic| graphic.uuid.as_deref() == Some(identifier));
    require(!found, "removed graphic remains present")
}

fn require_graphics_equal(
    baseline: &SemanticSnapshot,
    edited: &SemanticSnapshot,
) -> Result<(), Box<dyn std::error::Error>> {
    require(
        edited.graphics == baseline.graphics,
        "property mutation changed unrelated graphics",
    )
}

fn require_properties_equal(
    baseline: &SemanticSnapshot,
    edited: &SemanticSnapshot,
) -> Result<(), Box<dyn std::error::Error>> {
    require(
        edited.properties == baseline.properties,
        "object mutation changed unrelated properties",
    )
}

fn require(condition: bool, message: &str) -> Result<(), Box<dyn std::error::Error>> {
    if condition {
        Ok(())
    } else {
        Err(message.to_owned().into())
    }
}
