use std::fs;
use std::path::PathBuf;

use kicad_monkey_contracts::decode_native_svg_render_request_a0;
use kicad_monkey_contracts::generated::source_bundle_manifest::{
    SourceBundleManifestA0, SourceBundleSource, SourceKind,
};
use kicad_monkey_contracts::generated::{
    board_plot_document::BoardPlotDocumentA0, footprint_plot_document::FootprintPlotDocumentA0,
    schematic_plot_document::SchematicPlotDocumentA0, symbol_plot_document::SymbolPlotDocumentA0,
};
use kicad_monkey_core::{
    BoardNetClassAssignments, BoardPlotLimits, BoardTextVariables, PcbLimits, PlotDocumentMetadata,
    PlotDocumentProjectionLimits, SchematicBundleIndex, SchematicBundleLimits,
    SchematicPagePlotRequest, SourceBundle, SourceBundleLimits, board_plot_artifact_with_sidecars,
    project_board_plot_artifact_a0, project_schematic_page_plot_artifact_a0,
    schematic_page_plot_document,
};
use kicad_monkey_svg::{
    LayerPattern, LayerSelection, PlotterOperationKind, SvgBackground, SvgColor, SvgContextLimits,
    SvgErrorKind, SvgFitOptions, SvgIdentityMode, SvgRenderContextA1, SvgRenderLimits,
    SvgSemanticRole, SvgStyleOverride, SvgViewport, SvgVisibility, SvgWarning, ViewportPolicy,
    render_board_document_svg, render_board_svg, render_footprint_svg, render_native_svg_a0_compat,
    render_schematic_page_svg, render_schematic_svg, render_symbol_svg,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const VIEWPORT: ViewportPolicy = ViewportPolicy::Explicit(SvgViewport {
    min_x_nm: -10_000_000,
    min_y_nm: -10_000_000,
    width_nm: 20_000_000,
    height_nm: 20_000_000,
});

const BOARD: &str = r#"(kicad_pcb (version 20240108) (generator pcbnew)
  (general (thickness 1.6)) (paper "A4")
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (36 "B.SilkS" user "Back Silk Screen"))
  (segment (start 0 0) (end 1 0) (width 0.2) (layer "F.Cu") (net 0) (uuid "s")))"#;

const PHYSICAL_LAYER_BOARD: &str = r#"(kicad_pcb
  (version 20250830) (generator pcbnew)
  (layers (0 "F.Cu" signal) (2 "In1.Cu" power) (31 "B.Cu" signal))
  (net 1 "N")
  (via (at 10 10) (size 0.8) (drill 0.4) (layers "F.Cu" "B.Cu")
    (remove_unused_layers yes) (keep_end_layers no) (net 1) (uuid "drill-only"))
  (footprint "Test:Holes" (layer "F.Cu") (at 0 0)
    (pad "1" thru_hole circle (at 1 1) (size 1 1) (drill 0.5)
      (layers "F&B.Cu" "*.Mask"))
    (pad "2" np_thru_hole circle (at 3 1) (size 0.6 0.6) (drill 0.6)
      (layers "*.Mask"))))"#;

#[test]
fn direct_typed_footprint_and_symbol_render_without_transport_reencoding() {
    let footprint = first_document("footprint_plotter_a0_vectors.json");
    let footprint: FootprintPlotDocumentA0 =
        serde_json::from_value(footprint).expect("typed footprint");
    let symbol = first_document("symbol_plotter_a0_vectors.json");
    let symbol: SymbolPlotDocumentA0 = serde_json::from_value(symbol).expect("typed symbol");
    let context = SvgRenderContextA1::default()
        .validate(SvgContextLimits::default())
        .unwrap();

    let footprint_svg =
        render_footprint_svg(&footprint, VIEWPORT, &context, SvgRenderLimits::default())
            .expect("direct footprint SVG");
    let symbol_svg = render_symbol_svg(&symbol, VIEWPORT, &context, SvgRenderLimits::default())
        .expect("direct symbol SVG");
    assert_eq!(footprint_svg.source_kind, "MOD");
    assert_eq!(symbol_svg.source_kind, "SYM");
    assert!(footprint_svg.svg.contains("data-ref=\"footprint\""));
    assert!(symbol_svg.svg.contains("data-ref="));
}

#[test]
fn the_same_validated_context_overrides_both_direct_families() {
    let footprint = first_document("footprint_plotter_a0_vectors.json");
    let footprint: FootprintPlotDocumentA0 =
        serde_json::from_value(footprint).expect("typed footprint");
    let symbol = first_document("symbol_plotter_a0_vectors.json");
    let symbol: SymbolPlotDocumentA0 = serde_json::from_value(symbol).expect("typed symbol");
    let accent = SvgColor::parse("#12ab34ff").unwrap();
    let context = accent_context(&accent);
    let footprint_svg =
        render_footprint_svg(&footprint, VIEWPORT, &context, SvgRenderLimits::default())
            .expect("themed footprint");
    let symbol_svg = render_symbol_svg(&symbol, VIEWPORT, &context, SvgRenderLimits::default())
        .expect("themed symbol");
    assert!(footprint_svg.svg.contains("#12AB34"));
    assert!(symbol_svg.svg.contains("#12AB34"));
}

