"""
Test L0_034: render_pcb_ir_to_svg viewBox / translation behavior.
"""

from __future__ import annotations

import html
import json
import math
import re

from kicad_monkey import KiCadSvgRenderOptions, render_pcb_ir_to_svg
from kicad_monkey.kicad_pcb import KiCadPcb


_PCB_FIXTURE = """(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (44 "Edge.Cuts" user))
\t(gr_line (start 10 10) (end 50 10) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 50 10) (end 50 30) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 50 30) (end 10 30) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 10 30) (end 10 10) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
)
"""


_VIEWBOX_RE = re.compile(r'viewBox="([-\d.]+)\s+([-\d.]+)\s+([-\d.]+)\s+([-\d.]+)"')
_CIRCLE_R_RE = re.compile(r'<circle\b[^>]*\br="([-\d.]+)"')
_POLYGON_POINTS_RE = re.compile(r'<polygon\b[^>]*\bpoints="([^"]+)"')
_TRANSFORM_RE = re.compile(
    r'transform="translate\(([-\d.]+)\s+([-\d.]+)\)\s+rotate\(([-\d.]+)\)"'
)
_PCB_ENRICHMENT_RE = re.compile(
    r'<metadata id="pcb-enrichment-a0"[^>]*>(.*?)</metadata>',
    re.DOTALL,
)


def _extract_viewbox(svg: str) -> tuple[float, float, float, float]:
    match = _VIEWBOX_RE.search(svg)
    assert match is not None, f"viewBox not found in:\n{svg[:400]}"
    return (
        float(match.group(1)),
        float(match.group(2)),
        float(match.group(3)),
        float(match.group(4)),
    )


def _circle_radii(svg: str) -> list[float]:
    return [float(match.group(1)) for match in _CIRCLE_R_RE.finditer(svg)]


def _first_transformed_polygon_bbox(svg: str) -> tuple[float, float, float, float]:
    transform_match = _TRANSFORM_RE.search(svg)
    assert transform_match is not None, f"transform not found in:\n{svg[:400]}"
    tx, ty, angle = (float(part) for part in transform_match.groups())
    points_match = _POLYGON_POINTS_RE.search(svg)
    assert points_match is not None, f"polygon not found in:\n{svg[:400]}"

    rad = math.radians(angle)
    cos_a = math.cos(rad)
    sin_a = math.sin(rad)
    transformed: list[tuple[float, float]] = []
    for pair in points_match.group(1).split():
        x, y = (float(part) for part in pair.split(","))
        transformed.append((tx + x * cos_a - y * sin_a, ty + x * sin_a + y * cos_a))

    xs = [x for x, _y in transformed]
    ys = [y for _x, y in transformed]
    return (min(xs), min(ys), max(xs), max(ys))


def _pcb_enrichment_payload(svg: str) -> dict:
    match = _PCB_ENRICHMENT_RE.search(svg)
    assert match is not None, f"PCB enrichment metadata not found in:\n{svg[:800]}"
    return json.loads(html.unescape(match.group(1)))


def test_render_pcb_ir_to_svg_viewbox_uses_centerline_bounds():
    pcb = KiCadPcb.from_string(_PCB_FIXTURE)

    ir_svg = render_pcb_ir_to_svg(pcb)
    ir_vb = _extract_viewbox(ir_svg)

    assert ir_vb[0] == 0.0 and ir_vb[1] == 0.0
    assert ir_vb[2] == 40.0
    assert ir_vb[3] == 20.0


def test_render_pcb_ir_to_svg_translates_content_into_viewbox():
    """Bbox-based offset should land content inside 0..W/0..H user units."""

    pcb = KiCadPcb.from_string(_PCB_FIXTURE)
    svg = render_pcb_ir_to_svg(pcb)
    width, height = _extract_viewbox(svg)[2:]

    # Pull every numeric coordinate from polyline "points" attrs (the
    # current IR renderer emits gr_line as <polyline points="x,y x,y">)
    # and verify they fall within the bbox-derived viewBox (with a small
    # slack for half-stroke spill at the edges).
    point_lists = re.findall(r'points="([^"]+)"', svg)
    assert point_lists, "expected at least one polyline points attribute"
    coords: list[float] = []
    for points_attr in point_lists:
        for pair in points_attr.split():
            x_str, y_str = pair.split(",")
            coords.append(float(x_str))
            coords.append(float(y_str))
    assert coords, "expected at least one drawn coordinate"

    slack = 0.5  # mm
    for v in coords:
        assert -slack <= v <= max(width, height) + slack, (
            f"coord {v} outside viewBox 0..{width} / 0..{height}"
        )


