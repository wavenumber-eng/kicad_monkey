"""
Subtest: kicad-cli oracle gate
Stratum: L1_parsing
Purpose: Per-root-cause regression gate — for a curated set of minimal
reproducers, parse with kicad_monkey, emit, and assert that
``kicad-cli * upgrade --force`` accepts the result.

Each root cause has at least one fixture here. Cases that are known-broken pre-fix are marked
``xfail(strict=True)`` so they flip to **XPASS** (a hard failure) the
moment the fix lands — at which point the marker should be removed.

Skips entirely if no `kicad-cli` is resolvable on this machine.
"""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

from _suite_paths import TEST_CORPUS_ROOT
from kicad_cli_resolver import resolve_kicad_cli
from kicad_monkey import KiCadSchematic, KiCadSymbolLib

try:
    from kicad_monkey.kicad_pcb import KiCadPcb

    HAVE_PCB = True
except Exception:
    HAVE_PCB = False


# ---------------------------------------------------------------------------
# Resolve kicad-cli
# ---------------------------------------------------------------------------

CLI_VERB = {
    ".kicad_sch": ("sch", "upgrade"),
    ".kicad_sym": ("sym", "upgrade"),
    ".kicad_pcb": ("pcb", "upgrade"),
}


def _stage_file_with_siblings(src: Path, stage: Path) -> Path:
    shutil.copytree(src.parent, stage, dirs_exist_ok=True)
    return stage / src.name


# ---------------------------------------------------------------------------
# Corpus root resolution
# ---------------------------------------------------------------------------

_PACKAGE_CORPUS = TEST_CORPUS_ROOT


def _find_fixture(rel: str) -> Path | None:
    """Look up a fixture in the resolved ``KM_CORPUS`` archive."""
    candidate = _PACKAGE_CORPUS / rel
    return candidate if candidate.is_file() else None


# ---------------------------------------------------------------------------
# Curated minimal-reproducer cases
# ---------------------------------------------------------------------------

# Each tuple: (case_id, corpus-relative path, list[root_cause_ids], xfail_reason or None)
CASES = [
    # Guardrail — currently passes, must keep passing.
    (
        "guardrail_groups_load_save",
        r"kicad/upstream_qa/eeschema/groups_load_save/groups_load_save.kicad_sch",
        [],
        None,
    ),
    (
        "rc1_at_3tuple_sch",
        r"kicad/common/reference_schematics/input/flat_hierarchy.kicad_sch",
        ["#1"],
        # #1 fixed; #2 (empty lib_symbols dropped) and #3 (per-sheet instances)
        # are pure data-loss, not parser-fatal — they no longer block this gate.
        None,
    ),
    (
        "rc1_at_3tuple_sym",
        r"kicad/common/reference_symbols/input/C_2P_NP.kicad_sym",
        ["#1"],
        # #1 fixed; #7 (sym bulk content) was a misread of the unloadable
        # diff_sample. With #1 fixed kicad-cli loads our emit cleanly.
        None,
    ),
    (
        # Original inventory hypothesis (#4/#5/#6 — tenting / version /
        # plot-params) was wrong. Real cause: zone `(layers "*.Cu")` plural
        # form was being parsed as singular `(layer "")`, and kicad-cli
        # SEGFAULTed (rc 0xC0000005) on round-trip. Fixed via Zone.layers
        # plural-form support and FilledPolygon (island) sub-list emit.
        "rc9_zone_layers_plural",
        r"kicad/upstream_qa/pcbnew/plugins/kicad_sexpr/Issue19775_ZoneLayers/LayerWildcard.kicad_pcb",
        ["#9"],
        None,
    ),
    (
        # Root cause #8 (erratum to original inventory): KiCad uses
        # (id <bare-uuid>) NOT (uuid ...) for the first child of (generated),
        # and the parser SEGFAULTs (rc 0xC0000005) — not just rejects — when
        # the order is wrong or members are quoted. Fixed via GeneratedObject
        # parse/emit corrections.
        "rc8_generated_id_first",
        r"kicad/upstream_qa/pcbnew/tuning_generators_load_save/tuning_generators_load_save.kicad_pcb",
        ["#8"],
        None,
    ),
]


# ---------------------------------------------------------------------------
# Module-level fixtures
# ---------------------------------------------------------------------------


@pytest.fixture(scope="module")
def kicad_cli() -> Path:
    # This gate upgrades both schematic and PCB files, so it needs a build
    # with the pcbnew kiface loaded (schematic-only staged builds cannot
    # run `pcb upgrade`).
    cli = resolve_kicad_cli(required_capability="pcb_svg")
    if cli is None or not Path(cli).exists():
        pytest.skip("no PCB-capable kicad-cli resolvable on this machine")
    return Path(cli)


