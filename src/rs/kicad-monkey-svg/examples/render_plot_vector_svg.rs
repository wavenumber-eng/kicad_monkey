//! Render one checked-in typed Plotter-IR vector through a direct Rust API.

use std::path::PathBuf;

use kicad_monkey_contracts::generated::{
    board_plot_document::BoardPlotDocumentA0, footprint_plot_document::FootprintPlotDocumentA0,
    schematic_plot_document::SchematicPlotDocumentA0, symbol_plot_document::SymbolPlotDocumentA0,
};
use kicad_monkey_svg::{
    SvgArtifact, SvgFitOptions, SvgRenderContextA1, SvgRenderLimits, ViewportPolicy,
    render_board_document_svg, render_footprint_svg, render_schematic_svg, render_symbol_svg,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

fn typed_vector<T: DeserializeOwned>(path: &PathBuf, id: &str) -> Result<T, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    let payload: Value =
        serde_json::from_slice(&bytes).map_err(|error| format!("decode vectors: {error}"))?;
    let value = payload["vectors"]
        .as_array()
        .and_then(|vectors| vectors.iter().find(|value| value["id"] == id))
        .and_then(|value| value.get("expected"))
        .cloned()
        .ok_or_else(|| format!("vector {id:?} not found in {}", path.display()))?;
    serde_json::from_value(value).map_err(|error| format!("decode typed vector {id}: {error}"))
}

fn run() -> Result<(), String> {
    let mut arguments = std::env::args().skip(1);
    let usage = "usage: render_plot_vector_svg <family> <vectors.json> <id> <output.svg>";
    let family = arguments.next().ok_or(usage)?;
    let vectors = PathBuf::from(arguments.next().ok_or(usage)?);
    let id = arguments.next().ok_or(usage)?;
    let output = PathBuf::from(arguments.next().ok_or(usage)?);
    if arguments.next().is_some() {
        return Err(usage.to_owned());
    }
    let context = SvgRenderContextA1::default()
        .validate(Default::default())
        .map_err(|error| error.to_string())?;
    let viewport = ViewportPolicy::Fit(SvgFitOptions {
        // Browser coverage deliberately exercises the renderer's own fitted
        // bounds without hiding clipping behind caller-provided padding.
        padding_nm: 0,
        min_extent_nm: 1,
        fallback: None,
    });
    let limits = SvgRenderLimits::default();
    let artifact: SvgArtifact = match family.as_str() {
        "footprint" => render_footprint_svg(
            &typed_vector::<FootprintPlotDocumentA0>(&vectors, &id)?,
            viewport,
            &context,
            limits,
        ),
        "symbol" => render_symbol_svg(
            &typed_vector::<SymbolPlotDocumentA0>(&vectors, &id)?,
            viewport,
            &context,
            limits,
        ),
        "board" => render_board_document_svg(
            &typed_vector::<BoardPlotDocumentA0>(&vectors, &id)?,
            viewport,
            &context,
            limits,
        ),
        "schematic" => render_schematic_svg(
            &typed_vector::<SchematicPlotDocumentA0>(&vectors, &id)?,
            viewport,
            &context,
            limits,
        ),
        _ => return Err(format!("unsupported family {family:?}")),
    }
    .map_err(|error| error.to_string())?;
    std::fs::write(&output, artifact.svg.as_bytes())
        .map_err(|error| format!("write {}: {error}", output.display()))?;
    eprintln!(
        "rendered {} bytes; viewport={}x{}nm; warnings={:?}",
        artifact.svg.len(),
        artifact.viewport.width_nm,
        artifact.viewport.height_nm,
        artifact.warnings
    );
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
