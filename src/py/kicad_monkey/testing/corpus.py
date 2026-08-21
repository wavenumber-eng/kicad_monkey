"""Shared corpus helpers for kicad_monkey tests."""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import tempfile
import tomllib
import zipfile
from pathlib import Path
from pathlib import PurePosixPath
from typing import Any, Iterable


PACKAGE_ROOT = Path(__file__).resolve().parents[4]
DEFAULT_CORPUS_ARCHIVE = PACKAGE_ROOT / "tests" / "corpus" / "kicad.zip"
DEFAULT_CORPUS_ARCHIVE_MANIFEST = DEFAULT_CORPUS_ARCHIVE.with_name(
    "kicad.archive.toml"
)
DEFAULT_CORPUS_AUTHORING_ROOT = PACKAGE_ROOT / "tests" / "corpus"
DEFAULT_CORPUS_CACHE_ROOT = DEFAULT_CORPUS_AUTHORING_ROOT / ".unpacked"
CORPUS_ARCHIVE_ENV = "KM_CORPUS"
CORPUS_ROOT_ENV = "KM_CORPUS_ROOT"
CORPUS_RESOLVED_FROM_ENV = "KM_CORPUS_RESOLVED_FROM"


def _require_dir(path: Path, *, label: str) -> Path:
    if not path.exists():
        raise FileNotFoundError(f"{label} not found: {path}")
    if not path.is_dir():
        raise NotADirectoryError(f"{label} is not a directory: {path}")
    return path


def _is_inside(child: Path, parent: Path) -> bool:
    try:
        child.resolve().relative_to(parent.resolve())
    except ValueError:
        return False
    return True


def _stream_digest(source: Any) -> str:
    digest = hashlib.sha256()
    for chunk in iter(lambda: source.read(1024 * 1024), b""):
        digest.update(chunk)
    return digest.hexdigest()


def _archive_digest(archive: Path) -> str:
    with archive.open("rb") as source:
        return _stream_digest(source)


def _validate_reviewed_default_archive(archive: Path, digest: str) -> None:
    if archive.resolve() != DEFAULT_CORPUS_ARCHIVE.resolve():
        return
    manifest_path = DEFAULT_CORPUS_ARCHIVE_MANIFEST
    if not manifest_path.is_file():
        raise FileNotFoundError(
            f"Package corpus archive manifest not found: {manifest_path}"
        )
    metadata = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
    if (
        metadata.get("schema") != "kicad_monkey.corpus_archive.v1"
        or metadata.get("archive") != archive.name
    ):
        raise RuntimeError(f"Invalid package corpus archive manifest: {manifest_path}")
    try:
        expected_size = int(metadata["size"])
        expected_digest = str(metadata["sha256"]).lower()
    except (KeyError, TypeError, ValueError) as error:
        raise RuntimeError(
            f"Invalid package corpus archive manifest: {manifest_path}"
        ) from error
    if (
        expected_size < 0
        or len(expected_digest) != 64
        or any(character not in "0123456789abcdef" for character in expected_digest)
        or archive.stat().st_size != expected_size
        or digest != expected_digest
    ):
        raise RuntimeError(
            f"Package corpus archive does not match its reviewed manifest: {archive}"
        )


def _archive_cache_root(archive: Path) -> Path:
    if archive.resolve() == DEFAULT_CORPUS_ARCHIVE.resolve():
        return DEFAULT_CORPUS_CACHE_ROOT
    identity = hashlib.sha256(str(archive.resolve()).encode("utf-8")).hexdigest()[:16]
    return Path(tempfile.gettempdir()) / "kicad-monkey-corpus" / identity


def _validate_archive_members(archive: zipfile.ZipFile, target: Path) -> None:
    target_root = target.resolve()
    for member in archive.infolist():
        name = member.filename.replace("\\", "/")
        rel = PurePosixPath(name)
        if rel.is_absolute() or ".." in rel.parts or not rel.parts:
            raise RuntimeError(f"Unsafe corpus archive member: {member.filename!r}")
        if rel.parts[0] != "kicad":
            raise RuntimeError(
                "Corpus archive must contain a top-level kicad/ directory: "
                f"{member.filename!r}"
            )
        destination = (target / Path(*rel.parts)).resolve()
        if not _is_inside(destination, target_root):
            raise RuntimeError(f"Unsafe corpus archive destination: {destination}")


