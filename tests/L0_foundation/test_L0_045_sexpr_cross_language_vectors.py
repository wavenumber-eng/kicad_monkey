"""Language-neutral S-expression vectors for Python/Rust parity."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import pytest

from kicad_monkey.kicad_sexpr import SexprError, build_sexp, parse_sexp


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
VECTOR_PATH = PACKAGE_ROOT / "tests" / "parity" / "sexpr_l0_vectors.a0.json"


def _cases() -> list[dict[str, Any]]:
    payload = json.loads(VECTOR_PATH.read_text(encoding="utf-8"))
    assert payload["schema"] == "kicad_monkey.sexpr_parity_vectors.a0"
    cases = payload["cases"]
    assert isinstance(cases, list)
    return cases


@pytest.mark.parametrize("case", _cases(), ids=lambda case: str(case["id"]))
def test_python_parser_matches_language_neutral_vector(case: dict[str, Any]) -> None:
    """Keep the Python oracle tied to the vectors consumed by Rust tests."""
    if case["phase"] == "ok":
        parsed = parse_sexp(case["source"])
        built = build_sexp(parsed)
        assert built == case["built"]
        assert parse_sexp(built) == parsed
        assert build_sexp(parse_sexp(built)) == built
        return

    with pytest.raises(SexprError) as caught:
        parse_sexp(case["source"])
    assert caught.value.phase == case["phase"]
    assert case["message_contains"] in str(caught.value)
