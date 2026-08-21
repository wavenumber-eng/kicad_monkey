"""Mandatory native Phase 5 exit evidence across all Plotter-IR producers.

This gate intentionally has no skip path.  Release acceptance requires the
reviewed ZIP corpus, declared fonts, Cargo, and both PCB and schematic KiCad
SVG exporters.  Live KiCad checks prove successful save/plot acceptance; they
do not claim pixel or operation parity beyond the separately governed oracles.
"""

from __future__ import annotations

from collections import Counter
import contextlib
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
from typing import Any

import msgspec

from _suite_paths import KICAD_PACKAGE_ROOT
from _toolchain_paths import typespec_executable
from kicad_cli_resolver import kicad_cli_subprocess_env, resolve_kicad_cli
from kicad_monkey import (
    KiCadFootprint,
    KiCadPcb,
    KiCadProject,
    KiCadSchematic,
    KiCadSymbolLib,
    footprint_to_ir,
    lib_symbol_to_ir,
    pcb_to_ir,
    schematic_to_ir,
)
from kicad_monkey.contracts.generated import (
    decode_board_plot_document_a0,
    decode_footprint_plot_document_a0,
    decode_schematic_plot_document_a0,
    decode_symbol_plot_document_a0,
)
from kicad_monkey.kicad_render_cache import RenderCache
from kicad_monkey.kicad_render_cache_oracle import (
    compare_render_caches,
    run_kicad_pcb_render_cache_save_oracle,
)
from kicad_monkey.kicad_sexpr import parse_sexp
from kicad_monkey.testing.corpus import (
    get_kicad_corpus_case,
    get_kicad_corpus_root,
    iter_kicad_corpus_cases,
    resolve_kicad_manifest_path,
)


PACKAGE_ROOT = KICAD_PACKAGE_ROOT
SCRIPTS_DIR = PACKAGE_ROOT / "scripts"
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

from generate_schematic_plotter_vectors import expected_for as schematic_expected_for  # noqa: E402


_PRODUCERS = ("footprint", "symbol", "board", "schematic")
_VECTOR_FILES = {
    "footprint": ("footprint_plotter_a0_vectors.json", 6, 36),
    "symbol": ("symbol_plotter_a0_vectors.json", 2, 15),
    "board": ("board_plotter_a0_vectors.json", 13, 482),
    "schematic": ("schematic_plotter_a0_vectors.json", 9, 238),
}
_CORPUS_SELECTORS = {
    "footprint": (
        "footprint_ir",
        ".kicad_mod",
        "public_library/kicad_official_footprints/Resistor_SMD__R_0402_1005Metric",
        "input_file",
        "e05c7605248c220836f642ed1f526133edf0374acdfd5bb2631e58b4e377acfb",
        9,
    ),
    "symbol": (
        "symbol_ir",
        ".kicad_sym",
        "public_library/kicad_official_symbols/Connector_Generic__Conn_02x05_Odd_Even",
        "input_file",
        "ba0684eb07e4ad6d3e324330579fcb7c1750738ad2892f348845829a2dc8c96c",
        32,
    ),
    "board": (
        "pcb_ir",
        ".kicad_pcb",
        "synthetic/pcb_foundation/case001__track_top_1mil",
        "input_file",
        "cb9f2d421e6039a732315d6e62801d7cec753d33eecd6afd6b1893a717ed77e3",
        5,
    ),
    "schematic": (
        "schematic_ir",
        ".kicad_sch",
        "synthetic/schematic_svg/sallen_key",
        "input_file",
        "3645b099e1f933de74cddef7004c3e1dceae8d1d4b85bfa874bd552676f7ed06",
        289,
    ),
}
_FIXTURE_FONT = PACKAGE_ROOT / "tests/parity/fonts/shaping-variable-fixture.ttf"
_FIXTURE_FONT_SHA256 = "faa68bc8dee69291f89b181de3caa97172ac346900af996a9f5adc9045119e36"
_ARIAL = Path("C:/Windows/Fonts/arial.ttf")
_VALID_CORPUS_PHASES = {"read", "lex", "tree", "build", "reparse", "compare", "ok"}
_PHASE5_FREEZE = PACKAGE_ROOT / "tests/parity/plotter_ir_phase5_freeze.json"
_PHASE5_SCHEMA_NAMES = {
    f"{producer}Plot{suffix}.json"
    for producer in ("Board", "Footprint", "Schematic", "Symbol")
    for suffix in ("Document", "Request", "Result")
}
_PHASE5_UNION_ARM_COUNTS = {
    ("FootprintPlotDocument.json", "PlotterOperation"): 14,
    ("SymbolPlotDocument.json", "SymbolPlotRecord"): 2,
    ("BoardPlotDocument.json", "BoardPlotRecord"): 10,
    ("BoardPlotDocument.json", "BoardFootprintOperation"): 15,
    ("SchematicPlotDocument.json", "SchematicPlotRecord"): 23,
    ("SchematicPlotDocument.json", "SchematicSymbolOperation"): 16,
    ("SchematicPlotDocument.json", "SchematicSheetOperation"): 6,
}
_NATIVE_CORE_TARGETS = {
    "footprint": ("footprint_plotter_slice",),
    "symbol": ("symbol_plotter_slice",),
    "board": (
        "board_plotter_slice",
        "board_plotter_resource_limits",
        "board_dimension_slice",
        "board_footprint_slice",
        "plotter_text_cache",
    ),
    "schematic": ("schematic_plotter_slice",),
}
_NATIVE_CONTRACT_TARGETS = {
    "footprint": ("footprint_text_contracts",),
    "symbol": ("generated_contracts",),
    "board": ("board_plot_contracts",),
    "schematic": ("schematic_plot_contracts",),
}