#[test]
fn one_validated_context_overrides_all_four_direct_families() {
    let footprint: FootprintPlotDocumentA0 =
        serde_json::from_value(first_document("footprint_plotter_a0_vectors.json"))
            .expect("typed footprint");
    let symbol: SymbolPlotDocumentA0 =
        serde_json::from_value(first_document("symbol_plotter_a0_vectors.json"))
            .expect("typed symbol");
    let schematic: SchematicPlotDocumentA0 = serde_json::from_value(
        first_document_with_operations("schematic_plotter_a0_vectors.json"),
    )
    .expect("typed schematic");
    let board_source = board_plot_artifact_with_sidecars(
        BOARD,
        BoardPlotLimits::default(),
        PcbLimits::default(),
        &BoardNetClassAssignments::default(),
        &BoardTextVariables::default(),
    )
    .expect("board source artifact");
    let board = project_board_plot_artifact_a0(
        board_source,
        PlotDocumentMetadata {
            document_id: "direct-board-context".to_owned(),
            source_path: Some("direct.kicad_pcb".to_owned()),
        },
        PlotDocumentProjectionLimits::default(),
    )
    .expect("typed board artifact");
    let accent = SvgColor::parse("#12ab34ff").unwrap();
    let context = accent_context(&accent);

    let outputs = [
        render_footprint_svg(&footprint, VIEWPORT, &context, SvgRenderLimits::default())
            .expect("themed footprint"),
        render_symbol_svg(&symbol, VIEWPORT, &context, SvgRenderLimits::default())
            .expect("themed symbol"),
        render_board_svg(&board, VIEWPORT, &context, SvgRenderLimits::default())
            .expect("themed board"),
        render_schematic_svg(&schematic, VIEWPORT, &context, SvgRenderLimits::default())
            .expect("themed schematic"),
    ];
    assert_eq!(
        outputs
            .iter()
            .map(|output| output.source_kind)
            .collect::<Vec<_>>(),
        ["MOD", "SYM", "PCB", "SCH"]
    );
    for output in outputs {
        assert!(
            output.svg.contains("#12AB34"),
            "{} did not honor the shared style context",
            output.source_kind
        );
    }
}

fn accent_context(accent: &SvgColor) -> kicad_monkey_svg::ValidatedSvgRenderContextA1 {
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
    builder
        .build()
        .validate(SvgContextLimits::default())
        .unwrap()
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one matrix keeps all frozen native and browser-safe direct family outcomes together"
)]
fn direct_defaults_are_browser_scaled_while_native_a0_bytes_remain_frozen() {
    let context = ValidatedContext::default();
    let mut compared = 0usize;
    for (file, family, _, _) in [
        (
            "footprint_plotter_a0_vectors.json",
            "footprint",
            20_000_000,
            20_000_000,
        ),
        (
            "symbol_plotter_a0_vectors.json",
            "symbol",
            20_000_000,
            20_000_000,
        ),
        (
            "board_plotter_a0_vectors.json",
            "board",
            100_000_000,
            100_000_000,
        ),
        (
            "schematic_plotter_a0_vectors.json",
            "schematic",
            297_000_000,
            210_000_000,
        ),
    ] {
        for (source_id, document) in documents(file) {
            let document_id = document["document_id"].as_str().unwrap().to_owned();
            let expected = native_svg_expected(family, &source_id);
            let viewport = ViewportPolicy::Explicit(expected.0);
            let request = legacy_request(document.clone(), family, expected.0);
            let decoded =
                decode_native_svg_render_request_a0(&serde_json::to_vec(&request).unwrap())
                    .expect("legacy request");
            let legacy = render_native_svg_a0_compat(&decoded);
            let direct = match family {
                "footprint" => render_footprint_svg(
                    &serde_json::from_value::<FootprintPlotDocumentA0>(document).unwrap(),
                    viewport,
                    context.get(),
                    SvgRenderLimits::default(),
                ),
                "symbol" => render_symbol_svg(
                    &serde_json::from_value::<SymbolPlotDocumentA0>(document).unwrap(),
                    viewport,
                    context.get(),
                    SvgRenderLimits::default(),
                ),
                "board" => render_board_document_svg(
                    &serde_json::from_value::<BoardPlotDocumentA0>(document).unwrap(),
                    viewport,
                    context.get(),
                    SvgRenderLimits::default(),
                ),
                "schematic" => render_schematic_svg(
                    &serde_json::from_value::<SchematicPlotDocumentA0>(document).unwrap(),
                    viewport,
                    context.get(),
                    SvgRenderLimits::default(),
                ),
                _ => unreachable!(),
            };
            match (legacy, direct) {
                (Ok(legacy), Ok(direct)) => {
                    let frozen = expected.1.as_ref().expect("frozen SVG outcome");
                    assert_eq!(
                        legacy.svg.len(),
                        frozen.0,
                        "frozen bytes for {family}/{document_id}"
                    );
                    assert_eq!(
                        Sha256::digest(legacy.svg.as_bytes())
                            .iter()
                            .map(|byte| format!("{byte:02x}"))
                            .collect::<String>(),
                        frozen.1,
                        "frozen hash for {family}/{document_id}"
                    );
                    assert_eq!(direct.viewport, legacy.viewport);
                    assert_eq!(direct.source_kind, legacy.source_kind);
                    assert_eq!(direct.document_id, legacy.document_id);
                    let expected_viewbox = format!(
                        "viewBox=\"0 0 {} {}\"",
                        test_mm(expected.0.width_nm),
                        test_mm(expected.0.height_nm)
                    );
                    assert!(
                        direct.svg.contains(&expected_viewbox),
                        "browser-safe millimetre viewBox missing for {family}/{document_id}: {}",
                        direct.svg.lines().nth(1).unwrap_or_default()
                    );
                    assert_ne!(
                        direct.svg, legacy.svg,
                        "direct output must not retain native a0 raw-nanometre tokens"
                    );
                    compared += 1;
                }
                (Err(legacy), Err(direct)) => {
                    assert!(expected.1.is_none(), "unfrozen paired rejection");
                    assert_eq!(
                        family, "board",
                        "unexpected paired rejection for {family}/{document_id}: {legacy}; {direct}"
                    );
                }
                (legacy, direct) => panic!(
                    "renderer result mismatch for {family}/{document_id}: legacy={legacy:?}, direct={direct:?}"
                ),
            }
        }
    }
    assert_eq!(compared, 29);
}

