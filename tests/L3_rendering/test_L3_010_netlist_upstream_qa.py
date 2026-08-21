"""L3 oracle parity — upstream-QA schematics (Phase G Slice N-8, revised
2026-05-11).

For each mirrored KiCad 9/10-format upstream-QA netlist case, compile
our netlist via :class:`KiCadDesign` and structurally compare against
a *fresh* kicad-cli golden generated from the same ``.kicad_sch``.

Policy (2026-05-11): reference outputs are regenerated at test time
from the currently-staged kicad-cli rather than relying on the static
``.net`` files shipped in the upstream QA tree. The shipped goldens
were emitted years ago against pre-9.0 KiCad — they carry placeholder
``~`` values for every comp and other quirks that kicad-cli itself no
longer reproduces. Running the live oracle keeps both sides honest
against the current KiCad source. The pre-baked ``<case>.net`` files
in the corpus are retained for archaeology / spot-checks only.

The cached fresh golden lives under this stratum's ignored ``output/`` tree and is
invalidated whenever the source schematic's mtime advances.

Gated by:
* kicad-cli resolvable (``$KICAD_CLI`` / cache-staged / installed
  KiCad). Resolution mirrors L3_011's ``_resolve_cli``.
* the resolved ``KM_CORPUS`` archive contains the upstream-QA fixtures.
"""

from __future__ import annotations

import shutil
import subprocess
from pathlib import Path

import pytest

from _suite_paths import TEST_CORPUS_ROOT
from kicad_cli_resolver import resolve_kicad_cli
from kicad_monkey import (
    KiCadDesign,
    find_all_elements,
    find_element,
    get_value,
    parse_sexp,
)


# ---------------------------------------------------------------------------
# Corpus + CLI resolution (parity with L3_011)
# ---------------------------------------------------------------------------


def _resolve_corpus_root() -> Path | None:
    expected = TEST_CORPUS_ROOT / "kicad" / "netlist" / "upstream_qa"
    return TEST_CORPUS_ROOT if expected.is_dir() else None


def _resolve_cli() -> Path | None:
    return resolve_kicad_cli()


_CORPUS = _resolve_corpus_root()
_CLI = _resolve_cli()
_QA_ROOT = (
    (_CORPUS / "kicad" / "netlist" / "upstream_qa") if _CORPUS is not None else None
)
MIN_SUPPORTED_SCHEMATIC_VERSION = 20240716


def _candidate_top_schematic_path(root: Path) -> Path | None:
    candidate = root / f"{root.name}.kicad_sch"
    if candidate.is_file():
        return candidate
    schs = sorted(root.glob("*.kicad_sch"))
    return schs[0] if schs else None


def _schematic_file_format_version(path: Path) -> int | None:
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines()[:12]:
        stripped = line.strip()
        if stripped.startswith("(version ") and stripped.endswith(")"):
            value = stripped.removeprefix("(version ").removesuffix(")")
            return int(value) if value.isdigit() else None
    return None


def _is_supported_kicad_9_10_case(root: Path) -> bool:
    top_schematic = _candidate_top_schematic_path(root)
    if top_schematic is None:
        return False
    version = _schematic_file_format_version(top_schematic)
    return version is not None and version >= MIN_SUPPORTED_SCHEMATIC_VERSION


def _manifest_upstream_qa_cases() -> list[str]:
    try:
        from kicad_monkey.testing.corpus import (
            iter_kicad_corpus_cases,
            resolve_kicad_manifest_path,
        )

        out: list[str] = []
        for case in iter_kicad_corpus_cases(
            domain="netlist",
            origin="upstream_qa",
            status="active",
            required=False,
        ):
            root = resolve_kicad_manifest_path(case, "input_root")
            if (
                root is not None
                and root.is_dir()
                and _is_supported_kicad_9_10_case(root)
            ):
                out.append(root.name)
        return sorted(out)
    except Exception:
        return []


CASES: list[str] = _manifest_upstream_qa_cases() or (
    sorted(
        p.name
        for p in _QA_ROOT.iterdir()
        if p.is_dir() and _is_supported_kicad_9_10_case(p)
    )
    if _QA_ROOT is not None
    else []
)


# Drift entries — see L3_011 for triage rules. Keyed by case name.
# Each ``xfail(strict=False)`` so a fix elsewhere flips to XPASS without
# manual maintenance. Now that the goldens are regenerated from
# kicad-cli on every run, any remaining drift is a *real* compiler gap.
KNOWN_GAPS: dict[str, str] = {
    "component_classes": "one-pin unconnected net naming parity remains",
}


def _case_root(case: str) -> Path:
    assert _QA_ROOT is not None
    return _QA_ROOT / case


def _top_schematic_path(case: str) -> Path:
    root = _case_root(case)
    top_schematic = _candidate_top_schematic_path(root)
    if top_schematic is None:
        raise FileNotFoundError(f"no .kicad_sch under {root}")
    return top_schematic


# ---------------------------------------------------------------------------
# Fresh-golden emit (cached by mtime — parity with L3_011::_emit_golden)
# ---------------------------------------------------------------------------


_ORACLE_CACHE_ROOT = Path(__file__).parent / "output" / "netlist_upstream_qa"


