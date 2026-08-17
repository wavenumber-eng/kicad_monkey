use kicad_monkey_core::{PcbFamily, PcbGraphicKind, PcbLimits, PcbSelection, PcbView};

const SOURCE: &str = r#"(kicad_pcb
  (footprint "Demo:R_0603"
    (layer "B.Cu")
    (at 10 20 45)
    (locked yes)
    (uuid fp-id)
    (descr "Demo resistor")
    (tags "demo resistor")
    (attr smd dnp exclude_from_bom)
    (embedded_fonts yes)
    (duplicate_pad_numbers_are_jumpers no)
    (solder_mask_margin 0.1)
    (solder_paste_margin -0.02)
    (solder_paste_margin_ratio -0.15)
    (clearance 0.25)
    (zone_connect 2)
    (property "Reference" "R1" (at 1 2 90) (layer "F.SilkS")
      (hide yes) (unlocked yes) (uuid property-id)
      (effects (font (face "Inter") (size 1.5 2.5) (thickness 0.2)))
      (render_cache "R1" 90 (polygon (pts (xy 0 0) (xy 1 0) (xy 0 1)))))
    (property "Datasheet" "https://example.invalid")
    (fp_line (start 0 0) (end 1 0) (stroke (width 0.1) (type solid))
      (layer "F.SilkS") (uuid line-id))
    (fp_arc (start 1 0) (mid 0 1) (end -1 0)
      (stroke (width 0.2) (type dash)) (layer "F.Fab"))
    (fp_circle (center 0 0) (end 1 0) (stroke (width 0.05) (type solid))
      (fill none) (layer "F.CrtYd"))
    (fp_rect (start -1 -1) (end 1 1) (stroke (width 0.05) (type solid))
      (fill solid) (layer "F.Cu"))
    (fp_poly (pts (xy 0 0) (xy 1 0) (xy 0 1))
      (stroke (width 0.05) (type solid)) (fill solid) (layer "B.Cu")))
)"#;

#[test]
fn embedded_footprint_metadata_matches_python_defaults_and_authored_values() {
    let view = PcbView::parse(SOURCE, PcbLimits::default()).expect("board");
    let footprint = view.footprints().next().expect("footprint").expect("typed");
    assert_footprint_identity(&footprint);
    assert_footprint_settings(&footprint);
    assert_eq!(&SOURCE[footprint.source_range][..10], "(footprint");
}

fn assert_footprint_identity(footprint: &kicad_monkey_core::PcbFootprint) {
    assert_eq!(footprint.library_link, "Demo:R_0603");
    assert_eq!(footprint.reference.as_deref(), Some("R1"));
    assert_eq!(footprint.description, "Demo resistor");
    assert_eq!(footprint.tags, "demo resistor");
    assert_eq!(footprint.attributes, ["smd", "dnp", "exclude_from_bom"]);
    assert!(footprint.locked);
    assert!(footprint.embedded_fonts);
}

fn assert_footprint_settings(footprint: &kicad_monkey_core::PcbFootprint) {
    assert_eq!(footprint.duplicate_pad_numbers_are_jumpers, Some(false));
    assert_eq!(footprint.solder_mask_margin, Some(0.1));
    assert_eq!(footprint.solder_paste_margin, Some(-0.02));
    assert_eq!(footprint.solder_paste_margin_ratio, Some(-0.15));
    assert_eq!(footprint.clearance, Some(0.25));
    assert_eq!(footprint.zone_connect, Some(2));
    assert_eq!(footprint.property_count, 2);
    assert_eq!(footprint.graphic_count, 5);
}

#[test]
fn footprint_properties_and_graphics_are_typed_in_source_order() {
    let view = PcbView::parse(SOURCE, PcbLimits::default()).expect("board");
    let properties = view
        .footprint_properties()
        .collect::<Result<Vec<_>, _>>()
        .expect("properties");
    assert_properties(&properties);

    let graphics = view
        .footprint_graphics()
        .collect::<Result<Vec<_>, _>>()
        .expect("graphics");
    assert_graphics(&graphics);
}

fn assert_properties(properties: &[kicad_monkey_core::PcbFootprintProperty]) {
    assert_eq!(properties.len(), 2);
    assert_graphical_property(&properties[0]);
    assert!(!properties[1].graphical);
    assert_eq!(properties[1].layer, "F.SilkS");
}