#[test]
fn internal_layer_keeps_physical_pad_and_via_drills_without_removed_copper() {
    let board_source = board_plot_artifact_with_sidecars(
        PHYSICAL_LAYER_BOARD,
        BoardPlotLimits::default(),
        PcbLimits::default(),
        &BoardNetClassAssignments::default(),
        &BoardTextVariables::default(),
    )
    .expect("physical-layer board source");
    let board = project_board_plot_artifact_a0(
        board_source,
        PlotDocumentMetadata {
            document_id: "physical-layer-board".to_owned(),
            source_path: Some("physical.kicad_pcb".to_owned()),
        },
        PlotDocumentProjectionLimits::default(),
    )
    .expect("physical-layer board artifact");
    let context = SvgRenderContextA1::builder()
        .layer_selection(LayerSelection::include(
            vec![LayerPattern::parse("In1.Cu").unwrap()],
            true,
        ))
        .build()
        .validate(SvgContextLimits::default())
        .unwrap();
    let svg = render_board_svg(&board, VIEWPORT, &context, SvgRenderLimits::default())
        .expect("internal-layer physical SVG")
        .svg;

    assert_eq!(svg.matches("data-ref=\"pad_hole\"").count(), 2, "{svg}");
    assert!(svg.contains("data-ref=\"via\""));
    assert!(svg.contains("r=\"0.2\""));
    assert!(!svg.contains("r=\"0.4\""));

    let hidden_context = SvgRenderContextA1::builder()
        .layer_selection(LayerSelection::include(
            vec![LayerPattern::parse("In1.Cu").unwrap()],
            true,
        ))
        .layer_style(
            LayerPattern::parse("In1.Cu").unwrap(),
            SvgStyleOverride::new().with_visibility(false),
        )
        .build()
        .validate(SvgContextLimits::default())
        .unwrap();
    let hidden = render_board_svg(
        &board,
        VIEWPORT,
        &hidden_context,
        SvgRenderLimits::default(),
    )
    .unwrap()
    .svg;
    assert!(!hidden.contains("data-ref=\"pad_hole\""));
    assert!(!hidden.contains("r=\"200000\""));
}

#[test]
fn governed_yoshi_internal_layer_keeps_zone_connected_land_and_physical_hole() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let path = root.join(
        "packages/kicad_cruncher/tests/corpus/kicad/projects/yoshi_mainboard/input/11-10080__yoshi-mainboard__A.kicad_pcb",
    );
    let source = fs::read_to_string(path).expect("read governed Yoshi board");
    let board_source = board_plot_artifact_with_sidecars(
        &source,
        BoardPlotLimits::default(),
        PcbLimits::default(),
        &BoardNetClassAssignments::default(),
        &BoardTextVariables::default(),
    )
    .expect("project Yoshi board source");
    let board = project_board_plot_artifact_a0(
        board_source,
        PlotDocumentMetadata {
            document_id: "yoshi-mainboard".to_owned(),
            source_path: Some("11-10080__yoshi-mainboard__A.kicad_pcb".to_owned()),
        },
        PlotDocumentProjectionLimits::default(),
    )
    .expect("project Yoshi Plotter-IR");
    assert!(
        board
            .render_facts()
            .copper_stack()
            .iter()
            .any(|layer| layer == "In1.Cu")
    );
    let context = SvgRenderContextA1::builder()
        .layer_selection(LayerSelection::include(
            vec![LayerPattern::parse("In1.Cu").unwrap()],
            true,
        ))
        .build()
        .validate(SvgContextLimits::default())
        .unwrap();
    let svg = render_board_svg(&board, VIEWPORT, &context, SvgRenderLimits::default())
        .expect("render Yoshi In1.Cu directly")
        .svg;

    // USB-C shield pad S1 removes unused internal copper but explicitly keeps
    // its In1.Cu zone connection; both its land and drilled slot must survive.
    let shield_uuid = "24f889ca-2663-40e1-b599-9de50dcf9874";
    let shield_owners = svg
        .lines()
        .filter(|line| line.contains(shield_uuid))
        .collect::<Vec<_>>();
    assert!(
        shield_owners.iter().any(|line| {
            line.contains("data-ref=\"pad\"")
                && line.contains("data-layer-names=\"")
                && line.contains("In1.Cu")
        }),
        "Yoshi S1 retained land ownership missing: {shield_owners:?}"
    );
    assert!(
        shield_owners.iter().any(|line| {
            line.contains("data-ref=\"pad_hole\"")
                && line.contains("data-hole-render=\"drill\"")
                && line.contains("data-hole-kind=\"slot\"")
        }),
        "Yoshi S1 slot ownership missing: {shield_owners:?}"
    );
}

