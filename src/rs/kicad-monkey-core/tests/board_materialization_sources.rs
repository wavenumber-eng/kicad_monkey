//! Review batch 3 evidence: copper text, legacy stroked zone fills,
//! dielectric sub-layers, footprint zones, legacy per-via tenting, chamfer
//! flags on any pad shape, and custom-pad circle anchors.

use kicad_monkey_contracts::generated::materialization_request::MaterializationRequestA0;
use kicad_monkey_contracts::materialization_contract::validate_materialization_document;
use kicad_monkey_core::board_materialization::{filter_document, materialize_board};
use serde_json::{Value, json};

const ALL: [&str; 7] = [
    "components",
    "copper",
    "dielectric",
    "holes",
    "paste",
    "silkscreen",
    "solder_mask",
];

fn request(products: &[&str], slices: &[&str]) -> MaterializationRequestA0 {
    serde_json::from_value(json!({
        "type": "kicad_monkey.materialization.request",
        "version": "a0",
        "products": products,
        "slice_keys": slices,
        "tile_size_nm": 10_000_000,
        "max_source_bytes": "1048576",
        "max_operations": 1_000_000,
        "max_points": 10_000_000
    }))
    .expect("request decodes")
}

/// Two copper layers around a dielectric with two sub-layers. Board vias
/// are untented (`(tenting none)`). Copper text on both sides (the back one
/// knockout), footprint text on copper, a footprint zone with a stored
/// fill, chamfer flags on rect and circle pads, a custom pad with a circle
/// anchor, and three zones whose fills KiCad grows only in old formats
/// without `(filled_areas_thickness no)`.
const SOURCES: &str = r#"(kicad_pcb
  (version VERSION)
  (generator pcbnew)
  (general (thickness 1.6))
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (35 "F.Paste" user) (36 "B.SilkS" user)
    (37 "F.SilkS" user) (38 "B.Mask" user) (39 "F.Mask" user) (44 "Edge.Cuts" user))
  (setup
    (stackup
      (layer "F.SilkS" (type "Top Silk Screen"))
      (layer "F.Paste" (type "Top Solder Paste"))
      (layer "F.Mask" (type "Top Solder Mask") (thickness 0.01))
      (layer "F.Cu" (type "copper") (thickness 0.035))
      (layer "dielectric 1" (type "core") (thickness 0.5) (material "FR4") (epsilon_r 4.5)
        (loss_tangent 0.02) addsublayer (thickness 0.9 locked) (material "PP") (epsilon_r 3.8)
        (loss_tangent 0.01))
      (layer "B.Cu" (type "copper") (thickness 0.035))
      (layer "B.Mask" (type "Bottom Solder Mask") (thickness 0.01))
      (layer "B.SilkS" (type "Bottom Silk Screen")))
    (pad_to_mask_clearance 0)
    (tenting none))
  (net 0 "")
  (net 1 "GND")
  (gr_rect (start 0 0) (end 80 60) (stroke (width 0.05) (type solid)) (layer "Edge.Cuts"))
  (gr_text "COPPER" (at 20 10 0) (layer "F.Cu") (uuid cu-text)
    (effects (font (size 2 2) (thickness 0.3))))
  (gr_text "KO" (at 20 20 0) (layer "B.Cu" knockout) (uuid ko-text)
    (effects (font (size 2 2) (thickness 0.3)) (justify mirror)))
  (footprint "Demo:T"
    (layer "F.Cu")
    (at 30 40 90)
    (property "Reference" "T1" (at 0 -4 90) (layer "F.SilkS") (uuid ref-t1)
      (effects (font (size 1 1) (thickness 0.15))))
    (fp_text user "CU" (at 0 6 90) (layer "F.Cu") (uuid fp-cu-text)
      (effects (font (size 1.2 1.2) (thickness 0.2))))
    (pad "1" smd rect (at -3 0 90) (size 2 1.4) (layers "F.Cu" "F.Mask")
      (chamfer top_left bottom_right) (net 1 "GND") (uuid pad-rect-chamfer))
    (pad "2" smd circle (at 0 0 90) (size 1.6 1.6) (layers "F.Cu" "F.Mask")
      (chamfer_ratio 0.3) (chamfer top_right) (net 1 "GND") (uuid pad-circle-chamfer))
    (pad "3" smd custom (at 3 0 90) (size 2 1) (layers "F.Cu" "F.Mask") (net 1 "GND")
      (options (clearance outline) (anchor circle))
      (primitives (gr_rect (start 0 -0.3) (end 1.5 0.3) (width 0) (fill yes)))
      (uuid pad-custom-circle))
    (zone (net 0) (net_name "") (layer "F.Cu") (uuid fp-zone) (hatch edge 0.5)
      (connect_pads (clearance 0)) (min_thickness 0.25) (filled_areas_thickness no)
      (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
      (polygon (pts (xy 25 35) (xy 27 35) (xy 27 37) (xy 25 37)))
      (filled_polygon (layer "F.Cu") (pts (xy 25 35) (xy 27 35) (xy 27 37) (xy 25 37)))))
  (via (at 60 10) (size 0.8) (drill 0.4) (layers "F.Cu" "B.Cu") (tenting front) (net 1)
    (uuid via-front))
  (via (at 65 10) (size 0.8) (drill 0.4) (layers "F.Cu" "B.Cu") (tenting none) (net 1)
    (uuid via-none))
  (zone (net 1) (net_name "GND") (layer "F.Cu") (uuid legacy-cu) (hatch edge 0.5)
    (connect_pads (clearance 0.5)) (min_thickness 0.5)
    (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
    (polygon (pts (xy 50 30) (xy 70 30) (xy 70 50) (xy 50 50)))
    (filled_polygon (layer "F.Cu") (pts (xy 50.25 30.25) (xy 69.75 30.25) (xy 69.75 49.75)
      (xy 50.25 49.75))))
  (zone (net 0) (net_name "") (layer "F.SilkS") (uuid legacy-silk) (hatch edge 0.5)
    (connect_pads (clearance 0)) (min_thickness 0.4)
    (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
    (polygon (pts (xy 5 45) (xy 15 45) (xy 15 55) (xy 5 55)))
    (filled_polygon (layer "F.SilkS") (pts (xy 5.2 45.2) (xy 14.8 45.2) (xy 14.8 54.8)
      (xy 5.2 54.8))))
  (zone (net 0) (net_name "") (layer "B.SilkS") (uuid unstroked-silk) (hatch edge 0.5)
    (connect_pads (clearance 0)) (min_thickness 0.4) (filled_areas_thickness no)
    (fill yes (thermal_gap 0.5) (thermal_bridge_width 0.5))
    (polygon (pts (xy 20 45) (xy 30 45) (xy 30 55) (xy 20 55)))
    (filled_polygon (layer "B.SilkS") (pts (xy 20 45) (xy 30 45) (xy 30 55) (xy 20 55))))
)
"#;

/// A pre-20250210 format, whose stroked fills KiCad grows on load.
const LEGACY: &str = "20241229";

fn board(version: &str) -> String {
    SOURCES.replace("VERSION", version)
}

fn materialize(source: &str, products: &[&str]) -> Value {
    let document = materialize_board(source, &request(products, &[])).expect("materializes");
    validate_materialization_document(&document).expect("valid");
    serde_json::to_value(&document).expect("serialize")
}

/// Operations on `slice` bound to `uid`.
fn bound<'a>(document: &'a Value, slice: &str, uid: &str) -> Vec<&'a Value> {
    let binding = document["binding"].as_array().expect("binding");
    document["operations"]
        .as_array()
        .expect("operations")
        .iter()
        .filter(|op| op["slice_key"] == slice)
        .filter(|op| {
            binding
                .iter()
                .any(|entry| entry["operation_index"] == op["index"] && entry["source_uid"] == uid)
        })
        .collect()
}