def _selected_producers() -> tuple[str, ...]:
    configured = os.environ.get("KM_PHASE5_PRODUCERS", "").strip()
    if not configured:
        return _PRODUCERS
    selected = tuple(dict.fromkeys(part.strip().lower() for part in configured.split(",")))
    unknown = sorted(set(selected) - set(_PRODUCERS))
    assert selected and not unknown, (
        "KM_PHASE5_PRODUCERS must be a comma-separated subset of "
        f"{_PRODUCERS}; unknown={unknown}"
    )
    return selected


def _partition() -> tuple[int, int]:
    try:
        count = int(os.environ.get("KM_PHASE5_PARTITION_COUNT", "1"))
        index = int(os.environ.get("KM_PHASE5_PARTITION_INDEX", "0"))
    except ValueError as error:
        raise AssertionError("Phase 5 partition values must be integers") from error
    assert count > 0, "KM_PHASE5_PARTITION_COUNT must be positive"
    assert 0 <= index < count, (
        "KM_PHASE5_PARTITION_INDEX must be within "
        "[0, KM_PHASE5_PARTITION_COUNT)"
    )
    return index, count


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _canonical_json_sha256(path: Path) -> str:
    payload = json.loads(path.read_text(encoding="utf-8"))
    canonical = json.dumps(
        payload,
        ensure_ascii=False,
        separators=(",", ":"),
        sort_keys=True,
    ).encode("utf-8")
    return hashlib.sha256(canonical).hexdigest()


def _require_archive() -> Path:
    carrier_text = os.environ.get("KM_CORPUS", "").strip()
    assert carrier_text, "KM_CORPUS must name the reviewed kicad.zip"
    carrier = Path(carrier_text).expanduser()
    assert carrier.is_file() and carrier.suffix.lower() == ".zip", (
        "L3_022 requires a reviewed KM_CORPUS ZIP, not a loose authoring tree: "
        f"{carrier}"
    )
    root = get_kicad_corpus_root()
    assert (root / "manifest.json").is_file(), f"corpus manifest not found under {root}"
    return root


def _require_archive_and_fonts() -> Path:
    root = _require_archive()
    assert _FIXTURE_FONT.is_file(), f"declared fixture font not found: {_FIXTURE_FONT}"
    assert _sha256(_FIXTURE_FONT) == _FIXTURE_FONT_SHA256
    assert _ARIAL.is_file(), f"declared live KiCad cache font not found: {_ARIAL}"
    return root


