"""Release signoff tests for the public package."""

from __future__ import annotations

import json
import subprocess
import sys
import tomllib
from datetime import date
from importlib.metadata import version as distribution_version
from pathlib import Path

import kicad_cruncher
import pytest
from kicad_cruncher._version import cli_version_report, cli_version_text


def _project_root() -> Path:
    """Find the repository root from this test file."""
    for parent in Path(__file__).resolve().parents:
        if (parent / "pyproject.toml").exists():
            return parent
    raise RuntimeError("Could not locate repository root")


PACKAGE_ROOT = _project_root()
EXPECTED_VERSION = "2026.7.17"
EXPECTED_RELEASE_DATE = date(2026, 7, 17)
EXPECTED_RELEASE_NOTE = PACKAGE_ROOT / "docs" / "releases" / "2026-07-17.md"
CONTROLLED_DEPENDENCIES = {"kicad-monkey": "2026.7.28", "wn-geometer": "2026.6.10"}
CONTROLLED_DEPENDENCY_SPECIFIERS = {
    "kicad-monkey": ">=",
    "wn-geometer": "==",
}
DEV_STD_MINIMUM_VERSION = "2026.7.18"
DEV_STD_AUDIT_SCOPES = {
    "repo",
    "ci",
    "docs.design",
    "docs.links",
    "docs.cli",
    "docs.plans",
    "docs.requirements",
    "docs.release",
}


def test_version_contract_matches_date_based_release() -> None:
    """Verify that package version metadata follows the date release contract."""
    pyproject = tomllib.loads((PACKAGE_ROOT / "pyproject.toml").read_text(encoding="utf-8"))
    version = kicad_cruncher.version()

    assert pyproject["project"]["version"] == EXPECTED_VERSION
    assert kicad_cruncher.__version__ == EXPECTED_VERSION
    assert version.string == EXPECTED_VERSION
    assert (version.major, version.minor, version.patch, version.build) == (
        2026,
        7,
        17,
        None,
    )
    assert version.release_date == EXPECTED_RELEASE_DATE
    assert version.release_date <= date.today()
    assert pyproject["project"]["scripts"] == {
        "kicad-cruncher": "kicad_cruncher._cli:main",
        "kcr": "kicad_cruncher._cli:main",
    }


def test_controlled_dependency_pins_match_latest_release_versions() -> None:
    """Verify controlled dependencies use audited release constraints."""
    pyproject = tomllib.loads((PACKAGE_ROOT / "pyproject.toml").read_text(encoding="utf-8"))
    dependencies = set(pyproject["project"]["dependencies"])

    for distribution_name, expected_version in CONTROLLED_DEPENDENCIES.items():
        specifier = CONTROLLED_DEPENDENCY_SPECIFIERS[distribution_name]
        assert f"{distribution_name}{specifier}{expected_version}" in dependencies

    test_dependencies = set(pyproject["project"]["optional-dependencies"]["test"])
    dev_dependencies = set(pyproject["dependency-groups"]["dev"])
    dev_std_requirement = (
        f"wn-dev-std>={DEV_STD_MINIMUM_VERSION}; python_version >= '3.12'"
    )
    assert dev_std_requirement in test_dependencies
    assert dev_std_requirement in dev_dependencies


def test_configured_dev_std_audit_scopes_pass() -> None:
    """Verify the configured dev-std audit scopes are part of release signoff."""
    if sys.version_info < (3, 12):
        pytest.skip("wn-dev-std 2026.7.18 requires Python 3.12")

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "wn_dev_std",
            "audit",
            ".",
            "--format",
            "json",
        ],
        cwd=PACKAGE_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )

    assert completed.returncode == 0, completed.stderr + completed.stdout
    payload = json.loads(completed.stdout)
    assert payload["passed"] is True
    observed_scopes = {str(check.get("scope")) for check in payload["checks"]}
    assert DEV_STD_AUDIT_SCOPES.issubset(observed_scopes)


def test_dev_std_upstream_version_is_current() -> None:
    """Verify release signoff notices when the configured standard is stale."""
    if sys.version_info < (3, 12):
        pytest.skip("wn-dev-std 2026.7.18 requires Python 3.12")

    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "wn_dev_std",
            "audit",
            ".",
            "--check-upstream-version",
            "--format",
            "json",
        ],
        cwd=PACKAGE_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )

    assert completed.returncode == 0, completed.stderr + completed.stdout
    payload = json.loads(completed.stdout)
    assert payload["passed"] is True


def test_cli_emits_package_version() -> None:
    """Verify that CLI version commands emit the canonical package version text."""
    for args in (("--version",), ("version",)):
        completed = subprocess.run(
            [sys.executable, "-m", "kicad_cruncher", *args],
            check=False,
            capture_output=True,
            text=True,
        )

        assert completed.returncode == 0, completed.stderr
        assert completed.stdout.strip() == cli_version_report()
        assert completed.stdout.startswith("kicad-cruncher ")
        assert completed.stdout.splitlines()[0] == cli_version_text()

        for distribution_name, expected_version in CONTROLLED_DEPENDENCIES.items():
            installed_version = distribution_version(distribution_name)
            assert f"{distribution_name} {installed_version}" in completed.stdout
            parsed_installed = tuple(int(part) for part in installed_version.split("."))
            parsed_expected = tuple(int(part) for part in expected_version.split("."))
            if CONTROLLED_DEPENDENCY_SPECIFIERS[distribution_name] == "==":
                assert parsed_installed == parsed_expected
            else:
                assert parsed_installed >= parsed_expected


def test_release_notes_mention_package_version() -> None:
    """Verify that changelog and dated release notes mention the package version."""
    changelog = (PACKAGE_ROOT / "CHANGELOG.md").read_text(encoding="utf-8")
    release_note = EXPECTED_RELEASE_NOTE.read_text(encoding="utf-8")

    assert f"## {EXPECTED_VERSION}" in changelog
    assert f"`{EXPECTED_VERSION}`" in release_note
    assert EXPECTED_RELEASE_DATE.isoformat() in release_note


def test_developer_working_docs_are_excluded_from_release_artifacts() -> None:
    """Verify that developer-only docs and copied corpus fixtures are not packaged."""
    pyproject = tomllib.loads((PACKAGE_ROOT / "pyproject.toml").read_text(encoding="utf-8"))
    sdist = pyproject["tool"]["hatch"]["build"]["targets"]["sdist"]

    assert "docs/**" in sdist["include"]
    assert "docs/plans/**" in sdist["exclude"]
    assert "docs/research/**" in sdist["exclude"]
    assert "tests/corpus/**" in sdist["exclude"]


def test_python_signoff_does_not_regress() -> None:
    """Verify that the Python source signoff has no findings."""
    baseline = PACKAGE_ROOT / "tests" / "support_scripts" / "py_signoff_baseline.json"
    script = PACKAGE_ROOT / "tests" / "support_scripts" / "py_signoff.py"

    completed = subprocess.run(
        [
            sys.executable,
            str(script),
            "--root",
            str(PACKAGE_ROOT),
            "--baseline",
            str(baseline),
            "--format",
            "json",
        ],
        check=False,
        capture_output=True,
        text=True,
    )

    assert completed.returncode == 0, completed.stderr + completed.stdout
    payload = json.loads(completed.stdout)
    assert payload["finding_count"] == 0
