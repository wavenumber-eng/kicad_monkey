use kicad_monkey_core::{ErrorKind, PcbLimits, PcbPoint, PcbView, PcbZone, PcbZonePlacementSource};

const ZONES: &str = r#"(kicad_pcb
  (net 1 "GND")
  (zone (net 1) (net_name "GND") (locked yes) (layers "F.Cu" "B.Cu")
    (uuid zone-copper) (name "Power") (hatch edge 0.6) (priority 3)
    (placement (enabled yes) (component_class "RF"))
    (connect_pads (clearance 0.7))
    (min_thickness 0.3) (filled_areas_thickness yes)
    (fill yes (thermal_gap 0.4) (thermal_bridge_width 0.6)
      (island_removal_mode 2) (island_area_min 5))
    (property (layer "F.Cu") (hatch_position (xy 1 2)))
    (property (future_setting yes))
    (polygon (pts (xy 0 0) (xy 10 0) (xy 10 10)))
    (filled_polygon (layer "F.Cu") (island)
      (pts (xy 0 0) (xy 10 0) (xy 10 10) (xy 0 10))))
  (zone (net 0) (net_name "") (layer "F.Cu") (uuid zone-keepout)
    (keepout (tracks allowed) (vias not_allowed))
    (polygon (pts (xy 1 1) (xy 2 1) (xy 2 2))))
)"#;

#[test]
fn zones_expose_authored_and_filled_source_semantics() {
    let view = PcbView::parse(ZONES, PcbLimits::default()).expect("board");
    let zones = view.zones().collect::<Result<Vec<_>, _>>().expect("zones");
    assert_eq!(zones.len(), 2);

    let copper = &zones[0];
    assert_copper_identity(copper);
    assert_copper_fill(copper);
    assert_copper_geometry(copper);
    assert_keepout(&zones[1]);
}

fn assert_copper_identity(copper: &PcbZone) {
    assert_eq!(copper.net.ordinal, Some(1));
    assert_eq!(copper.net.name.as_deref(), Some("GND"));
    assert_eq!(copper.net_name.as_deref(), Some("GND"));
    assert!(copper.has_explicit_net_name);
    assert_eq!(copper.layers, ["F.Cu", "B.Cu"]);
    assert!(copper.layers_plural);
    assert!(copper.locked);
    assert_eq!(copper.uuid.as_deref(), Some("zone-copper"));
    assert_eq!(copper.name.as_deref(), Some("Power"));
    assert_eq!(copper.hatch_style, "edge");
    assert_eq!(copper.hatch_pitch, 0.6);
    assert_eq!(copper.priority, 3);
}

fn assert_copper_fill(copper: &PcbZone) {
    assert_eq!(copper.connect_pads_clearance, 0.7);
    assert_eq!(copper.min_thickness, 0.3);
    assert!(copper.filled_areas_thickness);
    assert!(copper.fill_enabled);
    assert_eq!(copper.thermal_gap, 0.4);
    assert_eq!(copper.thermal_bridge_width, 0.6);
    assert_eq!(copper.island_removal_mode, Some(2));
    assert_eq!(copper.island_area_min, Some(5.0));
    assert!(copper.keepout.is_none());

    let placement = copper.placement.as_ref().expect("placement");
    assert!(placement.enabled);
    assert_eq!(
        placement.source_type,
        PcbZonePlacementSource::ComponentClass
    );
    assert_eq!(placement.source_type.as_str(), "component_class");
    assert_eq!(placement.source, "RF");
}

fn assert_copper_geometry(copper: &PcbZone) {
    assert_eq!(copper.layer_properties.len(), 1);
    assert_eq!(copper.layer_properties[0].layer, "F.Cu");
    assert_eq!(
        copper.layer_properties[0].hatch_offset,
        PcbPoint { x: 1.0, y: 2.0 }
    );
    assert_eq!(copper.polygons.len(), 1);
    assert_eq!(copper.polygons[0].points.len(), 3);
    assert_eq!(copper.filled_polygons.len(), 1);
    assert_eq!(copper.filled_polygons[0].layer, "F.Cu");
    assert!(copper.filled_polygons[0].island);
    assert_eq!(copper.filled_polygons[0].points.len(), 4);
    assert!(ZONES[copper.source_range.clone()].starts_with("(zone"));
}

fn assert_keepout(keepout: &PcbZone) {
    assert_eq!(keepout.net.ordinal, Some(0));
    assert_eq!(keepout.net.name.as_deref(), Some(""));
    assert_eq!(keepout.net_name.as_deref(), Some(""));
    assert!(keepout.has_explicit_net_name);
    assert!(!keepout.layers_plural);
    let settings = keepout.keepout.as_ref().expect("keepout");
    assert_eq!(settings.tracks, "allowed");
    assert_eq!(settings.vias, "not_allowed");
    assert_eq!(settings.pads, "not_allowed");
    assert_eq!(settings.copperpour, "not_allowed");
    assert_eq!(settings.footprints, "not_allowed");
}

#[test]
fn absent_zone_fields_match_python_defaults() {
    let source = "(kicad_pcb (zone (layers) (property)))";
    let view = PcbView::parse(source, PcbLimits::default()).expect("board");
    let zone = view.zones().next().expect("zone").expect("decoded");
    assert_eq!(zone.net.ordinal, Some(0));
    assert_default_zone_scalars(&zone);
    assert_default_zone_collections(&zone);
}

fn assert_default_zone_scalars(zone: &PcbZone) {
    assert_eq!(zone.layers, Vec::<String>::new());
    assert!(!zone.layers_plural);
    assert!(!zone.locked);
    assert_eq!(zone.hatch_style, "edge");
    assert_eq!(zone.hatch_pitch, 0.5);
    assert_eq!(zone.priority, 0);
    assert_eq!(zone.connect_pads_clearance, 0.5);
    assert_eq!(zone.min_thickness, 0.25);
    assert!(!zone.filled_areas_thickness);
    assert!(!zone.fill_enabled);
    assert_eq!(zone.thermal_gap, 0.5);
    assert_eq!(zone.thermal_bridge_width, 0.5);
    assert!(zone.island_removal_mode.is_none());
    assert!(zone.island_area_min.is_none());
}

fn assert_default_zone_collections(zone: &PcbZone) {
    assert!(zone.keepout.is_none());
    assert!(zone.placement.is_none());
    assert!(zone.layer_properties.is_empty());
    assert!(zone.polygons.is_empty());
    assert!(zone.filled_polygons.is_empty());
}

#[test]
fn zone_collection_limits_are_lazy_and_fail_closed() {
    for limits in [
        PcbLimits {
            max_zone_polygons: 1,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_zone_points: 6,
            ..PcbLimits::default()
        },
        PcbLimits {
            max_zone_layer_properties: 0,
            ..PcbLimits::default()
        },
    ] {
        let view = PcbView::parse(ZONES, limits).expect("index remains lazy");
        assert_eq!(
            view.zones()
                .next()
                .expect("zone")
                .expect_err("zone limit")
                .kind,
            ErrorKind::ResourceLimit
        );
    }
}

#[test]
fn exact_zone_point_and_polygon_limits_are_accepted() {
    let view = PcbView::parse(
        ZONES,
        PcbLimits {
            max_zone_polygons: 2,
            max_zone_points: 7,
            max_zone_layer_properties: 1,
            ..PcbLimits::default()
        },
    )
    .expect("board");
    let zone = view.zones().next().expect("zone").expect("exact limits");
    assert_eq!(zone.polygons[0].points.len(), 3);
    assert_eq!(zone.filled_polygons[0].points.len(), 4);
}
