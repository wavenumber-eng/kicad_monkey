"""Alternating, provenance-rich A/B probe for the Rust S-expression benchmark."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import platform
import statistics
import subprocess
from typing import Any

from performance_provenance import sha256_file


METRICS = (
    "lex_drain_seconds",
    "lex_collect_seconds",
    "scan_seconds",
    "parse_seconds",
    "build_seconds",
    "sparse_memory_seconds",
    "sparse_stream_seconds",
)


def _run(executable: Path) -> dict[str, Any]:
    completed = subprocess.run(
        [str(executable)],
        capture_output=True,
        text=True,
        timeout=120,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"benchmark failed: {executable}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    payload = json.loads(completed.stdout)
    if payload["schema"] != "kicad_monkey.sexpr_benchmark.a1":
        raise RuntimeError(f"unexpected benchmark schema: {payload['schema']}")
    return payload


def _command_output(command: list[str], root: Path) -> str:
    return subprocess.run(
        command,
        cwd=root,
        capture_output=True,
        text=True,
        timeout=60,
        check=True,
    ).stdout.strip()


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-executable", required=True, type=Path)
    parser.add_argument("--candidate-executable", required=True, type=Path)
    parser.add_argument("--baseline-sha", required=True)
    parser.add_argument("--candidate-sha", required=True)
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--workspace", type=Path, default=Path.cwd())
    parser.add_argument("--output", required=True, type=Path)
    return parser.parse_args()


def main() -> None:
    args = _arguments()
    workspace = args.workspace.resolve()
    baseline_executable = args.baseline_executable.resolve()
    candidate_executable = args.candidate_executable.resolve()
    if args.rounds < 3:
        raise ValueError("at least three paired rounds are required")
    if not baseline_executable.is_file() or not candidate_executable.is_file():
        raise FileNotFoundError("both benchmark executables must exist")

    pairs: list[dict[str, Any]] = []
    fixture_identity: dict[str, Any] | None = None
    for index in range(args.rounds):
        order = (
            ("baseline", "candidate")
            if index % 2 == 0
            else (
                "candidate",
                "baseline",
            )
        )
        payloads: dict[str, dict[str, Any]] = {}
        for label in order:
            executable = (
                baseline_executable if label == "baseline" else candidate_executable
            )
            payloads[label] = _run(executable)
        identity = {
            key: payloads["baseline"][key]
            for key in (
                "fixture",
                "input_bytes",
                "token_count",
                "token_checksum",
                "selected_forms",
                "output_bytes",
                "sparse_input_bytes",
                "sparse_visited_forms",
                "sparse_selected_forms",
            )
        }
        if fixture_identity is None:
            fixture_identity = identity
        if identity != fixture_identity or any(
            payloads["candidate"][key] != value for key, value in identity.items()
        ):
            raise AssertionError("baseline and candidate fixture identities differ")
        ratios = {
            metric: payloads["baseline"][metric] / payloads["candidate"][metric]
            for metric in METRICS
        }
        pairs.append(
            {
                "index": index,
                "order": list(order),
                "baseline": payloads["baseline"],
                "candidate": payloads["candidate"],
                "baseline_over_candidate": ratios,
            }
        )

    summary = {}
    for metric in METRICS:
        values = [pair["baseline_over_candidate"][metric] for pair in pairs]
        summary[metric] = {
            "paired_ratios": values,
            "min_ratio": min(values),
            "median_ratio": statistics.median(values),
            "max_ratio": max(values),
        }
    evidence = {
        "schema": "kicad_monkey.sexpr_ab_performance.a0",
        "baseline_sha": args.baseline_sha,
        "candidate_sha": args.candidate_sha,
        "rounds": args.rounds,
        "profile": "release",
        "features": [],
        "fixture_identity": fixture_identity,
        "executables": {
            "baseline": {
                "path": str(baseline_executable),
                "sha256": sha256_file(baseline_executable),
            },
            "candidate": {
                "path": str(candidate_executable),
                "sha256": sha256_file(candidate_executable),
            },
        },
        "locks": {
            "cargo_sha256": sha256_file(workspace / "Cargo.lock"),
            "uv_sha256": sha256_file(workspace / "uv.lock"),
        },
        "host": {
            "platform": platform.platform(),
            "machine": platform.machine(),
            "processor": platform.processor(),
            "logical_cpus": os.cpu_count(),
        },
        "toolchain": {
            "rustc": _command_output(["rustc", "--version", "--verbose"], workspace),
            "cargo": _command_output(["cargo", "--version"], workspace),
        },
        "pairs": pairs,
        "summary": summary,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(evidence, indent=2, sort_keys=True) + "\n",
        encoding="utf-8",
    )
    print(f"Wrote {args.output.resolve()}")


if __name__ == "__main__":
    main()
