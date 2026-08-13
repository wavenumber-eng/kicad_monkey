use kicad_monkey_core::{ErrorKind, PcbLimits, PcbView};

const SOURCE: &str = r#"(kicad_pcb
  (via blind buried micro
    (at 4 5) (size 1) (drill 0.4) (layers "F.Cu" "B.Cu")
    (free yes)
    (tenting (front yes) (back no))
    (covering (front none) (back yes))
    (plugging (front no) (back invalid))
    (capping yes) (filling no) (uuid via-id)))"#;

#[test]
fn via_kind_dimensions_and_surface_treatments_match_python() {
    let via = PcbView::parse(SOURCE, PcbLimits::default())
        .expect("board")
        .vias()
        .next()
        .expect("via")
        .expect("typed via");
    assert_eq!(via.via_type.as_deref(), Some("blind"));
    assert_eq!((via.size, via.drill), (1.0, 0.4));
    assert_eq!(via.layers, ["F.Cu", "B.Cu"]);
    assert!(via.free);
    let tenting = via.tenting.expect("tenting");
    assert_eq!((tenting.front, tenting.back), (Some(true), Some(false)));
    assert!(SOURCE[tenting.source_range].starts_with("(tenting"));
    let covering = via.covering.expect("covering");
    assert_eq!((covering.front, covering.back), (None, Some(true)));
    let plugging = via.plugging.expect("plugging");
    assert_eq!((plugging.front, plugging.back), (Some(false), None));
    assert_eq!(via.capping, Some(true));
    assert_eq!(via.filling, Some(false));
}

#[test]
fn sparse_via_values_follow_python_defaults_and_truthiness() {
    let source = r#"(kicad_pcb
      (via (at 0 0) (free true) (tenting (front invalid))
        (covering) (capping none) (filling invalid)))"#;
    let via = PcbView::parse(source, PcbLimits::default())
        .expect("board")
        .vias()
        .next()
        .expect("via")
        .expect("typed via");
    assert_eq!((via.size, via.drill), (0.0, 0.0));
    assert!(!via.free, "Python only treats free=yes as true");
    assert!(via.tenting.is_none());
    assert!(via.covering.is_none());
    assert_eq!(via.capping, None);
    assert_eq!(via.filling, None);
    assert_eq!(via.via_type, None);
}

#[test]
fn via_detail_limits_accept_exact_boundaries_and_fail_closed_above_them() {
    let exact = PcbLimits {
        max_via_header_scalars: 3,
        max_via_children: 11,
        max_via_policy_children: 2,
        ..PcbLimits::default()
    };
    PcbView::parse(SOURCE, exact)
        .expect("board")
        .vias()
        .collect::<Result<Vec<_>, _>>()
        .expect("exact limits");

    for limits in [
        PcbLimits {
            max_via_header_scalars: 2,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_via_children: 10,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_via_policy_children: 1,
            ..PcbLimits::default()
        },
    ] {
        let error = PcbView::parse(SOURCE, limits)
            .expect("lazy via limit")
            .vias()
            .collect::<Result<Vec<_>, _>>()
            .expect_err("resource limit");
        assert_eq!(error.kind, ErrorKind::ResourceLimit);
    }
}

#[test]
fn via_and_hole_coordinates_share_component_wise_python_defaults() {
    for (at, expected) in [
        ("", (0.0, 0.0)),
        ("(at)", (0.0, 0.0)),
        ("(at 7)", (7.0, 0.0)),
    ] {
        let source =
            format!("(kicad_pcb (via {at} (size 1) (drill 0.4) (layers \"F.Cu\" \"B.Cu\")))");
        let view = PcbView::parse(&source, PcbLimits::default()).expect("board");
        let via = view.vias().next().expect("via").expect("typed via");
        assert_eq!((via.at_x, via.at_y), expected);
        let hole = view.holes().next().expect("hole").expect("typed hole");
        assert_eq!((hole.center.x, hole.center.y), expected);
    }

    let malformed = "(kicad_pcb (via (at nope) (drill 0.4)))";
    let view = PcbView::parse(malformed, PcbLimits::default()).expect("lazy board");
    for error in [
        view.vias().next().expect("via").expect_err("malformed via"),
        view.holes()
            .next()
            .expect("hole")
            .expect_err("malformed hole"),
    ] {
        assert_eq!(error.kind, ErrorKind::UnexpectedToken);
        assert_eq!(
            error.position.expect("absolute position").offset,
            malformed.find("nope").expect("token")
        );
    }
}
