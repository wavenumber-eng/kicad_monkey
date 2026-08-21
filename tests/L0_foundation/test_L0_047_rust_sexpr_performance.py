"""Reproducible Python/Rust L0 parser measurements orchestrated by Rack."""

from __future__ import annotations

import json
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


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
ITEMS = 20_000
ROUNDS = 3


def _fixture() -> str:
    return "(kicad_pcb\n" + "".join(
        f'  (footprint "Bench:R_0805" (property "Reference" "R{index}") '
        '(at 1.25 2.5 90) (pad "1" smd rect (at 0 0)))\n'
        for index in range(ITEMS)
    ) + ")\n"


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
    completed = subprocess.run(
        [
            cargo,
            "run",
            "--release",
            "--package",
            "kicad-monkey-core",
            "--example",
            "sexpr_l0_benchmark",
            "--locked",
        ],
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=180,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
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

    assert rust["schema"] == "kicad_monkey.sexpr_benchmark.a0"
    assert rust["input_bytes"] == len(source.encode("utf-8"))
    assert rust["output_bytes"] == len(built.encode("utf-8"))
    assert rust["selected_forms"] == ITEMS
    assert all(rust[name] > 0 for name in ("scan_mib_s", "parse_mib_s", "build_mib_s"))

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
    }
    output = PACKAGE_ROOT / "tests" / "rack_results" / "evidence"
    output.mkdir(parents=True, exist_ok=True)
    (output / "rust_sexpr_l0_benchmark.json").write_text(
        json.dumps(evidence, indent=2) + "\n",
        encoding="utf-8",
    )
