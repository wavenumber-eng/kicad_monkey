//! Text-box and table placement as KiCad 10.0 draws them: the text anchor
//! on the stroke-grown box, mirrored back-side text, KiCad's column width,
//! and no separator at the far edge of a negative row height.

use kicad_monkey_contracts::generated::materialization_request::MaterializationRequestA0;
use kicad_monkey_contracts::materialization_contract::validate_materialization_document;
use kicad_monkey_core::board_materialization::materialize_board;
use serde_json::{Value, json};

fn request(products: &[&str]) -> MaterializationRequestA0 {
    serde_json::from_value(json!({
        "type": "kicad_monkey.materialization.request",
        "version": "a0",
        "products": products,
        "tile_size_nm": 10_000_000,
        "max_source_bytes": "1048576",
        "max_operations": 1_000_000,
        "max_points": 10_000_000
    }))
    .expect("request decodes")
}

/// Two copper layers and both silkscreens; `BODY` is the items under test.
const BOARD: &str = r#"(kicad_pcb
  (version 20241229)
  (generator pcbnew)
  (general (thickness 1.6))
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (36 "B.SilkS" user) (37 "F.SilkS" user)
    (44 "Edge.Cuts" user))
  (net 0 "")
  (gr_rect (start 0 0) (end 80 80) (stroke (width 0.05) (type solid)) (layer "Edge.Cuts"))
  BODY
)
"#;

fn materialize(body: &str, products: &[&str]) -> Value {
    let source = BOARD.replace("BODY", body);
    let document = materialize_board(&source, &request(products)).expect("materializes");
    validate_materialization_document(&document).expect("valid");
    serde_json::to_value(&document).expect("serialize")
}

/// Board-frame centreline points of the swept paths of `width_nm` bound to
/// `uid` (text strokes; borders and separators use other widths).
fn strokes(document: &Value, uid: &str, width_nm: i64) -> Vec<(f64, f64)> {
    let binding = document["binding"].as_array().expect("binding");
    let mut points = Vec::new();
    for op in document["operations"].as_array().expect("operations") {
        let bound = binding
            .iter()
            .any(|entry| entry["operation_index"] == op["index"] && entry["source_uid"] == uid);
        let shape = &op["shape"];
        if !bound || shape["kind"] != "swept_path_2d" || shape["width_nm"] != width_nm {
            continue;
        }
        let affine = &op["affine"];
        let place = |p: &Value| {
            let (x, y) = (p[0].as_f64().expect("x"), p[1].as_f64().expect("y"));
            let m = |key: &str| affine[key].as_f64().expect("affine term");
            (
                m("m11") * x + m("m12") * y + m("tx_nm"),
                m("m21") * x + m("m22") * y + m("ty_nm"),
            )
        };
        points.push(place(&shape["start_nm"]));
        for segment in shape["segments"].as_array().expect("segments") {
            points.push(place(&segment["end_nm"]));
        }
    }
    points
}

fn min_x(points: &[(f64, f64)]) -> f64 {
    points.iter().map(|p| p.0).fold(f64::INFINITY, f64::min)
}

fn text_box(stroke: &str, layer: &str, justify: &str) -> String {
    format!(
        r#"(gr_text_box "Box text" (start 10 10) (end 40 20) (margins 0.5 0.5 0.5 0.5)
          (layer "{layer}") (uuid tb) (effects (font (size 1 1) (thickness 0.15))
          (justify {justify})) (border yes) (stroke (width {stroke}) (type solid)))"#
    )
}

#[test]
fn text_box_text_starts_inside_the_stroke_grown_box() {
    let thin = materialize(&text_box("0.001", "F.SilkS", "left top"), &["silkscreen"]);
    let thick = materialize(&text_box("0.6", "F.SilkS", "left top"), &["silkscreen"]);
    let shift = min_x(&strokes(&thick, "tb", 150_000)) - min_x(&strokes(&thin, "tb", 150_000));
    // KiCad anchors at the box grown by half its stroke: (0.6 - 0.001) / 2.
    assert!((shift + 299_500.0).abs() <= 2.0, "shift {shift}");
}

