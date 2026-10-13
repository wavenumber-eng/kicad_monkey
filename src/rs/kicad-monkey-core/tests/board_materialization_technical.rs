//! Slice 3 evidence: solder mask, paste, and silkscreen, with KiCad's
//! margin precedence, per-shape aperture growth, via tenting, copper
//! graphics, knockout text, and rule 2 across the technical products.

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

/// Two copper layers, no stackup. Board mask expansion 0.05 mm, paste
/// margin -0.02 mm with ratio -0.1, front vias tented and back vias not.
/// Footprint U1 at (10, 10) carries a board-margin rect pad, a pad with an
/// explicit zero mask margin, a trapezoid, a through-hole pad whose copper
/// is offset from its drill, reference text, and a zero-width paste polygon.
const TECH: &str = r#"(kicad_pcb
  (version VERSION)
  (generator pcbnew)
  (general (thickness 1.6))
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (32 "B.Adhes" user) (34 "B.Paste" user)
    (35 "F.Paste" user) (36 "B.SilkS" user) (37 "F.SilkS" user) (38 "B.Mask" user)
    (39 "F.Mask" user) (44 "Edge.Cuts" user))
  (setup
    (pad_to_mask_clearance 0.05)
    (pad_to_paste_clearance -0.02)
    (pad_to_paste_clearance_ratio -0.1)
    (tenting (front yes) (back no)))
  (net 0 "")
  (net 1 "GND")
  (footprint "Demo:U"
    (layer "F.Cu")
    (at 10 10)
    (property "Reference" "U1" (at 0 -3 0) (layer "F.SilkS") (uuid ref-u1)
      (effects (font (size 1 1) (thickness 0.15))))
    (fp_poly (pts (xy 3 3) (xy 4 3) (xy 4 4) (xy 3 4))
      (stroke (width 0) (type solid)) (fill yes) (layer "F.Paste") (uuid poly-paste))
    (pad "1" smd rect (at -2 0) (size 1 2) (layers "F.Cu" "F.Paste" "F.Mask") (net 1 "GND")
      (uuid pad-rect))
    (pad "2" smd roundrect (at 0 0) (size 1 2) (roundrect_rratio 0.25)
      (layers "F.Cu" "F.Mask") (solder_mask_margin 0) (uuid pad-zero))
    (pad "3" smd trapezoid (at 2 0) (size 1 2) (rect_delta 0 0.4) (layers "F.Cu" "F.Mask")
      (uuid pad-trap))
    (pad "4" thru_hole circle (at 5 0) (size 1.6 1.6) (drill 0.6 (offset 0.3 0))
      (layers "*.Cu" "*.Mask") (net 1 "GND") (uuid pad-offset)))
  (gr_rect (start 0 0) (end 40 30) (stroke (width 0.1) (type solid)) (layer "Edge.Cuts"))
  (gr_line (start 1 1) (end 9 1) (stroke (width 0.15) (type solid)) (layer "F.SilkS")
    (uuid silk-line))
  (gr_rect (start 30 20) (end 34 22) (stroke (width 0) (type solid)) (fill yes)
    (layers "F.Cu" "F.Mask") (net 1 "GND") (uuid copper-rect))
  (gr_text "KO" (at 20 25 0) (layer "F.SilkS" knockout) (uuid knockout-text)
    (effects (font (size 1 1) (thickness 0.15)) (justify left bottom)))
  (via (at 20 5) (size 0.8) (drill 0.4) (layers "F.Cu" "B.Cu") (net 1) (uuid via-a))
)
"#;

