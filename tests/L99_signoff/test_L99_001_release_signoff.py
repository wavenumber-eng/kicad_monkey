"""Release signoff tests for the public KiCad Monkey package."""

from __future__ import annotations

import tomllib
from datetime import date
from pathlib import Path
import re

import kicad_monkey
from kicad_monkey import __version__, version
from kicad_monkey.kicad_api_contract import collect_public_api_contract_failures


def _project_root() -> Path:
    """Find the repository root from this test file."""
    for parent in Path(__file__).resolve().parents:
        if (parent / "pyproject.toml").exists():
            return parent
    raise RuntimeError("Could not locate repository root")


PACKAGE_ROOT = _project_root()
EXPECTED_VERSION = "2026.6.18"
EXPECTED_RELEASE_DATE = date(2026, 6, 18)
PUBLIC_TEXT_PATHS = (
    "README.md",
    "AGENTS.md",
    "ARCHITECTURE.md",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
    "pyproject.toml",
    "docs",
    "src/py/kicad_monkey",
)
PUBLIC_TEXT_SUFFIXES = {".md", ".py", ".rst", ".toml", ".txt", ".yaml", ".yml"}
PUBLIC_TEXT_EXCLUDED_PARTS = {
    "__pycache__",
    ".mypy_cache",
    ".pytest_cache",
    ".ruff_cache",
}
FORBIDDEN_PUBLIC_TEXT_PATTERNS = (
    (re.compile(r"\baltium_monkey\b", re.IGNORECASE), "outside package reference"),
    (re.compile(r"\bAltiumDesign\b"), "outside package type reference"),
    (re.compile(r"\bSchGeometryRecord\b"), "outside package type reference"),
    (re.compile(r"\bdata_models\b", re.IGNORECASE), "external model package reference"),
    (re.compile(r"\bnetlist_a0\b", re.IGNORECASE), "external model schema reference"),
    (re.compile(r"\b(?:toolz|appz|toolz-tests)\b", re.IGNORECASE), "local workspace reference"),
    (
        re.compile(
            r"\b(?:lib_cruncher|bom_cruncher|pcb_cruncher)\b",
            re.IGNORECASE,
        ),
        "internal consumer reference",
    ),
    (re.compile(r"C:[/\\]eli", re.IGNORECASE), "local absolute path"),
    (re.compile(r"\bagent-worktrees\b", re.IGNORECASE), "local workspace path"),
    (re.compile(r"\bwn-hw\b", re.IGNORECASE), "local workspace repo"),
    (re.compile(r"\bprivate kicad_monkey\b", re.IGNORECASE), "private-suite reference"),
    (re.compile(r"\bprivate test\b", re.IGNORECASE), "private-test reference"),
    (re.compile(r"\bcruncher workflows\b", re.IGNORECASE), "internal workflow reference"),
    (re.compile(r"\bPhase\s+[A-Z0-9]", re.IGNORECASE), "development phase label"),
    (re.compile(r"\bSlice\s+[A-Z0-9]", re.IGNORECASE), "development slice label"),
    (
        re.compile(r"\b(?:C|D|E|F|G|N)-\d+(?:\.\d+)?[a-z]?\b"),
        "development rollout id",
    ),
    (
        re.compile(r"\b(?:this|later|follow-on)\s+slice\b", re.IGNORECASE),
        "development slice prose",
    ),
)


def test_version_contract_matches_date_based_release() -> None:
    """Verify that package metadata follows the date release contract."""
    pyproject = tomllib.loads((PACKAGE_ROOT / "pyproject.toml").read_text(encoding="utf-8"))
    parsed = version()

    assert pyproject["project"]["version"] == EXPECTED_VERSION
    assert __version__ == EXPECTED_VERSION
    assert kicad_monkey.__version__ == EXPECTED_VERSION
    assert parsed.string == EXPECTED_VERSION
    assert (parsed.major, parsed.minor, parsed.patch, parsed.build) == (
        2026,
        6,
        18,
        None,
    )
    assert parsed.release_date == EXPECTED_RELEASE_DATE
    assert parsed.release_date <= date.today()


