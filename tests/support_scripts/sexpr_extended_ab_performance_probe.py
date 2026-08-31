"""Paired select-all, corpus timing, and peak-RSS evidence for exact Rust revisions."""

from __future__ import annotations

import argparse
import json
import math
import os
from pathlib import Path
import statistics
import subprocess
import tomllib
from typing import Any

from performance_provenance import collect_performance_provenance, sha256_file
from process_peak_memory import run_with_peak_rss


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline-workspace", required=True, type=Path)
    parser.add_argument("--candidate-workspace", required=True, type=Path)
    parser.add_argument("--baseline-target", required=True, type=Path)
    parser.add_argument("--candidate-target", required=True, type=Path)
    parser.add_argument("--baseline-sha", required=True)
    parser.add_argument("--candidate-sha", required=True)
    parser.add_argument("--corpus-root", required=True, type=Path)
    parser.add_argument("--archive", required=True, type=Path)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--rounds", type=int, default=13)
    parser.add_argument("--case-id")
    parser.add_argument("--scanner", choices=("memory", "stream"))
    parser.add_argument("--select-only", action="store_true")
    parser.add_argument("--output", required=True, type=Path)
    return parser.parse_args()


def _command_output(command: list[str], root: Path) -> str:
    return subprocess.run(
        command,
        cwd=root,
        capture_output=True,
        text=True,
        timeout=60,
        check=True,
    ).stdout.strip()


def _build(
    workspace: Path, target: Path
) -> tuple[dict[str, Path], list[dict[str, Any]]]:
    suffix = ".exe" if os.name == "nt" else ""
    examples = {
        "sexpr_corpus_benchmark": [],
        "sexpr_selection_sort_benchmark": ["--features", "measurement"],
    }
    executables: dict[str, Path] = {}
    records: list[dict[str, Any]] = []
    for name, extra in examples.items():
        command = [
            "cargo",
            "build",
            "--release",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--example",
            name,
            *extra,
        ]
        completed = subprocess.run(
            command,
            cwd=workspace,
            env={**os.environ, "CARGO_TARGET_DIR": str(target)},
            capture_output=True,
            text=True,
            timeout=900,
            check=False,
        )
        if completed.returncode != 0:
            raise RuntimeError(
                f"build failed in {workspace}: {' '.join(command)}\n{completed.stderr}"
            )
        executable = target / "release" / "examples" / f"{name}{suffix}"
        if not executable.is_file():
            raise FileNotFoundError(executable)
        executables[name] = executable
        records.append(
            {
                "command": command,
                "cwd": str(workspace),
                "cargo_target_dir": str(target),
                "executable": str(executable),
                "executable_sha256": sha256_file(executable),
            }
        )
    return executables, records


