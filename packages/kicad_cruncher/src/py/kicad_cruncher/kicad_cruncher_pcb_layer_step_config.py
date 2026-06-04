"""Config helpers for PCB layer STEP fixture-alignment output."""

from __future__ import annotations

import re

PCB_LAYER_STEP_CONFIG_SCHEMA = "wn.kicad_cruncher.pcb_layer_step.config.v1"
PCB_LAYER_STEP_CONFIG_SCHEMA_V2 = "wn.kicad_cruncher.pcb_layer_step.config.v2"

PCB_LAYER_STEP_DEFAULT_CONFIG_TEXT = """{
  /*
     kicad-cruncher pcb-layer-step configuration

     pcb-layer-step creates compact fixture-alignment models, not full
     fabrication STEP exports. Keep only the features that help verify pogo-pin
     alignment against DUT pads.


     CONFIG SHAPE

       "defaults"
         Shared settings copied into every output.

       "outputs"
         One or more output definitions. Each output accepts the same fields as
         defaults and overrides only what it needs.


     COORDINATES

       Geometer receives XY geometry relative to the KiCad aux axis origin
       from setup/aux_axis_origin. Boards without an aux axis origin use
       absolute KiCad PCB coordinates in millimeters.

       "z_mm" controls the bottom Z plane of each body.
       "thickness_mm" controls extrusion thickness.


     COMMON OUTPUT FIELDS

       name
       output_step
       pcbdoc
       layer
       z_mm
       thickness_mm

       copper_color
       include_copper
       include_board_outline
       include_board_cutouts
       include_poured_polygons
       cut_holes

       drill_hole_mode
       max_boolean_drill_cuts
       drill_hole_color
       drill_plated_hole_color
       drill_non_plated_hole_color
       drill_overlay_thickness_mm
       drill_minimum_diameter_mm
       drill_hole_shape
       drill_ring_width_mm
       drill_plated_ring_shape

       fuse_copper
       fuse_board_outline
       arc_segments

       include_tracks
       include_arcs
       include_fills
       include_regions
       include_vias
       include_component_pads
       include_free_pads
       include_designators
       pad_color_rules


     STRUCTURED SECTIONS

       "board_outline"
         color:        STEP color for the outer board-outline body.
         cutout_color: STEP color for interior board-cutout outline bodies.
         cutouts:      true/false, include separate cutout outline bodies.
         width_mm:     visual stroke width for outline bodies.
         fuse:         true/false, request Geometer fusion for outline bodies.

       fuse_copper
         true/false, request Geometer fusion for copper bodies. When enabled,
         trace-only bodies are also clipped with component pad and via copper
         cutouts so traces stop cleanly at pads/vias in review exports.

       "features"
         tracks:   true/false, include copper tracks.
                   May also be {enabled, color, body} when the output should
                   split normal tracks into their own colored STEP body.
         arcs:     true/false, include copper arcs.
         fills:    true/false, reserved for future fill-style feature support.
         polygons: true/false, include poured-zone filled polygons.
                   May also be {enabled, color, body} when pours should be
                   colored separately from normal copper.
         regions:  true/false, include copper graphics such as gr_poly/gr_rect.
         vias:     true/false, include via copper.
         free_pads: true/false, reserved for future pad-source feature support;
                    KiCad pads are footprint-owned.

         component_pads can be a boolean or an object:

           false
             omit all component-owned pads.

           true
             include component-owned pads. If include_designators is empty or
             omitted, all component-owned pads are included.

           {"mode": "none"}
             omit all component-owned pads.

           {"mode": "all"}
             include component-owned pads. Leave include_designators empty for
             all component-owned pads.

           {"mode": "matching_designators", "include_designators": [...]}
             include component-owned pads whose component designator matches at
             least one pattern.

       "colors"
         default_copper: STEP color for copper that no pad rule captures.
         pad_rules:      list of per-designator color/body rules.

         Each pad rule supports:
           designators: pattern list, such as ["TP*"].
           color:       named color or #RRGGBB.
           body:        Geometer body id/name for the matched pads.

       "drills"
         mode:                 auto, cut, overlay, or none.
         minimum_diameter_mm:  omit drills smaller than this diameter.
         shape:                solid or ring.
         color:                default drill-overlay color.
         plated_color:         plated drill-overlay color.
         non_plated_color:     non-plated drill-overlay color.
         ring_width_mm:        fixed annulus width when shape is ring.
         plated_ring_shape:    annulus or pad.
         overlay_thickness_mm: Z thickness for overlay drill bodies.


     DESIGNATOR PATTERNS

       Patterns are case-insensitive shell-style matches.

       Examples:
         ["TP*"]
         ["TP*", "J*", "U1", "U2"]
         ["M*"]


     COLOR VALUES

       Colors may be #RRGGBB values or one of these names:

         black, blue, brown, copper, gray, green, grey, orange, purple,
         red, white, yellow.


     LAYER VALUES

       Common selectors are bottom, top, B.Cu, F.Cu, BOTTOM, TOP, 31, 0, or a
       native KiCad layer name. The default fixture-alignment layer is bottom
       copper, B.Cu.


     DRILL MODES

       none
         Omit drill visualization.

       cut
         Subtract drill holes from copper bodies.

       overlay
         Render separate visible drill-reference bodies.

       auto
         Cut small drill sets. Switch to overlays when the board has more than
         max_boolean_drill_cuts drill features.


     DRILL SHAPES

       solid
         Render drill disks or slotted capsules.

       ring
         Render rings with the drill hole removed.

       plated_ring_shape "annulus"
         Use a fixed-width ring around plated holes.

       plated_ring_shape "pad"
         Use the full plated pad outline as the ring. This is useful for
         mounting-hole pads such as M1.


     CLI OVERRIDES

       CLI overrides are available for the main layer, color, outline, drill,
       fusion, and Z/thickness settings. Run:

         kicad-cruncher pcb-layer-step --help
  */
  "schema": "wn.kicad_cruncher.pcb_layer_step.config.v2",

  /*
     DEFAULTS

     Values here are copied into every output below. Put board-wide defaults
     here, and put experiment/review-specific changes in one output object.

     The generated template uses bottom copper because fixture pogo pins
     usually contact the DUT bottom side. Change "layer" to "top", "F.Cu",
     "B.Cu", or a KiCad layer token when reviewing another layer.
  */
  "defaults": {
    "pcbdoc": null,
    "layer": "bottom",
    "z_mm": 0.0,
    "thickness_mm": 0.035,
    "include_board_outline": true,

    /*
       BOARD OUTLINE

       "cutouts": true emits separate outline bodies for interior Edge.Cuts
       loops. It does not force copper clipping; copper drill/board cutouts
       are controlled by the drill and copper options.
    */
    "board_outline": {
      "color": "#111111",
      "cutout_color": "#FF0000",
      "cutouts": true,
      "width_mm": 0.2,
      "fuse": true
    }
  },

  /*
     OUTPUTS

     Add more objects to generate multiple STEP views from one config.

     Common review variants:
       - default fixture_alignment: only TP* pads plus drill/outline context.
       - all bottom copper: set component_pads.mode="all", tracks/arcs/vias/
         polygons/regions=true, and fuse_copper=true.
       - connector review: use include_designators such as ["J*", "U1"].
  */
  "outputs": [
    {
      "name": "fixture_alignment",
      "output_step": "{board}__fixture_alignment.step",

      /*
         FEATURES

         This default keeps the STEP small: only component pads matching TP*
         are included as copper. Tracks/arcs/polygons/vias can be enabled here
         for broader context. If tracks have {color, body}, arcs use the same
         trace style so trace geometry stays visually consistent.
      */
      "features": {
        "component_pads": {
          "mode": "matching_designators",
          "include_designators": ["TP*"]
        },
        "free_pads": false,
        "tracks": {
          "enabled": false,
          "color": "#B87333",
          "body": "tracks"
        },
        "arcs": false,
        "fills": false,
        "polygons": {
          "enabled": false,
          "color": "#7A8F2A",
          "body": "polygons"
        },
        "regions": false,
        "vias": false
      },

      /*
         COLORS / BODIES

         default_copper is used by all copper that is not captured by a rule.
         pad_rules split matching pads into named Geometer bodies. The default
         rule makes TP* pads red and places them in the "test_points" body.
      */
      "colors": {
        "default_copper": "#B87333",
        "pad_rules": [
          {
            "designators": ["TP*"],
            "color": "red",
            "body": "test_points"
          }
        ]
      },

      /*
         DRILLS

         overlay is fast and review-friendly: holes are drawn as thin bodies
         above the copper instead of boolean-cutting every hole. The 0.85 mm
         minimum keeps small component vias out of the default fixture view.

         For dense all-copper reviews, keep overlay mode but set
         minimum_diameter_mm to 0.0 if every pad/via drill should be visible.
      */
      "drills": {
        "mode": "overlay",
        "minimum_diameter_mm": 0.85,
        "shape": "ring",
        "color": "#666666",
        "plated_color": "#666666",
        "non_plated_color": "#00AEEF",
        "ring_width_mm": 0.12,
        "plated_ring_shape": "pad",
        "overlay_thickness_mm": 0.001
      },

      /*
         FUSION / CLIPPING

         false is fastest for the default TP-only fixture view. Set true for
         all-copper review outputs so Geometer unions same-body copper. When a
         trace-only body is split out, fused mode also clips that trace body
         with component pad and via copper cutouts.
      */
      "fuse_copper": false
    }
  ]
}
"""


