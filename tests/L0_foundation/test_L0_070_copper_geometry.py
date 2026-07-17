"""Contract and geometry tests for the copper geometry document."""

from __future__ import annotations

import json
from pathlib import Path

import jsonschema
import pytest

from kicad_monkey import (
    KICAD_COPPER_GEOMETRY_SCHEMA,
    KiCadCopperGeometryDocument,
    emit_pcb_copper_geometry,
)
from kicad_monkey.kicad_pcb import KiCadPcb
from kicad_monkey.kicad_pcb_footprint import Footprint
from kicad_monkey.kicad_pcb_projection import KiCadPcbProjection
from kicad_monkey.kicad_pcb_zone import Zone


_PROJECT_ROOT = Path(__file__).resolve().parents[2]
_SCHEMA_PATH = (
    _PROJECT_ROOT / "docs" / "contracts" / "kicad_copper_geometry_a0.schema.json"
)


def _board_text(*, version: int, name_only_nets: bool) -> str:
    net_table = "" if name_only_nets else '(net 1 "GND") (net 2 "UNUSED")'
    inline_net = '(net "GND")' if name_only_nets else "(net 1)"
    pad_net = '(net "GND")' if name_only_nets else '(net 1 "GND")'
    return f"""
(kicad_pcb
  (version {version})
  (generator pcbnew)
  (general (thickness 1.6))
  (paper "A4")
  (layers
    (0 "F.Cu" signal)
    (2 "In1.Cu" power)
    (31 "B.Cu" signal)
    (36 "B.SilkS" user "b.silkscreen")
  )
  {net_table}
  (setup
    (stackup
      (layer "F.Cu" (type "copper") (thickness 0.035))
      (layer "dielectric 1" (type "core") (thickness 1.53))
      (layer "B.Cu" (type "copper") (thickness 0.035))
    )
  )
  (segment (start 0 0) (end 5 0) (width 0.5) (layer "F.Cu") {inline_net} (uuid "track"))
  (arc (start 5 0) (mid 6 1) (end 5 2) (width 0.4) (layer "In1.Cu") {inline_net} (uuid "arc"))
  (via (at 7 2) (size 1.2) (drill 0.6) (layers "F.Cu" "B.Cu") {inline_net} (uuid "via"))
  (zone {inline_net} (layer "B.Cu") (uuid "zone") (hatch edge 0.5)
    (fill yes)
    (filled_polygon (layer "B.Cu") (pts (xy 0 4) (xy 5 4) (xy 5 8) (xy 0 8)))
  )
  (footprint "Test:Package"
    (layer "F.Cu")
    (at 10 20 90)
    (uuid "fp")
    (property "Reference" "U1" (at 0 0 90) (layer "F.SilkS"))
    (pad "1" smd rect (at 2 0 30) (size 2 1) (layers "F.Cu") {pad_net} (uuid "pad-rect"))
    (pad "2" smd oval (at 0 3 45) (size 2 1) (layers "*.Cu") {pad_net} (uuid "pad-oval"))
    (pad "3" thru_hole circle (at 4 3) (size 2 2)
      (drill oval 1.2 0.8 (offset 0.1 0.2))
      (layers "*.Cu") {pad_net} (uuid "pad-th"))
    (pad "" np_thru_hole circle (at 6 3) (size 1 1)
      (drill 1) (layers "*.Cu") (uuid "pad-npth"))
    (pad "4" smd roundrect (at 0 6 10) (size 2 1)
      (layers "B.Cu") (roundrect_rratio 0.25) {pad_net} (uuid "pad-roundrect"))
    (pad "5" smd custom (at 3 6 15) (size 1 1) (layers "F.Cu") {pad_net}
      (options (clearance outline) (anchor rect))
      (primitives
        (gr_poly (pts (xy -1 -0.5) (xy 1 -0.5) (xy 0 1)) (width 0) (fill yes))
      )
      (uuid "pad-custom")
    )
  )
)
"""


def _write_board(
    tmp_path: Path,
    *,
    version: int = 20240108,
    name_only_nets: bool = False,
) -> Path:
    path = tmp_path / ("v10.kicad_pcb" if name_only_nets else "v9.kicad_pcb")
    path.write_text(
        _board_text(version=version, name_only_nets=name_only_nets),
        encoding="utf-8",
    )
    return path


