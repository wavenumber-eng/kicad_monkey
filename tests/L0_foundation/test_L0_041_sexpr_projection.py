"""Projection scanning for selected KiCad S-expression forms."""

from __future__ import annotations

from pathlib import Path

import pytest

from kicad_monkey import (
    SexpSelector,
    SexprLexError,
    SexprTreeError,
    iter_sexp_file_form_spans,
    iter_sexp_form_spans,
    parse_sexp_span,
)


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
