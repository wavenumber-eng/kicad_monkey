use std::sync::Arc;

use kicad_cruncher_cli::pcb_svg::PcbSvgJob;
use kicad_monkey_core::{
    BoardNetClassAssignments, BoardPlotLimits, BoardTextVariables, PcbLimits, PlotDocumentMetadata,
    board_plot_artifact_with_sidecars,
};
use kicad_monkey_svg::{
    LayerPattern, LayerSelection, SvgBackground, SvgColor, SvgContextLimits, SvgRenderContextA1,
    SvgRenderLimits, SvgStyleOverride, SvgViewport,
};

const BOARD: &str = r#"(kicad_pcb (version 20240108) (generator pcbnew)
  (general (thickness 1.6)) (paper "A4")
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal))
  (segment (start 0 0) (end 1 0) (width 0.2) (layer "F.Cu") (net 0) (uuid "front"))
  (segment (start 2 0) (end 3 0) (width 0.2) (layer "B.Cu") (net 0) (uuid "back")))"#;

#[test]
fn repeated_layer_reuses_artifact_but_style_or_side_does_not() {
    let source = board_plot_artifact_with_sidecars(
        BOARD,
        BoardPlotLimits::default(),
        PcbLimits::default(),
        &BoardNetClassAssignments::default(),
        &BoardTextVariables::default(),
    )
    .unwrap();
    let mut job = PcbSvgJob::new(
        source,
        PlotDocumentMetadata {
            document_id: "cache-board".into(),
            source_path: None,
        },
        SvgRenderLimits::default(),
    )
    .unwrap();
    let viewport = SvgViewport {
        min_x_nm: -1_000_000,
        min_y_nm: -1_000_000,
        width_nm: 5_000_000,
        height_nm: 2_000_000,
    };
    let context = |layer: &str, color: &str| {
        SvgRenderContextA1::builder()
            .background(SvgBackground::Transparent)
            .layer_selection(LayerSelection::include(
                vec![LayerPattern::parse(layer).unwrap()],
                true,
            ))
            .fallback_style(SvgStyleOverride::new().with_stroke(SvgColor::parse(color).unwrap()))
            .build()
            .validate(SvgContextLimits::default())
            .unwrap()
    };
    let front = context("F.Cu", "#112233");
    let first = job.render_physical(viewport, &front).unwrap();
    let repeated = job.render_physical(viewport, &front).unwrap();
    assert!(Arc::ptr_eq(&first, &repeated));
    let changed = job
        .render_physical(viewport, &context("F.Cu", "#445566"))
        .unwrap();
    let back = job
        .render_physical(viewport, &context("B.Cu", "#112233"))
        .unwrap();
    assert_ne!(first.svg, changed.svg);
    assert_ne!(first.svg, back.svg);
    assert_eq!(job.profile().render_requests, 4);
    assert_eq!(job.profile().render_cache_hits, 1);
    assert!(!job.plot().document().records.is_empty());
}

#[test]
fn generated_config_keeps_extension_fields_and_component_overrides() {
    use kicad_cruncher_cli::pcb_svg::generated::PcbSvgConfig;
    let value = serde_json::json!({
        "schema": "kicad_cruncher.pcb_svg.config.a0",
        "global": {"styles": {"assembly_hlr": {"fast": {"crease_angle_rad": 0.4}},
                               "custom_style": {"custom": 7}}},
        "components": {"R1": {"show_designator": false}},
        "layer_outputs": {}, "views": [],
    });
    let config: PcbSvgConfig = serde_json::from_value(value).unwrap();
    let saved = serde_json::to_value(config).unwrap();
    assert_eq!(saved["global"]["styles"]["custom_style"]["custom"], 7);
    assert_eq!(saved["components"]["R1"]["show_designator"], false);
    assert_eq!(
        saved["global"]["styles"]["assembly_hlr"]["fast"]["crease_angle_rad"],
        0.4
    );
}

#[test]
fn generated_config_preserves_empty_overrides_and_nullable_presence() {
    use kicad_cruncher_cli::pcb_svg::generated::PcbSvgConfig;
    let base = serde_json::json!({
        "schema": "kicad_cruncher.pcb_svg.config.a0",
        "global": {}, "layer_outputs": {}, "views": [],
    });
    for explicit in [false, true] {
        let mut value = base.clone();
        if explicit {
            value["global"]["pcbdoc"] = serde_json::Value::Null;
            value["global"]["styles"] = serde_json::json!({});
            value["layer_outputs"]["include_special_layers"] = serde_json::json!([]);
            value["components"] = serde_json::json!({});
        }
        let config: PcbSvgConfig = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(config).unwrap(), value);
    }
    let mut invalid = base;
    invalid["layer_outputs"]["include_special_layers"] = serde_json::Value::Null;
    assert!(serde_json::from_value::<PcbSvgConfig>(invalid).is_err());
}

#[test]
fn model_reader_reports_external_and_unsupported_embedded_references() {
    use kicad_cruncher_cli::pcb_svg::models::read_embedded_models;
    use kicad_monkey_core::PcbView;
    let source = r#"(kicad_pcb (version 20240108) (generator pcbnew)
        (footprint "Test" (layer "F.Cu") (property "Reference" "J1")
            (model "local.step")
            (model "kicad-embed://body.wrl")
            (model "kicad-embed://missing.step")
            (model "hidden.step" (hide yes))))"#;
    let view = PcbView::parse(source, PcbLimits::default()).unwrap();
    let models = read_embedded_models(&view).unwrap();
    assert!(models.instances.is_empty());
    assert_eq!(models.warnings.len(), 3);
    assert_eq!(models.warnings[0].reference, "J1");
    assert!(models.warnings[0].to_string().contains("local.step"));
    assert!(models.warnings[1].to_string().contains(".wrl"));
    assert!(models.warnings[2].reason.contains("not found"));
}

#[test]
fn model_reader_keeps_owner_scope_and_shares_repeated_payloads() {
    use kicad_cruncher_cli::pcb_svg::models::read_embedded_models;
    use kicad_monkey_core::PcbView;
    // Small checksummed compressed payloads exercise resource resolution, not STEP parsing.
    let source = r#"(kicad_pcb
      (footprint "Local" (property "Reference" "U1")
        (model "kicad-embed://shared.step") (model "kicad-embed://shared.step")
        (embedded_files (file (name "shared.step") (type model)
          (data |KLUv/SAUoQAAZm9vdHByaW50IFNURVAgYnl0ZXM=|)
          (checksum "8b01e1044ceba757efd80099b23c46c609e2a08cef0e3246743cf6e478a8538d"))))
      (footprint "Board" (property "Reference" "U2")
        (model "kicad-embed://shared.step"))
      (embedded_files (file (name "shared.step") (type model)
        (data |KLUv/SAQgQAAYm9hcmQgU1RFUCBieXRlcw==|)
        (checksum "abe6fcb5893e97bd9171b378b38cfda491232881a51b4c8d6420296f1eed1d8c"))))"#;
    let view = PcbView::parse(source, PcbLimits::default()).unwrap();
    let models = read_embedded_models(&view).unwrap();
    assert!(models.warnings.is_empty());
    assert_eq!(models.instances.len(), 3);
    assert_eq!(&*models.instances[0].step, b"footprint STEP bytes");
    assert_eq!(&*models.instances[2].step, b"board STEP bytes");
    assert!(Arc::ptr_eq(
        &models.instances[0].step,
        &models.instances[1].step
    ));
    assert_ne!(models.instances[0].sha256, models.instances[2].sha256);
}
