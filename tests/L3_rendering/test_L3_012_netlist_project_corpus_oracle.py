"""L3 oracle parity -- modern project-rooted corpus netlists.

This is the broad KiCad 9/10 project-corpus gate. It complements:

* L3_010: upstream netlist QA cases mirrored under ``kicad/netlist``;
* L3_011: the dedicated real-project gate under ``kicad/projects``.

Collection intentionally ignores pre-9 schematic files. Known drifts are marked
``xfail(strict=False)`` so each compiler fix flips the case to XPASS before the
entry is removed.

The comparison is exact for component sets, net base names, and terminal sets.
For repeated visible sheet-pin names, KiCad assigns ``_N`` suffix ownership from
internal connection-graph order; that order is not semantic for rendering, so
this broad corpus gate canonicalizes suffix-only permutations by terminal set.
"""

from __future__ import annotations

import re
import shutil
import subprocess
import tempfile
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


MIN_SUPPORTED_SCHEMATIC_VERSION = 20240716
PROJECT_BUCKETS = ("projects", "common", "upstream_qa", "netlist")


def _resolve_corpus() -> Path | None:
    return TEST_CORPUS_ROOT if (TEST_CORPUS_ROOT / "kicad").is_dir() else None


_CORPUS = _resolve_corpus()
_KICAD_ROOT = (_CORPUS / "kicad") if _CORPUS is not None else None
_CLI = resolve_kicad_cli()
_REF_OUTPUT_DIR = Path(__file__).parent / "output" / "project_corpus_netlist_oracle"
_STAGE_SKIP_NAMES = {".git", ".history", "_stage", "output", "reference_output"}


def _schematic_file_format_version(path: Path) -> int | None:
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines()[:12]:
        stripped = line.strip()
        if stripped.startswith("(version ") and stripped.endswith(")"):
            value = stripped.removeprefix("(version ").removesuffix(")")
            return int(value) if value.isdigit() else None
    return None


def _find_top_schematic(pro: Path) -> Path | None:
    candidate = pro.with_suffix(".kicad_sch")
    return candidate if candidate.is_file() else None


def _is_supported_project(pro: Path) -> bool:
    sch = _find_top_schematic(pro)
    if sch is None:
        return False
    version = _schematic_file_format_version(sch)
    return version is not None and version >= MIN_SUPPORTED_SCHEMATIC_VERSION


def _rel(pro: Path) -> str:
    assert _KICAD_ROOT is not None
    return pro.relative_to(_KICAD_ROOT).as_posix()


def _candidate_projects() -> list[Path]:
    manifest_projects = _manifest_project_corpus_cases()
    if manifest_projects:
        return manifest_projects

    if _KICAD_ROOT is None:
        return []
    out: list[Path] = []
    for bucket in PROJECT_BUCKETS:
        root = _KICAD_ROOT / bucket
        if not root.exists():
            continue
        for pro in sorted(root.rglob("*.kicad_pro")):
            parts = set(pro.parts)
            if parts & _STAGE_SKIP_NAMES:
                continue
            if bucket == "netlist" and "reference_output" in parts:
                continue
            if _is_supported_project(pro):
                out.append(pro)
    return sorted(out, key=_rel)


def _manifest_project_corpus_cases() -> list[Path]:
    try:
        from kicad_monkey.testing.corpus import (
            iter_kicad_corpus_cases,
            resolve_kicad_manifest_path,
        )

        out: list[Path] = []
        for case in iter_kicad_corpus_cases(
            domain="netlist_project_corpus",
            status="active",
            required=False,
        ):
            pro = resolve_kicad_manifest_path(case, "project_file")
            if pro is not None and pro.is_file() and _is_supported_project(pro):
                out.append(pro)
        return sorted(out, key=_rel)
    except Exception:
        return []


CASES = _candidate_projects()