#[test]
fn fit_returns_exact_nm_metadata_and_estimates_uncached_text_without_erasing_geometry() {
    let footprint: FootprintPlotDocumentA0 =
        serde_json::from_value(first_document("footprint_plotter_a0_vectors.json")).unwrap();
    let context = ValidatedContext::default();
    let artifact = render_footprint_svg(
        &footprint,
        ViewportPolicy::Fit(SvgFitOptions {
            padding_nm: 50_000,
            min_extent_nm: 1,
            fallback: None,
        }),
        context.get(),
        SvgRenderLimits::default(),
    )
    .expect("fitted footprint");
    let bounds = artifact.visible_bounds.expect("visible bounds");
    assert_eq!(
        artifact.viewport.width_nm,
        bounds.max_x_nm.abs_diff(bounds.min_x_nm) + 100_000
    );
    assert_eq!(
        artifact.viewport.height_nm,
        bounds.max_y_nm.abs_diff(bounds.min_y_nm) + 100_000
    );

    let textual: FootprintPlotDocumentA0 = serde_json::from_value(document_by_id(
        "footprint_plotter_a0_vectors.json",
        "standalone-properties-text-and-text-box",
    ))
    .unwrap();
    let fit = ViewportPolicy::Fit(SvgFitOptions {
        padding_nm: 0,
        min_extent_nm: 1,
        fallback: None,
    });
    let estimated =
        render_footprint_svg(&textual, fit, context.get(), SvgRenderLimits::default()).unwrap();
    assert!(estimated.visible_bounds.is_some());
    assert_eq!(
        estimated.warnings,
        [SvgWarning::EstimatedBoundsForUncachedText]
    );
    assert!(estimated.svg.contains("font-size=\"1.27\""));
    assert!(!estimated.svg.contains("font-size=\"1270000\""));
    let fallback = SvgViewport {
        min_x_nm: -1,
        min_y_nm: -2,
        width_nm: 10,
        height_nm: 20,
    };
    let artifact = render_footprint_svg(
        &textual,
        ViewportPolicy::Fit(SvgFitOptions {
            padding_nm: 0,
            min_extent_nm: 1,
            fallback: Some(fallback),
        }),
        context.get(),
        SvgRenderLimits::default(),
    )
    .expect("fit fallback");
    assert_ne!(
        artifact.viewport, fallback,
        "fit no longer needs its fallback"
    );
    assert_eq!(
        artifact.warnings,
        [SvgWarning::EstimatedBoundsForUncachedText]
    );
}