fn diagnostic_uids(document: &Value, code: &str) -> Vec<String> {
    document["diagnostics"]
        .as_array()
        .expect("diagnostics")
        .iter()
        .filter(|diagnostic| diagnostic["code"] == code)
        .filter_map(|diagnostic| diagnostic["source_uid"].as_str().map(str::to_owned))
        .collect()
}

#[test]
fn copper_text_is_unconnected_copper_with_its_knockout_confined() {
    let document = materialize(&board(LEGACY), &["copper"]);
    for (slice, uid) in [
        ("copper.front", "cu-text"),
        ("copper.front", "fp-cu-text"),
        ("copper.back", "ko-text"),
    ] {
        let ops = bound(&document, slice, uid);
        assert!(!ops.is_empty(), "{uid} is drawn on {slice}");
        assert!(
            ops.iter()
                .all(|op| op["material"] == "copper" && op["net_key"] == "")
        );
    }
    let knockout = bound(&document, "copper.back", "ko-text");
    assert!(knockout.iter().any(|op| op["action"] == "subtract"));
    assert!(knockout.iter().all(|op| op["group_key"] == "ko-text"));
}

#[test]
fn legacy_stroked_fills_grow_by_half_their_min_thickness() {
    let offset = |version: &str, products: &[&str], slice: &str, uid: &str| -> Vec<Value> {
        let document = materialize(&board(version), products);
        bound(&document, slice, uid)
            .into_iter()
            .map(|op| op["offset_nm"].clone())
            .collect()
    };
    assert_eq!(
        offset(LEGACY, &["copper"], "copper.front", "legacy-cu"),
        [json!(250_000)]
    );
    assert_eq!(
        offset(LEGACY, &["silkscreen"], "silkscreen.front", "legacy-silk"),
        [json!(200_000)]
    );
    assert_eq!(
        offset(LEGACY, &["silkscreen"], "silkscreen.back", "unstroked-silk"),
        [Value::Null]
    );
    assert_eq!(
        offset("20250324", &["copper"], "copper.front", "legacy-cu"),
        [Value::Null]
    );
}

