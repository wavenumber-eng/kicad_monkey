use std::{fs, path::PathBuf};

use kicad_monkey_contracts::generated::{
    board_plot_document::BoardPlotDocumentA0, footprint_plot_document::FootprintPlotDocumentA0,
    schematic_plot_document::SchematicPlotDocumentA0, symbol_plot_document::SymbolPlotDocumentA0,
};
use kicad_monkey_svg::{
    SvgColor, SvgContextLimits, SvgRenderContextA1, SvgRenderLimits, SvgSemanticRole,
    SvgStyleOverride, SvgViewport, ViewportPolicy, render_board_document_svg, render_footprint_svg,
    render_schematic_svg, render_symbol_svg,
};
use serde::de::DeserializeOwned;
use serde_json::Value;

const VIEWPORT: ViewportPolicy = ViewportPolicy::Explicit(SvgViewport {
    min_x_nm: -100_000_000,
    min_y_nm: -100_000_000,
    width_nm: 400_000_000,
    height_nm: 400_000_000,
});

#[test]
fn cruncher_consumes_all_four_direct_renderers_with_one_context() {
    let footprint: FootprintPlotDocumentA0 = first_nonempty("footprint_plotter_a0_vectors.json");
    let symbol: SymbolPlotDocumentA0 = first_nonempty("symbol_plotter_a0_vectors.json");
    let board: BoardPlotDocumentA0 = first_nonempty("board_plotter_a0_vectors.json");
    let schematic: SchematicPlotDocumentA0 = first_nonempty("schematic_plotter_a0_vectors.json");
    let accent = SvgColor::parse("#2468ACFF").unwrap();
    let mut builder = SvgRenderContextA1::builder();
    for role in [
        SvgSemanticRole::Copper,
        SvgSemanticRole::Drill,
        SvgSemanticRole::Mask,
        SvgSemanticRole::Silkscreen,
        SvgSemanticRole::Fabrication,
        SvgSemanticRole::Courtyard,
        SvgSemanticRole::BoardEdge,
        SvgSemanticRole::Worksheet,
        SvgSemanticRole::SchematicWire,
        SvgSemanticRole::SchematicBus,
        SvgSemanticRole::Junction,
        SvgSemanticRole::Label,
        SvgSemanticRole::Pin,
        SvgSemanticRole::SymbolBody,
        SvgSemanticRole::HierarchicalSheet,
        SvgSemanticRole::Text,
        SvgSemanticRole::Image,
        SvgSemanticRole::Other,
    ] {
        builder = builder.semantic_style(
            role,
            SvgStyleOverride::new()
                .with_stroke(accent.clone())
                .with_fill(accent.clone()),
        );
    }
    let context = builder
        .build()
        .validate(SvgContextLimits::default())
        .unwrap();
    let limits = SvgRenderLimits::default();

    let outputs = [
        render_footprint_svg(&footprint, VIEWPORT, &context, limits).unwrap(),
        render_symbol_svg(&symbol, VIEWPORT, &context, limits).unwrap(),
        render_board_document_svg(&board, VIEWPORT, &context, limits).unwrap(),
        render_schematic_svg(&schematic, VIEWPORT, &context, limits).unwrap(),
    ];
    assert_eq!(
        outputs
            .iter()
            .map(|artifact| artifact.source_kind)
            .collect::<Vec<_>>(),
        ["MOD", "SYM", "PCB", "SCH"]
    );
    for artifact in outputs {
        assert!(
            artifact.svg.contains("#2468AC"),
            "{} ignored Cruncher's shared direct-render context",
            artifact.source_kind
        );
    }
}

fn first_nonempty<T: DeserializeOwned>(file: &str) -> T {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../../..");
    let vectors: Value = serde_json::from_slice(
        &fs::read(root.join("tests/parity").join(file)).expect("read Plotter-IR vectors"),
    )
    .expect("decode Plotter-IR vectors");
    let document = vectors["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|vector| vector["expected"]["total_operations"].as_u64().unwrap_or(0) > 0)
        .expect("nonempty Plotter-IR vector")["expected"]
        .clone();
    serde_json::from_value(document).expect("typed Plotter-IR document")
}
