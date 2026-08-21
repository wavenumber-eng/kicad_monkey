"""L3 oracle parity — project-rooted (Phase G Slice N-10).

For every ``.kicad_pro`` under the resolved ``$KM_CORPUS`` projects tree,
load via :class:`KiCadDesign` and structurally compare our compiled
netlist against ``kicad-cli sch export netlist --format kicadsexpr``
on the project's top schematic.

KiCad designs always have a project file; per project direction
(2026-05-11) the L3 oracle harness only validates full projects,
not orphan ``.kicad_sch`` files. Standalone-schematic gaps that
arise from missing project context are now invisible here — they
belong in lower-level parser/render tests, not the netlist oracle.

The comparison is the same shape as L3_010 (component set +
per-net terminal sets). For repeated visible sheet-pin names, KiCad's
``_N`` suffix owner follows internal connection-graph order; that
order is not semantic, so suffix-only permutations are canonicalized by
terminal set. Cases that exercise compiler gaps already documented in
``KNOWN_GAPS`` are marked ``xfail(strict=False)``.

Gated by:
* a kicad-cli binary resolvable via ``$KICAD_CLI``,
  ``$KICAD_CLI_CACHE_ROOT``, or installed KiCad.
* the resolved ``KM_CORPUS`` archive containing project fixtures.

Run cost: one ``kicad-cli`` invocation per project (cached on disk
between sessions under this stratum's ignored ``output/`` tree).
"""

from __future__ import annotations

import re
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
# Corpus + CLI resolution
# ---------------------------------------------------------------------------


def _resolve_corpus() -> Path | None:
    return (
        TEST_CORPUS_ROOT if (TEST_CORPUS_ROOT / "kicad" / "projects").is_dir() else None
    )


def _resolve_cli() -> Path | None:
    """Mirror ``oracle_diff.py::_resolve_cli`` for kicad-cli."""
    return resolve_kicad_cli()


_CORPUS = _resolve_corpus()
_CLI = _resolve_cli()
_PROJECTS_DIR = (_CORPUS / "kicad" / "projects") if _CORPUS is not None else None
_REF_OUTPUT_DIR = Path(__file__).parent / "output" / "netlist_kicad_cli_oracle"
_STAGE_SKIP_NAMES = {".git", ".history", "_stage", "output", "reference_output"}


def _find_top_schematic(pro: Path) -> Path | None:
    """Return the top ``.kicad_sch`` adjacent to ``pro`` (stem-matched)."""
    candidate = pro.with_suffix(".kicad_sch")
    return candidate if candidate.is_file() else None


def _manifest_real_world_projects() -> list[Path]:
    try:
        from kicad_monkey.testing.corpus import (
            iter_kicad_corpus_cases,
            resolve_kicad_manifest_path,
        )

        out: list[Path] = []
        for case in iter_kicad_corpus_cases(
            domain="netlist",
            origin="real_world",
            status="active",
            required=False,
        ):
            pro = resolve_kicad_manifest_path(case, "project_file")
            if (
                pro is not None
                and pro.is_file()
                and _find_top_schematic(pro) is not None
            ):
                out.append(pro)
        return sorted(out)
    except Exception:
        return []


# Each parametrised case is the project file. Skip projects that lack a
# stem-matching ``.kicad_sch`` (KiCad allows project-without-schematic
# but our oracle has nothing to render).
CASES: list[Path] = _manifest_real_world_projects() or (
    sorted(
        p
        for p in _PROJECTS_DIR.rglob("*.kicad_pro")
        if _find_top_schematic(p) is not None
    )
    if _PROJECTS_DIR
    else []
)


# Drift entries — see L3_010 for triage rules. Keyed by project stem
# (file name without ``.kicad_pro``). ``xfail(strict=False)`` so a fix
# elsewhere flips to XPASS without manual maintenance.
KNOWN_GAPS: dict[str, str] = {
    "JumperlessV5r7": "one-pin unconnected net naming parity remains",
    "nRF9151_Feather": "one-pin unconnected net naming parity remains",
    "11-10084__speedy_processing_module__B": (
        "one-pin unconnected net naming parity remains"
    ),
}