fn board(version: &str) -> String {
    TECH.replace("VERSION", version)
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

#[test]
fn mask_openings_follow_kicad_margins_and_shapes() {
    let document = materialize(&board("20241229"), &["solder_mask"]);
    let rect = bound(&document, "mask.front", "pad-rect");
    assert_eq!(rect.len(), 1);
    // A rect grown by a positive margin becomes a rounded rect of that radius.
    assert_eq!(
        rect[0]["shape"],
        json!({ "kind": "rounded_rect_2d", "center_nm": [0, 0], "width_nm": 1_100_000,
                "height_nm": 2_100_000, "radius_nm": 50_000 })
    );
    assert_eq!(rect[0]["action"], "subtract");
    // An explicit zero is an override in format 9.0 files.
    let zero = bound(&document, "mask.front", "pad-zero");
    assert_eq!(zero[0]["shape"]["width_nm"], 1_000_000);
    // A trapezoid keeps its shape and carries a round offset.
    let trap = bound(&document, "mask.front", "pad-trap");
    assert_eq!(trap[0]["shape"]["kind"], "trapezoid_2d");
    assert_eq!(trap[0]["offset_nm"], 50_000);
    // The board profile is the mask before openings, on both sides.
    for slice in ["mask.front", "mask.back"] {
        assert_eq!(bound(&document, slice, "board_profile#0").len(), 1);
    }
}

#[test]
fn zero_margins_inherit_in_pre_nine_files() {
    let document = materialize(&board("20231120"), &["solder_mask"]);
    let zero = bound(&document, "mask.front", "pad-zero");
    assert_eq!(zero[0]["shape"]["width_nm"], 1_100_000);
}

#[test]
fn vias_open_the_mask_only_where_untented() {
    let document = materialize(&board("20241229"), &["solder_mask"]);
    assert!(bound(&document, "mask.front", "via-a").is_empty());
    let back = bound(&document, "mask.back", "via-a");
    assert_eq!(back[0]["shape"]["radius_nm"], 450_000);
    // A legacy `viasonmask` overrides `(tenting ...)`, as KiCad parses it last.
    let legacy = board("20241229").replace(
        "(tenting (front yes) (back no)))",
        "(tenting (front yes) (back no)) (pcbplotparams (viasonmask yes)))",
    );
    let document = materialize(&legacy, &["solder_mask"]);
    assert_eq!(bound(&document, "mask.front", "via-a").len(), 1);
}

#[test]
fn paste_applies_margin_and_ratio_per_axis_and_keeps_zero_width_fills() {
    let document = materialize(&board("20241229"), &["paste"]);
    let rect = bound(&document, "paste.front", "pad-rect");
    // -0.02 mm plus -10 % of each axis: 1.0 -> 0.76 mm, 2.0 -> 1.56 mm.
    assert_eq!(
        rect[0]["shape"],
        json!({ "kind": "rect_2d", "center_nm": [0, 0], "width_nm": 760_000,
                "height_nm": 1_560_000 })
    );
    let poly = bound(&document, "paste.front", "poly-paste");
    let kinds: Vec<&Value> = poly.iter().map(|op| &op["shape"]["kind"]).collect();
    assert_eq!(
        kinds,
        [&json!("planar_region_2d")],
        "no stroke on a zero-width fill"
    );
}

#[test]
fn copper_graphics_are_copper_and_open_the_mask_with_the_board_margin() {
    let document = materialize(&board("20241229"), &["copper", "solder_mask"]);
    let copper = bound(&document, "copper.front", "copper-rect");
    assert_eq!(copper.len(), 1);
    assert_eq!(copper[0]["net_key"], "GND");
    let opening = bound(&document, "mask.front", "copper-rect");
    assert_eq!(
        opening.len(),
        1,
        "one opening, not the raw mask-layer copy too"
    );
    assert_eq!(opening[0]["offset_nm"], 50_000);
}

#[test]
fn pad_holes_sit_at_the_pad_not_at_the_copper_offset() {
    let document = materialize(&board("20241229"), &["copper", "holes"]);
    let operations = document["operations"].as_array().expect("operations");
    let hole = operations
        .iter()
        .find(|op| op["kind"] == "hole" && op["shape"]["radius_nm"] == 300_000)
        .expect("the offset pad's hole");
    assert_eq!(hole["affine"]["tx_nm"], 15_000_000);
    let land = bound(&document, "copper.front", "pad-offset");
    assert_eq!(land[0]["affine"]["tx_nm"], 15_300_000);
}

#[test]
fn silkscreen_draws_lines_text_and_knockout_boxes() {
    let document = materialize(&board("20241229"), &["silkscreen"]);
    let line = bound(&document, "silkscreen.front", "silk-line");
    assert_eq!(line[0]["shape"]["width_nm"], 150_000);
    assert!(!bound(&document, "silkscreen.front", "ref-u1").is_empty());
    let knockout = bound(&document, "silkscreen.front", "knockout-text");
    let adds = knockout.iter().filter(|op| op["action"] == "add").count();
    let cuts = knockout
        .iter()
        .filter(|op| op["action"] == "subtract")
        .count();
    assert_eq!(adds, 1, "one knockout box");
    assert!(cuts > 0, "stroke glyphs are cut from the box");
}

#[test]
fn technical_products_keep_rule_two_and_determinism() {
    let source = board("20241229");
    let complete = materialize_board(&source, &request(&ALL, &[])).expect("complete");
    let again = materialize_board(&source, &request(&ALL, &[])).expect("again");
    assert_eq!(
        serde_json::to_string(&complete).expect("serialize"),
        serde_json::to_string(&again).expect("serialize")
    );
    for products in [
        &["solder_mask"][..],
        &["paste"][..],
        &["silkscreen"][..],
        &["copper", "solder_mask"][..],
    ] {
        for slices in [
            &[][..],
            &["mask.front"][..],
            &["paste.front", "silkscreen.back"][..],
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

/// Review batch 1: exposed tracks, NPTH copper, custom-primitive strokes,
/// rotated and rounded outline rectangles, mask-layer-only copper exposure,
/// and knockout cuts confined to their own box.
const BATCH: &str = r#"(kicad_pcb
  (version 20241229)
  (generator pcbnew)
  (general (thickness 1.6))
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (36 "B.SilkS" user) (37 "F.SilkS" user)
    (38 "B.Mask" user) (39 "F.Mask" user) (44 "Edge.Cuts" user))
  (setup (pad_to_mask_clearance 0.05))
  (net 0 "")
  (net 1 "GND")
  (footprint "Demo:H"
    (layer "F.Cu")
    (at 10 10 45)
    (fp_rect (start -2 -1) (end 2 1) (stroke (width 0.05) (type solid)) (fill none)
      (layer "Edge.Cuts") (uuid rotated-cut))
    (pad "" np_thru_hole circle (at 0 0) (size 3 3) (drill 2) (layers "*.Cu" "*.Mask")
      (uuid npth-ring))
    (pad "" np_thru_hole circle (at 5 0) (size 2 2) (drill 2) (layers "*.Cu" "*.Mask")
      (uuid npth-bare))
    (pad "1" smd custom (at 0 5) (size 0.5 0.5) (layers "F.Cu")
      (options (clearance outline) (anchor circle))
      (primitives
        (gr_rect (start -1 -0.5) (end 1 0.5) (width 0.4) (fill yes))
        (gr_poly (pts (xy 2 0) (xy 3 0) (xy 3 1)) (width 0.1) (fill none)))
      (uuid custom-pad)))
  (gr_rect (start 0 0) (end 40 30) (stroke (width 0.05) (type solid)) (layer "Edge.Cuts"))
  (gr_rect (start 30 20) (end 36 26) (radius 1) (stroke (width 0.05) (type solid))
    (layer "Edge.Cuts") (uuid rounded-cut))
  (segment (start 20 5) (end 30 5) (width 0.25) (layers "F.Cu" "F.Mask") (net 1)
    (uuid exposed-track))
  (gr_rect (start 25 10) (end 27 12) (stroke (width 0) (type solid)) (fill yes)
    (layer "F.Cu") (solder_mask_margin 0.3) (net 1) (uuid margin-only))
  (gr_text "KO" (at 5 25 0) (layer "F.SilkS" knockout) (uuid knockout-text)
    (effects (font (size 1 1) (thickness 0.15)) (justify left bottom)))
)
"#;

#[test]
fn exposed_tracks_open_the_mask_by_their_width_plus_margins() {
    let document = materialize(BATCH, &["copper", "solder_mask"]);
    let copper = bound(&document, "copper.front", "exposed-track");
    assert_eq!(copper[0]["shape"]["width_nm"], 250_000);
    let opening = bound(&document, "mask.front", "exposed-track");
    assert_eq!(
        opening[0]["shape"]["width_nm"], 350_000,
        "0.25 + 2 x 0.05 mm"
    );
    assert_eq!(opening[0]["action"], "subtract");
    // A margin alone does not expose copper; KiCad needs the mask layer.
    assert!(bound(&document, "mask.front", "margin-only").is_empty());
}

#[test]
fn npth_pads_keep_copper_unless_the_drill_covers_them() {
    let document = materialize(BATCH, &["copper"]);
    for slice in ["copper.front", "copper.back"] {
        let ring = bound(&document, slice, "npth-ring");
        assert_eq!(ring[0]["shape"]["radius_nm"], 1_500_000);
        assert!(bound(&document, slice, "npth-bare").is_empty());
    }
}

#[test]
fn custom_primitives_follow_kicad_fill_and_stroke() {
    let document = materialize(BATCH, &["copper"]);
    let shapes: Vec<&Value> = bound(&document, "copper.front", "custom-pad")
        .into_iter()
        .map(|op| &op["shape"])
        .collect();
    // The filled rect grows by half its 0.4 mm stroke, with round corners.
    assert!(shapes.contains(&&json!({
        "kind": "rounded_rect_2d", "center_nm": [0, 0], "width_nm": 2_400_000,
        "height_nm": 1_400_000, "radius_nm": 200_000
    })));
    // The unfilled polygon is only its outline.
    let kinds: Vec<&Value> = shapes.iter().map(|shape| &shape["kind"]).collect();
    assert!(!kinds.contains(&&json!("planar_region_2d")));
    assert_eq!(
        kinds
            .iter()
            .filter(|kind| ***kind == json!("swept_path_2d"))
            .count(),
        1
    );
}

#[test]
fn outline_rectangles_rotate_with_their_footprint_and_keep_corner_radius() {
    let document = materialize(BATCH, &["dielectric"]);
    let region = document["operations"]
        .as_array()
        .expect("operations")
        .iter()
        .find(|op| op["action"] == "add")
        .expect("the board region");
    let holes = region["shape"]["holes"].as_array().expect("cutouts");
    assert_eq!(holes.len(), 2);
    let segment_kinds = |hole: &Value| -> Vec<String> {
        hole["segments"]
            .as_array()
            .expect("segments")
            .iter()
            .map(|segment| segment["kind"].as_str().expect("kind").to_owned())
            .collect()
    };
    // The 45-degree footprint's 4 x 2 mm rectangle keeps its size: its first
    // edge is 4 mm long whatever direction it points.
    let rotated = holes
        .iter()
        .find(|hole| segment_kinds(hole).len() == 3)
        .expect("the rotated rectangle");
    let start = &rotated["start_nm"];
    let first = &rotated["segments"][0]["end_nm"];
    let length = ((first[0].as_f64().unwrap() - start[0].as_f64().unwrap())
        .hypot(first[1].as_f64().unwrap() - start[1].as_f64().unwrap())
        / 1_000.0)
        .round();
    assert_eq!(length, 4_000.0, "first edge length in micrometres");
    let rounded = holes
        .iter()
        .find(|hole| segment_kinds(hole).len() == 8)
        .expect("the rounded rectangle");
    assert_eq!(
        segment_kinds(rounded)
            .iter()
            .filter(|kind| *kind == "arc")
            .count(),
        4
    );
}

#[test]
fn knockout_cuts_are_confined_to_their_own_box() {
    let document = materialize(BATCH, &["silkscreen"]);
    let knockout = bound(&document, "silkscreen.front", "knockout-text");
    assert!(knockout.len() > 1);
    assert!(
        knockout.iter().all(|op| op["group_key"] == "knockout-text"),
        "the box and its glyph cuts share one group"
    );
}
