"""Exact-source L/P allocation evidence for sparse projection scanners."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
from typing import Any

from performance_provenance import collect_performance_provenance


def _arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    for label in ("baseline", "candidate"):
        parser.add_argument(f"--{label}-workspace", required=True, type=Path)
        parser.add_argument(f"--{label}-target", required=True, type=Path)
        parser.add_argument(f"--{label}-sha", required=True)
    parser.add_argument("--archive", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    return parser.parse_args()


def _build(workspace: Path, target: Path) -> tuple[Path, dict[str, Any]]:
    command = [
        "cargo",
        "build",
        "--release",
        "--locked",
        "--package",
        "kicad-monkey-core",
        "--example",
        "sexpr_projection_allocation_benchmark",
        "--features",
        "measurement",
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
            f"allocation build failed in {workspace}\n{completed.stderr}"
        )
    suffix = ".exe" if os.name == "nt" else ""
    executable = (
        target
        / "release"
        / "examples"
        / f"sexpr_projection_allocation_benchmark{suffix}"
    )
    if not executable.is_file():
        raise FileNotFoundError(executable)
    return executable, {
        "command": command,
        "cwd": str(workspace),
        "cargo_target_dir": str(target),
    }


def _measure(executable: Path, workspace: Path, scanner: str) -> dict[str, Any]:
    completed = subprocess.run(
        [str(executable), scanner],
        cwd=workspace,
        capture_output=True,
        text=True,
        timeout=180,
        check=False,
    )
    if completed.returncode != 0:
        raise RuntimeError(f"allocation probe failed: {completed.stderr}")
    payload = json.loads(completed.stdout)
    if payload["schema"] != "kicad_monkey.sexpr_projection_allocation_benchmark.a0":
        raise AssertionError("unexpected allocation payload")
    if payload["scanner"] != scanner or payload["control"]["allocation_calls"] != 1:
        raise AssertionError("allocation control or scanner identity failed")
    return payload


def main() -> None:
    args = _arguments()
    workspaces = {
        "baseline": args.baseline_workspace.resolve(),
        "candidate": args.candidate_workspace.resolve(),
    }
    targets = {
        "baseline": args.baseline_target.resolve(),
        "candidate": args.candidate_target.resolve(),
    }
    expected_shas = {
        "baseline": args.baseline_sha,
        "candidate": args.candidate_sha,
    }
    archive = args.archive.resolve()
    executables: dict[str, Path] = {}
    builds: dict[str, dict[str, Any]] = {}
    provenance: dict[str, dict[str, Any]] = {}
    measurements: dict[str, dict[str, dict[str, Any]]] = {}
    for label in ("baseline", "candidate"):
        executables[label], builds[label] = _build(workspaces[label], targets[label])
        provenance[label] = collect_performance_provenance(
            package_root=workspaces[label],
            executables={"sexpr_projection_allocation_benchmark": executables[label]},
            feature_sets={"sexpr_projection_allocation_benchmark": ["measurement"]},
            archive=archive,
        )
        if provenance[label]["git_sha"] != expected_shas[label]:
            raise AssertionError(f"{label} source SHA mismatch")
        if provenance[label]["git_status_porcelain"]:
            raise AssertionError(f"{label} source worktree is dirty")
        measurements[label] = {
            scanner: _measure(executables[label], workspaces[label], scanner)
            for scanner in ("memory", "stream")
        }

    reductions = {}
    for scanner in ("memory", "stream"):
        baseline = measurements["baseline"][scanner]
        candidate = measurements["candidate"][scanner]
        for key in ("fixture", "input_bytes", "visited_forms", "selected_forms"):
            if baseline[key] != candidate[key]:
                raise AssertionError(f"{scanner} fixture identity mismatch: {key}")
        reductions[scanner] = {
            metric: {
                "baseline": baseline["allocation"][metric],
                "candidate": candidate["allocation"][metric],
                "fraction_reduced": 1
                - candidate["allocation"][metric] / baseline["allocation"][metric],
            }
            for metric in ("allocation_calls", "allocated_bytes")
        }

    evidence = {
        "schema": "kicad_monkey.sexpr_projection_allocation_ab.a0",
        "baseline_sha": args.baseline_sha,
        "candidate_sha": args.candidate_sha,
        "builds": builds,
        "provenance": provenance,
        "measurements": measurements,
        "reductions": reductions,
    }
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(f"Wrote {output}")


if __name__ == "__main__":
    main()