def _our_emit(path: Path) -> str:
    suffix = path.suffix
    if suffix == ".kicad_sch":
        return KiCadSchematic.from_file(path).to_text()
    if suffix == ".kicad_sym":
        return KiCadSymbolLib.from_file(path).to_text()
    if suffix == ".kicad_pcb":
        if not HAVE_PCB:
            pytest.skip("kicad_monkey.kicad_pcb not importable on this branch")
        pcb = KiCadPcb.from_file(path)
        if hasattr(pcb, "to_text"):
            return pcb.to_text()
        from kicad_monkey import build_sexp  # type: ignore

        return build_sexp(pcb.to_sexp())
    raise ValueError(f"Unsupported file kind: {suffix}")


# Build the parametrize list with conditional xfail markers.
def _params():
    out = []
    for case_id, rel, causes, xfail_reason in CASES:
        marks = []
        if xfail_reason:
            marks.append(pytest.mark.xfail(strict=True, reason=xfail_reason))
        out.append(pytest.param(case_id, rel, causes, id=case_id, marks=marks))
    return out


@pytest.mark.parametrize("case_id, rel, causes", _params())
def test_kicad_cli_accepts_emitted(
    case_id: str,
    rel: str,
    causes: list[str],
    kicad_cli: Path,
    tmp_path: Path,
) -> None:
    """Parse the fixture with kicad_monkey, emit it, and verify
    ``kicad-cli * upgrade --force`` returns 0."""
    src = _find_fixture(rel)
    if src is None:
        pytest.skip(f"fixture {rel!r} not found in the resolved $KM_CORPUS archive")

    # Stage the source's parent directory (siblings — `.kicad_pro`, `sym-lib-table`, etc.).
    stage = tmp_path / "ours"
    target = _stage_file_with_siblings(src, stage)
    target.write_text(_our_emit(src), encoding="utf-8")

    suffix = src.suffix
    sub, verb = CLI_VERB[suffix]
    proc = subprocess.run(
        [str(kicad_cli), sub, verb, "--force", str(target)],
        capture_output=True,
        text=True,
        timeout=120,
    )
    output = (proc.stdout or "") + (proc.stderr or "")
    assert proc.returncode == 0, (
        f"kicad-cli rejected emitted {case_id} ({src.name}) — root causes: {causes!r}\n"
        f"--- stdout/stderr (first 800 chars) ---\n{output[:800]}"
    )


def test_kicad_cli_accepts_unfilled_copper_zone(
    kicad_cli: Path,
    tmp_path: Path,
) -> None:
    """Root cause #10: unfilled copper zones must emit bare ``(fill ...)``.

    KiCad's parser accepts only a bare ``yes`` token inside ``fill``
    (pcb_io_kicad_sexpr_parser.cpp, case T_fill); ``(fill no ...)`` fails the
    whole board load ("Failed to load board", exit 3). No corpus fixture
    carries an unfilled copper zone, so this case forces one: parse a known
    zone fixture, disable fill on its copper zones, emit, and require
    kicad-cli to accept the result.
    """
    if not HAVE_PCB:
        pytest.skip("kicad_monkey.kicad_pcb not importable on this branch")
    rel = (
        r"kicad/upstream_qa/pcbnew/plugins/kicad_sexpr/"
        r"Issue19775_ZoneLayers/LayerWildcard.kicad_pcb"
    )
    src = _find_fixture(rel)
    if src is None:
        pytest.skip(f"fixture {rel!r} not found in the resolved $KM_CORPUS archive")

    pcb = KiCadPcb.from_file(src)
    copper_zones = [zone for zone in pcb.zones if zone.keepout is None]
    assert copper_zones, "fixture must contain at least one copper zone"
    for zone in copper_zones:
        zone.fill_enabled = False
        zone.filled_polygons = []
        # Graphics shapes legitimately emit `(fill no)`; only zone fill
        # elements must never carry the `no` token.
        fill_elems = [
            elem
            for elem in zone.to_sexp()
            if isinstance(elem, list) and elem and elem[0] == "fill"
        ]
        assert fill_elems and "no" not in fill_elems[0]

    if hasattr(pcb, "to_text"):
        text = pcb.to_text()
    else:
        from kicad_monkey import build_sexp  # type: ignore

        text = build_sexp(pcb.to_sexp())

    stage = tmp_path / "ours"
    target = _stage_file_with_siblings(src, stage)
    target.write_text(text, encoding="utf-8")
    proc = subprocess.run(
        [str(kicad_cli), "pcb", "upgrade", "--force", str(target)],
        capture_output=True,
        text=True,
        timeout=120,
    )
    output = (proc.stdout or "") + (proc.stderr or "")
    assert proc.returncode == 0, (
        f"kicad-cli rejected unfilled-copper-zone emit ({src.name})\n"
        f"--- stdout/stderr (first 800 chars) ---\n{output[:800]}"
    )