def _stable_case(producer: str) -> tuple[dict[str, Any], Path]:
    _domain, suffix, case_id, path_key, digest, _operations = _CORPUS_SELECTORS[producer]
    case = get_kicad_corpus_case(case_id, required=True)
    assert case is not None and case.get("status") == "active"
    assert _domain in (case.get("domains") or [])
    path = resolve_kicad_manifest_path(case, path_key)
    assert path is not None and path.is_file() and path.suffix.lower() == suffix
    assert "input" in path.relative_to(get_kicad_corpus_root()).parts
    assert _sha256(path) == digest, f"reviewed {producer} corpus input drifted: {path}"
    return case, path


def _contract_symbol(document: dict[str, Any]) -> dict[str, Any]:
    projected = {
        key: document[key]
        for key in (
            "schema",
            "source_kind",
            "total_operations",
            "records",
            "source_path",
            "document_id",
            "coordinate_space",
        )
    }
    if projected["records"][0].get("extends") is None:
        del projected["records"][0]["extends"]
    return _normalize_integral_coordinates(projected)


def _normalize_integral_coordinates(value: Any) -> Any:
    if isinstance(value, dict):
        return {key: _normalize_integral_coordinates(child) for key, child in value.items()}
    if isinstance(value, list):
        return [_normalize_integral_coordinates(child) for child in value]
    if isinstance(value, float) and value.is_integer():
        return int(value)
    return value


@contextlib.contextmanager
def _without_shapely():
    saved = {
        name: sys.modules.pop(name)
        for name in list(sys.modules)
        if name == "shapely" or name.startswith("shapely.")
    }
    sys.modules["shapely"] = None
    try:
        yield
    finally:
        del sys.modules["shapely"]
        sys.modules.update(saved)


def _assert_vector_authority(producer: str) -> None:
    filename, expected_vectors, expected_operations = _VECTOR_FILES[producer]
    payload = json.loads((PACKAGE_ROOT / "tests/parity" / filename).read_text(encoding="utf-8"))
    vectors = payload["vectors"]
    assert len(vectors) == expected_vectors
    assert sum(vector["expected"]["total_operations"] for vector in vectors) == expected_operations

    for vector in vectors:
        if producer == "footprint":
            actual = footprint_to_ir(
                KiCadFootprint.from_string(vector["source"]),
                source_path=vector["source_path"],
                document_id=vector["document_id"],
            ).to_dict()
            decode = decode_footprint_plot_document_a0
        elif producer == "symbol":
            library = KiCadSymbolLib.from_text(vector["source"])
            symbol = next(item for item in library.symbols if item.name == vector["symbol_name"])
            actual = _contract_symbol(
                lib_symbol_to_ir(
                    symbol,
                    unit=vector["unit"],
                    style=vector["style"],
                    source_path=vector["source_path"],
                    document_id=vector["document_id"],
                    project_vars=vector.get("text_variables"),
                ).to_dict()
            )
            decode = decode_symbol_plot_document_a0
        elif producer == "board":
            pcb = KiCadPcb.from_string(vector["source"])
            project_raw: dict[str, Any] = {}
            if vector.get("net_class_assignments") is not None:
                project_raw["net_settings"] = {
                    "netclass_assignments": vector["net_class_assignments"]
                }
            if vector.get("text_variables") is not None:
                project_raw["text_variables"] = vector["text_variables"]
            if project_raw:
                pcb.project = KiCadProject.from_json_dict(project_raw)
            mode = vector.get("oracle_mode")
            assert mode in (None, "without_shapely")
            oracle = _without_shapely() if mode == "without_shapely" else contextlib.nullcontext()
            with oracle:
                actual = pcb_to_ir(
                    pcb,
                    source_path=vector["source_path"],
                    document_id=vector["document_id"],
                ).to_dict()
            decode = decode_board_plot_document_a0
        else:
            actual = schematic_expected_for(vector)
            decode = decode_schematic_plot_document_a0

        assert actual == vector["expected"], vector["id"]
        canonical = vector["expected"]
        decoded = decode(json.dumps(canonical).encode("utf-8"))
        assert json.loads(msgspec.json.encode(decoded)) == canonical