KNOWN_DRIFTS: dict[str, str] = {
    "common/board/input/speedy/speedy.kicad_pro": (
        "one-pin unconnected net naming parity remains"
    ),
    "common/speedy_reva/input/11-10084__speedy_processing_module__A.kicad_pro": (
        "one-pin unconnected net naming parity remains"
    ),
    "netlist/upstream_qa/component_classes/component_classes.kicad_pro": (
        "one-pin unconnected net naming parity remains"
    ),
    "projects/jumperless_v5r7/input/JumperlessV5r7.kicad_pro": (
        "one-pin unconnected net naming parity remains"
    ),
    "projects/nrf9151_feather/input/nRF9151_Feather.kicad_pro": (
        "one-pin unconnected net naming parity remains"
    ),
    "projects/speedy_processing_module/input/11-10084__speedy_processing_module__B.kicad_pro": (
        "one-pin unconnected net naming parity remains"
    ),
}


def _slug(pro: Path) -> str:
    stem = _rel(pro).removesuffix(".kicad_pro")
    return re.sub(r"[^A-Za-z0-9_.-]+", "__", stem)


def _emit_golden(pro: Path, *, cli: Path, dest_dir: Path) -> Path:
    sch = _find_top_schematic(pro)
    assert sch is not None
    dest_dir.mkdir(parents=True, exist_ok=True)
    out_path = dest_dir / f"{_slug(pro)}.net"
    if (
        out_path.exists()
        and out_path.stat().st_mtime >= sch.stat().st_mtime
        and out_path.stat().st_mtime >= cli.stat().st_mtime
    ):
        return out_path

    with tempfile.TemporaryDirectory(
        prefix=f"kicad_monkey_{_slug(pro)}_"
    ) as stage_name:
        stage = Path(stage_name)
        for child in pro.parent.iterdir():
            if child.name in _STAGE_SKIP_NAMES:
                continue
            if child.is_dir():
                shutil.copytree(
                    child,
                    stage / child.name,
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
            f"kicad-cli netlist export failed for {_rel(pro)}: {msg.strip()}"
        )
    return out_path


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


@pytest.mark.skipif(_KICAD_ROOT is None, reason="KM_CORPUS kicad root not present")
def test_project_corpus_inventory_matches_audit():
    rels = {_rel(pro) for pro in CASES}
    assert len(CASES) >= 54
    assert set(KNOWN_DRIFTS).issubset(rels)


@pytest.mark.skipif(_KICAD_ROOT is None, reason="KM_CORPUS kicad root not present")
@pytest.mark.skipif(
    _CLI is None, reason="kicad-cli not resolvable; set $KICAD_CLI or stage one"
)
@pytest.mark.parametrize("pro", CASES, ids=lambda p: _rel(p))
def test_project_corpus_netlist_matches_kicad_cli(pro: Path, request):
    rel = _rel(pro)
    if rel in KNOWN_DRIFTS:
        request.node.add_marker(
            pytest.mark.xfail(reason=KNOWN_DRIFTS[rel], strict=False)
        )

    assert _CLI is not None
    golden_path = _emit_golden(pro, cli=_CLI, dest_dir=_REF_OUTPUT_DIR)
    golden = _load_export(golden_path.read_text(encoding="utf-8"))

    design = KiCadDesign.from_project_file(pro)
    ours = _load_export(design.to_kicad_netlist_sexpr(date=""))

    ours_comps = _component_summaries(ours)
    golden_comps = _component_summaries(golden)
    assert ours_comps == golden_comps, (
        f"component set mismatch for {rel}\n"
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
        f"net name mismatch for {rel}\n"
        f"  missing nets:    {sorted(missing)}\n"
        f"  extra nets:      {sorted(extra)}"
    )

    for name, golden_terms in golden_compare.items():
        assert ours_compare[name] == golden_terms, (
            f"terminal set mismatch on net {name!r} in {rel}\n"
            f"  only in ours:    {sorted(ours_compare[name] - golden_terms)}\n"
            f"  only in golden:  {sorted(golden_terms - ours_compare[name])}"
        )