def _emit_fresh_golden(case: str, *, cli: Path) -> Path:
    """Run kicad-cli on the case's top sch and cache the netlist.

    The cache is separate from the extracted corpus and invalidates whenever
    the source schematic's mtime advances.
    """
    sch = _top_schematic_path(case)
    cache_dir = _ORACLE_CACHE_ROOT / case
    cache_dir.mkdir(parents=True, exist_ok=True)
    out_path = cache_dir / "_fresh.net"
    if (
        out_path.exists()
        and out_path.stat().st_mtime >= sch.stat().st_mtime
        and out_path.stat().st_mtime >= cli.stat().st_mtime
    ):
        return out_path

    # Stage into a scratch dir so kicad-cli sees sub-sheets and any
    # .kicad_pro / sym-lib-table without polluting the corpus.
    stage = cache_dir / "_stage"
    if stage.exists():
        shutil.rmtree(stage, ignore_errors=True)
    stage.mkdir(parents=True, exist_ok=True)
    for child in sch.parent.iterdir():
        if child.is_file():
            shutil.copy2(child, stage / child.name)

    staged_sch = stage / sch.name
    proc = subprocess.run(
        [
            str(cli),
            "sch",
            "export",
            "netlist",
            "--format",
            "kicadsexpr",
            "--output",
            str(out_path),
            str(staged_sch),
        ],
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0:
        msg = (proc.stdout or "") + (proc.stderr or "")
        raise RuntimeError(f"kicad-cli netlist export failed for {case}: {msg.strip()}")
    return out_path


# ---------------------------------------------------------------------------
# Structural diff helpers (parity with L3_011)
# ---------------------------------------------------------------------------


def _component_summaries(export_sexp) -> set[tuple[str, str]]:
    out: set[tuple[str, str]] = set()
    components_block = find_element(export_sexp, "components")
    if components_block is None:
        return out
    for comp in find_all_elements(components_block, "comp"):
        ref = get_value(comp, "ref") or ""
        value = get_value(comp, "value") or ""
        out.add((str(ref), str(value)))
    return out


def _net_summaries(export_sexp) -> dict[str, set[tuple[str, str]]]:
    out: dict[str, set[tuple[str, str]]] = {}
    nets_block = find_element(export_sexp, "nets")
    if nets_block is None:
        return out
    for net in find_all_elements(nets_block, "net"):
        name = get_value(net, "name") or ""
        terminals: set[tuple[str, str]] = set()
        for node in find_all_elements(net, "node"):
            ref = get_value(node, "ref") or ""
            pin = get_value(node, "pin") or ""
            terminals.add((str(ref), str(pin)))
        out.setdefault(str(name), set()).update(terminals)
    return out


def _load_export(text: str):
    sexp = parse_sexp(text)
    if isinstance(sexp, list) and sexp and sexp[0] == "export":
        return sexp
    if isinstance(sexp, list) and len(sexp) == 1:
        return sexp[0]
    return sexp


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


@pytest.mark.skipif(
    _QA_ROOT is None,
    reason="netlist upstream_qa mirror not present; run sync_upstream_qa_netlist_fixtures.py",
)
def test_corpus_present_and_has_all_cases():
    """Sanity: the mirror exists and carries current KiCad 9/10 cases."""
    expected = {
        "component_classes",
        "hierarchical_component_classes",
        "jumpers",
        "multinetclasses",
    }
    assert expected.issubset(set(CASES)), f"missing cases: {expected - set(CASES)}"


@pytest.mark.skipif(_QA_ROOT is None, reason="netlist upstream_qa mirror not present")
@pytest.mark.skipif(
    _CLI is None, reason="kicad-cli not resolvable; set $KICAD_CLI or stage one"
)
@pytest.mark.parametrize("case", CASES)
def test_netlist_structurally_matches_upstream_golden(case, request):
    if case in KNOWN_GAPS:
        request.node.add_marker(
            pytest.mark.xfail(reason=KNOWN_GAPS[case], strict=False)
        )

    sch = _top_schematic_path(case)
    design = KiCadDesign.from_schematic_file(sch)
    ours_text = design.to_kicad_netlist_sexpr(date="")

    assert _CLI is not None  # for type-checker
    golden_path = _emit_fresh_golden(case, cli=_CLI)
    golden_text = golden_path.read_text(encoding="utf-8")

    ours = _load_export(ours_text)
    golden = _load_export(golden_text)

    ours_comps = _component_summaries(ours)
    golden_comps = _component_summaries(golden)
    assert ours_comps == golden_comps, (
        f"component set mismatch for {case}\n"
        f"  only in ours:    {sorted(ours_comps - golden_comps)}\n"
        f"  only in golden:  {sorted(golden_comps - ours_comps)}"
    )

    ours_nets = _net_summaries(ours)
    golden_nets = _net_summaries(golden)
    ours_nonempty = {k: v for k, v in ours_nets.items() if v}
    golden_nonempty = {k: v for k, v in golden_nets.items() if v}

    missing = set(golden_nonempty) - set(ours_nonempty)
    extra = set(ours_nonempty) - set(golden_nonempty)
    assert not missing and not extra, (
        f"net name mismatch for {case}\n"
        f"  missing nets:    {sorted(missing)}\n"
        f"  extra nets:      {sorted(extra)}"
    )

    for name, golden_terms in golden_nonempty.items():
        assert ours_nonempty[name] == golden_terms, (
            f"terminal set mismatch on net {name!r} in {case}\n"
            f"  only in ours:    {sorted(ours_nonempty[name] - golden_terms)}\n"
            f"  only in golden:  {sorted(golden_terms - ours_nonempty[name])}"
        )
