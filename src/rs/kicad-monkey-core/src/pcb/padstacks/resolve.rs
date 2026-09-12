//! Borrowed effective copper-shape facts, following KiCad 10.0.6 PAD/PADSTACK.
//!
//! This is source interpretation, not land membership, geometry realization,
//! connectivity, board-side reflection, or downstream model construction.

use super::*;

/// Effective source-local shape and layer-local policy for one copper selector.
/// Distances remain millimetres and angles remain KiCad degrees. A drill offset
/// moves the copper shape, not the hole. No policy is inherited from a footprint
/// or a design rule here; absence remains absence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PcbResolvedPadCopperLayer<'a> {
    pub shape: &'a str,
    pub size: PcbPoint,
    pub offset: PcbPoint,
    pub rect_delta: PcbPoint,
    pub roundrect_rratio: f64,
    pub chamfer_ratio: f64,
    pub chamfer_corners: &'a [String],
    pub anchor: &'a str,
    pub custom_options: Option<&'a PcbPadCustomOptions>,
    pub custom_primitives: &'a [PcbPadCustomPrimitive],
    pub clearance: Option<f64>,
    pub thermal_bridge_width: Option<f64>,
    pub thermal_bridge_angle: f64,
    pub thermal_gap: Option<f64>,
    pub zone_connect: Option<i64>,
}

/// Resolve a copper shape without deciding whether copper is present there.
///
/// Accepts `F.Cu`, `B.Cu`, `InN.Cu`, and the front/inner/back selector `Inner`.
/// Normal pads use their root shape everywhere. Explicit non-front stack rows
/// start with KiCad's empty layer defaults, not copies of the root. Missing
/// custom rows fall back to the root; missing front/inner/back rows follow
/// KiCad's const lookup (inner row when available, otherwise root).
///
/// The sparse source DTO cannot recover arbitrary mutation order. Explicit
/// front rows, duplicate rows, nonstandard front/inner/back aliases, missing
/// modes, and unsupported selectors are rejected instead of guessed. Row-level
/// thermal angles stay authored facts; the released reader does not apply them
/// to their rows (see `explicit_row`).
/// This helper targets modern, conventionally ordered source. It is not proof
/// of pre-7 file migration or arbitrary reordered-field parity; those need a
/// version-aware, ordered source interpretation beyond this sparse DTO.
pub fn resolve_pad_copper_layer<'a>(
    pad: &'a PcbPad,
    layer: &str,
) -> Result<PcbResolvedPadCopperLayer<'a>, Error> {
    if !copper_selector(layer) {
        return Err(unsupported(
            "Expected a concrete copper layer or Inner selector",
        ));
    }
    let Some(stack) = &pad.padstack else {
        return Ok(root(pad));
    };
    let mode = stack
        .mode
        .as_deref()
        .ok_or_else(|| unsupported("Padstack mode is absent"))?;
    if !matches!(mode, "front_inner_back" | "custom") {
        return Err(unsupported("Unsupported padstack mode"));
    }
    let mut seen = std::collections::HashSet::new();
    for row in &stack.layers {
        if !seen.insert(row.layer.as_str()) {
            return Err(unsupported(
                "Duplicate padstack rows require ordered source interpretation",
            ));
        }
        if row.layer == "F.Cu" {
            return Err(unsupported(
                "Explicit F.Cu padstack rows require ordered source interpretation",
            ));
        }
        let valid = if mode == "front_inner_back" {
            matches!(row.layer.as_str(), "Inner" | "B.Cu")
        } else {
            row.layer != "Inner" && copper_selector(&row.layer)
        };
        if !valid {
            return Err(unsupported(
                "Unsupported padstack row selector for this mode",
            ));
        }
    }
    if layer == "F.Cu" {
        return Ok(root(pad));
    }
    if mode == "custom" && layer == "Inner" {
        return Err(unsupported(
            "Custom padstacks require a concrete inner layer",
        ));
    }
    let selector = if mode == "front_inner_back" && layer != "B.Cu" {
        "Inner"
    } else {
        layer
    };
    let selected = stack
        .layers
        .iter()
        .find(|row| row.layer == selector)
        .or_else(|| {
            (mode == "front_inner_back")
                .then(|| stack.layers.iter().find(|row| row.layer == "Inner"))
                .flatten()
        });
    Ok(selected.map_or_else(|| root(pad), explicit_row))
}

