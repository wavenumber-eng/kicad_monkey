//! Advisory four-family direct-render baseline used for release evidence.

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use kicad_monkey_contracts::generated::{
    board_plot_document::BoardPlotDocumentA0, footprint_plot_document::FootprintPlotDocumentA0,
    schematic_plot_document::SchematicPlotDocumentA0, symbol_plot_document::SymbolPlotDocumentA0,
};
use kicad_monkey_svg::{
    SvgArtifact, SvgRenderContextA1, SvgRenderLimits, SvgViewport, ViewportPolicy,
    render_board_document_svg, render_footprint_svg, render_schematic_svg, render_symbol_svg,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

const ROUNDS: usize = 25;
const VIEWPORT: ViewportPolicy = ViewportPolicy::Explicit(SvgViewport {
    min_x_nm: -100_000_000,
    min_y_nm: -100_000_000,
    width_nm: 200_000_000,
    height_nm: 200_000_000,
});

#[derive(Serialize)]
struct Measurement {
    schema: &'static str,
    family: String,
    fixture_id: String,
    document_json_bytes: usize,
    cold_render_ns: u128,
    warm_render_median_ns: u128,
    warm_rounds: usize,
    svg_bytes: usize,
}

fn vector<T: DeserializeOwned>(root: &Path, file: &str) -> Result<(String, T, usize), String> {
    let bytes = std::fs::read(root.join("tests/parity").join(file))
        .map_err(|error| format!("read {file}: {error}"))?;
    let parsed: Value =
        serde_json::from_slice(&bytes).map_err(|error| format!("decode {file}: {error}"))?;
    let item = parsed["vectors"]
        .as_array()
        .and_then(|vectors| vectors.first())
        .ok_or_else(|| format!("{file} has no vectors"))?;
    let id = item["id"]
        .as_str()
        .ok_or_else(|| format!("{file} first vector has no id"))?
        .to_owned();
    let document = item["expected"].clone();
    let document_bytes = serde_json::to_vec(&document)
        .map_err(|error| format!("encode {file} document: {error}"))?
        .len();
    let typed = serde_json::from_value(document)
        .map_err(|error| format!("decode {file} typed document: {error}"))?;
    Ok((id, typed, document_bytes))
}

fn measure(
    family: String,
    fixture_id: String,
    document_json_bytes: usize,
    mut render: impl FnMut() -> Result<SvgArtifact, kicad_monkey_svg::SvgError>,
) -> Result<Measurement, kicad_monkey_svg::SvgError> {
    let started = Instant::now();
    let first = render()?;
    let cold_render_ns = started.elapsed().as_nanos();
    let svg_bytes = first.svg.len();
    black_box(&first);

    let mut warm = Vec::with_capacity(ROUNDS);
    for _ in 0..ROUNDS {
        let started = Instant::now();
        let artifact = render()?;
        warm.push(started.elapsed().as_nanos());
        assert_eq!(artifact.svg.len(), svg_bytes);
        black_box(artifact);
    }
    warm.sort_unstable();
    Ok(Measurement {
        schema: "kicad_monkey.direct_svg_benchmark.a0",
        family,
        fixture_id,
        document_json_bytes,
        cold_render_ns,
        warm_render_median_ns: warm[ROUNDS / 2],
        warm_rounds: ROUNDS,
        svg_bytes,
    })
}

fn run() -> Result<Measurement, String> {
    let family = std::env::args().nth(1).ok_or_else(|| {
        "usage: direct_svg_benchmark <footprint|symbol|board|schematic>".to_owned()
    })?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let context = SvgRenderContextA1::default()
        .validate(Default::default())
        .map_err(|error| error.to_string())?;
    let limits = SvgRenderLimits::default();
    std::thread::sleep(Duration::from_millis(100));

    match family.as_str() {
        "footprint" => {
            let (id, document, bytes) =
                vector::<FootprintPlotDocumentA0>(&root, "footprint_plotter_a0_vectors.json")?;
            measure(family, id, bytes, || {
                render_footprint_svg(&document, VIEWPORT, &context, limits)
            })
        }
        "symbol" => {
            let (id, document, bytes) =
                vector::<SymbolPlotDocumentA0>(&root, "symbol_plotter_a0_vectors.json")?;
            measure(family, id, bytes, || {
                render_symbol_svg(&document, VIEWPORT, &context, limits)
            })
        }
        "board" => {
            let (id, document, bytes) =
                vector::<BoardPlotDocumentA0>(&root, "board_plotter_a0_vectors.json")?;
            measure(family, id, bytes, || {
                render_board_document_svg(&document, VIEWPORT, &context, limits)
            })
        }
        "schematic" => {
            let (id, document, bytes) =
                vector::<SchematicPlotDocumentA0>(&root, "schematic_plotter_a0_vectors.json")?;
            measure(family, id, bytes, || {
                render_schematic_svg(&document, VIEWPORT, &context, limits)
            })
        }
        _ => Err(kicad_monkey_svg::SvgError::new(
            kicad_monkey_svg::SvgErrorKind::InvalidDocument,
            format!("unsupported benchmark family {family}"),
        )),
    }
    .map_err(|error| error.to_string())
}

fn main() -> Result<(), String> {
    let measurement = run()?;
    println!(
        "{}",
        serde_json::to_string(&measurement).map_err(|error| error.to_string())?
    );
    std::thread::sleep(Duration::from_millis(25));
    Ok(())
}
