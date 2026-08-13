use kicad_monkey_core::{
    ErrorKind, PcbHoleOwner, PcbHoleShape, PcbLimits, PcbProfileOwner, PcbView,
};

const PHYSICAL: &str = r#"(kicad_pcb
  (footprint "Demo:Part" (layer "B.Cu") (at 100 50 90) (locked yes)
    (path "/root/U1") (sheetname "RF") (sheetfile "rf.kicad_sch") (uuid fp-id)
    (fp_line (start -1 0) (end 1 0) (stroke (width 0.1) (type default))
      (layer "Edge.Cuts") (uuid fp-edge))
    (fp_circle (center 0 0) (end 1 0) (stroke (width 0.1) (type default))
      (fill none) (layer "F.SilkS"))
    (pad "1" thru_hole circle (at 1 2 30) (size 2 2)
      (drill 0.8 (offset 0.1 -0.2)) (layers "*.Cu" "*.Mask") (uuid pad-1))
    (pad "" np_thru_hole oval (at -2 3) (size 2 3)
      (drill oval 1 2) (layers "*.Cu" "*.Mask") (uuid pad-2)))
  (gr_line (start 0 0) (end 10 0) (stroke (width 0.05) (type default))
    (layer "Edge.Cuts") (uuid board-edge))
  (gr_line (layer "F.SilkS"))
  (via (at 4 5) (size 1) (drill 0.4) (layers "F.Cu" "B.Cu") (uuid via-1))
)"#;

#[test]
fn holes_preserve_coordinate_space_shape_plating_and_source() {
    let view = PcbView::parse(PHYSICAL, PcbLimits::default()).expect("board");
    let holes = view.holes().collect::<Result<Vec<_>, _>>().expect("holes");
    assert_eq!(holes.len(), 3);
    assert_round_pad(&holes[0]);
    assert_oval_pad(&holes[1]);
    assert_via(&holes[2]);
}

fn assert_round_pad(round: &kicad_monkey_core::PcbHole) {
    assert_eq!(round.owner, PcbHoleOwner::Pad);
    assert_eq!(round.footprint_index, Some(0));
    assert_eq!(round.shape, PcbHoleShape::Round);
    assert_eq!((round.center.x, round.center.y), (1.0, 2.0));
    assert_eq!((round.offset.x, round.offset.y), (0.1, -0.2));
    assert_eq!((round.width, round.height, round.angle), (0.8, 0.8, 30.0));
    assert!(round.plated);
    assert!(PHYSICAL[round.source_range.clone()].starts_with("(pad"));
}

fn assert_oval_pad(oval: &kicad_monkey_core::PcbHole) {
    assert_eq!(oval.shape, PcbHoleShape::Oval);
    assert_eq!((oval.width, oval.height), (1.0, 2.0));
    assert!(!oval.plated);
}

fn assert_via(via: &kicad_monkey_core::PcbHole) {
    assert_eq!(via.owner, PcbHoleOwner::Via);
    assert_eq!(via.footprint_index, None);
    assert_eq!((via.center.x, via.center.y), (4.0, 5.0));
    assert_eq!((via.width, via.height), (0.4, 0.4));
    assert!(via.plated);
}

#[test]
fn footprint_transforms_are_explicit_and_profile_coordinates_stay_local() {
    let view = PcbView::parse(PHYSICAL, PcbLimits::default()).expect("board");
    let transform = view
        .footprint_transforms()
        .next()
        .expect("transform")
        .expect("decoded");
    assert_eq!(
        (transform.x, transform.y, transform.angle),
        (100.0, 50.0, 90.0)
    );
    assert_eq!(transform.layer, "B.Cu");
    assert!(transform.locked);
    assert_eq!(transform.path.as_deref(), Some("/root/U1"));
    assert_eq!(transform.sheet_name.as_deref(), Some("RF"));
    assert_eq!(transform.sheet_file.as_deref(), Some("rf.kicad_sch"));
    assert_eq!(transform.uuid.as_deref(), Some("fp-id"));

    let profile = view
        .profile_primitives()
        .collect::<Result<Vec<_>, _>>()
        .expect("profile");
    assert_eq!(profile.len(), 2);
    assert_eq!(profile[0].owner, PcbProfileOwner::Board);
    assert_eq!(profile[0].graphic.start.expect("start").x, 0.0);
    assert_eq!(
        profile[1].owner,
        PcbProfileOwner::Footprint { footprint_index: 0 }
    );
    assert_eq!(profile[1].graphic.start.expect("local start").x, -1.0);
    assert_eq!(profile[1].graphic.end.expect("local end").x, 1.0);
}

#[test]
fn physical_views_are_selective_and_limits_fail_closed() {
    let view = PcbView::parse(PHYSICAL, PcbLimits::default()).expect("board");
    assert_eq!(view.counts().footprint_graphics, 2);
    view.profile_primitives()
        .collect::<Result<Vec<_>, _>>()
        .expect("malformed non-profile graphic is not decoded");
    PcbView::parse(
        PHYSICAL,
        PcbLimits {
            max_footprint_graphics: 2,
            ..PcbLimits::default()
        },
    )
    .expect("exact footprint graphic limit");

    assert_eq!(
        PcbView::parse(
            PHYSICAL,
            PcbLimits {
                max_footprint_graphics: 1,
                ..PcbLimits::default()
            },
        )
        .expect_err("footprint graphic limit")
        .kind,
        ErrorKind::ResourceLimit
    );
    let view = PcbView::parse(
        PHYSICAL,
        PcbLimits {
            max_pad_children: 2,
            ..PcbLimits::default()
        },
    )
    .expect("hole decode remains lazy");
    assert_eq!(
        view.holes()
            .next()
            .expect("hole")
            .expect_err("pad child limit")
            .kind,
        ErrorKind::ResourceLimit
    );
}

#[test]
fn pad_and_via_layer_tokens_obey_exact_lazy_limits() {
    let exact = PcbView::parse(
        PHYSICAL,
        PcbLimits {
            max_layers: 2,
            ..PcbLimits::default()
        },
    )
    .expect("board");
    assert_eq!(
        exact
            .pads()
            .next()
            .expect("pad")
            .expect("exact layers")
            .layers
            .len(),
        2
    );
    assert_eq!(
        exact
            .vias()
            .next()
            .expect("via")
            .expect("exact layers")
            .layers
            .len(),
        2
    );

    let limited = PcbView::parse(
        PHYSICAL,
        PcbLimits {
            max_layers: 1,
            ..PcbLimits::default()
        },
    )
    .expect("layer-token decoding remains lazy");
    assert_eq!(
        limited
            .pads()
            .next()
            .expect("pad")
            .expect_err("pad layer limit")
            .kind,
        ErrorKind::ResourceLimit
    );
    assert_eq!(
        limited
            .vias()
            .next()
            .expect("via")
            .expect_err("via layer limit")
            .kind,
        ErrorKind::ResourceLimit
    );
}