#[test]
fn uncached_text_estimation_obeys_exact_bounds_work_and_zero_size_is_not_visible() {
    let textual: FootprintPlotDocumentA0 = serde_json::from_value(document_by_id(
        "footprint_plotter_a0_vectors.json",
        "standalone-properties-text-and-text-box",
    ))
    .unwrap();
    let context = ValidatedContext::default();
    let fit = ViewportPolicy::Fit(SvgFitOptions {
        padding_nm: 0,
        min_extent_nm: 1,
        fallback: None,
    });
    let baseline = render_footprint_svg(&textual, fit, context.get(), SvgRenderLimits::default())
        .expect("uncached text bounds baseline");
    assert!(baseline.metrics.bounds_work > baseline.metrics.operations);
    let exact = SvgRenderLimits {
        max_bounds_work: baseline.metrics.bounds_work,
        ..SvgRenderLimits::default()
    };
    render_footprint_svg(&textual, fit, context.get(), exact)
        .expect("exact uncached-text bounds work");
    let error = render_footprint_svg(
        &textual,
        fit,
        context.get(),
        SvgRenderLimits {
            max_bounds_work: baseline.metrics.bounds_work - 1,
            ..SvgRenderLimits::default()
        },
    )
    .expect_err("one-under uncached-text bounds work");
    assert_eq!(error.kind(), SvgErrorKind::ResourceLimit);
    assert!(error.to_string().contains("bounds work"));

    let mut zero_value = document_by_id(
        "footprint_plotter_a0_vectors.json",
        "standalone-properties-text-and-text-box",
    );
    let record = zero_value["records"][0].as_object_mut().unwrap();
    let mut text = record["operations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|operation| operation["kind"] == "Text")
        .unwrap()
        .clone();
    text["size_x_nm"] = json!(0);
    text["size_y_nm"] = json!(0);
    record.insert("operations".to_owned(), json!([text]));
    record.insert("operation_count".to_owned(), json!(1));
    zero_value["total_operations"] = json!(1);
    let zero: FootprintPlotDocumentA0 = serde_json::from_value(zero_value).unwrap();
    let explicit = render_footprint_svg(&zero, VIEWPORT, context.get(), SvgRenderLimits::default())
        .expect("zero-size text explicit viewport");
    assert!(explicit.visible_bounds.is_none());
    assert!(explicit.warnings.is_empty());
    let error = render_footprint_svg(&zero, fit, context.get(), SvgRenderLimits::default())
        .expect_err("zero-size text cannot invent fit geometry");
    assert_eq!(error.kind(), SvgErrorKind::EmptyBounds);

    for zero_dimension in ["size_x_nm", "size_y_nm"] {
        let mut cached_value = document_by_id(
            "board_plotter_a0_vectors.json",
            "board-text-follows-python-serializer",
        );
        let records = cached_value["records"].as_array_mut().unwrap();
        let record_index = records
            .iter()
            .position(|record| {
                record["operations"].as_array().is_some_and(|operations| {
                    operations
                        .iter()
                        .any(|operation| operation.get("render_cache").is_some())
                })
            })
            .unwrap();
        let mut record = records[record_index].clone();
        let mut cached = record["operations"]
            .as_array()
            .unwrap()
            .iter()
            .find(|operation| operation.get("render_cache").is_some())
            .unwrap()
            .clone();
        cached[zero_dimension] = json!(0);
        cached["index"] = json!(0);
        record["operations"] = json!([cached]);
        record["operation_count"] = json!(1);
        cached_value["records"] = json!([record]);
        cached_value["total_operations"] = json!(1);
        let cached_zero: BoardPlotDocumentA0 =
            serde_json::from_value(cached_value).expect("cached zero-size text document");
        let explicit = render_board_document_svg(
            &cached_zero,
            VIEWPORT,
            context.get(),
            SvgRenderLimits::default(),
        )
        .expect("cached zero-size text explicit viewport");
        assert!(explicit.visible_bounds.is_none());
        assert!(!explicit.svg.contains("<path d="));
        let error =
            render_board_document_svg(&cached_zero, fit, context.get(), SvgRenderLimits::default())
                .expect_err("cached zero-size text cannot invent fit geometry");
        assert_eq!(error.kind(), SvgErrorKind::EmptyBounds);
    }
}

#[test]
fn context_controls_are_effective_and_balanced_on_applicable_families() {
    let footprint: FootprintPlotDocumentA0 =
        serde_json::from_value(first_document("footprint_plotter_a0_vectors.json")).unwrap();
    let context = SvgRenderContextA1::builder()
        .background(SvgBackground::Transparent)
        .identity_mode(SvgIdentityMode::None)
        .operation_style(
            PlotterOperationKind::ThickSegment,
            SvgStyleOverride::new()
                .with_stroke(SvgColor::parse("#A1B2C3FF").unwrap())
                .with_stroke_width_nm(333_000),
        )
        .build()
        .validate(SvgContextLimits::default())
        .unwrap();
    let svg = render_footprint_svg(&footprint, VIEWPORT, &context, SvgRenderLimits::default())
        .unwrap()
        .svg;
    assert!(!svg.contains("fill=\"#FFFFFF\""));
    assert!(!svg.contains("data-ref="));
    assert!(svg.contains("stroke=\"#A1B2C3\""));
    assert!(svg.contains("stroke-width=\"0.333\""));

    let textual: FootprintPlotDocumentA0 = serde_json::from_value(document_by_id(
        "footprint_plotter_a0_vectors.json",
        "standalone-properties-text-and-text-box",
    ))
    .unwrap();
    let text_context = SvgRenderContextA1::builder()
        .raw_color_remap(
            SvgColor::parse("#0A141E80").unwrap(),
            SvgColor::parse("#334455FF").unwrap(),
        )
        .semantic_style(
            SvgSemanticRole::Text,
            SvgStyleOverride::new().with_opacity(0.5),
        )
        .font_face_override("Preview Font")
        .build()
        .validate(SvgContextLimits::default())
        .unwrap();
    let svg = render_footprint_svg(
        &textual,
        VIEWPORT,
        &text_context,
        SvgRenderLimits::default(),
    )
    .unwrap()
    .svg;
    assert!(svg.contains("fill=\"#334455\""));
    assert!(svg.contains("fill-opacity=\"0.5\""));
    assert!(svg.contains("font-family=\"Preview Font\""));

    let schematic: SchematicPlotDocumentA0 = serde_json::from_value(document_by_id(
        "schematic_plotter_a0_vectors.json",
        "placed-symbols-pins-fields-dnp-and-overplots",
    ))
    .unwrap();
    let pin_context = SvgRenderContextA1::builder()
        .visibility(SvgVisibility::new(false, true))
        .build()
        .validate(SvgContextLimits::default())
        .unwrap();
    let svg = render_schematic_svg(
        &schematic,
        VIEWPORT,
        &pin_context,
        SvgRenderLimits::default(),
    )
    .unwrap()
    .svg;
    assert!(!svg.contains(">IN</text>"));
    assert!(svg.contains(">1</text>"));
    assert_eq!(svg.matches("<g").count(), svg.matches("</g>").count());
}

#[test]
fn schematic_page_render_round_trips_occurrence_identity() {
    let source = br#"(kicad_sch (version 20250114) (generator eeschema)
      (generator_version "9.0") (uuid root-page) (paper "A4")
      (wire (pts (xy 0 0) (xy 2 0))
        (stroke (width 0) (type default)) (uuid wire-1))
      (sheet_instances (path "/" (page "1"))))"#
        .to_vec();
    let bundle = SourceBundle::from_manifest(
        SourceBundleManifestA0 {
            project_path: None,
            root_schematic_path: "root.kicad_sch".to_owned(),
            schema: "kicad_monkey.source_bundle_manifest.a0".to_owned(),
            sources: vec![SourceBundleSource {
                kind: SourceKind::Schematic,
                path: "root.kicad_sch".to_owned(),
                slot: 0_u32.into(),
                source_bytes: source.len().to_string().into(),
            }],
            type_: "kicad_monkey.source_bundle_manifest".to_owned(),
            version: "a0".to_owned(),
        },
        vec![source],
        SourceBundleLimits::default(),
    )
    .unwrap();
    let index = SchematicBundleIndex::build(&bundle, SchematicBundleLimits::default()).unwrap();
    let address = index
        .occurrences()
        .next()
        .expect("root occurrence")
        .occurrence_address
        .clone();
    let source =
        schematic_page_plot_document(&bundle, &index, SchematicPagePlotRequest::new(&address))
            .unwrap();
    let page =
        project_schematic_page_plot_artifact_a0(source, PlotDocumentProjectionLimits::default())
            .unwrap();
    let context = ValidatedContext::default();
    let artifact =
        render_schematic_page_svg(&page, VIEWPORT, context.get(), SvgRenderLimits::default())
            .unwrap();
    assert_eq!(
        artifact.occurrence_address.as_deref(),
        Some(address.as_str())
    );
    assert_eq!(artifact.document_id, "root-page");
}