fn root(pad: &PcbPad) -> PcbResolvedPadCopperLayer<'_> {
    let anchor = pad
        .custom_options
        .as_ref()
        .and_then(|options| options.anchor.as_deref())
        .unwrap_or("circle");
    PcbResolvedPadCopperLayer {
        shape: &pad.shape,
        size: PcbPoint {
            x: pad.size_x,
            y: pad.size_y,
        },
        offset: pad
            .drill
            .as_ref()
            .map_or(PcbPoint { x: 0.0, y: 0.0 }, |drill| drill.offset),
        rect_delta: PcbPoint {
            x: pad.rect_delta_x.unwrap_or(0.0),
            y: pad.rect_delta_y.unwrap_or(0.0),
        },
        roundrect_rratio: pad.roundrect_rratio.unwrap_or(0.25),
        chamfer_ratio: pad.chamfer_ratio.unwrap_or(0.2),
        chamfer_corners: &pad.chamfer_corners,
        anchor,
        custom_options: pad.custom_options.as_ref(),
        custom_primitives: &pad.custom_primitives,
        clearance: pad.clearance,
        thermal_bridge_width: pad.thermal_bridge_width,
        thermal_bridge_angle: pad.thermal_bridge_angle.unwrap_or(
            if pad.shape == "circle" || (pad.shape == "custom" && anchor == "circle") {
                45.0
            } else {
                90.0
            },
        ),
        thermal_gap: pad.thermal_gap,
        zone_connect: pad.zone_connect.or(Some(-1)),
    }
}

fn explicit_row(row: &PcbPadstackLayer) -> PcbResolvedPadCopperLayer<'_> {
    let shape = row.shape.as_deref().unwrap_or("circle");
    PcbResolvedPadCopperLayer {
        shape,
        size: row.size.unwrap_or(PcbPoint { x: 0.0, y: 0.0 }),
        offset: row.offset.unwrap_or(PcbPoint { x: 0.0, y: 0.0 }),
        rect_delta: row.rect_delta.unwrap_or(PcbPoint { x: 0.0, y: 0.0 }),
        roundrect_rratio: row.roundrect_rratio.unwrap_or(0.0),
        chamfer_ratio: row.chamfer_ratio.unwrap_or(0.0),
        chamfer_corners: &row.chamfer_corners,
        anchor: row
            .custom_options
            .as_ref()
            .and_then(|options| options.anchor.as_deref())
            .unwrap_or("rect"),
        custom_options: row.custom_options.as_ref(),
        custom_primitives: &row.custom_primitives,
        clearance: row.clearance,
        thermal_bridge_width: row.thermal_bridge_width,
        // KiCad 10.0.6 parsePadstack calls SetThermalSpokeAngle without curLayer,
        // temporarily writing F.Cu. parsePAD then assigns the root angle/default
        // after reading the stack, overwriting that write. Non-front rows retain
        // their shape defaults, not the authored row thermal_bridge_angle token.
        thermal_bridge_angle: if matches!(shape, "oval" | "rect" | "roundrect")
            || row.chamfer_ratio.is_some_and(|ratio| ratio > 0.0)
            || !row.chamfer_corners.is_empty()
        {
            90.0
        } else {
            45.0
        },
        thermal_gap: row.thermal_gap,
        zone_connect: row.zone_connect,
    }
}

fn copper_selector(layer: &str) -> bool {
    matches!(layer, "F.Cu" | "B.Cu" | "Inner")
        || layer
            .strip_prefix("In")
            .and_then(|suffix| suffix.strip_suffix(".Cu"))
            .and_then(|number| number.parse::<u8>().ok())
            .is_some_and(|number| (1..=62).contains(&number))
}