KNOWN_COMPONENT_METADATA_GAPS: dict[str, str] = {
    "CANBOB (MAGE-CANBOB-003)": "unit pin tie-order parity remains",
    "EDA-04903-V1-0": "multi-unit tstamp order parity remains",
    "EEZ DIB DCP405plus": "multi-unit tstamp order parity remains",
    "cm0": "multi-unit tstamp and unit pin tie-order parity remain",
    "icepi-zero": "unit pin tie-order parity remains",
    "JumperlessV5r7": "multi-unit tstamp order parity remains",
    "nRF9151_Feather": "unit pin tie-order and one metadata field parity remain",
    "11-10084__speedy_processing_module__B": "unit pin tie-order parity remains",
}


# ---------------------------------------------------------------------------
# kicad-cli oracle invocation (cached on disk)
# ---------------------------------------------------------------------------


def _emit_golden(pro: Path, *, cli: Path, dest_dir: Path) -> Path:
    """Run kicad-cli on the project's top sch to emit a golden ``.net``.

    Cache invalidates on schematic mtime — if the top schematic is
    newer than the cached golden we regenerate. Errors from kicad-cli
    surface as exceptions so the harness can xfail-or-skip the case
    rather than silently passing.
    """
    sch = _find_top_schematic(pro)
    assert sch is not None  # collection-time filter above
    dest_dir.mkdir(parents=True, exist_ok=True)
    out_path = dest_dir / f"{pro.stem}.net"
    latest_input_mtime = max(
        child.stat().st_mtime
        for child in pro.parent.rglob("*")
        if child.is_file()
        and not any(part in _STAGE_SKIP_NAMES for part in child.parts)
        and child.suffix != ".kicad_prl"
    )
    if (
        out_path.exists()
        and out_path.stat().st_mtime >= latest_input_mtime
        and out_path.stat().st_mtime >= cli.stat().st_mtime
        and out_path.stat().st_mtime >= Path(__file__).stat().st_mtime
    ):
        return out_path

    # Stage the project + siblings into a scratch dir so kicad-cli sees
    # the .kicad_pro / sub-sheets without polluting the corpus.
    stage = dest_dir / "_stage" / pro.stem
    if stage.exists():
        shutil.rmtree(stage, ignore_errors=True)
    stage.mkdir(parents=True, exist_ok=True)
    for child in pro.parent.iterdir():
        if child.name in _STAGE_SKIP_NAMES:
            continue
        if child.is_dir():
            shutil.copytree(
                child,
                stage / child.name,
                dirs_exist_ok=True,
                ignore=shutil.ignore_patterns(*_STAGE_SKIP_NAMES),
            )
        elif child.is_file() and child.suffix != ".kicad_prl":
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
        raise RuntimeError(
            f"kicad-cli netlist export failed for {pro.name}: {msg.strip()}"
        )
    return out_path


# ---------------------------------------------------------------------------
# Structural diff helpers (parity with L3_010)
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


def _child_text(sexp, name: str) -> str:
    value = get_value(sexp, name)
    return "" if value is None else str(value)


def _field_rows(comp) -> tuple[tuple[str, str], ...]:
    fields = find_element(comp, "fields")
    if fields is None:
        return ()
    rows: list[tuple[str, str]] = []
    for field in find_all_elements(fields, "field"):
        name = _child_text(field, "name")
        value = str(field[2]) if len(field) > 2 else ""
        rows.append((name, value))
    return tuple(rows)


def _property_rows(comp) -> tuple[tuple[str, str], ...]:
    return tuple(
        (_child_text(prop, "name"), _child_text(prop, "value"))
        for prop in find_all_elements(comp, "property")
    )


def _tstamp_rows(comp) -> tuple[str, ...]:
    tstamps = find_element(comp, "tstamps")
    if tstamps is None:
        return ()
    return tuple(str(value) for value in tstamps[1:])


