"""Release signoff tests for the public KiCad Monkey package."""

from __future__ import annotations

import hashlib
import importlib.util
import io
import json
import shutil
import subprocess
import sys
import tomllib
from datetime import date
from pathlib import Path
import re

import pytest

import kicad_monkey
from kicad_monkey import __version__, version
from kicad_monkey.kicad_api_contract import collect_public_api_contract_failures


def _project_root() -> Path:
    """Find the repository root from this test file."""
    for parent in Path(__file__).resolve().parents:
        if (parent / "pyproject.toml").exists():
            return parent
    raise RuntimeError("Could not locate repository root")


def _load_corpus_archive_module():
    """Load the corpus archive script as a module for focused script tests."""
    module_path = PACKAGE_ROOT / "scripts" / "kicad_corpus_archive.py"
    spec = importlib.util.spec_from_file_location("kicad_corpus_archive_test", module_path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    try:
        spec.loader.exec_module(module)
    finally:
        sys.modules.pop(spec.name, None)
    return module


PACKAGE_ROOT = _project_root()
EXPECTED_VERSION = "2026.8.18"
EXPECTED_RELEASE_DATE = date(2026, 8, 18)
EXPECTED_RUST_VERSION = "2026.8.21"
EXPECTED_RUST_RELEASE_DATE = date(2026, 8, 21)
CORPUS_ARCHIVE_PATH = "tests/corpus/kicad.zip"
CORPUS_ARCHIVE_MANIFEST_PATH = "tests/corpus/kicad.archive.toml"
DEV_STD_AUDIT_SCOPES = {"repo", "ci", "docs.design", "docs.links", "docs.plans"}
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
    cargo = tomllib.loads((PACKAGE_ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    parsed = version()

    assert pyproject["project"]["version"] == EXPECTED_VERSION
    assert cargo["workspace"]["package"]["version"] == EXPECTED_RUST_VERSION
    rust_version_parts = tuple(int(part) for part in EXPECTED_RUST_VERSION.split("."))
    assert rust_version_parts == (
        EXPECTED_RUST_RELEASE_DATE.year,
        EXPECTED_RUST_RELEASE_DATE.month,
        EXPECTED_RUST_RELEASE_DATE.day,
    )
    assert EXPECTED_RUST_RELEASE_DATE <= date.today()
    assert __version__ == EXPECTED_VERSION
    assert kicad_monkey.__version__ == EXPECTED_VERSION
    assert parsed.string == EXPECTED_VERSION
    assert (parsed.major, parsed.minor, parsed.patch, parsed.build, parsed.alpha) == (
        2026,
        8,
        18,
        None,
        None,
    )
    assert parsed.is_prerelease is False
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


def test_release_workflow_derives_release_date_from_version_helper() -> None:
    """Verify the publish workflow supports alpha date-version tags."""
    workflow = (PACKAGE_ROOT / ".github" / "workflows" / "release.yml").read_text(
        encoding="utf-8"
    )
    match = re.search(r'RELEASE_DATE="\$\(uv run python -c \'([^\']+)\'\)"', workflow)
    assert match is not None
    command = match.group(1)
    assert "parse_version" in command
    assert "map(int" not in command

    completed = subprocess.run(
        [sys.executable, "-c", command],
        cwd=PACKAGE_ROOT,
        check=False,
        capture_output=True,
        text=True,
    )

    assert completed.returncode == 0, completed.stderr + completed.stdout
    assert completed.stdout.strip() == EXPECTED_RELEASE_DATE.isoformat()
    assert 'startsWith(github.event.release.tag_name, \'kicad-monkey-v\')' in workflow
    assert 'test "kicad-monkey-v${VERSION}" = "${GITHUB_REF_NAME}"' in workflow


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


def test_monkey_sdist_workspace_excludes_downstream_members_without_drift() -> None:
    """Keep the sdist Cargo workspace standalone without copying Cruncher source."""
    checkout = tomllib.loads((PACKAGE_ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    standalone = tomllib.loads(
        (PACKAGE_ROOT / "packaging" / "Cargo.sdist.toml").read_text(encoding="utf-8")
    )
    expected = checkout.copy()
    expected_workspace = checkout["workspace"].copy()
    expected_workspace["members"] = [
        member
        for member in expected_workspace["members"]
        if not member.startswith("packages/")
    ]
    expected["workspace"] = expected_workspace
    assert standalone == expected

    pyproject = tomllib.loads(
        (PACKAGE_ROOT / "pyproject.toml").read_text(encoding="utf-8")
    )
    sdist = pyproject["tool"]["hatch"]["build"]["targets"]["sdist"]
    assert sdist["hooks"]["custom"]["path"] == "hatch_build.py"
    assert "Cargo.toml" not in sdist["include"]
    assert "packaging/Cargo.sdist.toml" in sdist["include"]
    assert "packages/**" in sdist["exclude"]
    assert "tests/.tmp/**" in sdist["exclude"]


def test_release_build_backend_stays_compatible_with_twine_metadata_check() -> None:
    """Keep generated core metadata within Twine 6.2's accepted range."""
    for project_path in (
        PACKAGE_ROOT / "pyproject.toml",
        PACKAGE_ROOT / "packages" / "kicad_cruncher" / "pyproject.toml",
    ):
        project = tomllib.loads(project_path.read_text(encoding="utf-8"))
        assert project["build-system"]["requires"] == ["hatchling==1.31.0"]


def test_public_corpus_archive_uses_manifest_not_lfs() -> None:
    """Verify the public corpus archive is restored from object storage, not LFS."""
    manifest = tomllib.loads(
        (PACKAGE_ROOT / CORPUS_ARCHIVE_MANIFEST_PATH).read_text(encoding="utf-8")
    )
    attributes = (PACKAGE_ROOT / ".gitattributes").read_text(encoding="utf-8")
    gitignore_lines = (PACKAGE_ROOT / ".gitignore").read_text(encoding="utf-8").splitlines()

    assert manifest["schema"] == "kicad_monkey.corpus_archive.v1"
    assert manifest["archive"] == "kicad.zip"
    assert manifest["url"].startswith("https://artifacts.wavenumber.net/")
    assert manifest["r2_key"].endswith("/kicad.zip")
    assert CORPUS_ARCHIVE_PATH in gitignore_lines
    assert CORPUS_ARCHIVE_PATH not in attributes
    assert "filter=lfs" not in attributes


def test_public_corpus_restore_uses_target_parent_for_temp_download(monkeypatch, tmp_path) -> None:
    """Verify archive restore keeps temp downloads on the target filesystem."""
    corpus_archive = _load_corpus_archive_module()
    payload = b"small corpus archive placeholder"
    manifest = corpus_archive.CorpusArchiveManifest(
        archive="kicad.zip",
        size=len(payload),
        sha256=hashlib.sha256(payload).hexdigest(),
        url="https://example.invalid/kicad.zip",
    )
    target = tmp_path / "corpus" / "kicad.zip"
    captured: dict[str, object] = {}

    class FakeTemporaryDirectory:
        def __init__(self, *args, **kwargs) -> None:
            captured["temp_dir_parent"] = kwargs.get("dir")
            parent = Path(kwargs.get("dir") or tmp_path)
            self.path = parent / "download-temp"

        def __enter__(self) -> str:
            self.path.mkdir(parents=True, exist_ok=True)
            return str(self.path)

        def __exit__(self, exc_type, exc_value, traceback) -> None:
            shutil.rmtree(self.path, ignore_errors=True)

    class FakeResponse(io.BytesIO):
        def __enter__(self):
            return self

        def __exit__(self, exc_type, exc_value, traceback) -> None:
            self.close()

    def fake_urlopen(request, timeout):
        captured["download_timeout"] = timeout
        return FakeResponse(payload)

    monkeypatch.setattr(corpus_archive.tempfile, "TemporaryDirectory", FakeTemporaryDirectory)
    monkeypatch.setattr(corpus_archive.urllib.request, "urlopen", fake_urlopen)

    restored = corpus_archive.restore_archive(
        target,
        manifest,
        explicit_url=None,
        check_zip=False,
    )

    assert restored is True
    assert target.read_bytes() == payload
    assert Path(captured["temp_dir_parent"]) == target.parent
    assert captured["download_timeout"] == corpus_archive.DOWNLOAD_TIMEOUT_SECONDS


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
