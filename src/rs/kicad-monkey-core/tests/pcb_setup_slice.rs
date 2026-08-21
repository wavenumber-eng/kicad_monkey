use kicad_monkey_core::{PcbFamily, PcbLimits, PcbSelection, PcbView};

const SOURCE: &str = r#"(kicad_pcb
  (setup
    (stackup
      (layer "F.Cu" (type "copper") (thickness 0.035 locked))
      (layer "dielectric 1" (type "core") (color "natural")
        (thickness 1.5) (material "FR4") (epsilon_r 4.6) (loss_tangent 0.02))
      (copper_finish "ENIG") (dielectric_constraints yes)
      (edge_connector bevelled) (edge_plating yes))
    (allow_soldermask_bridges_in_footprints no)
    (tenting front back)
    (covering (front yes) (back no))
    (plugging (front no) (back yes))
    (capping yes) (filling no)
    (aux_axis_origin 10 20) (grid_origin 1.5 2.5))
)"#;

#[test]
fn setup_and_stackup_match_close_to_format_semantics() {
    let view = PcbView::parse(SOURCE, PcbLimits::default()).expect("board");
    let setup = view.setup().expect("decode").expect("setup");
    assert_setup(&setup);
    assert_stackup(setup.stackup.as_ref().expect("stackup"));
}

fn assert_setup(setup: &kicad_monkey_core::PcbSetup) {
    assert_eq!(
        (setup.aux_axis_origin.x, setup.aux_axis_origin.y),
        (10.0, 20.0)
    );
    assert_eq!((setup.grid_origin.x, setup.grid_origin.y), (1.5, 2.5));
    assert!(!setup.allow_soldermask_bridges_in_footprints);
    assert!(setup.tenting_front && setup.tenting_back);
    assert!(setup.covering_front && !setup.covering_back);
    assert!(!setup.plugging_front && setup.plugging_back);
    assert!(setup.capping && !setup.filling);
    assert_eq!(&SOURCE[setup.source_range.clone()][..6], "(setup");
}

fn assert_stackup(stackup: &kicad_monkey_core::PcbStackup) {
    assert_eq!(stackup.copper_finish, "ENIG");
    assert!(stackup.dielectric_constraints);
    assert_eq!(stackup.edge_connector, "bevelled");
    assert!(stackup.edge_plating);
    assert_eq!(stackup.layers.len(), 2);
    assert_eq!(stackup.layers[0].name, "F.Cu");
    assert_eq!(stackup.layers[0].type_name, "copper");
    assert_eq!(stackup.layers[0].thickness, 0.035);
    assert!(stackup.layers[0].thickness_locked);
    assert_eq!(stackup.layers[1].material, "FR4");
    assert_eq!(stackup.layers[1].epsilon_r, Some(4.6));
    assert_eq!(stackup.layers[1].loss_tangent, Some(0.02));
}

#[test]
fn setup_selection_is_explicit_and_stackup_limits_fail_before_growth() {
    let hidden = PcbView::parse_selected(SOURCE, PcbLimits::default(), PcbSelection::none())
        .expect("hidden setup");
    assert!(hidden.setup().expect("hidden").is_none());
    let selected = PcbView::parse_selected(
        SOURCE,
        PcbLimits::default(),
        PcbSelection::only(PcbFamily::Setup),
    )
    .expect("selected");
    assert_eq!(
        selected.setup().expect("selected setup"),
        PcbView::parse(SOURCE, PcbLimits::default())
            .expect("full")
            .setup()
            .expect("full setup")
    );

    let limited = PcbView::parse(
        SOURCE,
        PcbLimits {
            max_stackup_layers: 1,
            ..PcbLimits::default()
        },
    )
    .expect("stackup is lazy");
    limited.setup().expect_err("layer limit");
}

#[test]
fn absent_setup_and_stackup_use_python_defaults() {
    let board = PcbView::parse("(kicad_pcb)", PcbLimits::default()).expect("board");
    assert!(board.setup().expect("absent").is_none());
    let setup = PcbView::parse("(kicad_pcb (setup))", PcbLimits::default())
        .expect("board")
        .setup()
        .expect("decode")
        .expect("setup");
    assert_eq!(
        (setup.aux_axis_origin.x, setup.aux_axis_origin.y),
        (0.0, 0.0)
    );
    assert_eq!((setup.grid_origin.x, setup.grid_origin.y), (0.0, 0.0));
    assert!(setup.stackup.is_none());
}
