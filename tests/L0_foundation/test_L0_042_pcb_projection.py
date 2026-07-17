"""PCB projection public API coverage."""

from __future__ import annotations

from pathlib import Path

from kicad_monkey import (
    Footprint,
    GrLine,
    KiCadPcb,
    KiCadPcbProjection,
    Layer,
    Pad,
    PcbModelReference,
    ProjectedSource,
    SexpFormSpan,
    Segment,
    Via,
)


_PCB_TEXT = """(kicad_pcb
  (version 20240108)
  (generator "kicad")
  (generator_version "10.0.3")
  (paper "A4")
  (title_block (title "Demo"))
  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal)
    (32 "B.Adhes" user "B.Adhesive"))
  (setup
    (stackup
      (layer "F.Cu" (type "copper") (thickness 0.035))))
  (net 0 "")
  (net 1 "/A")
  (property "Lifecycle" "Prototype")
  (gr_line (start 0 0) (end 10 0) (stroke (width 0.1) (type solid)) (layer "Edge.Cuts"))
  (footprint "Device:R" (layer "F.Cu") (at 1 2 0)
    (property "Reference" "R1" (at 0 0 0) (layer "F.SilkS"))
    (property "Value" "10k" (at 0 1 0) (layer "F.Fab"))
    (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net 1 "/A"))
    (model "${KICAD10_3DMODEL_DIR}/Resistor.3dshapes/R.step"
      (offset (xyz 0 0 0))
      (scale (xyz 1 1 1))
      (rotate (xyz 0 0 0))))
  (segment (start 0 0) (end 1 0) (width 0.1) (layer "F.Cu") (net 1) (uuid "seg1"))
  (via (at 1 1) (size 0.4) (drill 0.2) (layers "F.Cu" "B.Cu") (net 1) (uuid "via1"))
)
"""


_PCB_V10_NAME_ONLY_NET_TEXT = """(kicad_pcb
  (version 20240108)
  (generator "kicad")
  (generator_version "10.0.3")
  (paper "A4")
  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal))
  (footprint "Device:R" (layer "F.Cu") (at 1 2 0)
    (property "Reference" "R1" (at 0 0 0) (layer "F.SilkS"))
    (pad "1" smd rect (at 0 0) (size 1 1) (layers "F.Cu") (net "GND")))
  (segment (start 0 0) (end 1 0) (width 0.1) (layer "F.Cu") (net "GND") (uuid "seg1"))
  (via (at 1 1) (size 0.4) (drill 0.2) (layers "F.Cu" "B.Cu") (net "GND") (uuid "via1"))
)
"""


def _write_board(tmp_path: Path) -> Path:
    path = tmp_path / "projection-demo.kicad_pcb"
    path.write_text(_PCB_TEXT, encoding="utf-8")
    return path


def _write_name_only_net_board(tmp_path: Path) -> Path:
    path = tmp_path / "projection-v10-name-only.kicad_pcb"
    path.write_text(_PCB_V10_NAME_ONLY_NET_TEXT, encoding="utf-8")
    return path


def _line_column_for_offset(text: str, offset: int) -> tuple[int, int]:
    line = text.count("\n", 0, offset) + 1
    last_newline = text.rfind("\n", 0, offset)
    column = offset + 1 if last_newline < 0 else offset - last_newline
    return line, column


def _assert_span_matches_source_text(source_text: str, span: SexpFormSpan) -> None:
    assert span.text() == source_text[span.start_offset:span.end_offset]
    assert (span.line, span.column) == _line_column_for_offset(source_text, span.start_offset)
    assert (span.end_line, span.end_column) == _line_column_for_offset(source_text, span.end_offset)


def test_pcb_projection_hydrates_same_domain_object_types(tmp_path: Path) -> None:
    board_path = _write_board(tmp_path)
    projection = KiCadPcbProjection.from_file(board_path)

    footprint = projection.footprints()[0]
    pad = projection.pads()[0]
    via = projection.vias()[0]
    segment = projection.segments()[0]
    gr_line = projection.gr_lines()[0]
    layer = projection.layers()[0]

    assert isinstance(footprint, Footprint)
    assert isinstance(pad, Pad)
    assert isinstance(via, Via)
    assert isinstance(segment, Segment)
    assert isinstance(gr_line, GrLine)
    assert isinstance(layer, Layer)
    assert footprint.get_property_value("Reference") == "R1"
    assert layer.canonical_name == "F.Cu"


def test_pcb_projection_resolves_net_references_like_full_board(tmp_path: Path) -> None:
    board_path = _write_board(tmp_path)
    projection = KiCadPcbProjection.from_file(board_path)

    assert projection.segments()[0].net.name == "/A"
    assert projection.vias()[0].net.name == "/A"
    assert projection.pads()[0].net.name == "/A"


def test_pcb_projection_preserves_v10_name_only_net_references(tmp_path: Path) -> None:
    board_path = _write_name_only_net_board(tmp_path)
    projection = KiCadPcbProjection.from_file(board_path)
    board = KiCadPcb.from_file(board_path)

    assert projection.nets() == []
    assert board.nets == []

    for net_ref in (
        projection.segments()[0].net,
        projection.vias()[0].net,
        projection.pads()[0].net,
        board.segments[0].net,
        board.vias[0].net,
        board.footprints[0].pads[0].net,
    ):
        assert net_ref.name == "GND"
        assert net_ref.ordinal is None


def test_pcb_projection_preserves_source_metadata(tmp_path: Path) -> None:
    board_path = _write_board(tmp_path)
    projection = KiCadPcbProjection.from_file(board_path)
    footprint = projection.footprints()[0]

    source = projection.source(footprint)

    assert isinstance(source, ProjectedSource)
    assert projection.source_span(footprint) is source.span
    assert projection.source_text(footprint).lstrip().startswith('(footprint "Device:R"')
    assert projection.source_sexp(footprint)[0] == "footprint"


def test_pcb_projection_reports_nested_model_references(tmp_path: Path) -> None:
    board_path = _write_board(tmp_path)
    projection = KiCadPcbProjection.from_file(board_path)

    model_ref = projection.model_references()[0]

    assert isinstance(model_ref, PcbModelReference)
    assert model_ref.reference == "R1"
    assert model_ref.value == "10k"
    assert model_ref.path.endswith("/R.step")
    assert model_ref.model_span is not None


def test_pcb_projection_rebased_nested_source_spans_match_source_text(tmp_path: Path) -> None:
    board_path = _write_board(tmp_path)
    source_text = board_path.read_text(encoding="utf-8")
    projection = KiCadPcbProjection.from_file(board_path)

    pad = projection.pads()[0]
    pad_span = projection.source_span(pad)
    assert pad_span is not None
    assert pad_span.text().lstrip().startswith(f'(pad "{pad.number}" ')
    _assert_span_matches_source_text(source_text, pad_span)

    model_ref = projection.model_references()[0]
    model_span = model_ref.model_span
    assert model_span is not None
    assert projection.source_span(model_ref.model) is model_span
    assert model_span.text().lstrip().startswith(f'(model "{model_ref.path}"')
    _assert_span_matches_source_text(source_text, model_span)


def test_pcb_projection_from_board_returns_board_owned_instances(tmp_path: Path) -> None:
    board_path = _write_board(tmp_path)
    board = KiCadPcb.from_file(board_path)
    projection = KiCadPcbProjection.from_board(board)

    assert projection.footprints()[0] is board.footprints[0]
    assert projection.layers()[0] is board.layers[0]
    assert projection.vias()[0] is board.vias[0]
    assert projection.source_span(board.footprints[0]).head == "footprint"
