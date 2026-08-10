"""Monorepo boundary tests for KiCad Monkey and KiCad Cruncher."""

from __future__ import annotations

import subprocess
import sys
import tomllib
from pathlib import Path

import kicad_cruncher
import kicad_monkey


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]
CRUNCHER_ROOT = REPOSITORY_ROOT / "packages" / "kicad_cruncher"


def _pyproject(path: Path) -> dict[str, object]:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def test_workspace_keeps_one_way_package_dependency() -> None:
    """Cruncher depends on workspace Monkey and Monkey never depends on Cruncher."""

    monkey = _pyproject(REPOSITORY_ROOT / "pyproject.toml")
    cruncher = _pyproject(CRUNCHER_ROOT / "pyproject.toml")

    assert monkey["tool"]["uv"]["workspace"]["members"] == [
        "packages/kicad_cruncher"
    ]
    assert not any(
        str(requirement).startswith("kicad-cruncher")
        for requirement in monkey["project"]["dependencies"]
    )
    assert any(
        str(requirement).startswith("kicad-monkey")
        for requirement in cruncher["project"]["dependencies"]
    )
    assert cruncher["tool"]["uv"]["sources"]["kicad-monkey"] == {
        "workspace": True
    }
    assert (REPOSITORY_ROOT / "uv.lock").exists()
    assert not (CRUNCHER_ROOT / "uv.lock").exists()


def test_workspace_imports_both_packages_from_the_checkout() -> None:
    """Development resolves both distributions from their owned source trees."""

    monkey_source = Path(kicad_monkey.__file__).resolve()
    cruncher_source = Path(kicad_cruncher.__file__).resolve()

    assert monkey_source.is_relative_to(REPOSITORY_ROOT / "src" / "py")
    assert cruncher_source.is_relative_to(CRUNCHER_ROOT / "src" / "py")


def test_workspace_cruncher_cli_uses_live_monkey() -> None:
    """The CLI starts against the live workspace dependency in one checkout."""

    completed = subprocess.run(
        [sys.executable, "-m", "kicad_cruncher", "version"],
        cwd=REPOSITORY_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )

    assert completed.returncode == 0, completed.stderr
    assert f"kicad-monkey {kicad_monkey.__version__}" in completed.stdout
    assert f"kicad-cruncher {kicad_cruncher.__version__}" in completed.stdout


def test_repository_governance_routes_both_packages() -> None:
    """Only root workflows are active and release tags are package-qualified."""

    assert not any(path.is_file() for path in (CRUNCHER_ROOT / ".github").rglob("*"))
    ci = (REPOSITORY_ROOT / ".github" / "workflows" / "ci.yml").read_text(
        encoding="utf-8"
    )
    release = (
        REPOSITORY_ROOT / ".github" / "workflows" / "release.yml"
    ).read_text(encoding="utf-8")

    assert "Run Cruncher Rack gates against workspace Monkey" in ci
    assert "Run installed two-wheel compatibility test" in ci
    assert "Rehearse Cruncher upgrade and rollback" in ci
    assert "publish-monkey:" in release
    assert "publish-cruncher:" in release
    assert "verify-coordinated-monkey:" in release
    assert "publish-coordinated-cruncher:" in release
    assert "create-coordinated-releases:" in release
    assert "inputs.confirm == 'publish-both'" in release
    assert "environment: pypi\n" not in release
    assert release.count("environment: pypi-kicad-monkey\n") == 1
    assert release.count("environment: pypi-kicad-cruncher\n") == 2
    assert "for install_attempt in {1..20}; do" in release
    assert "did not become installable" in release
    assert "kicad-monkey-v${VERSION}" in release
    assert "kicad-cruncher-v${VERSION}" in release
    assert "packages-dir: packages/kicad_cruncher/dist/" in release


def test_history_import_hygiene_exception_is_explicit_and_ancestry_bounded() -> None:
    """Preserved history needs a maintainer label, full SHA, and merge boundary."""

    hygiene = (
        REPOSITORY_ROOT / ".github" / "workflows" / "pr-hygiene.yml"
    ).read_text(encoding="utf-8")

    assert 'label.name === "history-import"' in hygiene
    assert "Imported history head:" in hygiene
    assert "([0-9a-f]{40})" in hygiene
    assert '["show", "-s", "--format=%P", commit.sha]' in hygiene
    assert '["merge-base", "--is-ancestor", commit.sha, importedHistoryHead]' in hygiene
    assert "parents.slice(1).includes(importedHistoryHead)" in hygiene
