use kicad_monkey_contracts::generated::shaping_record::ShapingInput;
use kicad_monkey_core::{
    BoardDimensionOperation, BoardFootprintOperation, BoardPlotLimits, BoardPlotRecord,
    BoardTableOperation, BoardTextBoxOperation, BoardTextRenderCacheCoordinateSpace,
    BoardTextRenderCacheSource, BoardTextVariables, ErrorKind, PlotterTextCacheLimits,
    PlotterTextCacheResources, PlotterTextFont, TextBlockLayoutRequest, TextHorizontalAlignment,
    TextRenderCache, TextVerticalAlignment, board_plot_document_with_sidecars,
    board_plot_document_with_text_cache_sidecar, generate_text_render_cache_block_hinted_a0,
    linebreak_text_block_hinted_a0,
};
use serde::Deserialize;

const FONT_BYTES: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../../assets/fonts/kicad-stroke.ttf"
));

#[derive(Deserialize)]
struct Vectors {
    records: Vec<Record>,
}

#[test]
fn embedded_footprint_faced_text_generates_a_local_native_cache() {
    let source = r#"(kicad_pcb
      (footprint "Demo:Native" (at 10 20 90) (uuid "native-footprint")
        (property "Reference" "U1" (at 1 2 45) (layer "B.SilkS")
          (effects (font (face "Native Fixture") (size 1 1)))
          (uuid "native-reference"))))"#;
    let fonts = [font()];
    let resources = PlotterTextCacheResources {
        fonts: &fonts,
        limits: PlotterTextCacheLimits::default(),
    };
    let document = board_plot_document_with_text_cache_sidecar(
        source,
        BoardPlotLimits::default(),
        &Default::default(),
        &BoardTextVariables::default(),
        Some(&resources),
    )
    .expect("embedded footprint native cache");
    let BoardPlotRecord::Footprint(record) = &document.records[0] else {
        panic!("expected footprint record");
    };
    let BoardFootprintOperation::Text { operation, .. } = &record.operations[0] else {
        panic!("expected footprint text operation");
    };
    let cache = operation.render_cache.as_ref().expect("native cache");
    assert_eq!(
        cache.coordinate_space,
        BoardTextRenderCacheCoordinateSpace::FootprintLocal
    );
    assert_eq!(cache.source, BoardTextRenderCacheSource::NativeGenerated);
    assert!(!cache.exact);
    assert_eq!(cache.text, "U1");
    assert!(!cache.polygons.is_empty());
    assert_eq!(operation.render_cache_polygons.len(), cache.polygons.len());

    let error = board_plot_document_with_text_cache_sidecar(
        source,
        BoardPlotLimits {
            max_text_bytes: 3,
            ..BoardPlotLimits::default()
        },
        &Default::default(),
        &BoardTextVariables::default(),
        Some(&resources),
    )
    .expect_err("operation and native cache retain two copies of the text");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
}

#[derive(Deserialize)]
struct Record {
    shaping: ShapingInput,
}

#[derive(Deserialize)]
struct SidecarVectors {
    vectors: Vec<SidecarVector>,
}

#[derive(Deserialize)]
struct SidecarVector {
    source: String,
    expected_cache_sources: Vec<String>,
}

fn shaping(text: &str) -> ShapingInput {
    let vectors: Vectors = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/text_layout_vectors.json"
    )))
    .unwrap();
    let mut shaping = vectors.records.into_iter().next().unwrap().shaping;
    shaping.text = text.to_owned();
    shaping.features.clear();
    shaping
}

fn font() -> PlotterTextFont<'static> {
    PlotterTextFont {
        face: "Native Fixture",
        bold: false,
        italic: false,
        font_bytes: FONT_BYTES,
        shaping: shaping(""),
        fake_bold: false,
        fake_italic: false,
    }
}