#[test]
fn mirrored_text_box_text_runs_left_from_the_right_margin() {
    let document = materialize(
        &text_box("0.2", "B.SilkS", "left top mirror"),
        &["silkscreen"],
    );
    let text = strokes(&document, "tb", 150_000);
    assert!(!text.is_empty());
    let right_margin = 40_000_000.0 + 100_000.0 - 500_000.0;
    let max_x = text.iter().map(|p| p.0).fold(f64::NEG_INFINITY, f64::max);
    assert!(max_x <= right_margin + 200_000.0, "max x {max_x}");
    assert!(min_x(&text) > 10_000_000.0, "the text stays inside the box");
}

#[test]
fn footprint_text_box_wraps_to_kicads_column_in_a_rotated_footprint() {
    // The 8 x 4 mm box turns with the footprint, so its text column is the
    // 4 mm side less the margins; four short lines result.
    let body = r#"(footprint "T:R" (layer "F.Cu") (at 40 40 90)
        (fp_text_box "Footprint box wraps here" (start -4 -2) (end 4 2) (angle 90)
          (margins 0.3 0.3 0.3 0.3) (layer "F.SilkS") (uuid fptb)
          (effects (font (size 0.8 0.8) (thickness 0.12)) (justify left top))
          (border yes) (stroke (width 0.15) (type solid))))"#;
    let document = materialize(body, &["silkscreen"]);
    let text = strokes(&document, "fptb", 120_000);
    // At 180 degrees on the board the lines run along X and stack along Y,
    // one stroke-font pitch (1.68 * 0.9583 heights) apart. Four lines span
    // about 4.7 mm; the three of a wrap at the unrotated 7.4 mm width span
    // about 3.4 mm.
    let ys = text.iter().map(|p| p.1);
    let span = ys.clone().fold(f64::NEG_INFINITY, f64::max) - ys.fold(f64::INFINITY, f64::min);
    assert!(span > 4_200_000.0, "four lines, spanning {span} nm");
}

#[test]
fn a_negative_row_height_has_no_separator_at_its_far_edge() {
    let cell = |start: f64, end: f64, uuid: &str| {
        format!(
            r#"(table_cell "" (start 1 {start}) (end 8 {end}) (margins 1 1 1 1) (span 1 1)
              (layer "F.Cu") (uuid {uuid}) (effects (font (size 1 1))))"#
        )
    };
    let body = format!(
        r#"(table (column_count 1) (layer "F.Cu")
            (border (external yes) (header yes) (stroke (width 0.2) (type solid)))
            (separators (rows yes) (cols yes) (stroke (width 0.2) (type solid)))
            (column_widths 7) (row_heights 4 0.01 -3)
            (cells {} {} {}))"#,
        cell(10.0, 14.0, "c0"),
        cell(14.0, 14.01, "c1"),
        cell(14.01, 11.01, "c2"),
    );
    let document = materialize(&body, &["copper"]);
    let binding = document["binding"].as_array().expect("binding");
    let table_uid = binding
        .iter()
        .find_map(|entry| {
            let uid = entry["source_uid"].as_str()?;
            uid.starts_with("table").then(|| uid.to_owned())
        })
        .expect("table bound");
    let lines = strokes(&document, &table_uid, 200_000);
    let far_edge = 11_010_000.0;
    // Capsules carry the row lines; check none lies on the far edge.
    let ops = document["operations"].as_array().expect("operations");
    let at_far_edge = ops.iter().any(|op| {
        op["shape"]["kind"] == "capsule_2d"
            && op["shape"]["start_nm"][1].as_f64() == Some(far_edge)
            && op["shape"]["end_nm"][1].as_f64() == Some(far_edge)
    });
    assert!(!at_far_edge, "no separator at the negative row's far edge");
    assert!(lines.is_empty() || lines.iter().all(|p| p.1 != far_edge));
}