def _extract_corpus_archive(archive: Path) -> Path:
    cache_root = _archive_cache_root(archive)
    cache_root.mkdir(parents=True, exist_ok=True)
    with archive.open("rb") as archive_stream:
        digest = _stream_digest(archive_stream)
        _validate_reviewed_default_archive(archive, digest)
        version_root = cache_root / digest
        extracted_root = version_root / "kicad"
        if version_root.exists():
            if extracted_root.is_dir():
                return version_root
            raise RuntimeError(
                "Incomplete content-addressed corpus cache; remove this path and retry: "
                f"{version_root}"
            )

        temp_root = Path(
            tempfile.mkdtemp(prefix=f".{digest}.", suffix=".extracting", dir=cache_root)
        )
        try:
            if extracted_root.is_dir():
                return version_root
            archive_stream.seek(0)
            with zipfile.ZipFile(archive_stream) as zipped:
                _validate_archive_members(zipped, temp_root)
                zipped.extractall(temp_root)
            unpacked_kicad = temp_root / "kicad"
            if not unpacked_kicad.is_dir():
                raise RuntimeError(f"Corpus archive did not produce kicad/: {archive}")
            try:
                temp_root.rename(version_root)
            except OSError:
                if not extracted_root.is_dir():
                    raise
        except zipfile.BadZipFile as error:
            raise RuntimeError(
                f"KM_CORPUS is not a valid ZIP archive: {archive}"
            ) from error
        finally:
            shutil.rmtree(temp_root, ignore_errors=True)
    return version_root


def resolve_test_corpus_root(value: str | os.PathLike[str] | None = None) -> Path:
    """Resolve the canonical ZIP carrier to a directory containing ``kicad/``.

    ``KM_CORPUS`` canonically names a ZIP archive. A directory containing a
    top-level ``kicad/`` is accepted only for fixture-authoring workflows.
    Invalid explicit values fail closed instead of falling back silently.
    """

    configured = value if value is not None else os.environ.get(CORPUS_ARCHIVE_ENV)
    if configured is not None and str(configured).strip():
        carrier = Path(configured).expanduser()
        if carrier.is_dir():
            return _require_dir(carrier / "kicad", label="KM_CORPUS kicad root").parent
        if not carrier.is_file():
            raise FileNotFoundError(f"KM_CORPUS not found: {carrier}")
        return _extract_corpus_archive(carrier)

    if not DEFAULT_CORPUS_ARCHIVE.is_file():
        raise FileNotFoundError(
            "KiCad corpus archive not found; restore tests/corpus/kicad.zip or set "
            "KM_CORPUS to a reviewed kicad.zip"
        )
    return _extract_corpus_archive(DEFAULT_CORPUS_ARCHIVE)


def get_test_corpus_root() -> Path:
    configured = os.environ.get(CORPUS_ARCHIVE_ENV)
    resolved = os.environ.get(CORPUS_ROOT_ENV)
    if (
        configured
        and resolved
        and os.environ.get(CORPUS_RESOLVED_FROM_ENV)
        == str(Path(configured).expanduser().resolve())
    ):
        root = _require_dir(Path(resolved), label=CORPUS_ROOT_ENV)
        _require_dir(root / "kicad", label="KiCad corpus root")
        return root
    if configured:
        return resolve_test_corpus_root()
    return resolve_test_corpus_root()


def get_kicad_corpus_root() -> Path:
    return _require_dir(get_test_corpus_root() / "kicad", label="KiCad corpus root")


def get_kicad_corpus_manifest_path() -> Path:
    """Return the canonical KiCad corpus manifest path."""
    return get_kicad_corpus_root() / "manifest.json"