fn assert_graphical_property(property: &kicad_monkey_core::PcbFootprintProperty) {
    assert_eq!(property.footprint_index, 0);
    assert_eq!(property.name, "Reference");
    assert_eq!(property.value, "R1");
    assert_eq!((property.at.x, property.at.y), (1.0, 2.0));
    assert_eq!(property.angle, 90.0);
    assert_eq!(property.layer, "F.SilkS");
    assert!(property.hidden);
    assert!(property.unlocked);
    assert!(property.graphical);
    assert_eq!(property.effects.font.face.as_deref(), Some("Inter"));
    assert_eq!(
        (property.effects.font.size_x, property.effects.font.size_y),
        (2.5, 1.5)
    );
    let cache = property
        .render_cache_range
        .clone()
        .expect("property cache range");
    assert!(SOURCE[cache].starts_with("(render_cache"));
    assert_eq!(property.uuid.as_deref(), Some("property-id"));
}

fn assert_graphics(graphics: &[kicad_monkey_core::PcbFootprintGraphic]) {
    assert_eq!(graphics.len(), 5);
    assert!(graphics.iter().all(|item| item.footprint_index == 0));
    assert_eq!(
        graphics
            .iter()
            .map(|item| item.graphic.kind)
            .collect::<Vec<_>>(),
        [
            PcbGraphicKind::Line,
            PcbGraphicKind::Arc,
            PcbGraphicKind::Circle,
            PcbGraphicKind::Rect,
            PcbGraphicKind::Poly,
        ]
    );
    assert_eq!(graphics[0].graphic.layer.as_deref(), Some("F.SilkS"));
    assert_eq!(graphics[4].graphic.points.len(), 3);
}

#[test]
fn selected_footprint_children_equal_full_views_and_hide_parent_storage() {
    let full = PcbView::parse(SOURCE, PcbLimits::default()).expect("full");
    let properties = PcbView::parse_selected(
        SOURCE,
        PcbLimits::default(),
        PcbSelection::only(PcbFamily::FootprintProperties),
    )
    .expect("properties");
    assert_eq!(
        properties
            .footprint_properties()
            .collect::<Result<Vec<_>, _>>()
            .expect("selected"),
        full.footprint_properties()
            .collect::<Result<Vec<_>, _>>()
            .expect("full")
    );
    assert_eq!(properties.counts().footprint_properties, 2);
    assert_eq!(properties.counts().footprints, 0);
    assert_eq!(properties.counts().footprint_graphics, 0);
    assert_eq!(properties.footprints().count(), 0);

    let graphics = PcbView::parse_selected(
        SOURCE,
        PcbLimits::default(),
        PcbSelection::only(PcbFamily::FootprintGraphics),
    )
    .expect("graphics");
    assert_eq!(
        graphics
            .footprint_graphics()
            .collect::<Result<Vec<_>, _>>()
            .expect("selected"),
        full.footprint_graphics()
            .collect::<Result<Vec<_>, _>>()
            .expect("full")
    );
    assert_eq!(graphics.counts().footprint_graphics, 5);
    assert_eq!(graphics.counts().footprints, 0);
    assert_eq!(graphics.footprints().count(), 0);
}

#[test]
fn footprint_child_and_attribute_limits_fail_before_growth() {
    PcbView::parse_selected(
        SOURCE,
        PcbLimits {
            max_footprint_properties: 1,
            ..PcbLimits::default()
        },
        PcbSelection::only(PcbFamily::FootprintProperties),
    )
    .expect_err("property limit");
    PcbView::parse_selected(
        SOURCE,
        PcbLimits {
            max_footprint_graphics: 4,
            ..PcbLimits::default()
        },
        PcbSelection::only(PcbFamily::FootprintGraphics),
    )
    .expect_err("graphic limit");

    let view = PcbView::parse(
        SOURCE,
        PcbLimits {
            max_footprint_attributes: 2,
            ..PcbLimits::default()
        },
    )
    .expect("attributes decode lazily");
    let error = view
        .footprints()
        .next()
        .expect("footprint")
        .expect_err("attribute limit");
    assert_eq!(
        error.position.expect("absolute position").offset,
        SOURCE.find("(attr").unwrap()
    );
}

#[test]
fn duplicate_boolean_metadata_is_first_match_and_root_flag_linear() {
    let duplicates = (0..4_096)
        .map(|_| "    (embedded_fonts no)\n    (duplicate_pad_numbers_are_jumpers yes)\n")
        .collect::<String>();
    let source = format!(
        "(kicad_pcb\n  (footprint \"Duplicate\" locked\n    (embedded_fonts yes)\n    (duplicate_pad_numbers_are_jumpers no)\n{duplicates}  )\n)"
    );
    let view = PcbView::parse(&source, PcbLimits::default()).expect("duplicate fields");
    let footprint = view.footprints().next().expect("footprint").expect("typed");
    assert!(
        footprint.locked,
        "the root scalar flag remains authoritative"
    );
    assert!(footprint.embedded_fonts, "the first matching child wins");
    assert_eq!(footprint.duplicate_pad_numbers_are_jumpers, Some(false));
}
