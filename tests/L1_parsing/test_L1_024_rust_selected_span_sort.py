"""Isolate select-all scan work from final source-order sorting."""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import statistics
import subprocess
import tomllib
from typing import Any

import pytest

from kicad_monkey.testing.corpus import get_kicad_corpus_root
from support_scripts.advisory import advisory_benchmarks_enabled
from support_scripts.performance_provenance import collect_performance_provenance


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
MANIFEST_PATH = PACKAGE_ROOT / "tests" / "performance" / "sexpr_corpus_cases.toml"
EVIDENCE_PATH = (
    PACKAGE_ROOT
    / "tests"
    / "rack_results"
    / "evidence"
    / "rust_sexpr_selected_span_sort.json"
)
ROUNDS = 5


def _run(command: list[str], *, timeout: int = 300) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
    )
    assert completed.returncode == 0, (
        f"Command failed: {' '.join(command)}\n"
        f"stdout:\n{completed.stdout}\n"
        f"stderr:\n{completed.stderr}"
    )
    return completed


def _benchmark_executable(cargo: str) -> Path:
    _run(
        [
            cargo,
            "build",
            "--release",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--example",
            "sexpr_selection_sort_benchmark",
            "--features",
            "measurement",
        ],
        timeout=600,
    )
    metadata = json.loads(
        _run([cargo, "metadata", "--no-deps", "--format-version", "1"]).stdout
    )
    suffix = ".exe" if os.name == "nt" else ""
    executable = (
        Path(metadata["target_directory"])
        / "release"
        / "examples"
        / f"sexpr_selection_sort_benchmark{suffix}"
    )
    assert executable.is_file(), f"missing release benchmark executable: {executable}"
    return executable


def _measure(executable: Path, scanner: str, path: Path) -> dict[str, Any]:
    runs = [
        json.loads(_run([str(executable), scanner, str(path)], timeout=180).stdout)
        for _ in range(ROUNDS)
    ]
    assert all(
        run["schema"] == "kicad_monkey.sexpr_selection_sort_benchmark.a0"
        and run["scanner"] == scanner
        and run["scan_ns"] > 0
        and run["sort_ns"] > 0
        and run["selected_forms"] > 0
        for run in runs
    )
    best = min(runs, key=lambda run: run["scan_ns"] + run["sort_ns"])
    return {
        "scanner": scanner,
        "selected_forms": best["selected_forms"],
        "best_scan_ns": best["scan_ns"],
        "best_sort_ns": best["sort_ns"],
        "best_sort_fraction": best["sort_fraction"],
        "median_sort_fraction": statistics.median(run["sort_fraction"] for run in runs),
        "runs": runs,
    }


def test_select_all_reports_scan_and_sort_cost_separately() -> None:
    if not advisory_benchmarks_enabled():
        pytest.skip("advisory benchmark; run Rack with --lane strict")
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for Rust selection measurements"
    manifest = tomllib.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    corpus_root = get_kicad_corpus_root().resolve()
    executable = _benchmark_executable(cargo)

    evidence_cases: list[dict[str, Any]] = []
    for case in manifest["cases"]:
        path = (corpus_root / case["path"]).resolve()
        path.relative_to(corpus_root)
        measurements = [
            _measure(executable, scanner, path) for scanner in ("memory", "stream")
        ]
        assert measurements[0]["selected_forms"] == measurements[1]["selected_forms"]
        evidence_cases.append(
            {
                "id": case["id"],
                "tier": case["tier"],
                "path": case["path"],
                "input_bytes": case["bytes"],
                "sha256": case["sha256"],
                "measurements": measurements,
            }
        )

    evidence = {
        "schema": "kicad_monkey.sexpr_selected_span_sort_evidence.a0",
        "status": "measurement",
        "rounds": ROUNDS,
        "archive_sha256": manifest["archive_sha256"],
        "provenance": collect_performance_provenance(
            package_root=PACKAGE_ROOT,
            executables={"sexpr_selection_sort_benchmark": executable},
            feature_sets={"sexpr_selection_sort_benchmark": ["measurement"]},
            archive=Path(
                os.environ.get(
                    "KM_CORPUS", PACKAGE_ROOT / "tests" / "corpus" / "kicad.zip"
                )
            ).resolve(),
        ),
        "cases": evidence_cases,
    }
    EVIDENCE_PATH.parent.mkdir(parents=True, exist_ok=True)
    EVIDENCE_PATH.write_text(
        json.dumps(evidence, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