def test_render_pcb_ir_to_svg_empty_board_returns_empty_svg():
    pcb = KiCadPcb.from_string(
        """(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal))
)
"""
    )
    svg = render_pcb_ir_to_svg(pcb)
    assert 'viewBox="0 0 0 0"' in svg


def test_npth_mask_layer_renders_expanded_aperture_when_pad_matches_drill():
    pcb = KiCadPcb.from_string(
        """(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (36 "F.Mask" user) (44 "Edge.Cuts" user))
\t(setup (pad_to_mask_clearance 0.1016))
\t(footprint "Test:NPTH"
\t\t(layer "F.Cu")
\t\t(at 0 0 0)
\t\t(pad "" np_thru_hole circle
\t\t\t(at 10 10)
\t\t\t(size 2.5 2.5)
\t\t\t(drill 2.5)
\t\t\t(layers "*.Cu" "*.Mask")
\t\t)
\t)
)
"""
    )

    mask_svg = render_pcb_ir_to_svg(pcb, layers=["F.Mask"])
    silk_svg = render_pcb_ir_to_svg(pcb, layers=["Edge.Cuts"])

    mask_radii = sorted(round(radius, 4) for radius in _circle_radii(mask_svg))
    silk_radii = sorted(round(radius, 4) for radius in _circle_radii(silk_svg))

    assert mask_radii == [1.25, 1.3516]
    assert silk_radii == [1.25]


def test_npth_mask_layer_uses_pad_aperture_when_pad_exceeds_drill():
    pcb = KiCadPcb.from_string(
        """(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (36 "F.Mask" user))
\t(setup (pad_to_mask_clearance 0.05))
\t(footprint "Test:NPTH"
\t\t(layer "F.Cu")
\t\t(at 0 0 0)
\t\t(pad "" np_thru_hole circle
\t\t\t(at 10 10)
\t\t\t(size 2.0 2.0)
\t\t\t(drill 1.2)
\t\t\t(layers "*.Cu" "*.Mask")
\t\t)
\t)
)
"""
    )

    mask_svg = render_pcb_ir_to_svg(pcb, layers=["F.Mask"])
    mask_radii = sorted(round(radius, 4) for radius in _circle_radii(mask_svg))

    assert mask_radii == [0.6, 1.05]


def test_embedded_footprint_pad_angle_is_relative_to_placement():
    pcb = KiCadPcb.from_string(
        """(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (44 "Edge.Cuts" user))
\t(gr_line (start 0 0) (end 30 0) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 30 0) (end 30 30) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 30 30) (end 0 30) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 0 30) (end 0 0) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(footprint "Test:RotatedPad"
\t\t(layer "F.Cu")
\t\t(at 15 15 90)
\t\t(pad "1" smd rect
\t\t\t(at 0 0 90)
\t\t\t(size 4 2)
\t\t\t(layers "F.Cu" "F.Mask" "F.Paste")
\t\t)
\t)
)
"""
    )

    svg = render_pcb_ir_to_svg(pcb, layers=["F.Cu"])
    min_x, min_y, max_x, max_y = _first_transformed_polygon_bbox(svg)
    width = round(max_x - min_x, 4)
    height = round(max_y - min_y, 4)

    assert width == 2.0
    assert height == 4.0


def test_kicad_pcb_to_svg_uses_ir_renderer():
    pcb = KiCadPcb.from_string(_PCB_FIXTURE)

    assert pcb.to_svg() == render_pcb_ir_to_svg(pcb)


def test_render_pcb_ir_to_svg_default_profile_keeps_enriched_metadata():
    pcb = KiCadPcb.from_string(_PCB_FIXTURE)

    svg = render_pcb_ir_to_svg(pcb)

    assert 'data-ref="gr_line"' in svg
    assert 'data-enrichment-schema="kicad_monkey.pcb.svg.enrichment.a0"' in svg


def test_render_pcb_ir_to_svg_oracle_profile_suppresses_metadata():
    pcb = KiCadPcb.from_string(_PCB_FIXTURE)

    svg = render_pcb_ir_to_svg(pcb, profile="oracle")

    assert 'data-ref="' not in svg
    assert 'data-uuid="' not in svg
    assert 'data-enrichment-schema=' not in svg
    assert "pcb-enrichment-a0" not in svg
    assert 'id="' not in svg
    assert "<path" in svg