fn request<'a>(shaping: &'a ShapingInput) -> TextBlockLayoutRequest<'a> {
    TextBlockLayoutRequest {
        shaping,
        size_x: 1.0,
        size_y: 1.0,
        position_x: 0.0,
        position_y: 0.0,
        angle_degrees: 0.0,
        mirrored: false,
        horizontal_alignment: TextHorizontalAlignment::Left,
        vertical_alignment: TextVerticalAlignment::Top,
        line_spacing: 1.0,
        stroke_width: 0.125,
        max_error: 2.0,
        fake_bold: false,
        fake_italic: false,
    }
}

#[test]
fn native_outline_linebreaker_matches_kicad_pending_space_rules() {
    for (source, expected) in [
        ("", ""),
        ("A", "A"),
        ("A\n", "A\n"),
        ("\n", "\n"),
        ("A A", "A\nA"),
        ("A A  A", "A\nA\n\nA"),
        ("A A\nB B", "A\nA\nB\nB"),
        ("A _{A A}", "A\n_{A A}"),
        ("~{A A} A", "~{A A}\nA"),
    ] {
        let input = shaping(source);
        let wrapped =
            linebreak_text_block_hinted_a0(FONT_BYTES, request(&input), 1.5, Default::default())
                .expect("bounded outline linebreak");
        assert_eq!(wrapped, expected, "source {source:?}");
    }
}

#[test]
fn board_text_text_box_and_table_generate_native_caches_from_one_sidecar() {
    let source = r#"(kicad_pcb
      (gr_text "A" (at 10 10) (layer "F.SilkS")
        (effects (font (face "Native Fixture") (size 1 1))))
      (gr_text_box "A A" (start 0 0) (end 1.5 2) (layer "F.SilkS")
        (effects (font (face "Native Fixture") (size 1 1)) (justify left top)))
      (table (column_count 1) (layer "F.Cu")
        (cells (table_cell "A A" (start 0 0) (end 1.5 2)
          (effects (font (face "Native Fixture") (size 1 1)) (justify left top))))))"#;
    let fonts = [font()];
    let resources = PlotterTextCacheResources {
        fonts: &fonts,
        limits: PlotterTextCacheLimits::default(),
    };
    let document = board_plot_document_with_text_cache_sidecar(
        source,
        BoardPlotLimits::default(),
        &Default::default(),
        &BoardTextVariables::default(),
        Some(&resources),
    )
    .expect("native cache carrier bridge");

    let BoardPlotRecord::Text(text) = &document.records[0] else {
        panic!("expected board text");
    };
    assert_native_cache(&text.operations[0]);

    let BoardPlotRecord::TextBox(text_box) = &document.records[1] else {
        panic!("expected board text box");
    };
    let BoardTextBoxOperation::Text(text_box_op) = &text_box.operations[0] else {
        panic!("expected text-box text");
    };
    assert_eq!(text_box_op.text, "A\nA");
    assert_native_cache(text_box_op);

    let BoardPlotRecord::Table(table) = &document.records[2] else {
        panic!("expected table");
    };
    let table_op = table
        .operations
        .iter()
        .find_map(|operation| match operation {
            BoardTableOperation::Text(value) => Some(value),
            BoardTableOperation::Segment(_) => None,
        })
        .expect("expected table text");
    assert_eq!(table_op.text, "A\nA");
    assert_native_cache(table_op);
}

fn assert_native_cache(operation: &kicad_monkey_core::BoardTextOperation) {
    let cache = operation.render_cache.as_ref().expect("generated cache");
    assert_eq!(cache.source, BoardTextRenderCacheSource::NativeGenerated);
    assert!(!cache.exact);
    assert_eq!(cache.text, operation.text);
    assert!(!cache.polygons.is_empty());
    assert_eq!(cache.polygons.len(), operation.render_cache_polygons.len());
}

