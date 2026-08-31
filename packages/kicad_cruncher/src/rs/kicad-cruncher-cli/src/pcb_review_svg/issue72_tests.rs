use super::*;

#[test]
fn resolved_via_apertures_do_not_reexpand_to_the_physical_drill_span() {
    let document = json!({
        "schema": "kicad.plotter_ir.a0",
        "records": [{
            "kind": "via",
            "layers": ["F.Cu", "B.Cu"],
            "operation_count": 2,
            "operations": [
                {"kind": "FlashPadCircle", "role": "via_aperture", "layers": ["In2.Cu"]},
                {"kind": "Circle", "role": "via_drill", "layers": ["F.Cu", "B.Cu"]}
            ]
        }],
        "total_operations": 2
    });
    let filtered = filter_document(
        &document,
        &["In1.Cu".to_owned()],
        PcbReviewSvgLimits::default(),
        &mut FilterWork::new(usize::MAX),
    )
    .expect("filter resolved via");
    let operations = filtered["records"][0]["operations"]
        .as_array()
        .expect("filtered operations");
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0]["role"], "via_drill");
}

#[test]
fn inner_copper_keeps_pad_drills_outside_authored_pad_copper_membership() {
    let document = json!({
        "schema": "kicad.plotter_ir.a0",
        "records": [{
            "kind": "footprint",
            "operation_count": 6,
            "operations": [
                {"kind": "StartBlock", "layers": ["F.Cu", "B.Cu"]},
                {"kind": "FlashPadCircle", "layers": ["F.Cu", "B.Cu"]},
                {"kind": "EndBlock"},
                {"kind": "StartBlock", "layers": ["F.Cu", "In1.Cu", "B.Cu"]},
                {"kind": "Circle", "role": "pad_drill", "layers": ["F.Cu", "In1.Cu", "B.Cu"]},
                {"kind": "EndBlock"}
            ]
        }],
        "total_operations": 6
    });
    let filtered = filter_document(
        &document,
        &["In1.Cu".to_owned()],
        PcbReviewSvgLimits::default(),
        &mut FilterWork::new(usize::MAX),
    )
    .expect("filter pad drill");
    let operations = filtered["records"][0]["operations"]
        .as_array()
        .expect("filtered operations");
    assert_eq!(operations.len(), 3);
    assert_eq!(operations[1]["role"], "pad_drill");
}

#[test]
fn via_hole_layers_exclude_effective_aperture_only_layers() {
    let record = json!({
        "kind": "via",
        "layers": ["F.Cu", "B.Cu"],
        "operations": [
            {"kind": "FlashPadCircle", "role": "via_aperture", "layers": ["F.Cu", "In1.Cu", "B.Cu"]},
            {"kind": "Circle", "role": "via_drill", "layers": ["F.Cu", "B.Cu"]},
            {"kind": "Circle", "role": "via_mask_drill", "layers": ["F.Mask"]}
        ]
    });
    assert_eq!(via_hole_layers(&record), ["F.Cu", "B.Cu", "F.Mask"]);
    let attrs = via_hole_attrs(&record);
    assert_eq!(
        attrs.get("data-layer-names").map(String::as_str),
        Some("F.Cu,B.Cu,F.Mask")
    );
}