def test_render_pcb_ir_to_svg_oracle_options_suppress_metadata():
    pcb = KiCadPcb.from_string(_PCB_FIXTURE)
    options = KiCadSvgRenderOptions(profile="oracle")

    svg = render_pcb_ir_to_svg(pcb, options=options)

    assert 'data-ref="' not in svg
    assert 'data-uuid="' not in svg
    assert 'data-primitive="' not in svg
    assert "pcb-enrichment-a0" not in svg
    assert 'id="' not in svg


def test_kicad_pcb_to_svg_forwards_profile_to_ir_renderer():
    pcb = KiCadPcb.from_string(_PCB_FIXTURE)

    assert pcb.to_svg(profile="oracle") == render_pcb_ir_to_svg(
        pcb,
        profile="oracle",
    )


def test_render_pcb_ir_to_svg_enriched_profile_enriches_pcb_relationships():
    from kicad_monkey.kicad_project import KiCadProject

    pcb = KiCadPcb.from_string(
        """(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (31 "B.Cu" signal) (36 "F.Mask" user) (44 "Edge.Cuts" user))
\t(net 0 "")
\t(net 1 "GND")
\t(gr_line (start 0 0) (end 30 0) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 30 0) (end 30 20) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 30 20) (end 0 20) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 0 20) (end 0 0) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(segment (start 5 10) (end 20 10) (width 0.25) (layer "F.Cu") (net 1) (uuid "seg-uuid"))
\t(via (at 22 10) (size 0.8) (drill 0.4) (layers "F.Cu" "B.Cu") (net 1) (uuid "via-uuid"))
\t(footprint "Test:R"
\t\t(layer "F.Cu")
\t\t(at 10 10 0)
\t\t(uuid "fp-uuid")
\t\t(property "Reference" "R1")
\t\t(property "Value" "10k")
\t\t(pad "1" thru_hole circle
\t\t\t(at 0 0)
\t\t\t(size 1.2 1.2)
\t\t\t(drill 0.6)
\t\t\t(layers "*.Cu" "*.Mask")
\t\t\t(net 1 "GND")
\t\t\t(uuid "pad-uuid")
\t\t)
\t)
)
"""
    )
    pcb.project = KiCadProject.from_json_dict(
        {"net_settings": {"netclass_assignments": {"GND": ["Power"]}}}
    )

    svg = render_pcb_ir_to_svg(pcb, layers=["F.Cu"])

    assert 'data-primitive="track"' in svg
    assert 'data-element-key="seg-uuid"' in svg
    assert 'data-net-index="1"' in svg
    assert 'data-net="GND"' in svg
    assert 'data-net-class="Power"' in svg
    assert 'data-primitive="via"' in svg
    assert 'data-primitive="via-hole"' in svg
    assert 'data-primitive="footprint"' in svg
    assert 'data-component="R1"' in svg
    assert 'data-component-uid="fp-uuid"' in svg
    assert 'data-footprint="Test:R"' in svg
    assert 'data-primitive="pad"' in svg
    assert 'data-primitive="pad-hole"' in svg
    assert 'data-pad-number="1"' in svg
    assert 'data-pad-designator="R1-1"' in svg
    assert 'data-pad-type="thru_hole"' in svg
    assert 'data-hole-kind="round"' in svg
    assert 'data-hole-plating="plated"' in svg
    assert 'data-hole-diameter-mm="0.6"' in svg
    assert 'data-via-type="through"' in svg
    assert 'data-via-drill-mm="0.4"' in svg


