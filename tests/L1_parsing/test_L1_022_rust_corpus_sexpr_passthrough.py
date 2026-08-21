"""Rack-owned corpus pass-through gate for the Rust S-expression core."""

from __future__ import annotations

from collections import Counter
import json
import os
from pathlib import Path
import shutil
import subprocess
import time
from typing import Any

from kicad_monkey.testing.corpus import (
    KICAD_SEXPR_FILE_SUFFIXES,
    iter_kicad_sexpr_files,
)


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
EVIDENCE_PATH = (
    PACKAGE_ROOT
    / "tests"
    / "rack_results"
    / "evidence"
    / "rust_sexpr_l1_corpus.json"
)
_REAL_WORLD_PATH_MARKERS = {"projects", "real_world"}
_VALID_PHASES = {"read", "lex", "tree", "build", "reparse", "compare", "ok"}


def _normalized_path(value: str | Path) -> str:
    return os.path.normcase(os.path.abspath(value))


def _run_rust_corpus_gate(paths: list[Path]) -> tuple[list[dict[str, Any]], float]:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the promoted Rust corpus lane"
    assert all("\n" not in str(path) and "\r" not in str(path) for path in paths)

    started = time.perf_counter()
    completed = subprocess.run(
        [
            cargo,
            "run",
            "--release",
            "--locked",
            "--quiet",
            "--package",
            "kicad-monkey-core",
            "--example",
            "sexpr_corpus_gate",
        ],
        cwd=PACKAGE_ROOT,
        input="".join(f"{path}\n" for path in paths),
        capture_output=True,
        text=True,
        timeout=30 * 60,
        check=False,
    )
    wall_seconds = time.perf_counter() - started

    assert completed.returncode == 0, (
        "Rust corpus runner failed.\n"
        f"stdout tail:\n{completed.stdout[-4000:]}\n"
        f"stderr tail:\n{completed.stderr[-4000:]}"
    )
    try:
        records = [json.loads(line) for line in completed.stdout.splitlines() if line]
    except json.JSONDecodeError as exc:
        raise AssertionError(
            "Rust corpus runner emitted invalid JSON Lines.\n"
            f"stdout tail:\n{completed.stdout[-4000:]}\n"
            f"stderr tail:\n{completed.stderr[-4000:]}"
        ) from exc
    return records, wall_seconds


def _write_evidence(
    records: list[dict[str, Any]],
    by_suffix: Counter[str],
    wall_seconds: float,
) -> None:
    input_bytes = sum(int(record["input_bytes"]) for record in records)
    output_bytes = sum(int(record["output_bytes"]) for record in records)
    largest = max(records, key=lambda record: int(record["input_bytes"]))
    evidence = {
        "schema": "kicad_monkey.sexpr_corpus_evidence.a0",
        "runner": "src/rs/kicad-monkey-core/examples/sexpr_corpus_gate.rs",
        "files": len(records),
        "by_suffix": dict(sorted(by_suffix.items())),
        "input_bytes": input_bytes,
        "output_bytes": output_bytes,
        "wall_seconds": wall_seconds,
        "input_mib_per_second": (
            input_bytes / (1024 * 1024) / wall_seconds if wall_seconds else None
        ),
        "largest_file": {
            "path": largest["path"],
            "input_bytes": largest["input_bytes"],
            "elapsed_ns": largest["elapsed_ns"],
        },
        "phase_counts": dict(Counter(str(record["phase"]) for record in records)),
    }
    EVIDENCE_PATH.parent.mkdir(parents=True, exist_ok=True)
    EVIDENCE_PATH.write_text(
        json.dumps(evidence, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )


def test_rust_parser_round_trips_the_promoted_sexpr_corpus() -> None:
    """Require parse/build/reparse/equality and stable second output per file."""
    paths = list(iter_kicad_sexpr_files())
    assert paths, "No KiCad S-expression files discovered under corpus root"

    records, wall_seconds = _run_rust_corpus_gate(paths)
    assert len(records) == len(paths), (
        f"Rust returned {len(records)} records for {len(paths)} corpus files"
    )
    assert all(record.get("schema") == "kicad_monkey.sexpr_corpus_record.a0" for record in records)
    assert all(record.get("phase") in _VALID_PHASES for record in records)

    expected_paths = {_normalized_path(path) for path in paths}
    actual_paths = {_normalized_path(str(record["path"])) for record in records}
    assert actual_paths == expected_paths, "Rust corpus result paths differ from Rack discovery"

    by_suffix = Counter(path.suffix.lower() for path in paths)
    missing_suffixes = [suffix for suffix in KICAD_SEXPR_FILE_SUFFIXES if not by_suffix[suffix]]
    assert not missing_suffixes, f"Corpus walk found no files for suffixes: {missing_suffixes}"
    assert any(
        _REAL_WORLD_PATH_MARKERS & {part.lower() for part in path.parts}
        for path in paths
    ), "No real-world projects contributed to the Rust parser gate"

    _write_evidence(records, by_suffix, wall_seconds)

    failures = [record for record in records if record["phase"] != "ok"]
    if failures:
        lines = [
            f"  [{record['phase']}] {record['path']}: {str(record.get('error'))[:240]}"
            for record in failures[:50]
        ]
        if len(failures) > 50:
            lines.append(f"  ... ({len(failures) - 50} more)")
        phase_counts = Counter(str(record["phase"]) for record in records)
        raise AssertionError(
            f"Rust parser-only pass-through failed for {len(failures)} of "
            f"{len(records)} files. Phase breakdown: {dict(phase_counts)}.\n"
            + "\n".join(lines)
        )