def _assert_corpus_python_semantics(producer: str) -> None:
    case, path = _stable_case(producer)
    if producer == "footprint":
        footprint = KiCadFootprint.from_file(path)
        document = footprint_to_ir(
            footprint, source_path=path.name, document_id=footprint.name
        ).to_dict()
        decode = decode_footprint_plot_document_a0
    elif producer == "symbol":
        library = KiCadSymbolLib.from_file(path)
        symbol_name = str(case["symbol_name"])
        symbol = library.get_symbol(symbol_name)
        assert symbol is not None
        document = _contract_symbol(
            lib_symbol_to_ir(
                symbol,
                unit=1,
                style=0,
                source_path=path.name,
                document_id=symbol_name,
            ).to_dict()
        )
        decode = decode_symbol_plot_document_a0
    elif producer == "board":
        document = pcb_to_ir(
            KiCadPcb.from_file(path), source_path=path.name, document_id=path.stem
        ).to_dict()
        decode = decode_board_plot_document_a0
    else:
        schematic = KiCadSchematic.from_file(path)
        document = schematic_to_ir(
            schematic,
            source_path=path.name,
            document_id=schematic.uuid or path.stem,
            sheet_name=path.stem,
        ).to_dict()
        decode = decode_schematic_plot_document_a0

    assert document["total_operations"] == _CORPUS_SELECTORS[producer][-1]
    assert document["records"]
    decode(json.dumps(document).encode("utf-8"))


def test_phase5_vectors_and_manifest_inputs_match_python_authority() -> None:
    _require_archive_and_fonts()
    for producer in _selected_producers():
        _assert_vector_authority(producer)
        _assert_corpus_python_semantics(producer)


def _freeze_path(relative_path: str) -> Path:
    path = (PACKAGE_ROOT / relative_path).resolve()
    path.relative_to(PACKAGE_ROOT.resolve())
    return path


def _union_projection(schema: dict[str, Any], definition_name: str) -> list[dict[str, Any]]:
    definitions = schema["$defs"]
    arms = definitions[definition_name]["anyOf"]
    projection = []
    for arm in arms:
        assert set(arm) == {"$ref"}
        reference = arm["$ref"]
        assert reference.startswith("#/$defs/")
        definition = definitions[reference.removeprefix("#/$defs/")]
        kind = definition["properties"]["kind"]
        if "const" in kind:
            kinds = [kind["const"]]
        else:
            kind_ref = kind["$ref"]
            assert kind_ref.startswith("#/$defs/")
            kinds = definitions[kind_ref.removeprefix("#/$defs/")]["enum"]
        projection.append({"ref": reference, "kinds": kinds})
    return projection


def _assert_phase5_freeze() -> None:
    manifest = json.loads(_PHASE5_FREEZE.read_text(encoding="utf-8"))
    assert manifest["schema"] == "kicad_monkey.plotter_ir_phase5_freeze.v1"
    assert manifest["contract_version"] == "a0"
    artifacts = manifest["artifacts"]
    paths = [entry["path"] for entry in artifacts]
    assert len(paths) == len(set(paths)) == 12
    assert {Path(path).name for path in paths} == _PHASE5_SCHEMA_NAMES
    for entry in artifacts:
        path = _freeze_path(entry["path"])
        assert _canonical_json_sha256(path) == entry["canonical_sha256"], path
    union_keys = {
        (Path(entry["path"]).name, entry["definition"])
        for entry in manifest["unions"]
    }
    assert union_keys == set(_PHASE5_UNION_ARM_COUNTS)
    for entry in manifest["unions"]:
        path = _freeze_path(entry["path"])
        key = (path.name, entry["definition"])
        assert entry["expected_arm_count"] == _PHASE5_UNION_ARM_COUNTS[key]
        schema = json.loads(path.read_text(encoding="utf-8"))
        projection = _union_projection(schema, entry["definition"])
        assert len(projection) == entry["expected_arm_count"]
        assert projection == entry["arms"]
        for mirror in entry.get("mirror_paths", []):
            mirror_schema = json.loads(_freeze_path(mirror).read_text(encoding="utf-8"))
            assert _union_projection(mirror_schema, entry["definition"]) == entry["arms"]


