use super::*;

fn assert_standalone_cut() {
    let mut standalone = AuthoredStandaloneFootprint::new("CutOnly", "F.Cu");
    standalone.footprint.pads.push(
        authored_board()
            .footprints
            .remove(0)
            .footprint
            .pads
            .remove(3),
    );
    let standalone = standalone
        .to_document(PcbAuthoringLimits::default())
        .expect("standalone cut-only NPTH");
    let standalone_view = standalone.view().expect("standalone view");
    let standalone_pad = standalone_view
        .pads()
        .next()
        .expect("pad")
        .expect("typed pad");
    assert!(standalone_pad.has_layers && standalone_pad.layers.is_empty());
    assert_eq!(standalone_pad.plated, Some(false));
}

fn assert_trapezoid_policy(pad: &kicad_monkey_core::PcbPad) {
    assert_eq!(pad.shape, "trapezoid");
    assert_eq!(
        (pad.rect_delta_x, pad.rect_delta_y),
        (Some(0.2), Some(-0.1))
    );
    assert_eq!(pad.solder_mask_margin, Some(0.05));
    assert_eq!(pad.solder_paste_margin, Some(-0.02));
    assert_eq!(pad.solder_paste_margin_ratio, Some(-0.1));
    assert_eq!(pad.clearance, Some(0.08));
    assert_eq!(pad.thermal_bridge_width, Some(0.3));
    assert_eq!(pad.thermal_bridge_angle, Some(45.0));
    assert_eq!(pad.thermal_gap, Some(0.2));
    assert_eq!(pad.zone_connect, Some(2));
}

fn assert_chamfer_and_slot(pads: &[kicad_monkey_core::PcbPad]) {
    assert_eq!(pads[1].shape, "roundrect");
    assert_eq!(pads[1].roundrect_rratio, Some(0.15));
    assert_eq!(pads[1].chamfer_ratio, Some(0.2));
    assert_eq!(pads[1].chamfer_corners, ["top_left", "bottom_right"]);
    let drill = pads[2].drill.as_ref().expect("plated slot");
    assert_eq!((drill.width, drill.height), (0.8, Some(1.6)));
    assert_eq!((drill.offset.x, drill.offset.y), (0.1, -0.1));
    assert_eq!(pads[2].remove_unused_layers, Some(true));
    assert_eq!(pads[2].keep_end_layers, Some(false));
    assert_eq!(pads[2].zone_connect, Some(3));
    assert_eq!(
        pads[2]
            .zone_layer_connections
            .as_ref()
            .expect("pad forced layers")
            .forced_layers,
        ["In1.Cu", "In2.Cu"]
    );
}

pub(super) fn assert_pads(view: &kicad_monkey_core::PcbView<'_>) {
    let pads = view.pads().collect::<Result<Vec<_>, _>>().expect("pads");
    assert_eq!(pads.len(), 4);
    assert!(pads[3].layers.is_empty());
    assert!(pads[3].has_layers);
    assert_eq!(pads[3].plated, Some(false));
    let cut = pads[3].drill.as_ref().expect("cut-only NPTH drill");
    assert_eq!((cut.width, cut.offset.x, cut.offset.y), (0.8, 0.1, -0.1));
    assert_standalone_cut();
    assert_trapezoid_policy(&pads[0]);
    assert_chamfer_and_slot(&pads);
}

fn assert_through_via(via: &kicad_monkey_core::PcbVia) {
    assert_eq!(via.via_type, None);
    assert_eq!(via.layers, ["F.Cu", "B.Cu"]);
    let backdrill = via.backdrill.as_ref().expect("backdrill");
    assert_eq!(backdrill.size, Some(0.5));
    assert_eq!(
        (
            backdrill.layers.start.as_str(),
            backdrill.layers.end.as_str()
        ),
        ("B.Cu", "In2.Cu")
    );
    assert!(via.free);
    assert_eq!(
        (
            via.tenting.as_ref().and_then(|value| value.front),
            via.tenting.as_ref().and_then(|value| value.back)
        ),
        (Some(false), Some(true))
    );
    assert_eq!(via.remove_unused_layers, Some(true));
    assert_eq!(via.keep_end_layers, Some(true));
    assert_eq!(via.start_end_only, None);
}

pub(super) fn assert_vias(view: &kicad_monkey_core::PcbView<'_>) {
    let vias = view.vias().collect::<Result<Vec<_>, _>>().expect("vias");
    assert_eq!(vias.len(), 6);
    assert_through_via(&vias[0]);
    assert_blind_vias(&vias);
    assert_eq!(vias[3].via_type.as_deref(), Some("micro"));
    assert_eq!(vias[3].layers, ["F.Cu", "B.Cu"]);
    assert_eq!((vias[3].size, vias[3].drill), (0.3, 0.1));
    assert_eq!(vias[4].layers, ["F.Cu", "B.Cu"]);
    assert_eq!(vias[4].start_end_only, Some(true));
    assert!(vias[..5].iter().all(|value| value.net.ordinal == Some(1)));
    assert_eq!(vias[5].layers, ["F.Cu", "B.Cu"]);
    assert_eq!(vias[5].net.ordinal, Some(0));
    assert_eq!(vias[5].net.name, None);
}

fn assert_blind_vias(vias: &[kicad_monkey_core::PcbVia]) {
    assert_eq!(vias[1].via_type.as_deref(), Some("blind"));
    assert_eq!(vias[1].layers, ["F.Cu", "In1.Cu"]);
    assert_eq!(
        vias[1].covering.as_ref().and_then(|value| value.front),
        Some(true)
    );
    assert_eq!(vias[1].capping, Some(true));
    assert_eq!(vias[2].via_type.as_deref(), Some("blind"));
    assert_eq!(vias[2].layers, ["In1.Cu", "In2.Cu"]);
    assert_eq!(
        vias[2].plugging.as_ref().and_then(|value| value.back),
        Some(false)
    );
    assert_eq!(vias[2].filling, Some(false));
}