#[test]
fn direct_limits_accept_exact_and_reject_one_under() {
    let schematic: SchematicPlotDocumentA0 = serde_json::from_value(document_by_id(
        "schematic_plotter_a0_vectors.json",
        "schematic-graphics-rules-images-and-table-family-order",
    ))
    .unwrap();
    let context = ValidatedContext::default();
    let baseline = render_schematic_svg(
        &schematic,
        VIEWPORT,
        context.get(),
        SvgRenderLimits::default(),
    )
    .unwrap();
    let metrics = baseline.metrics;
    type Setter = fn(&mut SvgRenderLimits, usize);
    let cases: [(&str, usize, Setter); 10] = [
        ("records", metrics.records, |limits, value| {
            limits.max_records = value
        }),
        ("operations", metrics.operations, |limits, value| {
            limits.max_operations = value
        }),
        ("points", metrics.points, |limits, value| {
            limits.max_points = value
        }),
        ("text", metrics.text_bytes, |limits, value| {
            limits.max_text_bytes = value
        }),
        ("image", metrics.image_encoded_bytes, |limits, value| {
            limits.max_image_encoded_bytes = value;
        }),
        ("elements", metrics.svg_elements, |limits, value| {
            limits.max_svg_elements = value
        }),
        ("work", metrics.render_work, |limits, value| {
            limits.max_render_work = value
        }),
        ("SVG bytes", metrics.svg_bytes, |limits, value| {
            limits.max_svg_bytes = value
        }),
        ("result bytes", metrics.result_bytes, |limits, value| {
            limits.max_result_bytes = value
        }),
        ("bounds work", metrics.bounds_work, |limits, value| {
            limits.max_bounds_work = value
        }),
    ];
    for (name, exact, set) in cases {
        assert!(exact > 0, "{name} must be exercised");
        let mut limits = SvgRenderLimits::default();
        set(&mut limits, exact);
        render_schematic_svg(&schematic, VIEWPORT, context.get(), limits)
            .unwrap_or_else(|error| panic!("exact {name}={exact}: {error}"));
        set(&mut limits, exact - 1);
        let error = render_schematic_svg(&schematic, VIEWPORT, context.get(), limits)
            .expect_err(&format!("one-under {name} unexpectedly passed"));
        assert_eq!(error.kind(), SvgErrorKind::ResourceLimit, "{name}: {error}");
    }

    let blocked: SchematicPlotDocumentA0 = serde_json::from_value(document_by_id(
        "schematic_plotter_a0_vectors.json",
        "placed-symbols-pins-fields-dnp-and-overplots",
    ))
    .unwrap();
    let depth = render_schematic_svg(
        &blocked,
        VIEWPORT,
        context.get(),
        SvgRenderLimits::default(),
    )
    .unwrap()
    .metrics
    .block_depth;
    assert!(depth > 0);
    let exact = SvgRenderLimits {
        max_block_depth: depth,
        ..SvgRenderLimits::default()
    };
    render_schematic_svg(&blocked, VIEWPORT, context.get(), exact).unwrap();
    let one_under = SvgRenderLimits {
        max_block_depth: depth - 1,
        ..SvgRenderLimits::default()
    };
    assert!(render_schematic_svg(&blocked, VIEWPORT, context.get(), one_under).is_err());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one table-equivalent test audits every public structured error category"
)]
fn public_boundaries_emit_exact_structured_error_kinds() {
    let invalid_context = SvgRenderContextA1::builder()
        .fallback_style(SvgStyleOverride::new().with_opacity(f64::NAN))
        .build()
        .validate(SvgContextLimits::default())
        .unwrap_err();
    assert_eq!(invalid_context.kind(), SvgErrorKind::InvalidContext);

    let context = ValidatedContext::default();
    let footprint: FootprintPlotDocumentA0 =
        serde_json::from_value(first_document("footprint_plotter_a0_vectors.json")).unwrap();
    let invalid_viewport = render_footprint_svg(
        &footprint,
        ViewportPolicy::Explicit(SvgViewport {
            min_x_nm: 0,
            min_y_nm: 0,
            width_nm: 0,
            height_nm: 1,
        }),
        context.get(),
        SvgRenderLimits::default(),
    )
    .unwrap_err();
    assert_eq!(invalid_viewport.kind(), SvgErrorKind::InvalidViewport);

    let symbol: SymbolPlotDocumentA0 =
        serde_json::from_value(first_document("symbol_plotter_a0_vectors.json")).unwrap();
    let selector_context = SvgRenderContextA1::builder()
        .layer_selection(LayerSelection::include(
            vec![LayerPattern::parse("F.Cu").unwrap()],
            false,
        ))
        .build()
        .validate(SvgContextLimits::default())
        .unwrap();
    let selector = render_symbol_svg(
        &symbol,
        VIEWPORT,
        &selector_context,
        SvgRenderLimits::default(),
    )
    .unwrap_err();
    assert_eq!(selector.kind(), SvgErrorKind::UnsupportedSelector);

    let mut empty = footprint.clone();
    empty.total_operations = 0;
    for record in &mut empty.records {
        record.operations.clear();
        record.operation_count = 0;
    }
    let empty_bounds = render_footprint_svg(
        &empty,
        ViewportPolicy::Fit(SvgFitOptions {
            padding_nm: 0,
            min_extent_nm: 1,
            fallback: None,
        }),
        context.get(),
        SvgRenderLimits::default(),
    )
    .unwrap_err();
    assert_eq!(empty_bounds.kind(), SvgErrorKind::EmptyBounds);

    let overflow = render_footprint_svg(
        &footprint,
        ViewportPolicy::Fit(SvgFitOptions {
            padding_nm: u64::MAX,
            min_extent_nm: 1,
            fallback: None,
        }),
        context.get(),
        SvgRenderLimits::default(),
    )
    .unwrap_err();
    assert_eq!(overflow.kind(), SvgErrorKind::ArithmeticOverflow);

    let mut unserializable = footprint.clone();
    unserializable.records[0].uuid.push('\u{0001}');
    let serialization = render_footprint_svg(
        &unserializable,
        VIEWPORT,
        context.get(),
        SvgRenderLimits::default(),
    )
    .unwrap_err();
    assert_eq!(serialization.kind(), SvgErrorKind::Serialization);

    let invalid_board: BoardPlotDocumentA0 = serde_json::from_value(document_by_id(
        "board_plotter_a0_vectors.json",
        "tracks-follow-graphics-with-net-extras",
    ))
    .unwrap();
    let invalid_document = render_board_document_svg(
        &invalid_board,
        VIEWPORT,
        context.get(),
        SvgRenderLimits::default(),
    )
    .unwrap_err();
    assert_eq!(invalid_document.kind(), SvgErrorKind::InvalidDocument);

    let mut unbalanced_value = document_by_id(
        "schematic_plotter_a0_vectors.json",
        "placed-symbols-pins-fields-dnp-and-overplots",
    );
    let records = unbalanced_value["records"].as_array_mut().unwrap();
    let record = records
        .iter_mut()
        .find(|record| {
            record["operations"]
                .as_array()
                .and_then(|operations| operations.last())
                .is_some_and(|operation| operation["kind"] == "EndBlock")
        })
        .expect("blocked schematic record");
    let operations = record["operations"].as_array_mut().unwrap();
    let end = operations
        .iter()
        .rposition(|operation| operation["kind"] == "EndBlock")
        .unwrap();
    operations.remove(end);
    record["operation_count"] = json!(operations.len());
    let total = records
        .iter()
        .map(|record| record["operation_count"].as_u64().unwrap())
        .sum::<u64>();
    unbalanced_value["total_operations"] = json!(total);
    let unbalanced: SchematicPlotDocumentA0 =
        serde_json::from_value(unbalanced_value).expect("shape-valid unbalanced document");
    let block = render_schematic_svg(
        &unbalanced,
        VIEWPORT,
        context.get(),
        SvgRenderLimits::default(),
    )
    .unwrap_err();
    assert_eq!(block.kind(), SvgErrorKind::UnbalancedBlock, "{block}");
}