def load_kicad_corpus_manifest(*, required: bool = True) -> dict[str, Any] | None:
    """Load the resolved ``KM_CORPUS`` archive's ``kicad/manifest.json``.

    The manifest is the registry for promoted KiCad test assets. Legacy tests
    still have path helpers below while coverage migrates to manifest queries.
    """
    manifest_path = get_kicad_corpus_manifest_path()
    if not manifest_path.exists():
        if required:
            raise FileNotFoundError(f"KiCad corpus manifest not found: {manifest_path}")
        return None
    data = json.loads(manifest_path.read_text(encoding="utf-8-sig"))
    if not isinstance(data, dict):
        raise ValueError(
            f"KiCad corpus manifest must be a JSON object: {manifest_path}"
        )
    return data


def iter_kicad_corpus_cases(
    *,
    domain: str | None = None,
    origin: str | None = None,
    status: str | Iterable[str] | None = "active",
    required: bool = True,
) -> Iterable[dict[str, Any]]:
    """Yield manifest case entries filtered by domain/origin/status."""
    manifest = load_kicad_corpus_manifest(required=required)
    if manifest is None:
        return

    statuses: set[str] | None
    if status is None:
        statuses = None
    elif isinstance(status, str):
        statuses = {status}
    else:
        statuses = {str(item) for item in status}

    for case in manifest.get("cases") or []:
        if not isinstance(case, dict):
            continue
        if domain is not None and domain not in (case.get("domains") or []):
            continue
        if origin is not None and case.get("origin") != origin:
            continue
        if statuses is not None and str(case.get("status", "")) not in statuses:
            continue
        yield case


def get_kicad_corpus_case(
    case_id: str,
    *,
    required: bool = True,
) -> dict[str, Any] | None:
    """Return one manifest case by id."""
    for case in iter_kicad_corpus_cases(status=None, required=required):
        if case.get("id") == case_id:
            return case
    if required:
        raise KeyError(f"KiCad corpus case not found in manifest: {case_id}")
    return None


def resolve_kicad_manifest_path(case: dict[str, Any], key: str) -> Path | None:
    """Resolve a manifest relative path field against the KiCad corpus root."""
    value = case.get(key)
    if value in (None, ""):
        return None
    return get_kicad_corpus_root() / str(value)


def get_kicad_common_dir() -> Path:
    return _require_dir(get_kicad_corpus_root() / "common", label="KiCad common corpus")


def get_kicad_common_case_dir(case_name: str) -> Path:
    return _require_dir(
        get_kicad_common_dir() / case_name, label=f"KiCad common case '{case_name}'"
    )


def get_kicad_topic_dir(topic: str) -> Path:
    return _require_dir(get_kicad_corpus_root() / topic, label=f"KiCad topic '{topic}'")


def get_kicad_topic_input_dir(topic: str) -> Path:
    return _require_dir(
        get_kicad_topic_dir(topic) / "input", label=f"KiCad topic input '{topic}'"
    )


def get_kicad_common_boards_dir() -> Path:
    return _require_dir(
        get_kicad_common_dir() / "board" / "input", label="KiCad common boards input"
    )


def get_kicad_common_board_case_dir(case_name: str) -> Path:
    return _require_dir(
        get_kicad_common_boards_dir() / case_name,
        label=f"KiCad common board case '{case_name}'",
    )


def get_kicad_common_footprints_dir() -> Path:
    return _require_dir(
        get_kicad_common_dir() / "footprints" / "input",
        label="KiCad common footprints input",
    )


def get_kicad_common_reference_symbols_dir() -> Path:
    return _require_dir(
        get_kicad_common_dir() / "reference_symbols" / "input",
        label="KiCad reference symbols input",
    )


def get_kicad_common_reference_schematics_dir() -> Path:
    return _require_dir(
        get_kicad_common_dir() / "reference_schematics" / "input",
        label="KiCad reference schematics input",
    )


def get_kicad_common_reference_worksheets_dir() -> Path:
    return _require_dir(
        get_kicad_common_dir() / "reference_worksheets" / "input",
        label="KiCad reference worksheets input",
    )


def get_kicad_common_board_case_file(case_name: str, filename: str) -> Path:
    case_dir = _require_dir(
        get_kicad_common_boards_dir() / case_name,
        label=f"KiCad board case '{case_name}'",
    )
    return case_dir / filename


