//! Render one selected KiCad library symbol through the direct typed Rust API.

use std::path::PathBuf;

use kicad_monkey_core::{
    PlotDocumentMetadata, PlotDocumentProjectionLimits, SymbolPlotLimits,
    project_symbol_plot_document_a0, symbol_plot_document,
};
use kicad_monkey_svg::{
    SvgFitOptions, SvgRenderContextA1, SvgRenderLimits, ViewportPolicy, render_symbol_svg,
};

fn argument(label: &str) -> Result<String, String> {
    std::env::args()
        .nth(match label {
            "source" => 1,
            "symbol" => 2,
            "unit" => 3,
            "output" => 4,
            _ => unreachable!(),
        })
        .ok_or_else(|| {
            "usage: render_symbol_svg <source.kicad_sym> <symbol> <unit> <output.svg>".to_owned()
        })
}

fn run() -> Result<(), String> {
    let source_path = PathBuf::from(argument("source")?);
    let symbol = argument("symbol")?;
    let unit = argument("unit")?
        .parse::<u32>()
        .map_err(|error| format!("invalid unit: {error}"))?;
    let output_path = PathBuf::from(argument("output")?);
    let source = std::fs::read_to_string(&source_path)
        .map_err(|error| format!("read {}: {error}", source_path.display()))?;
    let plot = symbol_plot_document(&source, &symbol, Some(unit), 0, SymbolPlotLimits::default())
        .map_err(|error| error.to_string())?;
    let document = project_symbol_plot_document_a0(
        plot,
        PlotDocumentMetadata {
            document_id: format!("{symbol}-unit{unit}"),
            source_path: Some(source_path.display().to_string()),
        },
        PlotDocumentProjectionLimits::default(),
    )
    .map_err(|error| error.to_string())?;
    let context = SvgRenderContextA1::default()
        .validate(Default::default())
        .map_err(|error| error.to_string())?;
    let artifact = render_symbol_svg(
        &document,
        ViewportPolicy::Fit(SvgFitOptions {
            padding_nm: 2_000_000,
            min_extent_nm: 1,
            fallback: None,
        }),
        &context,
        SvgRenderLimits::default(),
    )
    .map_err(|error| error.to_string())?;
    std::fs::write(&output_path, artifact.svg.as_bytes())
        .map_err(|error| format!("write {}: {error}", output_path.display()))?;
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
