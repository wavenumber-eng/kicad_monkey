use super::*;

pub(super) fn assert_padstack_writer_semantics() {
    assert_board_padstacks();
    assert_sparse_local_padstack();
    assert_padstack_policy_rejections();
}

pub(super) fn assert_board_padstacks() {
    let source = authored_board().to_document(Default::default()).unwrap();
    let stack = source
        .view()
        .unwrap()
        .pads()
        .nth(2)
        .unwrap()
        .unwrap()
        .padstack
        .unwrap();
    assert_eq!(stack.mode.as_deref(), Some("front_inner_back"));
    assert_eq!(stack.layers[0].zone_connect, Some(-1));
    assert_eq!(stack.layers[1].shape.as_deref(), Some("rect"));
    assert_eq!(stack.layers[1].offset.unwrap().y, -0.3);
    assert_eq!(stack.layers[1].size.unwrap().y, 1.8);
    assert_eq!(stack.layers[1].clearance, Some(0.0));
    let via = source
        .view()
        .unwrap()
        .vias()
        .next()
        .unwrap()
        .unwrap()
        .padstack
        .unwrap();
    assert_eq!(via.mode.as_deref(), Some("custom"));
    assert_eq!(via.layers[0].size, Some(0.6));
    assert_eq!(via.layers.len(), 2, "fresh source retains sparse rows");
}

pub(super) fn assert_sparse_local_padstack() {
    let local = sparse_footprint();
    let document = local.to_document(Default::default()).unwrap();
    assert_eq!(
        document
            .view()
            .unwrap()
            .pads()
            .next()
            .unwrap()
            .unwrap()
            .padstack
            .unwrap()
            .layers
            .len(),
        1
    );
    if let Some(directory) = std::env::var_os("KM_PAD_VIA_OUTPUT_DIR") {
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(
            std::path::PathBuf::from(directory).join("SparseStack.kicad_mod"),
            document.source(),
        )
        .unwrap();
    }
    let mut placed = authored_board();
    placed.footprints[0].footprint = local.footprint;
    placed.footprints[0].layer = "B.Cu".into();
    placed.footprints[0].angle_degrees = 90.0;
    let local_geometry = document
        .view()
        .unwrap()
        .pads()
        .next()
        .unwrap()
        .unwrap()
        .padstack
        .unwrap()
        .layers[0]
        .clone();
    assert_eq!(local_geometry.shape.as_deref(), Some("custom"));
    assert_eq!(
        local_geometry
            .custom_options
            .as_ref()
            .unwrap()
            .anchor
            .as_deref(),
        Some("circle")
    );
    assert_eq!(local_geometry.custom_primitives.len(), 1);
    assert_eq!(local_geometry.custom_primitives[0].width, Some(0.1));
    let placed = placed.to_document(Default::default()).unwrap();
    let placed_pad = placed.view().unwrap().pads().next().unwrap().unwrap();
    let row = &placed_pad.padstack.as_ref().unwrap().layers[0];
    assert_eq!(
        (&row.shape, row.size, row.offset),
        (
            &local_geometry.shape,
            local_geometry.size,
            local_geometry.offset
        )
    );
    assert_eq!(placed_pad.angle, 0.0);
    assert_eq!(
        row.custom_primitives[0].geometry,
        local_geometry.custom_primitives[0].geometry
    );
    assert_eq!(
        placed
            .view()
            .unwrap()
            .footprints()
            .next()
            .unwrap()
            .unwrap()
            .angle,
        Some(90.0)
    );
}

pub(super) fn assert_padstack_policy_rejections() {
    for mode in [
        AuthoredPadstackMode::FrontInnerBack,
        AuthoredPadstackMode::Custom,
    ] {
        let mut board = authored_board();
        let stack = board.footprints[0].footprint.pads[2]
            .padstack
            .as_mut()
            .unwrap();
        stack.mode = mode;
        stack.layers[0].layer = AuthoredPadstackLayerSelector::CopperLayer("F.Mask".into());
        assert!(board.canonical_text(Default::default()).is_err());
    }
    let mut board = authored_board();
    board.footprints[0].footprint.pads[2]
        .padstack
        .as_mut()
        .unwrap()
        .layers[0]
        .thermal_bridge_angle_degrees = Some(23.0);
    assert!(board.canonical_text(Default::default()).is_err());
}