def get_kicad_topic_case_file(topic: str, case_name: str, filename: str) -> Path:
    case_dir = _require_dir(
        get_kicad_topic_dir(topic) / "input" / case_name,
        label=f"KiCad topic case '{topic}/{case_name}'",
    )
    return case_dir / filename


def get_kicad_pcb_foundation_dir() -> Path:
    """Return the synthetic-PCB foundation corpus root.

    Layout:

        <corpus>/kicad/pcb_foundation/<case>/
            input/<case files>
            reference_output/<oracle outputs>
            output/<test-run regenerated artifacts>

    Used by parsing, IR, SVG, IPC, viz, and data-model validation.
    """
    return _require_dir(
        get_kicad_corpus_root() / "pcb_foundation",
        label="KiCad PCB foundation corpus",
    )


def get_kicad_pcb_foundation_case_dir(case_name: str) -> Path:
    return _require_dir(
        get_kicad_pcb_foundation_dir() / case_name,
        label=f"KiCad pcb_foundation case '{case_name}'",
    )


def get_kicad_pcb_foundation_case_input_dir(case_name: str) -> Path:
    return _require_dir(
        get_kicad_pcb_foundation_case_dir(case_name) / "input",
        label=f"KiCad pcb_foundation case input '{case_name}'",
    )


def get_kicad_pcb_foundation_case_reference_output_dir(case_name: str) -> Path:
    return _require_dir(
        get_kicad_pcb_foundation_case_dir(case_name) / "reference_output",
        label=f"KiCad pcb_foundation case reference_output '{case_name}'",
    )


def get_kicad_upstream_qa_dir() -> Path:
    """Mirrored KiCad ``qa/data/`` tree (curated 41-file subset).

    Refresh via the package-local upstream QA fixture sync script.
    """
    return _require_dir(
        get_kicad_corpus_root() / "upstream_qa",
        label="KiCad upstream qa mirror",
    )


KICAD_SEXPR_FILE_SUFFIXES: tuple[str, ...] = (
    ".kicad_pcb",
    ".kicad_sch",
    ".kicad_sym",
    ".kicad_mod",
    ".kicad_wks",
)
"""KiCad S-expression file suffixes used by the parser-only pass-through gate.

``.kicad_pro`` is intentionally absent: KiCad project files are JSON, not
S-expression, so they cannot exercise ``parse_sexp``/``build_sexp``.
"""


def iter_kicad_sexpr_files(
    *,
    root: Path | None = None,
    suffixes: Iterable[str] | None = None,
    exclude_dirs: Iterable[str] = ("output", "review", "review_tmp"),
) -> Iterable[Path]:
    """Yield every KiCad S-expression file under ``root`` in stable order.

    Defaults to the canonical corpus root and the full set of S-expression
    file types. ``exclude_dirs`` drops generated/output trees so the
    pass-through gate does not chase stale or derived artefacts.
    """
    base = root if root is not None else get_kicad_corpus_root()
    allowed = tuple(s.lower() for s in (suffixes or KICAD_SEXPR_FILE_SUFFIXES))
    excluded = {name for name in exclude_dirs}

    found: list[Path] = []
    for path in base.rglob("*"):
        if not path.is_file():
            continue
        if path.suffix.lower() not in allowed:
            continue
        parts = set(path.relative_to(base).parts[:-1])
        if parts & excluded:
            continue
        found.append(path)

    found.sort()
    yield from found


def get_kicad_netlist_upstream_qa_dir() -> Path:
    """Mirrored KiCad ``qa/data/eeschema/netlists/`` tree (14 cases).

    Refresh via ``scripts/sync_upstream_qa_netlist_fixtures.py``.
    Each subdirectory is one case with a ``.kicad_sch`` (+ optional sub-
    schematics, ``.kicad_pro``) and a golden ``.net`` produced by
    upstream's own KiCad build.
    """
    return _require_dir(
        get_kicad_corpus_root() / "netlist" / "upstream_qa",
        label="KiCad netlist upstream qa mirror",
    )