fn unsupported(message: &'static str) -> Error {
    Error::build(ErrorKind::InvalidBuildValue, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pad(stack: &str) -> PcbPad {
        let source = format!(
            r#"(kicad_pcb (version 20260206)
          (footprint "test" (layer "F.Cu") (at 0 0)
            (pad "1" thru_hole rect (at 0 0) (size 2 3)
              (drill 0.8 (offset 0.2 -0.3)) (layers "*.Cu")
              (clearance 0.4) (thermal_bridge_angle 90) {stack})))"#
        );
        let view = PcbView::parse(&source, PcbLimits::default()).expect("source");
        view.pads().next().expect("pad").expect("typed pad")
    }

    #[test]
    fn copper_layer_defaults_fallbacks_and_sparse_rows_are_source_owned() {
        let normal = pad("");
        let root = resolve_pad_copper_layer(&normal, "B.Cu").unwrap();
        assert_eq!(root.size, PcbPoint { x: 2.0, y: 3.0 });
        assert_eq!(root.offset, PcbPoint { x: 0.2, y: -0.3 });
        assert_eq!(
            (root.anchor, root.roundrect_rratio, root.chamfer_ratio),
            ("circle", 0.25, 0.2)
        );
        let tib = pad(r#"(padstack (mode front_inner_back)
            (layer "Inner" (shape oval) (size 1 2))
            (layer "B.Cu" (size 0.6 0.6) (zone_connect -1)))"#);
        let inner = resolve_pad_copper_layer(&tib, "In3.Cu").unwrap();
        assert_eq!((inner.shape, inner.thermal_bridge_angle), ("oval", 90.0));
        assert_eq!(inner.offset, PcbPoint { x: 0.0, y: 0.0 });
        assert_eq!(inner.clearance, None);
        assert_eq!(inner.zone_connect, None);
        assert_eq!(inner.anchor, "rect");
        assert_eq!(inner.roundrect_rratio, 0.0);
        let back = resolve_pad_copper_layer(&tib, "B.Cu").unwrap();
        assert_eq!(
            (back.shape, back.thermal_bridge_angle, back.zone_connect),
            ("circle", 45.0, Some(-1))
        );
        let missing_back = pad(r#"(padstack (mode front_inner_back) (layer "Inner" (size 1 1)))"#);
        assert_eq!(
            resolve_pad_copper_layer(&missing_back, "B.Cu")
                .unwrap()
                .size
                .x,
            1.0
        );
        let custom = pad(r#"(padstack (mode custom) (layer "In2.Cu" (shape rect)))"#);
        assert_eq!(
            resolve_pad_copper_layer(&custom, "In1.Cu").unwrap().size.x,
            2.0
        );
        assert_eq!(
            resolve_pad_copper_layer(&custom, "In2.Cu").unwrap().size.x,
            0.0
        );
        assert_ambiguous_stacks_are_rejected();
        assert_authored_row_angles_follow_released_reader();
    }

    fn assert_authored_row_angles_follow_released_reader() {
        for (mode, inner) in [("custom", "In1.Cu"), ("front_inner_back", "Inner")] {
            let mut source = pad(&format!(
                r#"(padstack (mode {mode})
                    (layer "{inner}" (shape circle) (size 1 1) (thermal_bridge_angle 13))
                    (layer "B.Cu" (shape rect) (size 1 2) (thermal_bridge_angle 27)))"#
            ));
            source.shape = "circle".into();
            source.thermal_bridge_angle = Some(61.0);
            for (root_override, expected_front) in [(Some(61.0), 61.0), (None, 45.0)] {
                source.thermal_bridge_angle = root_override;
                for (layer, expected) in
                    [("F.Cu", expected_front), ("In1.Cu", 45.0), ("B.Cu", 90.0)]
                {
                    assert_eq!(
                        resolve_pad_copper_layer(&source, layer)
                            .unwrap()
                            .thermal_bridge_angle,
                        expected
                    );
                }
            }
            let rows = &source.padstack.as_ref().unwrap().layers;
            assert_eq!(rows[0].thermal_bridge_angle, Some(13.0));
            assert_eq!(rows[1].thermal_bridge_angle, Some(27.0));
        }
    }

    fn assert_ambiguous_stacks_are_rejected() {
        for stack in [
            r#"(padstack (mode custom) (layer "F.Cu" (size 1 1)))"#,
            r#"(padstack (mode custom) (layer "B.Cu") (layer "B.Cu"))"#,
        ] {
            assert!(resolve_pad_copper_layer(&pad(stack), "F.Cu").is_err());
        }
    }
}
