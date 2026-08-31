"""Projection scanning for selected KiCad S-expression forms."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from kicad_monkey import (
    Footprint,
    LibSymbol,
    SchSymbol,
    SexpSelector,
    SexprLexError,
    SexprTreeError,
    iter_kicad_objects_from_file,
    iter_kicad_objects_from_text,
    iter_sexp_file_form_spans,
    iter_sexp_form_spans,
    parse_sexp_span,
)
from kicad_monkey.kicad_model import EmbeddedFile, Model


PCB_TEXT = """(kicad_pcb
  # (footprint "Ignored:Comment")
  (setup
    (aux_axis_origin 1.0 2.0)
  )
  (footprint "Demo:R_0805"
    (property "Reference" "R1")
    (fp_text user "text with ) in a string")
    (pad "1" smd rect (at 1 2))
  )
  (footprint "Demo:C_0805"
    (property "Reference" "C1")
    (model "models/name(with-parens).step")
  )
)
"""

SCHEMATIC_TEXT = """(kicad_sch
  (version 20250114)
  (generator "kicad_monkey_test")
  (lib_symbols
    (symbol "demo:R"
      (property "Reference" "R" (id 0) (at 0 0 0))
      (symbol "demo:R_1_0"
        (rectangle (start 0 0) (end 2.54 2.54))
      )
    )
  )
  (symbol
    (lib_id "demo:R")
    (at 10 20 0)
    (unit 1)
    (property "Reference" "R1" (id 0) (at 10 20 0))
    (uuid "11111111-1111-1111-1111-111111111111")
  )
)
"""

PROJECTION_VECTOR_PATH = (
    Path(__file__).resolve().parents[1] / "parity" / "sexpr_projection_vectors.a0.json"
)


def test_projection_spans_match_language_neutral_vector() -> None:
    payload = json.loads(PROJECTION_VECTOR_PATH.read_text(encoding="utf-8"))
    assert payload["schema"] == "kicad_monkey.sexpr_projection_vectors.a0"

    actual = list(iter_sexp_form_spans(payload["source"]))
    assert len(actual) == len(payload["spans"])
    for span, expected in zip(actual, payload["spans"], strict=True):
        assert {
            "head": span.head,
            "path": list(span.path),
            "depth": span.depth,
            "start_python": span.start_offset,
            "end_python": span.end_offset,
            "line": span.line,
            "column": span.column,
            "end_line": span.end_line,
            "end_column": span.end_column,
        } == {
            key: expected[key]
            for key in (
                "head",
                "path",
                "depth",
                "start_python",
                "end_python",
                "line",
                "column",
                "end_line",
                "end_column",
            )
        }


def test_selector_filters_by_exact_path_and_depth() -> None:
    selector = SexpSelector(
        paths={("kicad_pcb", "footprint")},
        min_depth=1,
        max_depth=1,
    )

    spans = list(iter_sexp_form_spans(PCB_TEXT, selector))

    assert [span.head for span in spans] == ["footprint", "footprint"]
    assert [span.path for span in spans] == [
        ("kicad_pcb", "footprint"),
        ("kicad_pcb", "footprint"),
    ]
    assert [span.depth for span in spans] == [1, 1]
    assert "Ignored:Comment" not in "".join(span.text() for span in spans)


def test_form_span_text_and_parse_round_trip_selected_form() -> None:
    selector = SexpSelector(paths={("kicad_pcb", "footprint", "model")})

    [span] = list(iter_sexp_form_spans(PCB_TEXT, selector))

    assert span.text() == '(model "models/name(with-parens).step")'
    assert span.line == 13
    parsed = parse_sexp_span(span)
    assert parsed[0] == "model"
    assert str(parsed[1]) == "models/name(with-parens).step"


def test_file_form_spans_attach_source_path(tmp_path: Path) -> None:
    path = tmp_path / "board.kicad_pcb"
    path.write_text(PCB_TEXT, encoding="utf-8")

    [span] = list(
        iter_sexp_file_form_spans(
            path,
            SexpSelector(heads={"aux_axis_origin"}),
        )
    )

    assert span.source_path == str(path)
    assert span.text() == "(aux_axis_origin 1.0 2.0)"


def test_typed_reader_yields_board_footprint_objects() -> None:
    footprints = list(iter_kicad_objects_from_text(PCB_TEXT, Footprint))
    models = list(iter_kicad_objects_from_text(PCB_TEXT, Model))

    assert [footprint.library_link for footprint in footprints] == [
        "Demo:R_0805",
        "Demo:C_0805",
    ]
    assert [model.path for model in models] == ["models/name(with-parens).step"]


def test_typed_reader_distinguishes_library_and_placed_symbols() -> None:
    lib_symbols = list(iter_kicad_objects_from_text(SCHEMATIC_TEXT, LibSymbol))
    placed_symbols = list(iter_kicad_objects_from_text(SCHEMATIC_TEXT, SchSymbol))

    assert [symbol.name for symbol in lib_symbols] == ["demo:R"]
    assert [symbol.lib_id for symbol in placed_symbols] == ["demo:R"]


def test_typed_file_reader_uses_default_selector(tmp_path: Path) -> None:
    path = tmp_path / "board.kicad_pcb"
    path.write_text(PCB_TEXT, encoding="utf-8")

    footprints = list(iter_kicad_objects_from_file(path, Footprint))

    assert [footprint.get_property_value("Reference") for footprint in footprints] == [
        "R1",
        "C1",
    ]


def test_typed_reader_filters_embedded_file_forms() -> None:
    text = """(kicad_pcb
      (file "not-an-embedded-file-reference")
      (embedded_files
        (file
          (name "asset.step")
          (type model)
          (data "QUJD")
        )
      )
    )
    """

    files = list(iter_kicad_objects_from_text(text, EmbeddedFile))

    assert files == [EmbeddedFile(name="asset.step", file_type="model", data="QUJD")]


def test_selector_prunes_nested_forms() -> None:
    selector = SexpSelector(
        heads={"footprint", "pad"},
        prune_heads={"footprint"},
    )

    spans = list(iter_sexp_form_spans(PCB_TEXT, selector))

    assert [span.head for span in spans] == ["footprint", "footprint"]
    assert all(span.head != "pad" for span in spans)
    assert "(pad " in spans[0].text()


def test_projection_scanner_reports_unterminated_string() -> None:
    with pytest.raises(SexprLexError, match="Unterminated delimited string"):
        list(iter_sexp_form_spans('(kicad_pcb (title_block "missing end)'))


def test_projection_scanner_reports_unbalanced_opening_parenthesis() -> None:
    with pytest.raises(SexprTreeError, match="Unbalanced opening parenthesis"):
        list(iter_sexp_form_spans("(kicad_pcb (setup)"))