def test_render_pcb_ir_to_svg_enriched_metadata_carries_aliases_and_children():
    from kicad_monkey.kicad_project import KiCadProject

    pcb = KiCadPcb.from_string(
        """(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(5 "F.SilkS" user "Top Overlay")
\t\t(44 "Edge.Cuts" user)
\t)
\t(setup
\t\t(stackup
\t\t\t(layer "F.Cu" (type "copper") (thickness 0.035))
\t\t\t(layer "F.SilkS" (type "silkscreen"))
\t\t)
\t)
\t(footprint "Test:R"
\t\t(layer "F.Cu")
\t\t(at 10 10 0)
\t\t(uuid "fp-uuid")
\t\t(property "Reference" "R1" (at 0 0 0) (layer "F.SilkS"))
\t\t(fp_text user "${REFERENCE}" (at 0 2 0) (layer "F.SilkS"))
\t\t(fp_text user "NOTE" (at 0 4 0) (layer "F.SilkS"))
\t\t(fp_line (start -1 -1) (end 1 -1) (stroke (width 0.15) (type solid)) (layer "F.SilkS"))
\t)
)
"""
    )
    pcb.project = KiCadProject.from_json_dict(
        {"text_variables": {"DOC_NO": "11-10084"}}
    )

    svg = render_pcb_ir_to_svg(pcb, layers=["F.SilkS"])
    payload = _pcb_enrichment_payload(svg)

    assert payload["project"]["text_variables"] == {"DOC_NO": "11-10084"}
    assert payload["layers"]["layer_name_to_user_name"]["F.SilkS"] == "Top Overlay"
    assert payload["layers"]["layer_name_to_display_name"]["F.SilkS"] == "Top Overlay"
    assert payload["layers"]["layers"][1]["display_name"] == "Top Overlay"
    assert payload["board"]["stackup"]["layers"][1]["display_name"] == "Top Overlay"
    assert 'data-ref="property"' in svg
    assert 'data-footprint-text-role="designator"' in svg
    assert 'data-property-name="Reference"' in svg
    assert 'data-ref="fp_text"' in svg
    assert 'data-footprint-text-role="user"' in svg
    assert 'data-ref="fp_line"' in svg
    assert 'data-primitive="footprint-graphic"' in svg
    assert 'data-footprint-graphic-kind="line"' in svg


def test_render_pcb_ir_to_svg_npth_slot_hole_metadata():
    pcb = KiCadPcb.from_string(
        """(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (36 "F.Mask" user))
\t(footprint "Test:Slot"
\t\t(layer "F.Cu")
\t\t(at 0 0 0)
\t\t(uuid "fp-slot")
\t\t(property "Reference" "MH1")
\t\t(pad "" np_thru_hole oval
\t\t\t(at 10 10)
\t\t\t(size 0.4 1.0)
\t\t\t(drill oval 0.4 1.0)
\t\t\t(layers "*.Cu" "*.Mask")
\t\t\t(uuid "slot-pad")
\t\t)
\t)
)
"""
    )

    svg = render_pcb_ir_to_svg(pcb, layers=["F.Cu"])

    assert 'data-primitive="pad-hole"' in svg
    assert 'data-hole-kind="slot"' in svg
    assert 'data-hole-plating="non_plated"' in svg
    assert 'data-hole-render="drill"' in svg
    assert 'data-hole-width-mm="0.4"' in svg
    assert 'data-hole-height-mm="1.0"' in svg


def test_render_pcb_ir_to_svg_npth_hole_metadata_visible_on_inner_layers():
    pcb = KiCadPcb.from_string(
        """(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(1 "In1.Cu" signal)
\t\t(31 "B.Cu" signal)
\t\t(36 "F.Mask" user)
\t)
\t(footprint "Test:Mount"
\t\t(layer "F.Cu")
\t\t(at 0 0 0)
\t\t(uuid "fp-mount")
\t\t(property "Reference" "MH1")
\t\t(pad "" np_thru_hole circle
\t\t\t(at 10 10)
\t\t\t(size 2.5 2.5)
\t\t\t(drill 2.5)
\t\t\t(layers "F&B.Cu" "*.Mask")
\t\t\t(uuid "npth-pad")
\t\t)
\t)
)
"""
    )

    svg = render_pcb_ir_to_svg(pcb, layers=["In1.Cu"])

    assert 'data-primitive="pad-hole"' in svg
    assert 'data-hole-owner="npth-pad"' in svg
    assert 'data-hole-kind="round"' in svg
    assert 'data-hole-plating="non_plated"' in svg
    assert 'data-layer-names="F&amp;B.Cu,*.Mask"' in svg