def test_changelog_mentions_package_version() -> None:
    """Verify that release notes mention the current package version."""
    changelog = (PACKAGE_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
    release_notes = (
        PACKAGE_ROOT / "docs" / "releases" / f"{EXPECTED_RELEASE_DATE.isoformat()}.md"
    ).read_text(encoding="utf-8")

    assert f"## {EXPECTED_VERSION}" in changelog
    assert f"`{EXPECTED_VERSION}`" in release_notes


def test_public_package_metadata_is_declared() -> None:
    """Verify public package metadata needed for PyPI and GitHub is present."""
    pyproject = tomllib.loads((PACKAGE_ROOT / "pyproject.toml").read_text(encoding="utf-8"))
    project = pyproject["project"]

    assert project["license"]["text"] == "MIT"
    assert project["authors"] == [{"name": "Wavenumber LLC"}]
    assert project["urls"]["Repository"] == "https://github.com/wavenumber-eng/kicad_monkey"
    assert (PACKAGE_ROOT / "LICENSE").exists()


def test_public_repository_support_files_are_declared() -> None:
    """Verify public contribution, issue, CI, and release files exist."""
    required_paths = (
        "CONTRIBUTING.md",
        ".github/pull_request_template.md",
        ".github/ISSUE_TEMPLATE/bug_report.md",
        ".github/ISSUE_TEMPLATE/feature_request.md",
        ".github/workflows/ci.yml",
        ".github/workflows/release.yml",
    )

    missing = [path for path in required_paths if not (PACKAGE_ROOT / path).exists()]

    assert missing == []


def test_developer_working_docs_are_excluded_from_release_artifacts() -> None:
    """Verify that developer-only plan and research docs are not packaged."""
    pyproject = tomllib.loads((PACKAGE_ROOT / "pyproject.toml").read_text(encoding="utf-8"))
    sdist = pyproject["tool"]["hatch"]["build"]["targets"]["sdist"]

    assert "docs/**" in sdist["include"]
    assert "LICENSE" in sdist["include"]
    assert "CONTRIBUTING.md" in sdist["include"]
    assert "docs/plans/**" in sdist["exclude"]
    assert "docs/research/**" in sdist["exclude"]
    assert "tests/corpus/**" in sdist["exclude"]


def test_promoted_public_api_contract_has_no_failures() -> None:
    """Verify the promoted package-root API contract is part of L99 signoff."""
    assert collect_public_api_contract_failures() == []


def _iter_public_text_files() -> list[Path]:
    """Return public source/docs files that should not expose local history."""
    files: list[Path] = []
    for relative in PUBLIC_TEXT_PATHS:
        root = PACKAGE_ROOT / relative
        if root.is_file():
            candidates = [root]
        else:
            candidates = [path for path in root.rglob("*") if path.is_file()]
        for path in candidates:
            if path.suffix.lower() not in PUBLIC_TEXT_SUFFIXES:
                continue
            relative_parts = path.relative_to(PACKAGE_ROOT).parts
            if any(part in PUBLIC_TEXT_EXCLUDED_PARTS for part in relative_parts):
                continue
            if relative_parts[:2] in {("docs", "plans"), ("docs", "research")}:
                continue
            files.append(path)
    return sorted(set(files))


def test_public_text_has_no_private_or_rollout_references() -> None:
    """Verify public source/docs avoid local history and outside-project prose."""
    failures: list[str] = []
    for path in _iter_public_text_files():
        rel_path = path.relative_to(PACKAGE_ROOT).as_posix()
        text = path.read_text(encoding="utf-8")
        for line_number, line in enumerate(text.splitlines(), start=1):
            for pattern, reason in FORBIDDEN_PUBLIC_TEXT_PATTERNS:
                if pattern.search(line):
                    failures.append(f"{rel_path}:{line_number}: {reason}: {line.strip()}")

    assert failures == []