struct ValidatedContext(kicad_monkey_svg::ValidatedSvgRenderContextA1);

impl Default for ValidatedContext {
    fn default() -> Self {
        Self(
            SvgRenderContextA1::default()
                .validate(SvgContextLimits::default())
                .unwrap(),
        )
    }
}

impl ValidatedContext {
    fn get(&self) -> &kicad_monkey_svg::ValidatedSvgRenderContextA1 {
        &self.0
    }
}

fn first_document(file: &str) -> Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let parsed: Value = serde_json::from_slice(
        &fs::read(root.join("tests/parity").join(file)).expect("read vector"),
    )
    .expect("decode vectors");
    parsed["vectors"][0]["expected"].clone()
}

fn documents(file: &str) -> Vec<(String, Value)> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let parsed: Value = serde_json::from_slice(
        &fs::read(root.join("tests/parity").join(file)).expect("read vector"),
    )
    .expect("decode vectors");
    parsed["vectors"]
        .as_array()
        .expect("vectors")
        .iter()
        .map(|vector| {
            (
                vector["id"].as_str().expect("vector id").to_owned(),
                vector["expected"].clone(),
            )
        })
        .collect()
}

fn document_by_id(file: &str, id: &str) -> Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let parsed: Value = serde_json::from_slice(
        &fs::read(root.join("tests/parity").join(file)).expect("read vector"),
    )
    .expect("decode vectors");
    parsed["vectors"]
        .as_array()
        .expect("vectors")
        .iter()
        .find(|vector| vector["id"] == id)
        .expect("vector id")["expected"]
        .clone()
}