def test_render_pcb_ir_to_svg_via_ipc4761_metadata():
    pcb = KiCadPcb.from_string(
        """(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (31 "B.Cu" signal))
\t(net 0 "")
\t(net 1 "GND")
\t(via
\t\t(at 10 10)
\t\t(size 0.3)
\t\t(drill 0.15)
\t\t(layers "F.Cu" "B.Cu")
\t\t(tenting (front yes) (back no))
\t\t(covering (front no) (back yes))
\t\t(plugging (front no) (back no))
\t\t(capping yes)
\t\t(filling yes)
\t\t(net 1)
\t\t(uuid "via-ipc")
\t)
)
"""
    )

    svg = render_pcb_ir_to_svg(pcb, layers=["F.Cu"])

    assert 'data-primitive="via"' in svg
    assert 'data-primitive="via-hole"' in svg
    assert 'data-hole-kind="round"' in svg
    assert 'data-hole-plating="plated"' in svg
    assert 'data-hole-diameter-mm="0.15"' in svg
    assert 'data-via-type="through"' in svg
    assert 'data-ipc4761-metadata="true"' in svg
    assert 'data-ipc4761-tenting-front="true"' in svg
    assert 'data-ipc4761-tenting-back="false"' in svg
    assert 'data-ipc4761-covering-front="false"' in svg
    assert 'data-ipc4761-covering-back="true"' in svg
    assert 'data-ipc4761-plugging-front="false"' in svg
    assert 'data-ipc4761-plugging-back="false"' in svg
    assert 'data-ipc4761-capping="true"' in svg
    assert 'data-ipc4761-filling="true"' in svg


def test_render_pcb_ir_to_svg_embeds_enrichment_payload():
    pcb = KiCadPcb.from_string(
        """(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (44 "Edge.Cuts" user))
\t(net 0 "")
\t(net 1 "GND")
\t(gr_line (start 0 0) (end 20 0) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 20 0) (end 20 20) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 20 20) (end 0 20) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 0 20) (end 0 0) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(footprint "Test:R"
\t\t(layer "F.Cu")
\t\t(at 10 10 90)
\t\t(uuid "fp-uuid")
\t\t(property "Reference" "R1")
\t\t(property "Value" "10k")
\t)
)
"""
    )

    svg = render_pcb_ir_to_svg(pcb, layers=["F.Cu"])
    payload = _pcb_enrichment_payload(svg)

    assert payload["schema"] == "kicad_monkey.pcb.svg.enrichment.a0"
    assert payload["view"] == {
        "kind": "layer_set",
        "included_layers": ["F.Cu"],
        "profile": "enriched",
        "includes_board_outline": False,
    }
    assert payload["layers"]["layer_ordinal_to_name"]["0"] == "F.Cu"
    assert payload["layers"]["layer_name_to_role"]["F.Cu"] == "copper"
    assert payload["lookup"]["net_index_to_name"]["1"] == "GND"
    assert payload["lookup"]["component_index_to_designator"] == {"0": "R1"}
    assert payload["lookup"]["component_index_to_uid"] == {"0": "fp-uuid"}
    assert payload["components"][0]["designator"] == "R1"
    assert payload["components"][0]["footprint"] == "Test:R"
    assert payload["components"][0]["rotation_deg"] == 90.0


def test_render_pcb_ir_to_svg_embeds_stackup_payload():
    pcb = KiCadPcb.from_string(
        """(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(general (thickness 1.6))
\t(layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
\t(setup
\t\t(stackup
\t\t\t(layer "F.Cu" (type "copper") (thickness 0.035 locked) (material "Cu") (color "#AA5500"))
\t\t\t(layer "dielectric 1" (type "core") (thickness 1.53) (material "FR4") (epsilon_r 4.5) (loss_tangent 0.02))
\t\t\t(layer "B.Cu" (type "copper") (thickness 0.035) (material "Cu"))
\t\t\t(copper_finish "ENIG")
\t\t\t(dielectric_constraints yes)
\t\t\t(edge_connector bevelled)
\t\t\t(edge_plating yes)
\t\t)
\t)
\t(gr_line (start 0 0) (end 20 0) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 20 0) (end 20 20) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 20 20) (end 0 20) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 0 20) (end 0 0) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
)
"""
    )

    svg = render_pcb_ir_to_svg(pcb)
    payload = _pcb_enrichment_payload(svg)
    stackup = payload["board"]["stackup"]

    assert stackup["present"] is True
    assert round(stackup["computed_thickness_mm"], 3) == 1.6
    assert stackup["copper_finish"] == "ENIG"
    assert stackup["dielectric_constraints"] is True
    assert stackup["edge_connector"] == "bevelled"
    assert stackup["edge_plating"] is True
    assert stackup["layers"][0] == {
        "index": 0,
        "name": "F.Cu",
        "display_name": "F.Cu",
        "type": "copper",
        "role": "copper",
        "thickness_mm": 0.035,
        "thickness_locked": True,
        "material": "Cu",
        "epsilon_r": None,
        "loss_tangent": None,
        "color": "#AA5500",
        "sublayers": [],
    }
    assert stackup["layers"][1]["role"] == "dielectric"
    assert stackup["layers"][1]["material"] == "FR4"
    assert stackup["layers"][1]["epsilon_r"] == 4.5
    assert stackup["layers"][1]["loss_tangent"] == 0.02