def test_phase5_contract_freeze_and_codegen_are_current() -> None:
    _require_archive()
    cargo = shutil.which("cargo")
    npm = shutil.which("npm")
    assert cargo is not None, "Cargo is required by mandatory L3_022"
    assert npm is not None, "npm is required by mandatory L3_022 contract checks"
    assert typespec_executable(PACKAGE_ROOT).is_file(), (
        "TypeSpec dependencies are required by L3_022; run `npm ci`"
    )
    _assert_phase5_freeze()
    for script in (
        "scripts/generate_board_plotter_vectors.py",
        "scripts/generate_schematic_plotter_vectors.py",
        "scripts/generate_stroke_font_widths.py",
    ):
        _run([sys.executable, script, "--check"], timeout=10 * 60)
    _run([npm, "run", "generate:contracts"], timeout=10 * 60)
    _assert_phase5_freeze()
    _run([npm, "run", "check:typespec"], timeout=10 * 60)
    _run([npm, "run", "check:python-generation"], timeout=10 * 60)
    _run([npm, "run", "check:typescript-generation"], timeout=10 * 60)
    _run(
        [
            cargo,
            "run",
            "--locked",
            "--jobs",
            "4",
            "--package",
            "kicad-monkey-codegen",
            "--",
            "--check",
        ],
        timeout=20 * 60,
    )


def _producer_corpus_paths(producer: str) -> list[Path]:
    domain, suffix, *_rest = _CORPUS_SELECTORS[producer]
    root = get_kicad_corpus_root()
    paths: dict[str, Path] = {}
    for case in iter_kicad_corpus_cases(domain=domain, status="active", required=True):
        candidates = [case.get("board_file"), case.get("input_file")]
        for value in candidates:
            if not value:
                continue
            path = root / str(value)
            if path.suffix.lower() != suffix:
                continue
            assert path.is_file(), f"manifest {producer} input is absent: {path}"
            assert "input" in path.relative_to(root).parts, path
            paths[path.relative_to(root).as_posix()] = path
            break
    ordered = [paths[key] for key in sorted(paths)]
    assert ordered, f"manifest has no active {producer} {suffix} inputs"
    return ordered


