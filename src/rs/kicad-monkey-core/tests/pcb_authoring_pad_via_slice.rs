#[path = "support/source_authoring/pad_via_fixtures.rs"]
mod fixtures;
#[path = "support/source_authoring/pad_via_readback.rs"]
mod pad_via_readback;
#[path = "support/source_authoring/padstack_readback.rs"]
mod padstack_readback;
use fixtures::*;
use padstack_readback::assert_padstack_writer_semantics;

use kicad_monkey_core::{
    AuthoredBackdrill, AuthoredChamferCorner, AuthoredDrill, AuthoredFootprint,
    AuthoredFootprintOccurrence, AuthoredFrontBackPolicy, AuthoredLayer, AuthoredNet,
    AuthoredNetRef, AuthoredPad, AuthoredPadKind, AuthoredPadShape, AuthoredPadstack,
    AuthoredPadstackLayer, AuthoredPadstackLayerSelector, AuthoredPadstackMode,
    AuthoredPadstackZoneConnection, AuthoredPcb, AuthoredPoint, AuthoredStandaloneFootprint,
    AuthoredVia, AuthoredViaKind, AuthoredViaStack, AuthoredViaStackLayer, AuthoredZoneConnection,
    ErrorKind, PcbAuthoringLimits,
};

#[test]
fn typed_zone_connection_codes_cover_the_complete_kicad_domain() {
    assert_eq!(
        [
            AuthoredZoneConnection::NoConnection,
            AuthoredZoneConnection::ThermalRelief,
            AuthoredZoneConnection::Solid,
            AuthoredZoneConnection::ThermalReliefForThroughHole,
        ]
        .map(AuthoredZoneConnection::source_code),
        [0, 1, 2, 3]
    );
}

#[test]
fn typed_pad_via_source_round_trips_every_supported_policy() {
    assert_padstack_writer_semantics();
    let document = authored_board()
        .to_document(PcbAuthoringLimits::default())
        .expect("fresh pad/via board");
    publish_for_independent_oracle(document.source());
    let view = document.view().expect("board view");

    pad_via_readback::assert_pads(&view);
    pad_via_readback::assert_vias(&view);
}

#[test]
fn pad_via_invalid_or_lossy_combinations_fail_before_emission() {
    let limits = PcbAuthoringLimits::default();
    let mut cases = Vec::new();
    let mut empty_smd = authored_board();
    empty_smd.footprints[0].footprint.pads[0].layers.clear();
    cases.push(empty_smd);
    let mut missing_drill = authored_board();
    missing_drill.footprints[0].footprint.pads[3].drill = None;
    cases.push(missing_drill);

    let mut through_span = authored_board();
    through_span.vias[0].end_layer = "In2.Cu".to_owned();
    cases.push(through_span);

    let mut complete_blind = authored_board();
    complete_blind.vias[1].end_layer = "B.Cu".to_owned();
    cases.push(complete_blind);

    let mut nonadjacent_micro = authored_board();
    nonadjacent_micro.vias[3].start_layer = "F.Cu".to_owned();
    cases.push(nonadjacent_micro);

    let mut reversed = authored_board();
    reversed.vias[2].start_layer = "In2.Cu".to_owned();
    reversed.vias[2].end_layer = "In1.Cu".to_owned();
    cases.push(reversed);

    let mut oversized_drill = authored_board();
    oversized_drill.vias[0].drill_mm = 0.9;
    cases.push(oversized_drill);

    let mut negative_net = authored_board();
    negative_net.vias[0].net_code = -1;
    cases.push(negative_net);

    let mut empty_policy = authored_board();
    empty_policy.vias[0].tenting = Some(AuthoredFrontBackPolicy::default());
    cases.push(empty_policy);

    let mut orphan_land_policy = authored_board();
    orphan_land_policy.vias[0].remove_unused_layers = None;
    cases.push(orphan_land_policy);

    let mut lossy_false_start_end = authored_board();
    lossy_false_start_end.vias[0].start_end_only = Some(false);
    cases.push(lossy_false_start_end);

    let mut conflicting_land_modes = authored_board();
    conflicting_land_modes.vias[0].start_end_only = Some(true);
    cases.push(conflicting_land_modes);

    let mut lossy_false_remove = authored_board();
    lossy_false_remove.vias[0].remove_unused_layers = Some(false);
    lossy_false_remove.vias[0].keep_end_layers = None;
    cases.push(lossy_false_remove);

    let mut duplicate_chamfer = authored_board();
    duplicate_chamfer.footprints[0].footprint.pads[1].shape =
        AuthoredPadShape::ChamferedRoundRect {
            radius_ratio: 0.1,
            chamfer_ratio: 0.2,
            corners: vec![
                AuthoredChamferCorner::TopLeft,
                AuthoredChamferCorner::TopLeft,
            ],
        };
    cases.push(duplicate_chamfer);

    let mut orphan_pad_land_policy = authored_board();
    orphan_pad_land_policy.footprints[0].footprint.pads[2].remove_unused_layers = None;
    cases.push(orphan_pad_land_policy);

    let mut smd_forced_layers = authored_board();
    smd_forced_layers.footprints[0].footprint.pads[0].zone_layer_connections =
        vec!["In1.Cu".to_owned()];
    cases.push(smd_forced_layers);

    let mut orphan_forced_layers = authored_board();
    orphan_forced_layers.footprints[0].footprint.pads[2].remove_unused_layers = None;
    orphan_forced_layers.footprints[0].footprint.pads[2].keep_end_layers = None;
    cases.push(orphan_forced_layers);

    let mut orphan_via_forced_layers = authored_board();
    orphan_via_forced_layers.vias[5].zone_layer_connections = vec!["In1.Cu".to_owned()];
    cases.push(orphan_via_forced_layers);

    let mut duplicate_pad_layers = authored_board();
    duplicate_pad_layers.footprints[0].footprint.pads[0]
        .layers
        .push("F.Cu".to_owned());
    cases.push(duplicate_pad_layers);

    let mut duplicate_pad_forced_layers = authored_board();
    duplicate_pad_forced_layers.footprints[0].footprint.pads[2]
        .zone_layer_connections
        .push("In1.Cu".to_owned());
    cases.push(duplicate_pad_forced_layers);

    let mut duplicate_via_forced_layers = authored_board();
    duplicate_via_forced_layers.vias[0]
        .zone_layer_connections
        .push("In2.Cu".to_owned());
    cases.push(duplicate_via_forced_layers);

    for case in cases {
        let error = case
            .canonical_text(limits)
            .expect_err("invalid authored source");
        assert_eq!(error.kind, ErrorKind::InvalidBuildValue);
    }
}