def test_copper_geometry_schema_and_supported_families(tmp_path: Path) -> None:
    board_path = _write_board(tmp_path)
    document = emit_pcb_copper_geometry(board_path)
    assert isinstance(document, KiCadCopperGeometryDocument)
    payload = document.to_dict()

    schema = json.loads(_SCHEMA_PATH.read_text(encoding="utf-8"))
    jsonschema.Draft202012Validator(schema).validate(payload)

    assert payload["schema"] == KICAD_COPPER_GEOMETRY_SCHEMA
    assert [layer["name"] for layer in payload["layers"]] == [
        "F.Cu",
        "In1.Cu",
        "B.Cu",
    ]
    assert {feature["kind"] for feature in payload["features"]} == {
        "track",
        "track_arc",
        "via",
        "pad",
        "zone_fill",
    }
    assert all(
        feature["outer_nm"][0] != feature["outer_nm"][-1]
        for feature in payload["features"]
    )
    via = next(feature for feature in payload["features"] if feature["kind"] == "via")
    assert via["layer_indexes"] == [0, 1, 2]
    assert len(via["holes_nm"]) == 1


def test_pad_transforms_multilayer_expansion_and_drills(tmp_path: Path) -> None:
    document = emit_pcb_copper_geometry(_write_board(tmp_path))

    rect = next(feature for feature in document.features if feature.source_uid == "pad-rect")
    center_x = sum(point[0] for point in rect.outer_nm) / len(rect.outer_nm)
    center_y = sum(point[1] for point in rect.outer_nm) / len(rect.outer_nm)
    assert round(center_x) == 10_000_000
    assert round(center_y) == 18_000_000

    oval = next(feature for feature in document.features if feature.source_uid == "pad-oval")
    assert oval.layer_indexes == (0, 1, 2)
    assert next(
        drill for drill in document.drills if drill.source_uid == "pad-th"
    ).oval
    plated = next(drill for drill in document.drills if drill.source_uid == "pad-th")
    assert (plated.width_nm, plated.height_nm, plated.plated) == (
        1_200_000,
        800_000,
        True,
    )
    npth = next(drill for drill in document.drills if drill.source_uid == "pad-npth")
    assert not npth.plated
    assert not any(
        feature.source_uid == "pad-npth" for feature in document.features
    )


def test_v9_ordinal_and_v10_name_only_nets(tmp_path: Path) -> None:
    v9 = emit_pcb_copper_geometry(_write_board(tmp_path))
    v10_path = tmp_path / "name-only.kicad_pcb"
    v10_path.write_text(
        _board_text(version=20260101, name_only_nets=True),
        encoding="utf-8",
    )
    v10 = emit_pcb_copper_geometry(v10_path)

    assert [(net.name, net.source_ordinal) for net in v9.nets] == [
        ("GND", 1),
        ("UNUSED", 2),
    ]
    assert [(net.name, net.source_ordinal) for net in v10.nets] == [("GND", None)]
    assert all(
        feature.net_index == 0
        for feature in v10.features
        if feature.source_uid != "pad-npth"
    )


def test_projection_full_board_and_path_inputs_are_deterministic(tmp_path: Path) -> None:
    board_path = _write_board(tmp_path)
    path_payload = emit_pcb_copper_geometry(board_path).to_dict()
    projection_payload = emit_pcb_copper_geometry(
        KiCadPcbProjection.from_file(board_path)
    ).to_dict()
    full_payload = emit_pcb_copper_geometry(KiCadPcb.from_file(board_path)).to_dict()

    assert path_payload == projection_payload == full_payload
    assert json.dumps(path_payload, sort_keys=True, separators=(",", ":")) == json.dumps(
        emit_pcb_copper_geometry(board_path).to_dict(),
        sort_keys=True,
        separators=(",", ":"),
    )


def test_path_emit_bypasses_full_zone_and_footprint_hydration(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    board_path = _write_board(tmp_path)

    def fail_full_hydration(*args: object, **kwargs: object) -> object:
        raise AssertionError("full container hydration must not run")

    monkeypatch.setattr(Footprint, "from_sexp", fail_full_hydration)
    monkeypatch.setattr(Zone, "from_sexp", fail_full_hydration)

    document = emit_pcb_copper_geometry(board_path)

    assert document.stats["zone_fills"] == 1
    assert document.stats["pads"] == 6


def test_slim_filled_polygon_regex_matches_full_parser(
    tmp_path: Path,
) -> None:
    board_path = tmp_path / "filled-edge-cases.kicad_pcb"
    text = _board_text(version=20240108, name_only_nets=False).replace(
        '(filled_polygon (layer "B.Cu") '
        '(pts (xy 0 4) (xy 5 4) (xy 5 8) (xy 0 8)))',
        '(filled_polygon (layer "B.Cu") (island) '
        '(pts (xy 0e0 4.0) (xy 5e0 4) (xy 5 8e0) (xy 0 8)))',
    )
    board_path.write_text(text, encoding="utf-8")

    slim = emit_pcb_copper_geometry(board_path).to_dict()
    full = emit_pcb_copper_geometry(KiCadPcb.from_file(board_path)).to_dict()

    assert slim == full
    zone_features = [
        feature
        for feature in slim["features"]
        if feature["kind"] == "zone_fill"
    ]
    assert zone_features[0]["island"] is True