# ---------------------------------------------------------------------------
# Phase B: layer filtering (record-level)
# ---------------------------------------------------------------------------


_LAYER_FILTER_FIXTURE = """(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(31 "B.Cu" signal)
\t\t(37 "F.SilkS" user)
\t\t(44 "Edge.Cuts" user)
\t)
\t(gr_line (start 0 0) (end 30 0) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 30 0) (end 30 20) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 30 20) (end 0 20) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 0 20) (end 0 0) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
\t(gr_line (start 5 5) (end 25 5) (stroke (width 0.1) (type solid)) (layer "F.SilkS"))
\t(gr_text "TOP"
\t\t(at 15 15 0)
\t\t(layer "F.SilkS")
\t\t(effects (font (size 1 1) (thickness 0.15)))
\t)
\t(segment (start 5 10) (end 25 10) (width 0.25) (layer "F.Cu") (net 0))
)
"""


def test_render_pcb_ir_to_svg_layer_filter_drops_other_layers():
    """Filtering to F.SilkS keeps F.SilkS records and drops Edge.Cuts/F.Cu."""

    from kicad_monkey import pcb_to_ir
    from kicad_monkey.kicad_pcb_ir_svg import _filter_records_by_layer

    pcb = KiCadPcb.from_string(_LAYER_FILTER_FIXTURE)
    doc = pcb_to_ir(pcb)

    all_kinds = sorted({record.kind for record in doc.records})
    assert "gr_line" in all_kinds
    assert "gr_text" in all_kinds
    assert "segment" in all_kinds

    filtered = _filter_records_by_layer(doc.records, ["F.SilkS"])
    kinds = [record.kind for record in filtered]

    # F.SilkS has 1 gr_line and 1 gr_text. The 4 Edge.Cuts lines and the
    # F.Cu segment should be dropped.
    assert kinds.count("gr_line") == 1
    assert kinds.count("gr_text") == 1
    assert "segment" not in kinds


def test_render_pcb_ir_to_svg_layer_filter_none_keeps_all():
    from kicad_monkey import pcb_to_ir
    from kicad_monkey.kicad_pcb_ir_svg import _filter_records_by_layer

    pcb = KiCadPcb.from_string(_LAYER_FILTER_FIXTURE)
    doc = pcb_to_ir(pcb)

    assert len(_filter_records_by_layer(doc.records, None)) == len(doc.records)


def test_layer_filter_keeps_footprint_pad_blocks_balanced():
    """Layer filtering removes Start/payload/End as one strict-contract unit."""

    from kicad_monkey import pcb_to_ir
    from kicad_monkey.kicad_pcb_ir_svg import _filter_records_by_layer

    pcb = KiCadPcb.from_string(
        """(kicad_pcb
  (version 20240108)
  (generator "pcbnew")
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (36 "B.SilkS" user))
  (footprint "Fixture:BlockFilter"
    (layer "F.Cu")
    (at 10 10)
    (fp_line (start 0 0) (end 1 0) (stroke (width 0.1) (type solid)) (layer "B.SilkS"))
    (pad "1" thru_hole circle (at 0 0) (size 1 1) (drill 0.5) (layers "*.Cu" "*.Mask"))
  )
)
"""
    )
    doc = pcb_to_ir(pcb)
    filtered = _filter_records_by_layer(doc.records, ["F.Cu"])
    footprint = next(record for record in filtered if record.kind == "footprint")
    kinds = [str(operation.kind.value) for operation in footprint.operations]
    assert len(kinds) % 3 == 0
    assert all(
        kinds[index] == "StartBlock" and kinds[index + 2] == "EndBlock"
        for index in range(0, len(kinds), 3)
    )
    assert "FlashPadCircle" in kinds

    silk_filtered = _filter_records_by_layer(doc.records, ["B.SilkS"])
    silk_footprint = next(record for record in silk_filtered if record.kind == "footprint")
    silk_kinds = [str(operation.kind.value) for operation in silk_footprint.operations]
    assert silk_kinds == ["ThickSegment"]