fn test_mm(value_nm: u64) -> String {
    let whole = value_nm / 1_000_000;
    let fraction = value_nm % 1_000_000;
    if fraction == 0 {
        whole.to_string()
    } else {
        format!("{whole}.{fraction:06}")
            .trim_end_matches('0')
            .to_owned()
    }
}

fn native_svg_expected(family: &str, document_id: &str) -> (SvgViewport, Option<(usize, String)>) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let parsed: Value = serde_json::from_slice(
        &fs::read(root.join("tests/parity/native_svg_a0_vectors.json"))
            .expect("read native SVG freeze"),
    )
    .expect("decode native SVG freeze");
    let id = format!("{family}:{document_id}");
    let case = parsed["cases"]
        .as_array()
        .expect("native cases")
        .iter()
        .find(|case| case["id"] == id)
        .unwrap_or_else(|| panic!("missing frozen native SVG case {id}"));
    (
        SvgViewport {
            min_x_nm: case["viewport"]["min_x_nm"].as_i64().unwrap(),
            min_y_nm: case["viewport"]["min_y_nm"].as_i64().unwrap(),
            width_nm: case["viewport"]["width_nm"].as_u64().unwrap(),
            height_nm: case["viewport"]["height_nm"].as_u64().unwrap(),
        },
        (case["outcome"] == "svg").then(|| {
            (
                case["svg_bytes"].as_u64().unwrap().try_into().unwrap(),
                case["svg_sha256"].as_str().unwrap().to_owned(),
            )
        }),
    )
}

fn legacy_request(document: Value, family: &str, viewport: SvgViewport) -> Value {
    json!({
        "type": "kicad_monkey.native.svg.request",
        "version": "a0",
        "profile": "plotter-base-a0",
        "document": {"kind": family, "value": document},
        "viewport": {
            "min_x_nm": viewport.min_x_nm,
            "min_y_nm": viewport.min_y_nm,
            "width_nm": viewport.width_nm,
            "height_nm": viewport.height_nm
        },
        "limits": {
            "max_records": 1000000,
            "max_operations": 4000000,
            "max_points": "16000000",
            "max_text_bytes": "268435456",
            "max_image_encoded_bytes": "268435456",
            "max_block_depth": 4096,
            "max_svg_elements": "8000000",
            "max_render_work": "64000000",
            "max_svg_bytes": "536870912",
            "max_result_bytes": "805306368"
        }
    })
}

fn first_document_with_operations(file: &str) -> Value {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let parsed: Value = serde_json::from_slice(
        &fs::read(root.join("tests/parity").join(file)).expect("read vector"),
    )
    .expect("decode vectors");
    parsed["vectors"]
        .as_array()
        .expect("vectors")
        .iter()
        .find_map(|vector| {
            (vector["expected"]["total_operations"].as_u64().unwrap_or(0) > 0)
                .then(|| vector["expected"].clone())
        })
        .expect("vector with operations")
}
