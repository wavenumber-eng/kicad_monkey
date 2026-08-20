use kicad_monkey_contracts::generated::shaping_record::ShapingInput;
use kicad_monkey_core::{
    BoardPlotLimits, BoardTextVariables, PlotterTextCacheLimits, PlotterTextCacheResources,
    PlotterTextFont, board_plot_document_with_text_cache_sidecar,
};
use kicad_monkey_wasm::project_board_plot_document_a0;
use serde::Deserialize;

const FONT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/fonts/kicad-stroke.ttf"
));

#[derive(Deserialize)]
struct LayoutVectors {
    records: Vec<LayoutRecord>,
}

#[derive(Deserialize)]
struct LayoutRecord {
    shaping: ShapingInput,
}

#[derive(Deserialize)]
struct SidecarVectors {
    vectors: Vec<SidecarVector>,
}

#[derive(Deserialize)]
struct SidecarVector {
    source: String,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let layout: LayoutVectors = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/text_layout_vectors.json"
    )))?;
    let vectors: SidecarVectors = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/board_text_cache_sidecar_vectors.json"
    )))?;
    let mut shaping = layout
        .records
        .into_iter()
        .next()
        .ok_or("missing shaping vector")?
        .shaping;
    shaping.text.clear();
    shaping.features.clear();
    let fonts = [PlotterTextFont {
        face: "Native Fixture",
        bold: false,
        italic: false,
        font_bytes: FONT_BYTES,
        shaping,
        fake_bold: false,
        fake_italic: false,
    }];
    let resources = PlotterTextCacheResources {
        fonts: &fonts,
        limits: PlotterTextCacheLimits::default(),
    };
    let vector = vectors.vectors.first().ok_or("missing sidecar vector")?;
    let document = board_plot_document_with_text_cache_sidecar(
        &vector.source,
        BoardPlotLimits::default(),
        &Default::default(),
        &BoardTextVariables::default(),
        Some(&resources),
    )?;
    let contract = project_board_plot_document_a0(
        document,
        Some("board_text_cache_sidecar_vectors.json".to_owned()),
        "matching-stale-missing-carrier-caches".to_owned(),
        kicad_monkey_core::BoardPlotContractLimits::default(),
    )?;
    serde_json::to_writer(std::io::stdout(), &contract)?;
    Ok(())
}
