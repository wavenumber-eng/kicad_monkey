"""Repo-corpus render-cache parity audit against the kicad-cli save oracle.

For every corpus board bearing render_cache blocks, strips the caches, has
kicad-cli regenerate them via the save oracle, then regenerates each text's
cache through the Python reference pipeline and compares per object path at
tolerance 0.002.  The corpus must stay fully closed: every oracle entry is
covered by a collected request and every generated cache matches.
"""

from __future__ import annotations

import hashlib
from collections import Counter
from pathlib import Path

import pytest

from kicad_cli_resolver import resolve_kicad_cli
from kicad_monkey.kicad_pcb import KiCadPcb
from kicad_monkey.kicad_render_cache import RenderCacheResolver
from kicad_monkey.kicad_render_cache_oracle import (
    collect_render_cache_requests_from_pcb,
    compare_render_caches,
    run_kicad_pcb_render_cache_save_oracle,
)
from kicad_monkey.testing.corpus import get_kicad_corpus_root

_CLI = resolve_kicad_cli(required_capability="pcb_svg")


def _corpus_boards(root: Path) -> list[Path]:
    """Unique render_cache-bearing boards, deduplicated by content."""
    seen: set[str] = set()
    boards: list[Path] = []
    for path in sorted(root.rglob("*.kicad_pcb")):
        data = path.read_bytes()
        if b"render_cache" not in data:
            continue
        digest = hashlib.sha256(data).hexdigest()
        if digest in seen:
            continue
        seen.add(digest)
        boards.append(path)
    return boards


def _audit_board(
    board: Path, work_dir: Path, reasons: Counter
) -> tuple[int, int, int]:
    """Return (matched, mismatched, uncovered) for one board."""
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=board,
        work_dir=work_dir,
        timeout=600,
    )
    expected = {entry.object_path: entry for entry in oracle.entries}
    pcb = KiCadPcb.from_file(oracle.oracle_pcb)
    resolver = RenderCacheResolver()
    matched = mismatched = uncovered = 0
    for request in collect_render_cache_requests_from_pcb(pcb):
        entry = expected.pop(request.object_path, None)
        if entry is None:
            # Requests the oracle did not cache (e.g. stroke-font texts)
            # are out of scope for save parity.
            continue
        if request.text_params is None:
            uncovered += 1
            reasons["no_text_params"] += 1
            continue
        result = resolver.generate_cache(request)
        if not result.cache:
            uncovered += 1
            reasons[
                "generate:" + ",".join(result.validation.reasons or ["unknown"])
            ] += 1
            continue
        comparison = compare_render_caches(entry.cache, result.cache, tolerance=0.002)
        if comparison.matched:
            matched += 1
        else:
            mismatched += 1
            reasons[
                "mismatch:" + (comparison.reasons[0] if comparison.reasons else "?")
            ] += 1
    for _entry in expected.values():
        uncovered += 1
        reasons["oracle_entry_without_request"] += 1
    return matched, mismatched, uncovered


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
def test_corpus_render_caches_fully_match_save_oracle(tmp_path: Path) -> None:
    root = get_kicad_corpus_root()
    boards = _corpus_boards(root)
    assert boards, "expected render_cache-bearing boards in the corpus"

    reasons: Counter = Counter()
    failures: list[str] = []
    total_matched = 0
    for index, board in enumerate(boards):
        rel = board.relative_to(root)
        matched, mismatched, uncovered = _audit_board(
            board, tmp_path / f"{index:03d}_{board.stem}", reasons
        )
        total_matched += matched
        if mismatched or uncovered:
            failures.append(
                f"{rel}: matched={matched} mismatched={mismatched} "
                f"uncovered={uncovered}"
            )

    detail = "; ".join(f"{reason}={count}" for reason, count in reasons.most_common())
    assert not failures, f"corpus render-cache gaps: {failures} ({detail})"
    assert total_matched > 0
