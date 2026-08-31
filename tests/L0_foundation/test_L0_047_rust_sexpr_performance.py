"""Reproducible Python/Rust L0 parser measurements orchestrated by Rack."""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess
import time
from typing import Any, Callable

import pytest

from kicad_monkey.kicad_sexpr import (
    SexpSelector,
    build_sexp,
    iter_sexp_form_spans,
    parse_sexp,
)
from support_scripts.advisory import advisory_benchmarks_enabled
from support_scripts.performance_provenance import collect_performance_provenance


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
ITEMS = 20_000
ROUNDS = 3


def _run(command: list[str], *, timeout: int = 300) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    return completed


def _example_executable(
    cargo: str,
    name: str,
    *,
    features: tuple[str, ...] = (),
) -> Path:
    command = [
        cargo,
        "build",
        "--release",
        "--locked",
        "--package",
        "kicad-monkey-core",
        "--example",
        name,
    ]
    if features:
        command.extend(["--features", ",".join(features)])
    _run(command, timeout=600)
    metadata = json.loads(
        _run([cargo, "metadata", "--no-deps", "--format-version", "1"]).stdout
    )
    suffix = ".exe" if os.name == "nt" else ""
    executable = (
        Path(metadata["target_directory"]) / "release" / "examples" / f"{name}{suffix}"
    )
    assert executable.is_file(), f"missing benchmark executable: {executable}"
    return executable


def _provenance(executables: dict[str, Path]) -> dict[str, Any]:
    archive = Path(
        os.environ.get("KM_CORPUS", PACKAGE_ROOT / "tests" / "corpus" / "kicad.zip")
    ).resolve()
    return collect_performance_provenance(
        package_root=PACKAGE_ROOT,
        executables=executables,
        feature_sets={"allocation": ["measurement"], "timing": []},
        archive=archive,
    )


def _fixture() -> str:
    return (
        "(kicad_pcb\n"
        + "".join(
            f'  (footprint "Bench:R_0805" (property "Reference" "R{index}") '
            '(at 1.25 2.5 90) (pad "1" smd rect (at 0 0)))\n'
            for index in range(ITEMS)
        )
        + ")\n"
    )


def _best(operation: Callable[[], Any]) -> float:
    best = float("inf")
    for _ in range(ROUNDS):
        started = time.perf_counter()
        operation()
        best = min(best, time.perf_counter() - started)
    return best


def test_release_rust_measurement_uses_the_same_frozen_python_workload() -> None:
    if not advisory_benchmarks_enabled():
        pytest.skip("advisory benchmark; run Rack with --lane strict")
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for Rust performance evidence"
    executable = _example_executable(cargo, "sexpr_l0_benchmark")
    completed = _run([str(executable)])
    rust = json.loads(completed.stdout.strip())

    source = _fixture()
    selector = SexpSelector(heads={"footprint"})
    tree = parse_sexp(source)
    built = build_sexp(tree)
    python = {
        "scan_seconds": _best(lambda: list(iter_sexp_form_spans(source, selector))),
        "parse_seconds": _best(lambda: parse_sexp(source)),
        "build_seconds": _best(lambda: build_sexp(tree)),
    }

    assert rust["schema"] == "kicad_monkey.sexpr_benchmark.a1"
    assert rust["input_bytes"] == len(source.encode("utf-8"))
    assert rust["output_bytes"] == len(built.encode("utf-8"))
    assert rust["selected_forms"] == ITEMS
    assert rust["sparse_selected_forms"] * 1_000 < rust["sparse_visited_forms"]
    assert rust["token_count"] > 0
    assert rust["rounds"] == 5
    assert all(
        len(rust[name]) == rust["rounds"]
        for name in (
            "lex_drain_raw_seconds",
            "lex_collect_raw_seconds",
            "scan_raw_seconds",
            "parse_raw_seconds",
            "build_raw_seconds",
            "sparse_memory_raw_seconds",
            "sparse_stream_raw_seconds",
        )
    )
    assert all(
        rust[name] > 0
        for name in (
            "lex_drain_mib_s",
            "lex_collect_mib_s",
            "scan_mib_s",
            "parse_mib_s",
            "build_mib_s",
            "sparse_memory_mib_s",
            "sparse_stream_mib_s",
        )
    )

    evidence = {
        "schema": "kicad_monkey.sexpr_cross_language_benchmark.a0",
        "workload": rust["fixture"],
        "rust": rust,
        "python": python,
        "speedup": {
            "scan": python["scan_seconds"] / rust["scan_seconds"],
            "parse": python["parse_seconds"] / rust["parse_seconds"],
            "build": python["build_seconds"] / rust["build_seconds"],
        },
        "threshold_status": "pending_corpus_and_platform_ratification",
        "provenance": _provenance({"sexpr_l0_benchmark": executable}),
    }
    output = PACKAGE_ROOT / "tests" / "rack_results" / "evidence"
    output.mkdir(parents=True, exist_ok=True)
    (output / "rust_sexpr_l0_benchmark.json").write_text(
        json.dumps(evidence, indent=2) + "\n",
        encoding="utf-8",
    )


def test_sparse_projection_allocation_probe_is_controlled_and_scanner_specific() -> (
    None
):
    if not advisory_benchmarks_enabled():
        pytest.skip("advisory benchmark; run Rack with --lane strict")
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for Rust performance evidence"
    executable = _example_executable(
        cargo,
        "sexpr_projection_allocation_benchmark",
        features=("measurement",),
    )
    measurements: dict[str, dict[str, Any]] = {}
    for scanner in ("memory", "stream"):
        completed = _run([str(executable), scanner])
        measurement = json.loads(completed.stdout.strip())
        assert measurement["schema"] == (
            "kicad_monkey.sexpr_projection_allocation_benchmark.a0"
        )
        assert measurement["scanner"] == scanner
        assert measurement["selected_forms"] > 0
        assert measurement["selected_forms"] * 1_000 < measurement["visited_forms"]
        assert measurement["control"]["allocation_calls"] == 1
        assert measurement["control"]["reallocation_calls"] == 0
        assert measurement["control"]["allocated_bytes"] >= 4_096
        assert measurement["allocation"]["allocation_calls"] > 0
        measurements[scanner] = measurement

    output = PACKAGE_ROOT / "tests" / "rack_results" / "evidence"
    output.mkdir(parents=True, exist_ok=True)
    evidence = {
        "schema": "kicad_monkey.sexpr_sparse_projection_allocations.a0",
        "provenance": _provenance(
            {"sexpr_projection_allocation_benchmark": executable}
        ),
        "measurements": measurements,
    }
    (output / "rust_sexpr_sparse_projection_allocations.json").write_text(
        json.dumps(evidence, indent=2) + "\n",
        encoding="utf-8",
    )