#[test]
fn carrier_bridge_prefers_matching_and_regenerates_stale_or_missing_caches() {
    let vectors: SidecarVectors = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../tests/parity/board_text_cache_sidecar_vectors.json"
    )))
    .unwrap();
    let vector = &vectors.vectors[0];
    let fonts = [font()];
    let resources = PlotterTextCacheResources {
        fonts: &fonts,
        limits: PlotterTextCacheLimits::default(),
    };
    let document = board_plot_document_with_text_cache_sidecar(
        &vector.source,
        BoardPlotLimits::default(),
        &Default::default(),
        &BoardTextVariables::default(),
        Some(&resources),
    )
    .expect("all cache provenance paths");
    let sources = document
        .records
        .iter()
        .map(|record| match record {
            BoardPlotRecord::Text(text) => {
                text.operations[0]
                    .render_cache
                    .as_ref()
                    .expect("cache")
                    .source
            }
            BoardPlotRecord::TextBox(text_box) => {
                text_box
                    .operations
                    .iter()
                    .find_map(|operation| match operation {
                        BoardTextBoxOperation::Text(text) => text.render_cache.as_ref(),
                        BoardTextBoxOperation::Border(_) => None,
                    })
                    .expect("text-box cache")
                    .source
            }
            BoardPlotRecord::Table(table) => {
                table
                    .operations
                    .iter()
                    .find_map(|operation| match operation {
                        BoardTableOperation::Text(text) => text.render_cache.as_ref(),
                        BoardTableOperation::Segment(_) => None,
                    })
                    .expect("table cache")
                    .source
            }
            BoardPlotRecord::Dimension(dimension) => {
                dimension
                    .operations
                    .iter()
                    .find_map(|operation| match operation {
                        BoardDimensionOperation::Text(text) => text.render_cache.as_ref(),
                        BoardDimensionOperation::Geometry(_) => None,
                    })
                    .expect("dimension cache")
                    .source
            }
            _ => panic!("expected text carrier record"),
        })
        .collect::<Vec<_>>();
    let actual = sources
        .iter()
        .map(|source| match source {
            BoardTextRenderCacheSource::ExistingFile => "existing_file_cache",
            BoardTextRenderCacheSource::NativeGenerated => "native_generated_cache",
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, vector.expected_cache_sources);
}

#[test]
fn no_sidecar_and_whitespace_paths_preserve_legacy_cacheless_output() {
    let no_sidecar = r#"(kicad_pcb
      (gr_text_box "A A" (start 0 0) (end 1.5 2) (layer "F.SilkS")
        (effects (font (face "Native Fixture") (size 1 1)))))"#;
    let document = board_plot_document_with_sidecars(
        no_sidecar,
        BoardPlotLimits::default(),
        &Default::default(),
        &BoardTextVariables::default(),
    )
    .expect("legacy no-sidecar path");
    let BoardPlotRecord::TextBox(record) = &document.records[0] else {
        panic!("expected text box");
    };
    let BoardTextBoxOperation::Text(operation) = &record.operations[0] else {
        panic!("expected text operation");
    };
    assert_eq!(operation.text, "A A");
    assert!(operation.multiline);
    assert!(operation.render_cache.is_none());

    let whitespace = r#"(kicad_pcb
      (gr_text "   " (at 0 0) (layer "F.SilkS")
        (effects (font (face "Native Fixture") (size 1 1)))))"#;
    let fonts = [font()];
    let resources = PlotterTextCacheResources {
        fonts: &fonts,
        limits: PlotterTextCacheLimits::default(),
    };
    let document = board_plot_document_with_text_cache_sidecar(
        whitespace,
        BoardPlotLimits::default(),
        &Default::default(),
        &BoardTextVariables::default(),
        Some(&resources),
    )
    .expect("whitespace carrier");
    let BoardPlotRecord::Text(record) = &document.records[0] else {
        panic!("expected text");
    };
    assert!(record.operations[0].render_cache.is_none());
}