def _run(
    command: list[str],
    *,
    timeout: int,
    input_text: str | None = None,
    extra_env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    environment = os.environ.copy()
    environment["CARGO_BUILD_JOBS"] = "4"
    environment["RUST_TEST_THREADS"] = "2"
    if extra_env:
        environment.update(extra_env)
    completed = subprocess.run(
        command,
        cwd=PACKAGE_ROOT,
        input=input_text,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=timeout,
        check=False,
        env=environment,
    )
    assert completed.returncode == 0, (
        f"command failed: {' '.join(command)}\n"
        f"stdout tail:\n{completed.stdout[-4000:]}\n"
        f"stderr tail:\n{completed.stderr[-4000:]}"
    )
    return completed


def _selected_native_targets(
    targets_by_producer: dict[str, tuple[str, ...]],
) -> tuple[str, ...]:
    return tuple(
        dict.fromkeys(
            target
            for producer in _selected_producers()
            for target in targets_by_producer[producer]
        )
    )


def _run_cargo_test_targets(
    cargo: str,
    package: str,
    targets: tuple[str, ...],
) -> None:
    assert targets
    command = [cargo, "test", "--locked", "--jobs", "4", "--package", package]
    for target in targets:
        command.extend(("--test", target))
    _run(command, timeout=30 * 60)


def test_complete_native_phase5_semantic_and_resource_suites() -> None:
    _require_archive_and_fonts()
    cargo = shutil.which("cargo")
    assert cargo is not None, "Cargo is required by mandatory L3_022"
    _run_cargo_test_targets(
        cargo,
        "kicad-monkey-core",
        _selected_native_targets(_NATIVE_CORE_TARGETS),
    )
    _run_cargo_test_targets(
        cargo,
        "kicad-monkey-contracts",
        _selected_native_targets(_NATIVE_CONTRACT_TARGETS),
    )


def test_cargo_quality_and_real_wasm_are_clean() -> None:
    _require_archive_and_fonts()
    cargo = shutil.which("cargo")
    runner = shutil.which("wasm-bindgen-test-runner")
    assert cargo is not None, "Cargo is required by mandatory L3_022"
    assert runner is not None, (
        "wasm-bindgen-test-runner is required by L3_022; install the "
        "lock-compatible wasm-bindgen CLI"
    )
    commands = (
        [cargo, "fmt", "--all", "--", "--check"],
        [cargo, "check", "--workspace", "--all-targets", "--locked", "--jobs", "4"],
        [
            cargo,
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--locked",
            "--jobs",
            "4",
            "--",
            "-D",
            "warnings",
        ],
        [cargo, "test", "--workspace", "--locked", "--jobs", "4"],
        [cargo, "test", "--doc", "--workspace", "--locked", "--jobs", "4"],
    )
    for command in commands:
        _run(command, timeout=30 * 60)
    _run(
        [cargo, "doc", "--workspace", "--no-deps", "--locked", "--jobs", "4"],
        timeout=30 * 60,
        extra_env={"RUSTDOCFLAGS": "-D warnings"},
    )
    _run(
        [
            cargo,
            "test",
            "--package",
            "kicad-monkey-wasm",
            "--target",
            "wasm32-unknown-unknown",
            "--locked",
            "--jobs",
            "4",
        ],
        timeout=30 * 60,
    )
    for feature in ("sexpr", "footprint", "symbol", "board"):
        _run(
            [
                cargo,
                "check",
                "--package",
                "kicad-monkey-wasm",
                "--target",
                "wasm32-unknown-unknown",
                "--no-default-features",
                "--features",
                feature,
                "--locked",
                "--jobs",
                "4",
            ],
            timeout=30 * 60,
        )


def test_rust_sexpr_gate_accepts_deterministic_manifest_partition() -> None:
    _require_archive_and_fonts()
    cargo = shutil.which("cargo")
    assert cargo is not None, "Cargo is required by mandatory L3_022"
    index, count = _partition()
    paths = sorted(
        {path for producer in _selected_producers() for path in _producer_corpus_paths(producer)},
        key=lambda path: path.relative_to(get_kicad_corpus_root()).as_posix(),
    )
    selected = paths[index::count]
    assert selected, (
        f"Phase 5 corpus partition {index}/{count} is empty for {len(paths)} inputs"
    )
    assert all("\n" not in str(path) and "\r" not in str(path) for path in selected)
    completed = _run(
        [
            cargo,
            "run",
            "--release",
            "--locked",
            "--quiet",
            "--jobs",
            "4",
            "--package",
            "kicad-monkey-core",
            "--example",
            "sexpr_corpus_gate",
        ],
        timeout=20 * 60,
        input_text="".join(f"{path}\n" for path in selected),
    )
    records = [json.loads(line) for line in completed.stdout.splitlines() if line]
    assert len(records) == len(selected)
    assert all(record.get("schema") == "kicad_monkey.sexpr_corpus_record.a0" for record in records)
    assert all(record.get("phase") in _VALID_CORPUS_PHASES for record in records)
    failures = [record for record in records if record.get("phase") != "ok"]
    assert not failures, failures[:20]
    expected = {os.path.normcase(str(path.resolve())) for path in selected}
    actual = {os.path.normcase(str(Path(record["path"]).resolve())) for record in records}
    assert actual == expected
    suffixes = Counter(path.suffix.lower() for path in selected)
    assert sum(suffixes.values()) == len(selected)


def _cli_run(cli: Path, arguments: list[str], *, timeout: int = 240) -> subprocess.CompletedProcess[str]:
    environment = os.environ.copy()
    environment.update(kicad_cli_subprocess_env(cli) or {})
    return subprocess.run(
        [str(cli), *arguments],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=timeout,
        check=False,
        env=environment,
    )


def test_live_kicad_accepts_pcb_and_schematic_svg_plots(tmp_path: Path) -> None:
    _require_archive_and_fonts()
    pcb_cli = resolve_kicad_cli(required_capability="pcb_svg")
    schematic_cli = resolve_kicad_cli(required_capability="schematic_svg")
    assert pcb_cli is not None, "PCB SVG-capable kicad-cli is required by L3_022"
    assert schematic_cli is not None, "schematic SVG-capable kicad-cli is required by L3_022"
    _case, board = _stable_case("board")
    _case, schematic = _stable_case("schematic")

    board_svg = tmp_path / "board.svg"
    pcb_result = _cli_run(
        pcb_cli,
        [
            "pcb",
            "export",
            "svg",
            "--black-and-white",
            "--layers",
            "F.Cu",
            "--mode-single",
            "--page-size-mode",
            "2",
            "--exclude-drawing-sheet",
            "--output",
            str(board_svg),
            str(board),
        ],
    )
    assert pcb_result.returncode == 0, pcb_result.stdout + pcb_result.stderr
    assert board_svg.is_file() and board_svg.stat().st_size > 100
    assert "<svg" in board_svg.read_text(encoding="utf-8", errors="replace")

    schematic_dir = tmp_path / "schematic"
    schematic_dir.mkdir()
    schematic_result = _cli_run(
        schematic_cli,
        [
            "sch",
            "export",
            "svg",
            "--exclude-drawing-sheet",
            "--output",
            str(schematic_dir),
            str(schematic),
        ],
    )
    assert schematic_result.returncode == 0, schematic_result.stdout + schematic_result.stderr
    schematic_svgs = sorted(schematic_dir.glob("*.svg"))
    assert schematic_svgs and all(path.stat().st_size > 100 for path in schematic_svgs)
    assert all(
        "<svg" in path.read_text(encoding="utf-8", errors="replace")
        for path in schematic_svgs
    )


def _write_cache_board(path: Path) -> None:
    path.write_text(
        """(kicad_pcb
  (version 20241229)
  (generator "pcbnew")
  (generator_version "9.0")
  (general (thickness 1.6) (legacy_teardrops no))
  (paper "A4")
  (layers
    (0 "F.Cu" signal)
    (2 "B.Cu" signal)
    (5 "F.SilkS" user "F.Silkscreen")
    (7 "B.SilkS" user "B.Silkscreen")
    (1 "F.Mask" user)
    (3 "B.Mask" user)
    (25 "Edge.Cuts" user))
  (setup (pad_to_mask_clearance 0))
  (gr_text "TE"
    (at 10 10 0)
    (layer "F.SilkS")
    (uuid "11111111-1111-1111-1111-111111111111")
    (effects
      (font (face "Arial") (size 2 2) (thickness 0.2))
      (justify left top)))
)
""",
        encoding="utf-8",
    )


def test_live_kicad_save_cache_matches_native_board_cache(tmp_path: Path) -> None:
    _require_archive_and_fonts()
    cli = resolve_kicad_cli(required_capability="pcb_svg")
    assert cli is not None, "PCB save-capable kicad-cli is required by L3_022"
    cargo = shutil.which("cargo")
    assert cargo is not None, "Cargo is required by mandatory L3_022"
    source = tmp_path / "phase5_cache.kicad_pcb"
    _write_cache_board(source)
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=cli,
        source_pcb=source,
        work_dir=tmp_path / "oracle",
        timeout=240,
    )
    assert len(oracle.entries) == 1

    font_bytes = _ARIAL.read_bytes()
    request = tmp_path / "font.json"
    request.write_text(
        json.dumps(
            {
                "face": "Arial",
                "bold": False,
                "italic": False,
                "shaping": {
                    "font_id": "windows_arial_regular",
                    "font_sha256": hashlib.sha256(font_bytes).hexdigest(),
                    "face_index": 0,
                    "variations": [],
                    "text": "",
                    "text_index_unit": "utf8_byte_offset",
                    "scale_x": 2048,
                    "scale_y": 2048,
                    "direction": "left_to_right",
                    "script": "Latn",
                    "language": "en",
                    "features": [],
                    "buffer_properties": {
                        "cluster_level": "monotone_graphemes",
                        "beginning_of_text": True,
                        "end_of_text": True,
                        "default_ignorables": "normal",
                        "do_not_insert_dotted_circle": False,
                        "produce_unsafe_to_concat": False,
                        "produce_safe_to_insert_tatweel": False,
                    },
                },
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    completed = _run(
        [
            cargo,
            "run",
            "--release",
            "--locked",
            "--quiet",
            "--jobs",
            "4",
            "--package",
            "kicad-monkey-core",
            "--example",
            "board_plot_text_cache_gate",
            "--",
            str(_ARIAL),
            str(request),
            str(source),
        ],
        timeout=20 * 60,
    )
    native = RenderCache.from_sexp(parse_sexp(f"(holder {completed.stdout})"))
    assert native is not None
    comparison = compare_render_caches(oracle.entries[0].cache, native, tolerance=0.002)
    assert comparison.matched, comparison