def resolve_pcb_layer_selector(selector: str | int | None) -> str:
    """Resolve CLI/user layer selectors to a KiCad canonical layer token."""
    if selector is None:
        return "B.Cu"
    if isinstance(selector, int):
        return _layer_by_numeric_selector(selector)

    text = str(selector).strip()
    if not text:
        return "B.Cu"
    if text.isdigit():
        return _layer_by_numeric_selector(int(text))

    normalized = _normalize_layer_selector(text)
    layer = _layer_aliases().get(normalized)
    if layer is not None:
        return layer
    return _canonical_layer_spelling(text)


def _normalize_layer_selector(value: str) -> str:
    return re.sub(r"[\s_\-]+", "", value).upper()


def _layer_aliases() -> dict[str, str]:
    return {
        "TOP": "F.Cu",
        "TOPLAYER": "F.Cu",
        "FRONT": "F.Cu",
        "FCU": "F.Cu",
        "F.CU": "F.Cu",
        "BOTTOM": "B.Cu",
        "BOTTOMLAYER": "B.Cu",
        "BOT": "B.Cu",
        "BACK": "B.Cu",
        "BCU": "B.Cu",
        "B.CU": "B.Cu",
        "EDGE.CUTS": "Edge.Cuts",
        "EDGECUTS": "Edge.Cuts",
    }


def _layer_by_numeric_selector(value: int) -> str:
    if value == 0:
        return "F.Cu"
    if value == 31:
        return "B.Cu"
    if 1 <= value <= 30:
        return f"In{value}.Cu"
    raise ValueError(f"Unknown KiCad layer ordinal: {value!r}")


def _canonical_layer_spelling(value: str) -> str:
    lower = value.casefold()
    common = {
        "f.cu": "F.Cu",
        "b.cu": "B.Cu",
        "edge.cuts": "Edge.Cuts",
        "f.silks": "F.SilkS",
        "b.silks": "B.SilkS",
        "f.fab": "F.Fab",
        "b.fab": "B.Fab",
        "f.mask": "F.Mask",
        "b.mask": "B.Mask",
        "f.paste": "F.Paste",
        "b.paste": "B.Paste",
    }
    return common.get(lower, value)