#[test]
fn carrier_geometry_matches_python_text_params_mapping() {
    let source = r#"(kicad_pcb
      (gr_text_box "R R" (start 0 0) (end 4 2) (angle 90) (margins 0.2 0.3 0.4 0.5)
        (stroke (width 0.2) (type solid)) (layer "F.SilkS")
        (effects (font (face "Native Fixture") (size 1 1) (line_spacing 0)) (justify right bottom mirror)))
      (table (column_count 1) (layer "F.Cu")
        (cells (table_cell "R R" (start 10 0) (end 11.5 2)
          (effects (font (face "Native Fixture") (size 1 1) (line_spacing 0)))))))"#;
    let fonts = [font()];
    let resources = PlotterTextCacheResources {
        fonts: &fonts,
        limits: PlotterTextCacheLimits::default(),
    };
    let document = board_plot_document_with_text_cache_sidecar(
        source,
        BoardPlotLimits::default(),
        &Default::default(),
        &BoardTextVariables::default(),
        Some(&resources),
    )
    .expect("geometry-sensitive carriers");

    let BoardPlotRecord::TextBox(text_box) = &document.records[0] else {
        panic!("expected text box");
    };
    let BoardTextBoxOperation::Text(text_box_op) = &text_box.operations[0] else {
        panic!("expected text-box operation");
    };
    assert_eq!(text_box_op.text, "R\nR");
    assert_cache_matches_layout(
        text_box_op,
        3.6,
        1.9,
        TextHorizontalAlignment::Right,
        TextVerticalAlignment::Bottom,
        true,
    );

    let BoardPlotRecord::Table(table) = &document.records[1] else {
        panic!("expected table");
    };
    let table_op = table
        .operations
        .iter()
        .find_map(|operation| match operation {
            BoardTableOperation::Text(value) => Some(value),
            BoardTableOperation::Segment(_) => None,
        })
        .expect("table text");
    assert_eq!(table_op.text, "R\nR");
    assert_cache_matches_layout(
        table_op,
        10.75,
        1.0,
        TextHorizontalAlignment::Center,
        TextVerticalAlignment::Center,
        false,
    );
}

fn assert_cache_matches_layout(
    operation: &kicad_monkey_core::BoardTextOperation,
    x: f64,
    y: f64,
    horizontal_alignment: TextHorizontalAlignment,
    vertical_alignment: TextVerticalAlignment,
    mirrored: bool,
) {
    let input = shaping(&operation.text);
    let expected = generate_text_render_cache_block_hinted_a0(
        FONT_BYTES,
        TextBlockLayoutRequest {
            shaping: &input,
            size_x: operation.size_x_nm as f64 / 1_000_000.0,
            size_y: operation.size_y_nm as f64 / 1_000_000.0,
            position_x: x,
            position_y: y,
            angle_degrees: operation.orient_deg,
            mirrored,
            horizontal_alignment,
            vertical_alignment,
            line_spacing: 1.0,
            stroke_width: operation.pen_width_nm as f64 / 1_000_000.0,
            max_error: 2.0,
            fake_bold: false,
            fake_italic: false,
        },
        Default::default(),
        Default::default(),
    )
    .expect("expected direct cache");
    assert_eq!(
        operation
            .render_cache
            .as_ref()
            .expect("carrier cache")
            .polygons,
        cache_polygons_nm(expected)
    );
}

