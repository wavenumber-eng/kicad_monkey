"""Rack-owned source quality tool checks."""

from __future__ import annotations

import json
import re
import subprocess
import sys
from pathlib import Path


def _project_root() -> Path:
    """Find the repository root from this test file."""
    for parent in Path(__file__).resolve().parents:
        if (parent / "pyproject.toml").exists():
            return parent
    raise RuntimeError("Could not locate repository root")


PACKAGE_ROOT = _project_root()
RUFF_BASELINE_PATHS = (
    "src/py/kicad_monkey",
    "tests/L99_signoff",
    "tests/L0_foundation/test_L0_038_public_api_contract.py",
    "scripts/package_kicad_corpus.py",
)
QUALITY_STATUS_DOC = PACKAGE_ROOT / "docs" / "design" / "quality-signoff-status.md"
COMPLEXITY_BASELINE_PATH = "src/py/kicad_monkey"
COMPLEXITY_MAX_BASELINE = 27
COMPLEXITY_COUNT_BASELINES = {
    10: 129,
    20: 18,
    30: 0,
    50: 0,
}
COMPLEXITY_EXCESS_BASELINES = {
    10: 614,
    20: 75,
    30: 0,
}
COMPLEXITY_MESSAGE_RE = re.compile(r"\((?P<complexity>\d+) > 10\)")


def _run_module(module: str, *args: str) -> subprocess.CompletedProcess[str]:
    """Run a Python module from the repository root and capture output."""
    return subprocess.run(
        [sys.executable, "-m", module, *args],
        cwd=PACKAGE_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )


def test_package_source_ruff_check_passes() -> None:
    """Verify source-wide linting is part of Rack signoff."""
    completed = _run_module("ruff", "check", *RUFF_BASELINE_PATHS)

    assert completed.returncode == 0, completed.stderr + completed.stdout


def test_package_source_complexity_ratchet_does_not_regress() -> None:
    """Verify source complexity stays within the current public-signoff ratchet."""
    completed = _run_module(
        "ruff",
        "check",
        "--select",
        "C901",
        "--config",
        "lint.mccabe.max-complexity=10",
        "--output-format",
        "json",
        COMPLEXITY_BASELINE_PATH,
    )

    try:
        diagnostics = json.loads(completed.stdout or "[]")
    except json.JSONDecodeError as exc:
        raise AssertionError(completed.stderr + completed.stdout) from exc

    complexities: list[tuple[int, str, int, str]] = []
    for diagnostic in diagnostics:
        if diagnostic.get("code") != "C901":
            continue
        match = COMPLEXITY_MESSAGE_RE.search(str(diagnostic.get("message", "")))
        if match is None:
            raise AssertionError(f"Could not parse complexity diagnostic: {diagnostic!r}")
        filename = Path(str(diagnostic["filename"])).relative_to(PACKAGE_ROOT)
        row = int(diagnostic["location"]["row"])
        complexities.append(
            (
                int(match.group("complexity")),
                filename.as_posix(),
                row,
                str(diagnostic["message"]),
            )
        )

    observed_max = max((value for value, _, _, _ in complexities), default=0)
    failures: list[str] = []
    if observed_max > COMPLEXITY_MAX_BASELINE:
        failures.append(
            f"max complexity {observed_max} exceeds baseline {COMPLEXITY_MAX_BASELINE}"
        )

    for threshold, baseline in COMPLEXITY_COUNT_BASELINES.items():
        observed = sum(1 for value, _, _, _ in complexities if value > threshold)
        if observed > baseline:
            failures.append(
                f"{observed} functions exceed complexity {threshold}; baseline is {baseline}"
            )

    for threshold, baseline in COMPLEXITY_EXCESS_BASELINES.items():
        observed = sum(max(0, value - threshold) for value, _, _, _ in complexities)
        if observed > baseline:
            failures.append(
                f"excess complexity over {threshold} is {observed}; baseline is {baseline}"
            )

    top = "\n".join(
        f"  {path}:{row}: {message}"
        for _, path, row, message in sorted(complexities, reverse=True)[:10]
    )
    assert failures == [], "Complexity ratchet regression:\n" + "\n".join(failures) + "\n" + top


def test_package_pyright_check_passes() -> None:
    """Verify package-wide type checking is part of Rack signoff."""
    completed = _run_module("pyright")

    assert completed.returncode == 0, completed.stderr + completed.stdout


def test_quality_status_documents_broader_ratchet_state() -> None:
    """Verify the quality gate documents the broader ratchet strategy."""
    text = QUALITY_STATUS_DOC.read_text(encoding="utf-8")

    assert "public-release bootstrap audit" in text
    assert "package-wide ruff" in text
    assert "package-wide pyright" in text
    assert "kicad-cruncher" in text