def _run_json(command: list[str], cwd: Path, timeout: float = 300) -> dict[str, Any]:
    completed = subprocess.run(
        command,
        cwd=cwd,
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(
            f"command failed: {' '.join(command)}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    return json.loads(completed.stdout)


def _median_sign_interval(values: list[float]) -> dict[str, float | int] | None:
    count = len(values)
    rank = 0
    coverage = 0.0
    for candidate_rank in range(1, (count + 1) // 2 + 1):
        tail = sum(math.comb(count, index) for index in range(candidate_rank))
        candidate_coverage = 1.0 - (2.0 * tail / (2**count))
        if candidate_coverage < 0.95:
            break
        rank = candidate_rank
        coverage = candidate_coverage
    if rank == 0:
        return None
    ordered = sorted(values)
    return {
        "coverage": coverage,
        "lower_ratio": ordered[rank - 1],
        "upper_ratio": ordered[count - rank],
        "lower_order_statistic": rank,
        "upper_order_statistic": count - rank + 1,
    }


def _summary(pairs: list[dict[str, Any]], metric: str) -> dict[str, Any]:
    ratios = [float(pair["baseline_over_candidate"][metric]) for pair in pairs]
    return {
        "paired_ratios": ratios,
        "min_ratio": min(ratios),
        "median_ratio": statistics.median(ratios),
        "max_ratio": max(ratios),
        "median_sign_interval": _median_sign_interval(ratios),
    }


def _select_all_pairs(
    executables: dict[str, Path],
    workspaces: dict[str, Path],
    path: Path,
    scanner: str,
    rounds: int,
) -> list[dict[str, Any]]:
    pairs: list[dict[str, Any]] = []
    for index in range(rounds):
        order = (
            ["baseline", "candidate"] if index % 2 == 0 else ["candidate", "baseline"]
        )
        payloads = {
            label: _run_json(
                [str(executables[label]), scanner, str(path)],
                workspaces[label],
            )
            for label in order
        }
        if any(
            payload["schema"] != "kicad_monkey.sexpr_selection_sort_benchmark.a0"
            or payload["scanner"] != scanner
            for payload in payloads.values()
        ):
            raise AssertionError("unexpected select-all payload")
        for payload in payloads.values():
            payload["total_ns"] = int(payload["scan_ns"]) + int(payload["sort_ns"])
        pairs.append(
            {
                "index": index,
                "order": order,
                "baseline": payloads["baseline"],
                "candidate": payloads["candidate"],
                "baseline_over_candidate": {
                    metric: payloads["baseline"][metric] / payloads["candidate"][metric]
                    for metric in ("scan_ns", "sort_ns", "total_ns")
                },
            }
        )
    return pairs


def _corpus_pairs(
    executables: dict[str, Path],
    workspaces: dict[str, Path],
    path: Path,
    operation: str,
    rounds: int,
) -> list[dict[str, Any]]:
    pairs: list[dict[str, Any]] = []
    for index in range(rounds):
        order = (
            ["baseline", "candidate"] if index % 2 == 0 else ["candidate", "baseline"]
        )
        payloads: dict[str, dict[str, Any]] = {}
        for label in order:
            completed, peak_rss = run_with_peak_rss(
                [str(executables[label]), operation, str(path)],
                cwd=workspaces[label],
                timeout=300,
            )
            if completed.returncode != 0:
                raise RuntimeError(
                    f"corpus benchmark failed: {completed.stdout}\n{completed.stderr}"
                )
            payload = json.loads(completed.stdout)
            if payload["schema"] != "kicad_monkey.sexpr_corpus_benchmark.a0":
                raise AssertionError("unexpected corpus payload")
            if peak_rss <= 0:
                raise AssertionError("peak RSS sampler returned no observation")
            payload["peak_rss_bytes"] = peak_rss
            payloads[label] = payload
        pairs.append(
            {
                "index": index,
                "order": order,
                "baseline": payloads["baseline"],
                "candidate": payloads["candidate"],
                "baseline_over_candidate": {
                    metric: payloads["baseline"][metric] / payloads["candidate"][metric]
                    for metric in ("total_operation_ns", "peak_rss_bytes")
                },
            }
        )
    return pairs


def main() -> None:
    args = _arguments()
    if args.rounds < 9:
        raise ValueError("at least nine paired rounds are required")
    workspaces = {
        "baseline": args.baseline_workspace.resolve(),
        "candidate": args.candidate_workspace.resolve(),
    }
    targets = {
        "baseline": args.baseline_target.resolve(),
        "candidate": args.candidate_target.resolve(),
    }
    archive = args.archive.resolve()
    manifest_path = args.manifest.resolve()
    manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest["archive_sha256"] != sha256_file(archive):
        raise AssertionError("corpus archive identity differs from manifest")

    executable_sets: dict[str, dict[str, Path]] = {}
    builds: dict[str, list[dict[str, Any]]] = {}
    provenance: dict[str, dict[str, Any]] = {}
    for label in ("baseline", "candidate"):
        executable_sets[label], builds[label] = _build(
            workspaces[label], targets[label]
        )
        provenance[label] = collect_performance_provenance(
            package_root=workspaces[label],
            executables=executable_sets[label],
            feature_sets={
                "sexpr_corpus_benchmark": [],
                "sexpr_selection_sort_benchmark": ["measurement"],
            },
            archive=archive,
        )
    expected_shas = {"baseline": args.baseline_sha, "candidate": args.candidate_sha}
    for label in ("baseline", "candidate"):
        if provenance[label]["git_sha"] != expected_shas[label]:
            raise AssertionError(f"{label} source SHA mismatch")
        if provenance[label]["git_status_porcelain"]:
            raise AssertionError(f"{label} source worktree is dirty")

    select_all: list[dict[str, Any]] = []
    corpus: list[dict[str, Any]] = []
    corpus_root = args.corpus_root.resolve()
    for case in manifest["cases"]:
        if args.case_id is not None and case["id"] != args.case_id:
            continue
        path = (corpus_root / case["path"]).resolve()
        path.relative_to(corpus_root)
        if path.stat().st_size != case["bytes"] or sha256_file(path) != case["sha256"]:
            raise AssertionError(f"corpus case drift: {case['id']}")
        scanners = (args.scanner,) if args.scanner is not None else ("memory", "stream")
        for scanner in scanners:
            print(f"select-all {case['id']} {scanner}", flush=True)
            pairs = _select_all_pairs(
                {
                    label: executable_sets[label]["sexpr_selection_sort_benchmark"]
                    for label in executable_sets
                },
                workspaces,
                path,
                scanner,
                args.rounds,
            )
            select_all.append(
                {
                    "case": case,
                    "scanner": scanner,
                    "pairs": pairs,
                    "summary": {
                        metric: _summary(pairs, metric)
                        for metric in ("scan_ns", "sort_ns", "total_ns")
                    },
                }
            )
        for operation in () if args.select_only else manifest["operations"]:
            print(f"corpus {case['id']} {operation}", flush=True)
            pairs = _corpus_pairs(
                {
                    label: executable_sets[label]["sexpr_corpus_benchmark"]
                    for label in executable_sets
                },
                workspaces,
                path,
                operation,
                args.rounds,
            )
            corpus.append(
                {
                    "case": case,
                    "operation": operation,
                    "pairs": pairs,
                    "summary": {
                        metric: _summary(pairs, metric)
                        for metric in ("total_operation_ns", "peak_rss_bytes")
                    },
                }
            )

    evidence = {
        "schema": "kicad_monkey.sexpr_extended_ab_performance.a0",
        "baseline_sha": args.baseline_sha,
        "candidate_sha": args.candidate_sha,
        "rounds": args.rounds,
        "manifest": {
            "path": str(manifest_path),
            "sha256": sha256_file(manifest_path),
            "archive_sha256": manifest["archive_sha256"],
        },
        "builds": builds,
        "provenance": provenance,
        "select_all": select_all,
        "corpus": corpus,
        "toolchain_check": {
            label: {
                "rustc": _command_output(
                    ["rustc", "--version", "--verbose"], workspaces[label]
                ),
                "cargo": _command_output(["cargo", "--version"], workspaces[label]),
            }
            for label in workspaces
        },
    }
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(f"Wrote {output}", flush=True)


if __name__ == "__main__":
    main()