fn cache_polygons_nm(cache: TextRenderCache) -> Vec<Vec<Vec<[i64; 2]>>> {
    cache
        .polygons
        .into_iter()
        .filter_map(|polygon| {
            let contours = polygon
                .contours
                .into_iter()
                .filter(|contour| contour.points.len() >= 3)
                .map(|contour| {
                    contour
                        .points
                        .into_iter()
                        .map(|point| {
                            [
                                (point.x * 1_000_000.0).round_ties_even() as i64,
                                (point.y * 1_000_000.0).round_ties_even() as i64,
                            ]
                        })
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            (!contours.is_empty()).then_some(contours)
        })
        .collect()
}

#[test]
fn sidecar_font_and_hash_limits_fail_closed_before_publication() {
    let source = r#"(kicad_pcb
      (gr_text "A" (at 0 0) (layer "F.SilkS")
        (effects (font (face "Native Fixture") (size 1 1)))))"#;
    let fonts = [font()];
    let resources = PlotterTextCacheResources {
        fonts: &fonts,
        limits: PlotterTextCacheLimits {
            max_fonts: 0,
            ..PlotterTextCacheLimits::default()
        },
    };
    let error = board_plot_document_with_text_cache_sidecar(
        source,
        BoardPlotLimits::default(),
        &Default::default(),
        &BoardTextVariables::default(),
        Some(&resources),
    )
    .expect_err("font selection ceiling");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);

    let resources = PlotterTextCacheResources {
        fonts: &fonts,
        limits: PlotterTextCacheLimits {
            max_hash_bytes: FONT_BYTES.len(),
            ..PlotterTextCacheLimits::default()
        },
    };
    let error = board_plot_document_with_text_cache_sidecar(
        source,
        BoardPlotLimits::default(),
        &Default::default(),
        &BoardTextVariables::default(),
        Some(&resources),
    )
    .expect_err("per-document font hash-work ceiling");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);

    let single_word_box = r#"(kicad_pcb
      (gr_text_box "A" (start 0 0) (end 2 2) (layer "F.SilkS")
        (effects (font (face "Native Fixture") (size 1 1)))))"#;
    let resources = PlotterTextCacheResources {
        fonts: &fonts,
        limits: PlotterTextCacheLimits {
            // One validation hash plus the two hashes used by cache generation.
            // The no-op single-word linebreaker must not consume another session.
            max_hash_bytes: FONT_BYTES.len() * 3,
            ..PlotterTextCacheLimits::default()
        },
    };
    board_plot_document_with_text_cache_sidecar(
        single_word_box,
        BoardPlotLimits::default(),
        &Default::default(),
        &BoardTextVariables::default(),
        Some(&resources),
    )
    .expect("exact hash budget for a single-word text box");

    let resources = PlotterTextCacheResources {
        fonts: &fonts,
        limits: PlotterTextCacheLimits {
            max_font_bytes: FONT_BYTES.len() - 1,
            ..PlotterTextCacheLimits::default()
        },
    };
    let error = board_plot_document_with_text_cache_sidecar(
        source,
        BoardPlotLimits::default(),
        &Default::default(),
        &BoardTextVariables::default(),
        Some(&resources),
    )
    .expect_err("aggregate font-byte ceiling");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
}

#[test]
fn sidecar_identity_and_engine_limits_fail_closed_before_publication() {
    let source = r#"(kicad_pcb
      (gr_text "A" (at 0 0) (layer "F.SilkS")
        (effects (font (face "Native Fixture") (size 1 1)))))"#;
    let fonts = [font()];

    let mut invalid_font = font();
    invalid_font.face = "Unused Fixture";
    invalid_font.shaping.font_sha256.0 = "0".repeat(64);
    let invalid_fonts = [invalid_font];
    let invalid_resources = PlotterTextCacheResources {
        fonts: &invalid_fonts,
        limits: PlotterTextCacheLimits::default(),
    };
    let error = board_plot_document_with_text_cache_sidecar(
        "not-a-board",
        BoardPlotLimits::default(),
        &Default::default(),
        &BoardTextVariables::default(),
        Some(&invalid_resources),
    )
    .expect_err("unused font identity is authenticated before board parsing");
    assert_eq!(error.kind, ErrorKind::InvalidBuildValue);

    let resources = PlotterTextCacheResources {
        fonts: &fonts,
        limits: PlotterTextCacheLimits {
            cache: kicad_monkey_core::TextRenderCacheLimits {
                max_points: 0,
                ..Default::default()
            },
            ..PlotterTextCacheLimits::default()
        },
    };
    let error = board_plot_document_with_text_cache_sidecar(
        source,
        BoardPlotLimits::default(),
        &Default::default(),
        &BoardTextVariables::default(),
        Some(&resources),
    )
    .expect_err("cache point ceiling");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);

    let resources = PlotterTextCacheResources {
        fonts: &fonts,
        limits: PlotterTextCacheLimits::default(),
    };
    let error = board_plot_document_with_text_cache_sidecar(
        source,
        BoardPlotLimits {
            max_cache_polygons: 0,
            ..BoardPlotLimits::default()
        },
        &Default::default(),
        &BoardTextVariables::default(),
        Some(&resources),
    )
    .expect_err("board polygon ceiling reaches native generation");
    assert_eq!(error.kind, ErrorKind::ResourceLimit);
}