def _unit_rows(comp) -> tuple[tuple[str, tuple[str, ...]], ...]:
    units = find_element(comp, "units")
    if units is None:
        return ()
    rows: list[tuple[str, tuple[str, ...]]] = []
    for unit in find_all_elements(units, "unit"):
        pins = find_element(unit, "pins")
        pin_nums = (
            tuple(_child_text(pin, "num") for pin in find_all_elements(pins, "pin"))
            if pins is not None
            else ()
        )
        rows.append((_child_text(unit, "name"), pin_nums))
    return tuple(rows)


def _component_metadata_rows(export_sexp) -> dict[tuple[str, str, str], tuple]:
    out: dict[tuple[str, str, str], tuple] = {}
    components_block = find_element(export_sexp, "components")
    if components_block is None:
        return out
    for comp in find_all_elements(components_block, "comp"):
        sheetpath = find_element(comp, "sheetpath") or []
        libsource = find_element(comp, "libsource") or []
        key = (
            _child_text(comp, "ref"),
            _child_text(sheetpath, "names"),
            _child_text(sheetpath, "tstamps"),
        )
        out[key] = (
            ("value", _child_text(comp, "value")),
            ("footprint", _child_text(comp, "footprint")),
            ("datasheet", _child_text(comp, "datasheet")),
            ("description", _child_text(comp, "description")),
            ("fields", _field_rows(comp)),
            (
                "libsource",
                (
                    _child_text(libsource, "lib"),
                    _child_text(libsource, "part"),
                    _child_text(libsource, "description"),
                ),
            ),
            ("properties", _property_rows(comp)),
            ("tstamps", _tstamp_rows(comp)),
            ("units", _unit_rows(comp)),
        )
    return out


def _format_component_metadata_diff(
    ours: dict[tuple[str, str, str], tuple],
    golden: dict[tuple[str, str, str], tuple],
) -> str:
    missing = sorted(set(golden) - set(ours))
    extra = sorted(set(ours) - set(golden))
    changed = [
        key for key in sorted(set(ours) & set(golden)) if ours[key] != golden[key]
    ]
    lines = [
        f"  missing component rows: {missing[:10]}",
        f"  extra component rows:   {extra[:10]}",
        f"  changed rows:           {changed[:10]}",
    ]
    for key in changed[:3]:
        lines.append(f"  row {key}:")
        lines.append(f"    ours:   {ours[key]}")
        lines.append(f"    golden: {golden[key]}")
    return "\n".join(lines)


def _net_summaries(export_sexp) -> dict[str, set[tuple[str, str]]]:
    out: dict[str, set[tuple[str, str]]] = {}
    nets_block = find_element(export_sexp, "nets")
    if nets_block is None:
        return out
    for net in find_all_elements(nets_block, "net"):
        name = get_value(net, "name") or ""
        terms: set[tuple[str, str]] = set()
        for node in find_all_elements(net, "node"):
            ref = get_value(node, "ref") or ""
            pin = get_value(node, "pin") or ""
            terms.add((str(ref), str(pin)))
        out.setdefault(str(name), set()).update(terms)
    return out


_DUPLICATE_SUFFIX_RE = re.compile(r"^(?P<base>.+)_(?P<suffix>[1-9][0-9]*)$")


def _duplicate_suffix_base(name: str) -> str:
    match = _DUPLICATE_SUFFIX_RE.match(name)
    return match.group("base") if match else name


def _bases_with_duplicate_suffixes(
    ours: dict[str, set[tuple[str, str]]],
    golden: dict[str, set[tuple[str, str]]],
) -> set[str]:
    counts: dict[str, int] = {}
    suffixed: set[str] = set()
    for nets in (ours, golden):
        for name in nets:
            base = _duplicate_suffix_base(name)
            counts[base] = counts.get(base, 0) + 1
            if base != name:
                suffixed.add(base)
    return {base for base in suffixed if counts.get(base, 0) > 1}


def _canonicalize_duplicate_suffixes(
    nets: dict[str, set[tuple[str, str]]],
    bases: set[str],
) -> dict[str, set[tuple[str, str]]]:
    out: dict[str, set[tuple[str, str]]] = {}
    for name, terminals in nets.items():
        base = _duplicate_suffix_base(name)
        if base in bases:
            term_sig = ",".join(f"{ref}:{pin}" for ref, pin in sorted(terminals))
            key = f"{base} <{term_sig}>"
        else:
            key = name
        out.setdefault(key, set()).update(terminals)
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


