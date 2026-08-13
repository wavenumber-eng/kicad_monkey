use kicad_monkey_core::{PcbDocument, PcbFamily, PcbLimits, PcbSelection, PcbView};

const SOURCE: &str = r#"(kicad_pcb
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
  (net 1 "GND")
  (property "Owner" "team")
  (footprint "Demo" (layer "F.Cu") (at 10 20 45) (uuid fp-id)
    (fp_line (start 0 0) (end 1 0) (stroke (width 0.1) (type default))
      (layer "Edge.Cuts"))
    (pad "1" thru_hole circle (at 1 2) (size 2 2) (drill 0.8)
      (layers "*.Cu" "*.Mask") (net 1) (uuid pad-id))
    (model "demo.step"))
  (segment (start 0 0) (end 1 0) (width 0.2) (layer "F.Cu") (net 1))
  (via (at 2 3) (size 1) (drill 0.4) (layers "F.Cu" "B.Cu") (net 1))
  (zone (net 1) (layer "F.Cu") (polygon (pts (xy 0 0) (xy 1 0) (xy 1 1))))
  (arc (start 0 0) (mid 1 1) (end 2 0) (width 0.2) (layer "F.Cu") (net 1))
  (gr_line (start 0 0) (end 5 0) (stroke (width 0.1) (type default))
    (layer "Edge.Cuts"))
  (group "G" (uuid group-id))
  (future_form (payload keep))
)"#;

#[test]
fn selected_net_bearing_families_equal_full_views_without_exposing_dependencies() {
    let full = PcbView::parse(SOURCE, PcbLimits::default()).expect("full");
    assert_pad_selection(&full);
    assert_zone_selection(&full);
}

fn assert_pad_selection(full: &PcbView<'_>) {
    let selected = PcbView::parse_selected(
        SOURCE,
        PcbLimits::default(),
        PcbSelection::only(PcbFamily::Pads),
    )
    .expect("pads");
    assert_eq!(selected.selection(), PcbSelection::only(PcbFamily::Pads));
    assert_eq!(
        selected
            .pads()
            .collect::<Result<Vec<_>, _>>()
            .expect("selected"),
        full.pads().collect::<Result<Vec<_>, _>>().expect("full")
    );
    assert_eq!(selected.counts().pads, full.counts().pads);
    assert_eq!(selected.counts().nets, 0);
    assert_eq!(selected.counts().footprints, 0);
    assert_eq!(selected.nets().count(), 0);
    assert_eq!(selected.footprints().count(), 0);
    assert_eq!(selected.models().count(), 0);
}

fn assert_zone_selection(full: &PcbView<'_>) {
    let selected = PcbView::parse_selected(
        SOURCE,
        PcbLimits::default(),
        PcbSelection::only(PcbFamily::Zones),
    )
    .expect("zones");
    assert_eq!(
        selected
            .zones()
            .collect::<Result<Vec<_>, _>>()
            .expect("selected"),
        full.zones().collect::<Result<Vec<_>, _>>().expect("full")
    );
    assert_eq!(selected.counts().zones, 1);
    assert_eq!(selected.counts().nets, 0);
    assert_eq!(selected.nets().count(), 0);
    assert_eq!(selected.pads().count(), 0);
}

#[test]
fn selected_physical_facts_equal_full_views_and_hide_storage_dependencies() {
    let full = PcbView::parse(SOURCE, PcbLimits::default()).expect("full");
    let holes = PcbView::parse_selected(
        SOURCE,
        PcbLimits::default(),
        PcbSelection::only(PcbFamily::Holes),
    )
    .expect("holes");
    assert_eq!(
        holes
            .holes()
            .collect::<Result<Vec<_>, _>>()
            .expect("selected"),
        full.holes().collect::<Result<Vec<_>, _>>().expect("full")
    );
    assert_eq!(holes.pads().count(), 0);
    assert_eq!(holes.vias().count(), 0);
    assert_eq!(holes.nets().count(), 0);
    assert_eq!(holes.footprints().count(), 0);
    assert_eq!(holes.counts().pads, 0);
    assert_eq!(holes.counts().vias, 0);

    let profile = PcbView::parse_selected(
        SOURCE,
        PcbLimits::default(),
        PcbSelection::only(PcbFamily::Profile),
    )
    .expect("profile");
    assert_eq!(
        profile
            .profile_primitives()
            .collect::<Result<Vec<_>, _>>()
            .expect("selected"),
        full.profile_primitives()
            .collect::<Result<Vec<_>, _>>()
            .expect("full")
    );
    assert_eq!(profile.graphics().count(), 0);
    assert_eq!(profile.footprints().count(), 0);

    let transforms = PcbView::parse_selected(
        SOURCE,
        PcbLimits::default(),
        PcbSelection::only(PcbFamily::FootprintTransforms),
    )
    .expect("transforms");
    assert_eq!(
        transforms
            .footprint_transforms()
            .collect::<Result<Vec<_>, _>>()
            .expect("selected"),
        full.footprint_transforms()
            .collect::<Result<Vec<_>, _>>()
            .expect("full")
    );
    assert_eq!(transforms.footprints().count(), 0);
}

#[test]
fn unselected_family_limits_do_not_block_focused_reads() {
    let limits = PcbLimits {
        max_pads: 0,
        max_models: 0,
        max_segments: 0,
        max_vias: 0,
        max_graphics: 0,
        max_groups: 0,
        ..PcbLimits::default()
    };
    let selected = PcbView::parse_selected(SOURCE, limits, PcbSelection::only(PcbFamily::Zones))
        .expect("unselected ceilings are irrelevant");
    assert_eq!(selected.zones().count(), 1);
    PcbView::parse_selected(SOURCE, limits, PcbSelection::only(PcbFamily::Pads))
        .expect_err("selected pad ceiling");
}

#[test]
fn none_selection_keeps_structure_and_mutation_safe() {
    let selection = PcbSelection::none();
    assert!(!selection.contains(PcbFamily::Nets));
    let view = PcbView::parse_selected(SOURCE, PcbLimits::default(), selection).expect("structure");
    assert_eq!(view.counts().unknown_top_level, 1);
    assert_eq!(view.top_level_forms().count(), 11);
    assert_eq!(view.pads().count(), 0);
    let edit = view
        .set_property("Owner", "new")
        .expect("on-demand edit scan");
    assert!(edit.changed);

    let document = PcbDocument::parse(SOURCE.to_owned(), PcbLimits::default()).expect("document");
    assert_eq!(
        document
            .view_selected(PcbSelection::only(PcbFamily::Groups))
            .expect("selected document")
            .groups()
            .count(),
        1
    );
}
