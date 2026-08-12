"""Named-corpus release performance and peak-memory evidence for Rust."""

from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import tomllib
from typing import Any

from kicad_monkey.testing.corpus import get_kicad_corpus_root
from support_scripts.process_peak_memory import run_with_peak_rss


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
MANIFEST_PATH = PACKAGE_ROOT / "tests" / "performance" / "sexpr_corpus_cases.toml"
EVIDENCE_PATH = (
    PACKAGE_ROOT
    / "tests"
    / "rack_results"
    / "evidence"
    / "rust_sexpr_corpus_performance_memory.json"
)
ROUNDS = 3
_COMMON_TIMING_FIELDS = (
    "read_ns",
    "parse_ns",
    "total_operation_ns",
)
_ROUNDTRIP_TIMING_FIELDS = (
    "build_ns",
    "reparse_ns",
    "compare_ns",
    "second_build_ns",
)


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
            "sexpr_corpus_benchmark",
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
        / f"sexpr_corpus_benchmark{suffix}"
    )
    assert executable.is_file(), f"missing release benchmark executable: {executable}"
    return executable


def _measure_operation(
    executable: Path,
    path: Path,
    operation: str,
) -> dict[str, Any]:
    runs: list[dict[str, Any]] = []
    peak_rss: list[int] = []
    for _ in range(ROUNDS):
        completed, run_peak_rss = run_with_peak_rss(
            [str(executable), operation, str(path)],
            cwd=PACKAGE_ROOT,
            timeout=180,
        )
        assert completed.returncode == 0, (
            f"benchmark failed for {path}\n"
            f"stdout:\n{completed.stdout}\n"
            f"stderr:\n{completed.stderr}"
        )
        assert run_peak_rss > 0, f"peak RSS sampler did not observe {path}"
        payload = json.loads(completed.stdout)
        assert payload["schema"] == "kicad_monkey.sexpr_corpus_benchmark.a0"
        assert payload["operation"] == operation
        assert all(int(payload[field]) > 0 for field in _COMMON_TIMING_FIELDS)
        for field in _ROUNDTRIP_TIMING_FIELDS:
            if operation == "roundtrip":
                assert int(payload[field]) > 0
            else:
                assert payload[field] is None
        runs.append(payload)
        peak_rss.append(run_peak_rss)

    best_run = min(runs, key=lambda run: int(run["total_operation_ns"]))
    input_bytes = int(best_run["input_bytes"])
    total_seconds = int(best_run["total_operation_ns"]) / 1_000_000_000
    return {
        "operation": operation,
        "input_bytes": input_bytes,
        "output_bytes": (
            int(best_run["output_bytes"])
            if best_run["output_bytes"] is not None
            else None
        ),
        "best": {
            field: int(best_run[field])
            for field in (*_COMMON_TIMING_FIELDS, *_ROUNDTRIP_TIMING_FIELDS)
            if best_run[field] is not None
        },
        "median_total_operation_ns": int(
            statistics.median(int(run["total_operation_ns"]) for run in runs)
        ),
        "peak_rss_bytes": max(peak_rss),
        "peak_rss_to_input_ratio": max(peak_rss) / input_bytes,
        "best_input_mib_per_second": (
            input_bytes / (1024 * 1024) / total_seconds
        ),
        "runs": [
            {
                **{
                    field: int(run[field])
                    for field in (*_COMMON_TIMING_FIELDS, *_ROUNDTRIP_TIMING_FIELDS)
                    if run[field] is not None
                },
                "peak_rss_bytes": run_peak,
            }
            for run, run_peak in zip(runs, peak_rss, strict=True)
        ],
    }


def test_named_corpus_release_performance_and_peak_memory() -> None:
    """Measure semantic round trips without ratifying one-machine thresholds."""
    cargo = shutil.which("cargo")
    rustc = shutil.which("rustc")
    assert cargo is not None, "cargo is required for Rust corpus measurements"
    assert rustc is not None, "rustc is required for Rust corpus measurements"

    manifest = tomllib.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    assert manifest["schema"] == "kicad_monkey.performance_corpus.a0"
    assert manifest["promotion"]["threshold_status"] == (
        "pending_cross_platform_ratification"
    )
    cases = manifest["cases"]
    assert [case["tier"] for case in cases] == ["small", "medium", "large"]
    assert len({case["id"] for case in cases}) == len(cases)

    corpus_root = get_kicad_corpus_root().resolve()
    executable = _benchmark_executable(cargo)
    measurements: list[dict[str, Any]] = []
    for case in cases:
        path = (corpus_root / case["path"]).resolve()
        path.relative_to(corpus_root)
        source = path.read_bytes()
        assert len(source) == case["bytes"], f"size drift for {case['id']}"
        assert hashlib.sha256(source).hexdigest() == case["sha256"], (
            f"content drift for {case['id']}"
        )
        operation_measurements = [
            _measure_operation(executable, path, operation)
            for operation in manifest["operations"]
        ]
        assert all(
            operation_measurement["input_bytes"] == case["bytes"]
            for operation_measurement in operation_measurements
        )
        measurements.append(
            {
                "id": case["id"],
                "tier": case["tier"],
                "path": case["path"],
                "sha256": case["sha256"],
                "operations": operation_measurements,
            }
        )

    evidence = {
        "schema": "kicad_monkey.sexpr_corpus_performance_memory.a0",
        "threshold_status": manifest["promotion"]["threshold_status"],
        "archive_sha256": manifest["archive_sha256"],
        "rounds": ROUNDS,
        "host": {
            "platform": platform.platform(),
            "machine": platform.machine(),
            "processor": platform.processor(),
            "logical_cpus": os.cpu_count(),
        },
        "toolchain": {
            "rustc": _run([rustc, "--version", "--verbose"]).stdout.strip(),
            "cargo": _run([cargo, "--version"]).stdout.strip(),
        },
        "cases": measurements,
    }
    EVIDENCE_PATH.parent.mkdir(parents=True, exist_ok=True)
    EVIDENCE_PATH.write_text(
        json.dumps(evidence, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