@pytest.mark.skipif(_CORPUS is None, reason="KM_CORPUS kicad/projects not present")
@pytest.mark.skipif(
    _CLI is None, reason="kicad-cli not resolvable; set $KICAD_CLI or stage one"
)
@pytest.mark.parametrize("pro", CASES, ids=lambda p: p.stem)
def test_netlist_matches_kicad_cli_oracle(pro: Path, request):
    stem = pro.stem
    if stem in KNOWN_GAPS:
        request.node.add_marker(
            pytest.mark.xfail(reason=KNOWN_GAPS[stem], strict=False)
        )

    assert _CLI is not None
    golden_path = _emit_golden(pro, cli=_CLI, dest_dir=_REF_OUTPUT_DIR)
    golden_text = golden_path.read_text(encoding="utf-8")

    design = KiCadDesign.from_project_file(pro)
    ours_text = design.to_kicad_netlist_sexpr(date="")

    ours = _load_export(ours_text)
    golden = _load_export(golden_text)

    ours_comps = _component_summaries(ours)
    golden_comps = _component_summaries(golden)
    assert ours_comps == golden_comps, (
        f"component set mismatch for {stem}\n"
        f"  only in ours:    {sorted(ours_comps - golden_comps)}\n"
        f"  only in golden:  {sorted(golden_comps - ours_comps)}"
    )

    ours_nets = _net_summaries(ours)
    golden_nets = _net_summaries(golden)
    ours_nonempty = {k: v for k, v in ours_nets.items() if v}
    golden_nonempty = {k: v for k, v in golden_nets.items() if v}

    duplicate_suffix_bases = _bases_with_duplicate_suffixes(
        ours_nonempty,
        golden_nonempty,
    )
    ours_compare = _canonicalize_duplicate_suffixes(
        ours_nonempty,
        duplicate_suffix_bases,
    )
    golden_compare = _canonicalize_duplicate_suffixes(
        golden_nonempty,
        duplicate_suffix_bases,
    )

    missing = set(golden_compare) - set(ours_compare)
    extra = set(ours_compare) - set(golden_compare)
    assert not missing and not extra, (
        f"net name mismatch for {stem}\n"
        f"  missing nets:    {sorted(missing)}\n"
        f"  extra nets:      {sorted(extra)}"
    )

    for name, golden_terms in golden_compare.items():
        assert ours_compare[name] == golden_terms, (
            f"terminal set mismatch on net {name!r} in {stem}\n"
            f"  only in ours:    {sorted(ours_compare[name] - golden_terms)}\n"
            f"  only in golden:  {sorted(golden_terms - ours_compare[name])}"
        )


@pytest.mark.skipif(_CORPUS is None, reason="KM_CORPUS kicad/projects not present")
@pytest.mark.skipif(
    _CLI is None, reason="kicad-cli not resolvable; set $KICAD_CLI or stage one"
)
@pytest.mark.parametrize("pro", CASES, ids=lambda p: p.stem)
def test_component_metadata_matches_kicad_cli_oracle(pro: Path, request):
    stem = pro.stem
    if stem in KNOWN_COMPONENT_METADATA_GAPS:
        request.node.add_marker(
            pytest.mark.xfail(
                reason=KNOWN_COMPONENT_METADATA_GAPS[stem],
                strict=False,
            )
        )

    assert _CLI is not None
    golden_path = _emit_golden(pro, cli=_CLI, dest_dir=_REF_OUTPUT_DIR)
    golden_text = golden_path.read_text(encoding="utf-8")

    design = KiCadDesign.from_project_file(pro)
    ours = _load_export(design.to_kicad_netlist_sexpr(date=""))
    golden = _load_export(golden_text)

    ours_component_metadata = _component_metadata_rows(ours)
    golden_component_metadata = _component_metadata_rows(golden)
    assert ours_component_metadata == golden_component_metadata, (
        f"component metadata mismatch for {stem}\n"
        f"{_format_component_metadata_diff(ours_component_metadata, golden_component_metadata)}"
    )