def test_layer_filter_retains_direct_npth_hole_without_looping():
    from kicad_monkey.kicad_pcb_ir_svg import _filter_records_by_layer
    from kicad_monkey.kicad_plotter_ir import (
        KiCadPlotterOp,
        KiCadPlotterOpKind,
        KiCadPlotterRecord,
    )

    hole = KiCadPlotterOp(
        kind=KiCadPlotterOpKind.CIRCLE,
        payload={"role": "npth_hole", "layer": "B.Cu"},
    )
    record = KiCadPlotterRecord(
        uuid="npth",
        kind="fixture",
        object_id="npth",
        operations=[hole],
    )
    filtered = _filter_records_by_layer([record], ["F.Cu"])
    assert filtered[0].operations == [hole]


def test_layer_filter_keeps_zone_fill_layers_positionally_aligned():
    from kicad_monkey.kicad_pcb_ir_svg import _filter_records_by_layer
    from kicad_monkey.kicad_plotter_ir import (
        KiCadPlotterOp,
        KiCadPlotterOpKind,
        KiCadPlotterRecord,
    )

    front = KiCadPlotterOp(
        kind=KiCadPlotterOpKind.PLOT_POLY,
        payload={"points": [[0, 0], [1, 0], [0, 1]], "fill": "FILLED_SHAPE"},
    )
    back = KiCadPlotterOp(
        kind=KiCadPlotterOpKind.PLOT_POLY,
        payload={"points": [[2, 0], [3, 0], [2, 1]], "fill": "FILLED_SHAPE"},
    )
    record = KiCadPlotterRecord(
        uuid="zone-two-layer",
        kind="zone",
        object_id="zone-two-layer",
        operations=[front, back],
        extras={
            "fill_layers": ["F.Cu", "B.Cu"],
            "fill_island": ["front", "back"],
        },
    )

    filtered = _filter_records_by_layer([record], ["B.Cu"])

    assert filtered[0].operations == [back]
    assert filtered[0].extras["fill_layers"] == ["B.Cu"]
    assert filtered[0].extras["fill_island"] == ["back"]


def test_render_pcb_ir_to_svg_layer_filter_via_keeps_for_any_layer_in_span():
    """Through-hole vias span multiple layers; selecting any one keeps the via."""

    from kicad_monkey import pcb_to_ir
    from kicad_monkey.kicad_pcb_ir_svg import _filter_records_by_layer

    pcb = KiCadPcb.from_string(
        """(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
\t(via (at 10 10) (size 0.8) (drill 0.4) (layers "F.Cu" "B.Cu") (net 0))
\t(gr_line (start 0 0) (end 20 0) (stroke (width 0.15) (type solid)) (layer "Edge.Cuts"))
)
"""
    )
    doc = pcb_to_ir(pcb)

    # F.Cu selection keeps the via (which spans F.Cu+B.Cu) and drops Edge.Cuts gr_line.
    via_only = _filter_records_by_layer(doc.records, ["F.Cu"])
    via_kinds = [record.kind for record in via_only]
    assert "via" in via_kinds
    assert "gr_line" not in via_kinds

    # B.Cu selection keeps the via too.
    via_only_back = _filter_records_by_layer(doc.records, ["B.Cu"])
    assert any(record.kind == "via" for record in via_only_back)

    # Edge.Cuts selection keeps the gr_line and drops the via.
    edge_only = _filter_records_by_layer(doc.records, ["Edge.Cuts"])
    edge_kinds = [record.kind for record in edge_only]
    assert "gr_line" in edge_kinds
    assert "via" not in edge_kinds