#[test]
fn dielectric_sub_layers_are_their_own_slices() {
    let document = materialize(&board(LEGACY), &["dielectric"]);
    let slices = document["stack"]["slices"].as_array().expect("slices");
    let slice = |key: &str| {
        slices
            .iter()
            .find(|slice| slice["key"] == key)
            .unwrap_or_else(|| panic!("{key} present"))
    };
    let first = slice("dielectric.1");
    assert_eq!(first["thickness_nm"], 500_000);
    assert_eq!(first["relative_permittivity"], 4.5);
    let second = slice("dielectric.1.sub2");
    assert_eq!(second["thickness_nm"], 900_000);
    assert_eq!(second["relative_permittivity"], 3.8);
    assert_eq!(second["material_name"], "PP");
    assert_eq!(second["order"], first["order"].as_u64().expect("order") + 1);
    for key in ["dielectric.1", "dielectric.1.sub2"] {
        assert_eq!(
            bound(&document, key, "via-front").len(),
            1,
            "the via is cut from {key}"
        );
    }
}

#[test]
fn footprint_zone_fills_are_reported_not_drawn() {
    let document = materialize(&board(LEGACY), &ALL);
    assert_eq!(
        diagnostic_uids(&document, "footprint_zone_fill_ignored"),
        ["fp-zone"]
    );
    let binding = document["binding"].as_array().expect("binding");
    assert!(binding.iter().all(|entry| entry["source_uid"] != "fp-zone"));
}

#[test]
fn legacy_via_tenting_tents_only_the_named_sides() {
    let document = materialize(&board(LEGACY), &["solder_mask"]);
    let openings = |slice: &str, uid: &str| bound(&document, slice, uid).len();
    assert_eq!(openings("mask.front", "via-front"), 0);
    assert_eq!(openings("mask.back", "via-front"), 1);
    assert_eq!(openings("mask.front", "via-none"), 1);
    assert_eq!(openings("mask.back", "via-none"), 1);
}

#[test]
fn chamfer_flags_chamfer_any_pad_and_circle_anchors_use_the_x_size() {
    let document = materialize(&board(LEGACY), &["copper", "solder_mask"]);
    for uid in ["pad-rect-chamfer", "pad-circle-chamfer"] {
        for slice in ["copper.front", "mask.front"] {
            let ops = bound(&document, slice, uid);
            assert_eq!(ops.len(), 1, "{uid} on {slice}");
            assert_eq!(ops[0]["shape"]["kind"], "chamfered_rect_2d");
        }
    }
    let rect = &bound(&document, "copper.front", "pad-rect-chamfer")[0]["shape"];
    assert_eq!(
        rect["chamfered_corners"],
        json!(["top_left", "bottom_right"])
    );
    // The rect pad keeps KiCad's default 0.25 rounding and 0.2 chamfer.
    assert_eq!(rect["radius_nm"], 350_000);
    assert_eq!(rect["chamfer_nm"], 280_000);
    let anchor = bound(&document, "copper.front", "pad-custom-circle")
        .into_iter()
        .find(|op| op["shape"]["kind"] == "disk_2d")
        .expect("circle anchor");
    assert_eq!(anchor["shape"]["radius_nm"], 1_000_000);
}

#[test]
fn batch_three_sources_keep_rule_two_and_determinism() {
    let source = board(LEGACY);
    let complete = materialize_board(&source, &request(&ALL, &[])).expect("complete");
    let again = materialize_board(&source, &request(&ALL, &[])).expect("again");
    assert_eq!(
        serde_json::to_string(&complete).expect("serialize"),
        serde_json::to_string(&again).expect("serialize")
    );
    for products in [
        &["copper"][..],
        &["dielectric"][..],
        &["silkscreen"][..],
        &["copper", "solder_mask"][..],
    ] {
        for slices in [
            &[][..],
            &["copper.back"][..],
            &["dielectric.1.sub2", "silkscreen.front"][..],
        ] {
            let selective = materialize_board(&source, &request(products, slices)).expect("ok");
            let filtered = filter_document(&complete, &selective.scope);
            assert_eq!(
                serde_json::to_value(&selective).expect("serialize"),
                serde_json::to_value(&filtered).expect("serialize"),
                "rule 2 for {products:?} on {slices:?}"
            );
        }
    }
}