def test_layer_filter_separates_resolved_via_aperture_from_physical_drill_span():
    from kicad_monkey import pcb_to_ir
    from kicad_monkey.kicad_pcb_bounds import compute_pcb_svg_bounding_box
    from kicad_monkey.kicad_pcb_ir_svg import _filter_records_by_layer

    pcb = KiCadPcb.from_string(
        """(kicad_pcb
  (version 20250830)
  (generator "pcbnew")
  (layers (0 "F.Cu" signal) (2 "In1.Cu" power) (4 "In2.Cu" power) (31 "B.Cu" signal))
  (net 1 "N")
  (via (at 10 10) (size 0.8) (drill 0.4) (layers "F.Cu" "B.Cu")
    (remove_unused_layers yes) (keep_end_layers no)
    (zone_layer_connections "In2.Cu") (net 1) (uuid "policy")))"""
    )
    records = pcb_to_ir(pcb).records

    connected = _filter_records_by_layer(records, ["In2.Cu"])[0]
    assert [operation.payload["role"] for operation in connected.operations] == [
        "via_aperture",
        "via_drill",
    ]

    removed = _filter_records_by_layer(records, ["In1.Cu"])[0]
    assert [operation.payload["role"] for operation in removed.operations] == ["via_drill"]

    connected_svg = render_pcb_ir_to_svg(pcb, layers=["In2.Cu"])
    removed_svg = render_pcb_ir_to_svg(pcb, layers=["In1.Cu"])
    assert 'data-ref="via"' in connected_svg
    assert 'data-ref="via"' not in removed_svg
    assert 'data-ref="drill_overlay"' in connected_svg
    assert 'data-ref="drill_overlay"' in removed_svg

    connected_bounds = compute_pcb_svg_bounding_box(pcb, ["In2.Cu"])
    removed_bounds = compute_pcb_svg_bounding_box(pcb, ["In1.Cu"])
    assert math.isclose(connected_bounds.width, 0.8)
    assert math.isclose(removed_bounds.width, 0.4)

    drill_only = KiCadPcb.from_string(
        """(kicad_pcb
  (layers (0 "F.Cu" signal) (2 "In1.Cu" power) (31 "B.Cu" signal))
  (net 1 "N")
  (via (at 10 10) (size 0.8) (drill 0.4) (layers "F.Cu" "B.Cu")
    (remove_unused_layers yes) (keep_end_layers no) (net 1) (uuid "drill-only")))"""
    )
    all_layer_bounds = compute_pcb_svg_bounding_box(drill_only, None)
    assert math.isclose(all_layer_bounds.width, 0.4)
    assert math.isclose(all_layer_bounds.height, 0.4)


def test_inner_layer_keeps_pth_and_npth_pad_drills_outside_copper_membership():
    from kicad_monkey import pcb_to_ir
    from kicad_monkey.kicad_pcb_ir_svg import _filter_records_by_layer
    from kicad_monkey.kicad_plotter_ir import KiCadPlotterOpKind

    pcb = KiCadPcb.from_string(
        """(kicad_pcb
  (layers (0 "F.Cu" signal) (2 "In1.Cu" power) (31 "B.Cu" signal))
  (footprint "Test:Holes" (layer "F.Cu") (at 0 0)
    (pad "1" thru_hole circle (at 1 1) (size 1 1) (drill 0.5)
      (layers "F&B.Cu" "*.Mask"))
    (pad "2" np_thru_hole circle (at 3 1) (size 0.6 0.6) (drill 0.6)
      (layers "*.Mask"))))"""
    )
    filtered = _filter_records_by_layer(pcb_to_ir(pcb).records, ["In1.Cu"])
    footprint = next(record for record in filtered if record.kind == "footprint")
    roles = [
        operation.payload.get("role")
        for operation in footprint.operations
        if operation.kind not in {KiCadPlotterOpKind.START_BLOCK, KiCadPlotterOpKind.END_BLOCK}
    ]
    assert roles == ["pad_drill", "npth_hole"]

    svg = render_pcb_ir_to_svg(pcb, layers=["In1.Cu"])
    assert svg.count('data-ref="pad_hole"') == 2


def test_render_pcb_ir_to_svg_layer_filter_emits_filtered_svg():
    """End-to-end: rendering with a layer filter produces a shorter SVG."""

    pcb = KiCadPcb.from_string(_LAYER_FILTER_FIXTURE)
    full_svg = render_pcb_ir_to_svg(pcb)
    silks_svg = render_pcb_ir_to_svg(pcb, layers=["F.SilkS"])

    # ViewBox stays the same because all-layer bounding box is independent
    # of the requested render layers.
    assert _extract_viewbox(full_svg) == _extract_viewbox(silks_svg)

    # Counting `data-ref="gr_line"` should drop from 5 (4 Edge.Cuts + 1
    # F.SilkS) to 1 once we filter to F.SilkS.
    assert full_svg.count('data-ref="gr_line"') == 5
    assert silks_svg.count('data-ref="gr_line"') == 1
