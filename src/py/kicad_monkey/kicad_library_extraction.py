"""Project-local library extraction primitives for KiCad projects.

The helpers in this module are intentionally non-destructive.  They scan and
copy parsed KiCad assets so higher-level workflow tools can build inspectable
library bundles without mutating the source schematic or PCB files.
"""

from __future__ import annotations

import copy
import base64
import hashlib
import json
import os
import re
import shutil
import subprocess
import tempfile
from dataclasses import asdict, dataclass, field
from enum import StrEnum
from pathlib import Path
from typing import Any, Callable, Iterable, Sequence

from .kicad_base import find_all_elements, find_element
from .kicad_footprint import KiCadFootprint
from .kicad_lib_symbol import LibSymbol
from .kicad_model import EmbeddedFile, Model
from .kicad_parameter_aliases import canonicalize_part_parameters
from .kicad_pcb import KiCadPcb
from .kicad_pcb_footprint import Footprint
from .kicad_pcb_other import Image, NetRef
from .kicad_pcb_projection import KiCadPcbProjection
from .kicad_environment import KiCadEnvironment
from .kicad_project import find_adjacent_kicad_project_path
from .kicad_project_libraries import KiCadLibraryTable, KiCadLibraryTableKind
from .kicad_sch_image import SchImage
from .kicad_sch_enums import PropertyId, StandardPropertyKey
from .kicad_sch_symbol import SchSymbol
from .kicad_sexpr import SexpSelector, iter_sexp_form_spans, parse_sexp
from .kicad_symbol_lib import KiCadSymbolLib
from .kicad_worksheet import KiCadWorksheet

try:
    import zstandard as _zstandard
except ImportError:
    _zstandard = None


_FOOTPRINT_START_RE = re.compile(
    r'(?m)^[ \t]*(\(\s*(?:footprint|module)\s+(?:"((?:\\.|[^"\\])*)"|([^\s()]+)))'
)
class KiCadExtractionMode(StrEnum):
    """Asset extraction policy."""

    INTERNAL = "internal"
    PROJECT_LOCAL = "project_local"


class KiCadModelReferenceKind(StrEnum):
    """Classification of a KiCad 3D model path."""

    EMBEDDED = "embedded"
    KICAD_ENV = "kicad_env"
    ENV_VAR = "env_var"
    PROJECT_RELATIVE = "project_relative"
    ABSOLUTE = "absolute"
    OTHER = "other"


class KiCadExtractionDedupePolicy(StrEnum):
    """How internal extraction collapses repeated assets."""

    NAME = "name"
    FINGERPRINT = "fingerprint"
    LIBRARY_LINK = "library_link"


@dataclass(frozen=True)
class KiCadModelReferenceScan:
    """One model reference discovered in a project."""

    source_kind: str
    source_path: str
    owner: str
    model_path: str
    reference_kind: str
    resolved_path: str = ""
    exists: bool = False
    embedded_name: str = ""
    has_embedded_payload: bool = False
    payload_scope: str = ""
    diagnostics: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True)
class KiCadFootprintModelCoverage:
    """Placed PCB footprint group that has no 3D model records."""

    footprint: str
    source_path: str
    designators: tuple[str, ...]
    instance_count: int

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True)
class KiCadProjectAssetScan:
    """Summary of project-local KiCad assets."""

    project_path: str
    project_root: str
    schematics: tuple[str, ...]
    pcbs: tuple[str, ...]
    symbol_libraries: tuple[str, ...]
    pretty_libraries: tuple[str, ...]
    footprint_files: tuple[str, ...]
    model_references: tuple[KiCadModelReferenceScan, ...]
    footprints_without_models: tuple[KiCadFootprintModelCoverage, ...] = ()
    diagnostics: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        data = asdict(self)
        data["model_references"] = [ref.to_dict() for ref in self.model_references]
        data["footprints_without_models"] = [
            ref.to_dict() for ref in self.footprints_without_models
        ]
        return data


@dataclass(frozen=True)
class _ProjectAssetScope:
    schematics: tuple[Path, ...]
    pcbs: tuple[Path, ...]
    symbol_libraries: tuple[Path, ...]
    pretty_libraries: tuple[Path, ...]
    footprint_files: tuple[Path, ...]
    diagnostics: tuple[str, ...] = ()


@dataclass(frozen=True)
class KiCadEmbeddedAssetRecord:
    """One decoded embedded project payload written to disk."""

    kind: str
    source_kind: str
    source_path: str
    owner: str
    source_name: str
    output_file: str
    sha256: str
    byte_size: int
    mime_type: str = ""
    file_type: str = ""
    deduplicated: bool = False
    diagnostics: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass(frozen=True)
class KiCadCliValidationResult:
    """Result from a KiCad CLI parse/upgrade validation run."""

    input_path: str
    command: tuple[str, ...]
    returncode: int
    stdout: str = ""
    stderr: str = ""
    output_path: str = ""

    @property
    def ok(self) -> bool:
        return self.returncode == 0

    def to_dict(self) -> dict[str, Any]:
        data = asdict(self)
        data["ok"] = self.ok
        return data


@dataclass(frozen=True)
class KiCadSymbolExtractionRecord:
    """Extracted symbol plus its source metadata."""

    name: str
    source_path: str
    mode: str
    symbol: Any
    raw_fields: dict[str, str] = field(default_factory=dict)
    canonical_fields: dict[str, str] = field(default_factory=dict)
    diagnostics: tuple[str, ...] = ()

    def metadata_dict(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "source_path": self.source_path,
            "mode": self.mode,
            "raw_fields": dict(self.raw_fields),
            "canonical_fields": dict(self.canonical_fields),
            "diagnostics": list(self.diagnostics),
        }


@dataclass(frozen=True)
class KiCadFootprintExtractionRecord:
    """Extracted footprint plus its source metadata."""

    name: str
    library_link: str
    source_path: str
    source_reference: str
    mode: str
    footprint: KiCadFootprint
    raw_fields: dict[str, str] = field(default_factory=dict)
    canonical_fields: dict[str, str] = field(default_factory=dict)
    model_references: tuple[KiCadModelReferenceScan, ...] = ()
    diagnostics: tuple[str, ...] = ()

    def metadata_dict(self) -> dict[str, Any]:
        return {
            "name": self.name,
            "library_link": self.library_link,
            "source_path": self.source_path,
            "source_reference": self.source_reference,
            "mode": self.mode,
            "raw_fields": dict(self.raw_fields),
            "canonical_fields": dict(self.canonical_fields),
            "model_references": [ref.to_dict() for ref in self.model_references],
            "diagnostics": list(self.diagnostics),
        }


_MODEL_VAR_RE = re.compile(r"\$\{([^}]+)\}|%([^%]+)%|\$([A-Za-z_][A-Za-z0-9_]*)")
_ProgressCallback = Callable[[str], None]


def _emit_progress(progress: _ProgressCallback | None, message: str) -> None:
    if progress is not None:
        progress(message)


def _file_size_label(path: Path) -> str:
    try:
        size = path.stat().st_size
    except OSError:
        return "unknown size"
    if size >= 1_000_000:
        return f"{size / 1_000_000:.1f} MB"
    if size >= 1_000:
        return f"{size / 1_000:.1f} kB"
    return f"{size} B"


def _resolve_project_path(path: Path | str) -> Path:
    input_path = Path(path)
    if input_path.is_file() and input_path.suffix == ".kicad_pro":
        return input_path
    if input_path.is_file() and input_path.suffix in {".kicad_pcb", ".kicad_sch"}:
        adjacent = find_adjacent_kicad_project_path(input_path)
        if adjacent is not None:
            return adjacent
        raise FileNotFoundError(f"No adjacent .kicad_pro found for {input_path}")
    if input_path.is_dir():
        direct = sorted(input_path.glob("*.kicad_pro"))
        if direct:
            return direct[0]
        nested_input = input_path / "input"
        if nested_input.is_dir():
            nested = sorted(nested_input.glob("*.kicad_pro"))
            if nested:
                return nested[0]
    raise FileNotFoundError(f"KiCad project not found: {input_path}")


def _project_root(project_path: Path | str) -> Path:
    return _resolve_project_path(project_path).parent


def _stable_relative(path: Path, root: Path) -> str:
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return str(path)


def _iter_project_files(project_path: Path | str, suffix: str) -> list[Path]:
    root = _project_root(project_path)
    return sorted(
        path
        for path in root.rglob(f"*{suffix}")
        if not any(part in {"output", "review", "review_tmp"} for part in path.parts)
    )


def _dedupe_paths(paths: Iterable[Path]) -> tuple[Path, ...]:
    out: list[Path] = []
    seen: set[str] = set()
    for path in paths:
        key = str(path.resolve() if path.exists() else path)
        if key in seen:
            continue
        seen.add(key)
        out.append(path)
    return tuple(sorted(out))


def _project_stem_files(project_path: Path | str, suffix: str) -> tuple[Path, ...]:
    resolved_project = _resolve_project_path(project_path)
    root = resolved_project.parent
    candidate = root / f"{resolved_project.stem}{suffix}"
    if candidate.is_file():
        return (candidate,)
    return tuple(sorted(path for path in root.glob(f"*{suffix}") if path.is_file()))


def _resolve_project_local_table_uri(project_path: Path | str, uri: str) -> Path | None:
    root = _project_root(project_path).resolve()
    text = str(uri or "").strip()
    if not text:
        return None
    text = text.replace("\\", "/")
    if text.startswith("${KIPRJMOD}"):
        text = str(root) + text[len("${KIPRJMOD}"):]
    elif text.startswith("$KIPRJMOD"):
        text = str(root) + text[len("$KIPRJMOD"):]
    elif "${" in text or "$(" in text:
        return None
    path = Path(text)
    if not path.is_absolute():
        path = root / path
    resolved = path.resolve()
    try:
        resolved.relative_to(root)
    except ValueError:
        return None
    return resolved


def _project_local_symbol_libraries(
    project_path: Path | str,
    diagnostics: list[str],
) -> tuple[Path, ...]:
    root = _project_root(project_path)
    table_path = root / KiCadLibraryTableKind.SYMBOL.file_name
    try:
        table = KiCadLibraryTable.from_file(table_path, KiCadLibraryTableKind.SYMBOL)
    except Exception as exc:
        diagnostics.append(f"failed to parse symbol library table {table_path}: {exc}")
        return ()

    paths: list[Path] = []
    for entry in table.entries:
        if entry.disabled:
            continue
        resolved = _resolve_project_local_table_uri(project_path, entry.uri)
        if resolved is None:
            continue
        if resolved.is_file() and resolved.suffix == ".kicad_sym":
            paths.append(resolved)
        elif resolved.is_dir():
            paths.extend(path for path in resolved.glob("*.kicad_sym") if path.is_file())
        elif not resolved.exists():
            diagnostics.append(f"local symbol library does not exist: {resolved}")
    return _dedupe_paths(paths)


def _project_local_footprint_libraries(
    project_path: Path | str,
    diagnostics: list[str],
) -> tuple[tuple[Path, ...], tuple[Path, ...]]:
    root = _project_root(project_path)
    table_path = root / KiCadLibraryTableKind.FOOTPRINT.file_name
    try:
        table = KiCadLibraryTable.from_file(table_path, KiCadLibraryTableKind.FOOTPRINT)
    except Exception as exc:
        diagnostics.append(f"failed to parse footprint library table {table_path}: {exc}")
        return (), ()

    pretty_dirs: list[Path] = []
    footprint_files: list[Path] = []
    for entry in table.entries:
        if entry.disabled:
            continue
        resolved = _resolve_project_local_table_uri(project_path, entry.uri)
        if resolved is None:
            continue
        if resolved.is_dir() and resolved.suffix == ".pretty":
            pretty_dirs.append(resolved)
            footprint_files.extend(path for path in resolved.glob("*.kicad_mod") if path.is_file())
        elif resolved.is_file() and resolved.suffix == ".kicad_mod":
            footprint_files.append(resolved)
        elif not resolved.exists():
            diagnostics.append(f"local footprint library does not exist: {resolved}")
    return _dedupe_paths(pretty_dirs), _dedupe_paths(footprint_files)


def _project_asset_scope(project_path: Path | str) -> _ProjectAssetScope:
    diagnostics: list[str] = []
    pretty_libraries, footprint_files = _project_local_footprint_libraries(
        project_path,
        diagnostics,
    )
    return _ProjectAssetScope(
        schematics=_project_stem_files(project_path, ".kicad_sch"),
        pcbs=_project_stem_files(project_path, ".kicad_pcb"),
        symbol_libraries=_project_local_symbol_libraries(project_path, diagnostics),
        pretty_libraries=pretty_libraries,
        footprint_files=footprint_files,
        diagnostics=tuple(diagnostics),
    )


def _library_member_name(name: str) -> str:
    return str(name).split(":", 1)[1] if ":" in str(name) else str(name)


def _symbol_member_name(symbol: Any) -> str:
    return _library_member_name(str(getattr(symbol, "name", "")))


def _safe_asset_filename(name: str) -> str:
    clean = _library_member_name(name)
    for char in r'<>:"/\|?*':
        clean = clean.replace(char, "_")
    for char in " \t\n\r":
        clean = clean.replace(char, "_")
    return clean or "unnamed"


def _mime_type_for_suffix(suffix: str) -> str:
    normalized = suffix.lower()
    return {
        ".bmp": "image/bmp",
        ".gif": "image/gif",
        ".jpeg": "image/jpeg",
        ".jpg": "image/jpeg",
        ".otf": "font/otf",
        ".png": "image/png",
        ".step": "model/step",
        ".stp": "model/step",
        ".svg": "image/svg+xml",
        ".ttf": "font/ttf",
        ".wrl": "model/vrml",
    }.get(normalized, "application/octet-stream")


def _payload_suffix_and_mime(data: bytes, fallback_suffix: str = "") -> tuple[str, str]:
    if data.startswith(b"\x89PNG\r\n\x1a\n"):
        return ".png", "image/png"
    if data.startswith(b"\xff\xd8\xff"):
        return ".jpg", "image/jpeg"
    if data.startswith(b"GIF87a") or data.startswith(b"GIF89a"):
        return ".gif", "image/gif"
    if data.startswith(b"BM"):
        return ".bmp", "image/bmp"
    stripped = data[:256].lstrip().lower()
    if stripped.startswith(b"<svg") or stripped.startswith(b"<?xml"):
        return ".svg", "image/svg+xml"
    if b"iso-10303-21" in data[:512].lower():
        return ".step", "model/step"
    suffix = fallback_suffix.lower()
    if suffix:
        return suffix, _mime_type_for_suffix(suffix)
    return ".bin", "application/octet-stream"


def _decode_plain_base64_payload(value: str, *, label: str) -> bytes:
    try:
        return base64.b64decode(value)
    except Exception as exc:
        raise ValueError(f"failed to decode embedded asset {label}: {exc}") from exc


def _asset_filename_with_detected_suffix(source_name: str, data: bytes, fallback_stem: str) -> tuple[str, str]:
    source_path = Path(source_name)
    suffix, mime_type = _payload_suffix_and_mime(data, source_path.suffix)
    if source_path.suffix:
        filename = _safe_asset_filename(source_name)
    else:
        filename = f"{_safe_asset_filename(source_name or fallback_stem)}{suffix}"
    return filename, mime_type


def _asset_output_path(
    output_dir: Path,
    filename: str,
    used_names_by_dir: dict[Path, set[str]],
) -> Path:
    output_dir.mkdir(parents=True, exist_ok=True)
    used_names = used_names_by_dir.setdefault(output_dir, set())
    return output_dir / _unique_file_name(filename, used_names)


def _write_embedded_asset_payload(
    *,
    data: bytes,
    output_dir: Path,
    subdir: str,
    fallback_stem: str,
    kind: str,
    source_kind: str,
    source_path: Path,
    owner: str,
    source_name: str,
    file_type: str = "",
    dedupe_payloads: bool,
    written: dict[str, Path],
    used_names_by_dir: dict[Path, set[str]],
) -> KiCadEmbeddedAssetRecord:
    digest = hashlib.sha256(data).hexdigest()
    filename, mime_type = _asset_filename_with_detected_suffix(source_name, data, fallback_stem)
    key = f"sha256:{digest}" if dedupe_payloads else f"{source_kind}:{owner}:{source_name}:{digest}"
    deduplicated = key in written
    if deduplicated:
        path = written[key]
    else:
        path = _asset_output_path(output_dir / subdir, filename, used_names_by_dir)
        path.write_bytes(data)
        written[key] = path
    return KiCadEmbeddedAssetRecord(
        kind=kind,
        source_kind=source_kind,
        source_path=str(source_path),
        owner=owner,
        source_name=source_name,
        output_file=str(path),
        sha256=digest,
        byte_size=len(data),
        mime_type=mime_type,
        file_type=file_type,
        deduplicated=deduplicated,
    )


def _symbol_fields(symbol: Any) -> dict[str, str]:
    return {
        str(getattr(prop, "key", "")): str(getattr(prop, "value", ""))
        for prop in getattr(symbol, "properties", ()) or ()
        if getattr(prop, "key", "")
    }


def _is_power_symbol(symbol: Any, fields: dict[str, str] | None = None) -> bool:
    if bool(getattr(symbol, "power", False)):
        return True
    symbol_fields = fields if fields is not None else _symbol_fields(symbol)
    return symbol_fields.get("Reference") == "#PWR"


def _footprint_fields(footprint: Any) -> dict[str, str]:
    return {
        str(getattr(prop, "name", "")): str(getattr(prop, "value", ""))
        for prop in getattr(footprint, "properties", ()) or ()
        if getattr(prop, "name", "")
    }


def _canonical_identity_fields(fields: dict[str, str]) -> dict[str, str]:
    return canonicalize_part_parameters(fields)


def _embedded_name(path: str) -> str:
    return path.removeprefix("kicad-embed://")


def _embedded_file_map(files: Iterable[EmbeddedFile]) -> dict[str, EmbeddedFile]:
    out: dict[str, EmbeddedFile] = {}
    for file in files:
        if not getattr(file, "name", ""):
            continue
        existing = out.get(file.name)
        if existing is None or (not existing.data and file.data):
            out[file.name] = file
    return out


def _embedded_file_has_payload(file: EmbeddedFile | None) -> bool:
    return bool(file is not None and file.data)


def _find_matching_paren(text: str, start: int) -> int:
    depth = 0
    in_string = False
    escaped = False
    for index in range(start, len(text)):
        char = text[index]
        if in_string:
            if escaped:
                escaped = False
            elif char == "\\":
                escaped = True
            elif char == '"':
                in_string = False
            continue
        if char == '"':
            in_string = True
        elif char == "(":
            depth += 1
        elif char == ")":
            depth -= 1
            if depth == 0:
                return index
    raise ValueError("unbalanced S-expression while scanning embedded file")


def _read_kicad_text(path: Path) -> str:
    try:
        return path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return path.read_text(encoding="utf-8-sig")


def _iter_embedded_files_from_text(text: str) -> Iterable[EmbeddedFile]:
    for match in re.finditer(r"\(\s*file(?=\s|\))", text):
        start = match.start()
        try:
            end = _find_matching_paren(text, start)
            sexp = parse_sexp(text[start : end + 1])
            if isinstance(sexp, list) and sexp and sexp[0] == "file":
                yield EmbeddedFile.from_sexp(sexp)
        except Exception:
            continue


def _iter_embedded_files_from_file(path: Path) -> Iterable[EmbeddedFile]:
    yield from _iter_embedded_files_from_text(_read_kicad_text(path))


def _unescape_footprint_link(value: str) -> str:
    return (
        value
        .replace(r"\\", "\\")
        .replace(r"\"", '"')
        .replace(r"\n", "\n")
        .replace(r"\r", "\r")
        .replace(r"\t", "\t")
    )


def _footprint_library_link_from_match(match: re.Match[str]) -> str:
    quoted = match.group(2)
    bare = match.group(3)
    return _unescape_footprint_link(quoted) if quoted is not None else str(bare or "")


def _iter_board_footprints_from_file(
    path: Path,
    *,
    unique_library_links: set[str] | None = None,
) -> Iterable[Footprint]:
    text = _read_kicad_text(path)
    for match in _FOOTPRINT_START_RE.finditer(text):
        library_link = _footprint_library_link_from_match(match)
        if (
            unique_library_links is not None
            and library_link
            and library_link in unique_library_links
        ):
            continue
        if unique_library_links is not None and library_link:
            unique_library_links.add(library_link)
        start = match.start(1)
        end = _find_matching_paren(text, start)
        footprint = Footprint.from_sexp(parse_sexp(text[start:end + 1], source_path=path))
        yield footprint


def _iter_schematic_lib_symbols_from_file(path: Path) -> Iterable[LibSymbol]:
    sexp = parse_sexp(_read_kicad_text(path))
    for item in sexp:
        if not isinstance(item, list) or not item or item[0] != "lib_symbols":
            continue
        for child in item[1:]:
            if isinstance(child, list) and child and child[0] == "symbol":
                yield LibSymbol.from_sexp(child)


def _iter_schematic_placed_symbols_from_file(path: Path) -> Iterable[SchSymbol]:
    sexp = parse_sexp(_read_kicad_text(path))
    for item in sexp:
        if not isinstance(item, list) or not item or item[0] != "symbol":
            continue
        if find_element(item, "lib_id") is None:
            continue
        yield SchSymbol.from_sexp(item)


def _project_symbol_footprint_overrides(project_path: Path | str) -> dict[str, str]:
    """Return consistent placed-symbol footprint overrides keyed by lib id/member."""
    candidates: dict[str, set[str]] = {}
    for schematic_path in _iter_project_files(project_path, ".kicad_sch"):
        for symbol in _iter_schematic_placed_symbols_from_file(schematic_path):
            lib_id = str(getattr(symbol, "lib_id", "") or "")
            footprint = symbol.get_property_value(StandardPropertyKey.FOOTPRINT, "")
            if not lib_id or not footprint:
                continue
            candidates.setdefault(lib_id, set()).add(footprint)

    out: dict[str, str] = {}
    for lib_id, footprints in candidates.items():
        if len(footprints) != 1:
            continue
        footprint = next(iter(footprints))
        out[lib_id] = footprint
        out.setdefault(_library_member_name(lib_id), footprint)
    return out


def _is_step_model_name(name: str) -> bool:
    return Path(name).suffix.lower() in {".step", ".stp"}


def _embedded_file_payload_bytes(file: EmbeddedFile) -> bytes:
    try:
        compressed = base64.b64decode(file.data)
    except Exception as exc:
        raise ValueError(f"failed to decode embedded file {file.name}: {exc}") from exc
    if _zstandard is None:
        raise RuntimeError("zstd support is unavailable; install 'zstandard'")
    try:
        return _zstandard.ZstdDecompressor().decompress(compressed)
    except Exception as exc:
        try:
            with _zstandard.ZstdDecompressor().stream_reader(compressed) as reader:
                return reader.read()
        except Exception as stream_exc:
            raise ValueError(
                f"failed to decompress embedded file {file.name}: {stream_exc}"
            ) from exc


def _compress_embedded_file_payload(data: bytes) -> str:
    if _zstandard is None:
        raise RuntimeError("zstd support is unavailable; install 'zstandard'")
    compressed = _zstandard.ZstdCompressor().compress(data)
    return base64.b64encode(compressed).decode("ascii")


def _embedded_file_from_path(path: Path, *, name: str | None = None) -> EmbeddedFile:
    payload = path.read_bytes()
    return EmbeddedFile(
        name=name or path.name,
        file_type="model",
        data=_compress_embedded_file_payload(payload),
        checksum=hashlib.sha256(payload).hexdigest(),
    )


@dataclass(frozen=True)
class _ModelSearch:
    """Resolution inputs for KiCad 3D model references.

    ``variables`` maps path-variable names to directories.  ``fallback_model_dir``
    is the highest installed KiCad ``3dmodels`` directory, used when a
    ``KICAD<major>_3DMODEL_DIR`` reference names a version that is not installed.
    ``search_dirs`` are explicit override roots tried (by the path tail) for any
    leading-variable reference.  ``resolve_wrl_siblings`` lets a ``.wrl`` reference
    fall back to its sibling ``.step``/``.stp`` model.
    """

    variables: dict[str, str] = field(default_factory=dict)
    fallback_model_dir: Path | None = None
    search_dirs: tuple[Path, ...] = ()
    resolve_wrl_siblings: bool = True


@dataclass(frozen=True)
class _ModelFileResolution:
    """Outcome of resolving one model reference to a concrete file."""

    path: Path | None
    exists: bool
    via_sibling: bool = False
    var_unresolved: bool = False


def _build_model_search(
    env: dict[str, str] | None = None,
    *,
    model_search_dirs: Sequence[Path] | None = None,
    use_installed_kicad: bool = True,
    resolve_wrl_siblings: bool = True,
) -> _ModelSearch:
    """Build a model resolver from installed KiCad plus explicit caller input.

    Variable precedence (low to high): installed-KiCad model-dir defaults,
    ``kicad_common.json`` overrides, the process environment, then ``env``.  This
    lets project-lib and health resolve ``${KICAD<major>_3DMODEL_DIR}`` references
    even when KiCad is not the running process, while still honouring any variable
    the caller or user has set explicitly.  Search dirs are tried in order:
    explicit ``model_search_dirs`` first, then KiCad's configured
    ``system.extra_3d_search_dirs``.
    """
    variables: dict[str, str] = {}
    fallback_model_dir: Path | None = None
    config_search_dirs: list[Path] = []
    if use_installed_kicad:
        try:
            kicad_env = KiCadEnvironment()
            variables.update(kicad_env.path_variable_map())
            config_search_dirs.extend(kicad_env.extra_3d_model_search_dirs())
            highest = kicad_env.highest_installation()
            if highest is not None:
                model_dir = highest.root / "share" / "kicad" / "3dmodels"
                if model_dir.exists():
                    fallback_model_dir = model_dir
        except Exception:
            pass
    variables.update(os.environ)
    if env:
        variables.update(env)
    explicit_search_dirs = [Path(path) for path in (model_search_dirs or ())]
    return _ModelSearch(
        variables=variables,
        fallback_model_dir=fallback_model_dir,
        search_dirs=tuple(explicit_search_dirs + config_search_dirs),
        resolve_wrl_siblings=resolve_wrl_siblings,
    )


def _expand_model_path(model_path: str, *, project_root: Path, env: dict[str, str]) -> tuple[Path | None, bool]:
    unresolved = False

    def replace_var(match: re.Match[str]) -> str:
        nonlocal unresolved
        name = next(group for group in match.groups() if group is not None)
        value = str(project_root) if name == "KIPRJMOD" else env.get(name)
        if value is None:
            unresolved = True
            return match.group(0)
        return value

    expanded = _MODEL_VAR_RE.sub(replace_var, model_path)
    if unresolved:
        return None, True
    path = Path(expanded)
    if not path.is_absolute() and expanded:
        path = project_root / path
    return path, False


_LEADING_VAR_RE = re.compile(r"^\$\{[^}]+\}[\\/]?(.*)$")
_KICAD_ENV_PREFIX_RE = re.compile(r"^\$\{KICAD\d+_3DMODEL_DIR\}")


def _is_kicad_env_model(model_path: str) -> bool:
    return bool(_KICAD_ENV_PREFIX_RE.match(model_path))


def _leading_var_tail(model_path: str) -> str | None:
    match = _LEADING_VAR_RE.match(model_path)
    return match.group(1) if match else None


def _existing_model_file(path: Path, *, resolve_wrl_siblings: bool) -> tuple[Path | None, bool]:
    """Return (path, via_sibling) for an existing model file, else (None, False).

    For a ``.wrl`` reference the sibling ``.step``/``.stp`` is preferred when present
    (the tool embeds STEP), falling back to the literal ``.wrl`` only when no STEP
    sibling exists.
    """
    if resolve_wrl_siblings and path.suffix.lower() == ".wrl":
        for sibling_suffix in (".step", ".stp"):
            sibling = path.with_suffix(sibling_suffix)
            if sibling.exists():
                return sibling, True
    if path.exists():
        return path, False
    return None, False


def _resolve_model_file(
    model_path: str,
    *,
    project_root: Path,
    search: _ModelSearch,
) -> _ModelFileResolution:
    """Resolve a model reference to a concrete file using the search ladder.

    Order: the primary variable expansion (matching install for KICAD env refs),
    then the highest-install fallback directory for ``KICAD<major>_3DMODEL_DIR``
    references, then any explicit override ``search_dirs`` (by path tail).  An
    embeddable STEP hit (including a ``.wrl`` reference's sibling ``.step``/``.stp``)
    is preferred over a non-STEP hit across all candidate directories; otherwise the
    first existing file wins.
    """
    primary, unresolved = _expand_model_path(
        model_path, project_root=project_root, env=search.variables
    )
    candidates: list[Path] = []
    if primary is not None:
        candidates.append(primary)
    tail = _leading_var_tail(model_path)
    if tail:
        if _is_kicad_env_model(model_path) and search.fallback_model_dir is not None:
            candidates.append(search.fallback_model_dir / tail)
        candidates.extend(extra / tail for extra in search.search_dirs)
    non_step_hit: tuple[Path, bool] | None = None
    for candidate in candidates:
        found, via_sibling = _existing_model_file(
            candidate, resolve_wrl_siblings=search.resolve_wrl_siblings
        )
        if found is None:
            continue
        if found.suffix.lower() in {".step", ".stp"}:
            return _ModelFileResolution(path=found, exists=True, via_sibling=via_sibling)
        if non_step_hit is None:
            non_step_hit = (found, via_sibling)
    if non_step_hit is not None:
        return _ModelFileResolution(path=non_step_hit[0], exists=True, via_sibling=non_step_hit[1])
    if candidates:
        return _ModelFileResolution(path=candidates[0], exists=False)
    return _ModelFileResolution(path=None, exists=False, var_unresolved=unresolved)


def _unique_embedded_name(source_name: str, existing: set[str]) -> str:
    stem = _safe_asset_filename(Path(source_name).stem)
    suffix = Path(source_name).suffix or ".step"
    candidate = f"{stem}{suffix}"
    index = 1
    while candidate in existing:
        index += 1
        candidate = f"{stem}_{index}{suffix}"
    existing.add(candidate)
    return candidate


def _classify_model_path(
    model_path: str,
    *,
    project_root: Path,
    embedded_files: dict[str, EmbeddedFile],
    board_embedded_files: dict[str, EmbeddedFile] | None = None,
    search: _ModelSearch,
) -> tuple[KiCadModelReferenceKind, str, bool, str, bool, str, tuple[str, ...]]:
    diagnostics: list[str] = []
    if model_path.startswith("kicad-embed://"):
        name = _embedded_name(model_path)
        local_file = embedded_files.get(name)
        if _embedded_file_has_payload(local_file):
            return (
                KiCadModelReferenceKind.EMBEDDED,
                "",
                False,
                name,
                True,
                "footprint",
                (),
            )
        board_file = board_embedded_files.get(name) if board_embedded_files else None
        if _embedded_file_has_payload(board_file):
            return (
                KiCadModelReferenceKind.EMBEDDED,
                "",
                False,
                name,
                True,
                "board",
                (),
            )
        diagnostics.append(f"missing embedded payload for {name}")
        return (
            KiCadModelReferenceKind.EMBEDDED,
            "",
            False,
            name,
            False,
            "",
            tuple(diagnostics),
        )

    if model_path.startswith("${KICAD") and "_3DMODEL_DIR}" in model_path:
        kind = KiCadModelReferenceKind.KICAD_ENV
    elif _MODEL_VAR_RE.search(model_path):
        kind = KiCadModelReferenceKind.ENV_VAR
    elif Path(model_path).is_absolute():
        kind = KiCadModelReferenceKind.ABSOLUTE
    elif model_path:
        kind = KiCadModelReferenceKind.PROJECT_RELATIVE
    else:
        kind = KiCadModelReferenceKind.OTHER

    resolution = _resolve_model_file(model_path, project_root=project_root, search=search)
    if resolution.path is None:
        diagnostics.append("unresolved environment variable")
        return kind, model_path, False, "", False, "", tuple(diagnostics)
    if not resolution.exists:
        diagnostics.append("model path does not exist")
    elif resolution.via_sibling:
        diagnostics.append("resolved via sibling model file")
    return kind, str(resolution.path), resolution.exists, "", False, "", tuple(diagnostics)


def _scan_models_for_owner(
    *,
    source_kind: str,
    source_path: Path,
    owner: str,
    models: Iterable[Model],
    embedded_files: Iterable[EmbeddedFile],
    project_root: Path,
    board_embedded_files: Iterable[EmbeddedFile] = (),
    search: _ModelSearch,
) -> list[KiCadModelReferenceScan]:
    embedded_map = _embedded_file_map(embedded_files)
    board_embedded_map = _embedded_file_map(board_embedded_files)
    records: list[KiCadModelReferenceScan] = []
    for model in models:
        (
            kind,
            resolved_path,
            exists,
            embedded_name,
            has_payload,
            payload_scope,
            diagnostics,
        ) = _classify_model_path(
            model.path,
            project_root=project_root,
            embedded_files=embedded_map,
            board_embedded_files=board_embedded_map,
            search=search,
        )
        records.append(KiCadModelReferenceScan(
            source_kind=source_kind,
            source_path=str(source_path),
            owner=owner,
            model_path=model.path,
            reference_kind=kind.value,
            resolved_path=resolved_path,
            exists=exists,
            embedded_name=embedded_name,
            has_embedded_payload=has_payload,
            payload_scope=payload_scope,
            diagnostics=diagnostics,
        ))
    return records


def _footprint_file_owner_from_text(path: Path, text: str) -> str:
    match = _FOOTPRINT_START_RE.search(text)
    if match is None:
        return path.stem
    return _library_member_name(_footprint_library_link_from_match(match)) or path.stem


def _scan_footprint_file_models(
    path: Path,
    *,
    project_root: Path,
    search: _ModelSearch,
) -> list[KiCadModelReferenceScan]:
    text = _read_kicad_text(path)
    owner = _footprint_file_owner_from_text(path, text)
    models: list[Model] = []
    embedded_files: list[EmbeddedFile] = []
    selector = SexpSelector(
        heads={"embedded_files", "model"},
        min_depth=1,
        max_depth=1,
    )
    for span in iter_sexp_form_spans(text, selector, source_path=path):
        sexp = span.parse()
        if not isinstance(sexp, list) or not sexp:
            continue
        if sexp[0] == "model":
            models.append(Model.from_sexp(sexp))
        elif sexp[0] == "embedded_files":
            for child in sexp[1:]:
                if isinstance(child, list) and child and child[0] == "file":
                    embedded_files.append(EmbeddedFile.from_sexp(child))
    return _scan_models_for_owner(
        source_kind="footprint_library",
        source_path=path,
        owner=owner,
        models=models,
        embedded_files=embedded_files,
        project_root=project_root,
        search=search,
    )


def _scan_3d_models_with_diagnostics(
    project_path: Path | str,
    *,
    progress: _ProgressCallback | None = None,
    scope: _ProjectAssetScope | None = None,
    search: _ModelSearch | None = None,
) -> tuple[
    tuple[KiCadModelReferenceScan, ...],
    tuple[KiCadFootprintModelCoverage, ...],
    tuple[str, ...],
]:
    resolved_project = _resolve_project_path(project_path)
    root = resolved_project.parent
    if search is None:
        search = _build_model_search()
    active_scope = scope if scope is not None else _project_asset_scope(resolved_project)
    records: list[KiCadModelReferenceScan] = []
    footprints_without_models: dict[tuple[str, str], list[str]] = {}
    diagnostics: list[str] = list(active_scope.diagnostics)

    pcb_paths = list(active_scope.pcbs)
    for pcb_index, pcb_path in enumerate(pcb_paths, start=1):
        _emit_progress(
            progress,
            "parsing PCB "
            f"{pcb_index}/{len(pcb_paths)}: "
            f"{_stable_relative(pcb_path, root)} ({_file_size_label(pcb_path)})",
        )
        try:
            projection = KiCadPcbProjection.from_file(pcb_path)
            footprints = projection.footprints()
            board_embedded_files = tuple(projection.embedded_files())
        except Exception as exc:
            diagnostics.append(f"failed to parse PCB {pcb_path}: {exc}")
            continue
        _emit_progress(
            progress,
            "scanning PCB footprints: "
            f"{len(footprints)} placed footprints, "
            f"{len(board_embedded_files)} board embedded files",
        )
        for footprint in footprints:
            owner = getattr(footprint, "library_link", "") or getattr(footprint, "reference", "")
            models = getattr(footprint, "models", ()) or ()
            if not models:
                reference = footprint.get_property_value("Reference", "") or getattr(
                    footprint,
                    "uuid",
                    "",
                )
                footprints_without_models.setdefault(
                    (str(pcb_path), owner or "<unknown footprint>"),
                    [],
                ).append(reference or "<unnamed>")
                continue
            records.extend(_scan_models_for_owner(
                source_kind="pcb_footprint",
                source_path=pcb_path,
                owner=owner,
                models=models,
                embedded_files=getattr(footprint, "embedded_files", ()) or (),
                project_root=root,
                board_embedded_files=board_embedded_files,
                search=search,
            ))

    fp_paths = list(active_scope.footprint_files)
    if fp_paths:
        _emit_progress(
            progress,
            f"scanning table-listed footprint library files: {len(fp_paths)} files",
        )
    for fp_index, fp_path in enumerate(fp_paths, start=1):
        _emit_progress(
            progress,
            "parsing footprint "
            f"{fp_index}/{len(fp_paths)}: "
            f"{_stable_relative(fp_path, root)} ({_file_size_label(fp_path)})",
        )
        try:
            records.extend(_scan_footprint_file_models(fp_path, project_root=root, search=search))
        except Exception as exc:
            diagnostics.append(f"failed to parse footprint {fp_path}: {exc}")
            continue
    no_model_records = [
        KiCadFootprintModelCoverage(
            footprint=footprint,
            source_path=source_path,
            designators=tuple(sorted(designators)),
            instance_count=len(designators),
        )
        for (source_path, footprint), designators in sorted(footprints_without_models.items())
    ]
    return tuple(records), tuple(no_model_records), tuple(diagnostics)


def scan_3d_models(
    project_path: Path | str,
    *,
    env: dict[str, str] | None = None,
    model_search_dirs: Sequence[Path] | None = None,
    use_installed_kicad: bool = True,
) -> tuple[KiCadModelReferenceScan, ...]:
    """Scan PCB and local footprint-library model references.

    Resolution of external ``${KICAD<major>_3DMODEL_DIR}`` and custom path-variable
    references uses installed KiCad defaults (see :func:`_build_model_search`).
    Pass ``model_search_dirs`` to add explicit model roots, or
    ``use_installed_kicad=False`` to skip installation auto-detection.
    """
    search = _build_model_search(
        env,
        model_search_dirs=model_search_dirs,
        use_installed_kicad=use_installed_kicad,
    )
    records, _footprints_without_models, _diagnostics = _scan_3d_models_with_diagnostics(
        project_path,
        search=search,
    )
    return records


def scan_project_assets(
    project_path: Path | str,
    *,
    progress: _ProgressCallback | None = None,
    env: dict[str, str] | None = None,
    model_search_dirs: Sequence[Path] | None = None,
    use_installed_kicad: bool = True,
) -> KiCadProjectAssetScan:
    """Return a structured inventory of a KiCad project's local assets.

    External model references are resolved against installed KiCad defaults plus
    any explicit ``model_search_dirs``; ``use_installed_kicad=False`` disables
    installation auto-detection for deterministic, KiCad-free runs.
    """
    resolved_project = _resolve_project_path(project_path)
    root = resolved_project.parent
    search = _build_model_search(
        env,
        model_search_dirs=model_search_dirs,
        use_installed_kicad=use_installed_kicad,
    )
    _emit_progress(progress, f"inventorying project files and local library tables under {root}")
    scope = _project_asset_scope(resolved_project)
    schematics = tuple(str(path) for path in scope.schematics)
    pcbs = tuple(str(path) for path in scope.pcbs)
    symbol_libraries = tuple(str(path) for path in scope.symbol_libraries)
    pretty_libraries = tuple(str(path) for path in scope.pretty_libraries)
    footprint_files = tuple(str(path) for path in scope.footprint_files)
    _emit_progress(
        progress,
        "inventory complete: "
        f"{len(schematics)} schematics, {len(pcbs)} PCBs, "
        f"{len(symbol_libraries)} symbol libs, {len(pretty_libraries)} pretty libs, "
        f"{len(footprint_files)} footprint files",
    )
    model_references, footprints_without_models, diagnostics = _scan_3d_models_with_diagnostics(
        resolved_project,
        progress=progress,
        scope=scope,
        search=search,
    )
    _emit_progress(
        progress,
        "asset scan complete: "
        f"{len(model_references)} model refs, "
        f"{len(footprints_without_models)} footprint groups without models",
    )
    return KiCadProjectAssetScan(
        project_path=str(resolved_project),
        project_root=str(root),
        schematics=schematics,
        pcbs=pcbs,
        symbol_libraries=symbol_libraries,
        pretty_libraries=pretty_libraries,
        footprint_files=footprint_files,
        model_references=model_references,
        footprints_without_models=footprints_without_models,
        diagnostics=diagnostics,
    )


def embed_external_model_payloads(
    footprint: KiCadFootprint,
    *,
    project_root: Path | str,
    env: dict[str, str] | None = None,
    model_search_dirs: Sequence[Path] | None = None,
    use_installed_kicad: bool = True,
    search: _ModelSearch | None = None,
) -> KiCadFootprint:
    """Return a footprint with resolvable external STEP/STP models embedded.

    Existing ``kicad-embed://`` model references are preserved. Resolvable
    external STEP/STP paths are copied into the footprint's ``embedded_files``
    block and their model paths are rewritten to ``kicad-embed://...``.

    References are resolved through the installed-KiCad search ladder, so
    ``${KICAD<major>_3DMODEL_DIR}`` models embed even when the variable is not set
    in the environment. A ``.wrl`` reference embeds its sibling ``.step``/``.stp``
    model when present. Pass ``model_search_dirs`` to add explicit roots or
    ``use_installed_kicad=False`` to skip installation auto-detection.
    """
    if search is None:
        search = _build_model_search(
            env,
            model_search_dirs=model_search_dirs,
            use_installed_kicad=use_installed_kicad,
        )
    out = copy.deepcopy(footprint)
    local = _embedded_file_map(out.embedded_files)
    existing_names = set(local)
    root = Path(project_root)

    for model in out.models:
        if model.path.startswith("kicad-embed://"):
            continue
        resolution = _resolve_model_file(model.path, project_root=root, search=search)
        found = resolution.path if resolution.exists else None
        if found is None or not _is_step_model_name(found.name):
            continue
        embedded_name = _unique_embedded_name(found.name, existing_names)
        out.embedded_files.append(_embedded_file_from_path(found, name=embedded_name))
        model.path = f"kicad-embed://{embedded_name}"
    return out


def strip_symbol_metadata(
    symbol: Any,
    policy: KiCadExtractionMode | str = KiCadExtractionMode.INTERNAL,
) -> Any:
    """Return a copy of ``symbol`` stripped according to the extraction policy."""
    mode = KiCadExtractionMode(policy)
    stripped = copy.deepcopy(symbol)
    if mode == KiCadExtractionMode.PROJECT_LOCAL:
        return stripped

    keep_keys = {StandardPropertyKey.REFERENCE, StandardPropertyKey.VALUE}
    keep_key_values = {key.value for key in keep_keys}

    def should_keep_property(prop: Any) -> bool:
        key = str(getattr(prop, "key", "") or "")
        if key:
            return key in keep_key_values
        return getattr(prop, "id", None) in {PropertyId.REFERENCE, PropertyId.VALUE}

    stripped.properties = [
        prop for prop in getattr(stripped, "properties", []) or ()
        if should_keep_property(prop)
    ]
    stripped.in_bom = True
    stripped.on_board = True
    stripped.exclude_from_sim = False
    return stripped


def _symbol_fingerprint(symbol: Any) -> str:
    lib = KiCadSymbolLib(symbols=[copy.deepcopy(symbol)])
    return hashlib.sha256(lib.to_text().encode("utf-8")).hexdigest()


def _symbol_record_key(
    *,
    symbol: Any,
    source_path: Path,
    policy: KiCadExtractionMode,
    dedupe_policy: KiCadExtractionDedupePolicy,
) -> str:
    if policy == KiCadExtractionMode.PROJECT_LOCAL:
        return _symbol_member_name(symbol)
    if dedupe_policy == KiCadExtractionDedupePolicy.FINGERPRINT:
        return _symbol_fingerprint(strip_symbol_metadata(symbol, policy))
    return str(getattr(symbol, "name", ""))


_GENERATED_ALIAS_MEMBER_RE = re.compile(r"^(?P<base>.+)_\d+$")


def _generated_symbol_alias_base_member(symbol: Any, known_members: set[str]) -> str | None:
    """Return the base member for generated duplicate symbol aliases.

    KiCad schematic ``lib_symbols`` can contain generated aliases such as
    ``KSZ9896CTXC_1`` next to a base ``KSZ9896CTXC`` symbol.  Those are not
    separate reusable library items for project-local folder libraries.
    """
    member = _symbol_member_name(symbol)
    match = _GENERATED_ALIAS_MEMBER_RE.match(member)
    if match is None:
        return None
    base = match.group("base")
    if base not in known_members:
        return None
    get_value = getattr(symbol, "get_property_value", None)
    value = (
        str(get_value(StandardPropertyKey.VALUE, ""))
        if callable(get_value)
        else str(getattr(symbol, "value", ""))
    )
    return base if value == base else None


def _rename_symbol_for_folder_library(symbol: Any, member_name: str) -> Any:
    renamed = copy.deepcopy(symbol)
    old_names = {
        str(getattr(renamed, "name", "")),
        _library_member_name(str(getattr(renamed, "name", ""))),
    }
    renamed.name = member_name
    for subsymbol in getattr(renamed, "subsymbols", ()) or ():
        current = str(getattr(subsymbol, "name", ""))
        for old_name in sorted(old_names, key=len, reverse=True):
            if not old_name:
                continue
            if current == old_name:
                subsymbol.name = member_name
                break
            if current.startswith(f"{old_name}_"):
                subsymbol.name = f"{member_name}{current[len(old_name):]}"
                break
    return renamed


def build_footprint_library_link_map(
    records: Iterable[KiCadFootprintExtractionRecord],
) -> dict[str, str]:
    """Return lookup keys that map original footprint refs to output members."""
    out: dict[str, str] = {}
    for record in records:
        member = _library_member_name(record.name)
        keys = {
            record.name,
            member,
            record.library_link,
            _library_member_name(record.library_link),
            getattr(record.footprint, "name", ""),
            _library_member_name(str(getattr(record.footprint, "name", ""))),
        }
        for key in keys:
            key_text = str(key or "")
            if key_text:
                out.setdefault(key_text, member)
    return out


def _local_footprint_link(
    value: str,
    *,
    footprint_library_nickname: str,
    footprint_name_map: dict[str, str],
) -> str:
    if not value:
        return value
    if value.startswith(f"{footprint_library_nickname}:"):
        return value
    member = footprint_name_map.get(value) or footprint_name_map.get(_library_member_name(value))
    if member is None:
        return value
    return f"{footprint_library_nickname}:{member}"


def _relink_symbol_footprint_property(
    symbol: Any,
    *,
    footprint_library_nickname: str | None,
    footprint_name_map: dict[str, str] | None,
) -> Any:
    if not footprint_library_nickname or not footprint_name_map:
        return symbol
    relinked = copy.deepcopy(symbol)
    get_value = getattr(relinked, "get_property_value", None)
    set_value = getattr(relinked, "set_property_value", None)
    if not callable(get_value) or not callable(set_value):
        return relinked
    old_value = str(get_value(StandardPropertyKey.FOOTPRINT, ""))
    new_value = _local_footprint_link(
        old_value,
        footprint_library_nickname=footprint_library_nickname,
        footprint_name_map=footprint_name_map,
    )
    if new_value != old_value:
        set_value(StandardPropertyKey.FOOTPRINT, new_value)
    return relinked


def _normalise_standalone_footprint_name(footprint: KiCadFootprint, name: str) -> None:
    footprint.name = _library_member_name(name)


def _normalise_angle_degrees(angle: float) -> float:
    normalised = float(angle) % 360.0
    if abs(normalised) < 1e-9 or abs(normalised - 360.0) < 1e-9:
        return 0.0
    rounded = round(normalised)
    if abs(normalised - rounded) < 1e-9:
        return float(rounded)
    return normalised


def _normalise_standalone_footprint_orientations(
    standalone: KiCadFootprint,
    source: Footprint,
) -> None:
    """Convert board-instance child orientations to footprint-library orientations.

    KiCad's footprint library export clones the board footprint and calls
    ``FOOTPRINT::SetOrientation( ANGLE_0 )`` before writing it.  Pads store board
    absolute orientation while their position is footprint-relative, so a rotated
    placed footprint must have the parent angle removed before the standalone
    footprint is written.
    """

    footprint_angle = float(source.at_angle or 0.0)
    if _normalise_angle_degrees(footprint_angle) == 0.0:
        return

    for item in (
        *standalone.pads,
        *standalone.properties,
        *standalone.fp_texts,
    ):
        if hasattr(item, "at_angle"):
            item.at_angle = _normalise_angle_degrees(item.at_angle - footprint_angle)

    for text_box in standalone.fp_text_boxes:
        text_box.angle = _normalise_angle_degrees(text_box.angle - footprint_angle)


def _omit_library_export_footprint_uuid(standalone: KiCadFootprint) -> None:
    standalone.uuid = None


def _normalise_standalone_footprint_for_library_export(
    standalone: KiCadFootprint,
    source: Footprint,
) -> None:
    standalone.placed = False
    standalone.set_property_value("Reference", "REF**", create=True)
    _strip_pad_instance_metadata(standalone, reset_uuid=False)
    _omit_library_export_footprint_uuid(standalone)
    _normalise_standalone_footprint_orientations(standalone, source)


def _strip_pad_instance_metadata(
    footprint: KiCadFootprint,
    *,
    reset_uuid: bool = True,
) -> None:
    for pad in footprint.pads:
        pad.net = NetRef()
        pad.pinfunction = None
        pad.pintype = None
        if reset_uuid:
            pad.uuid = None


def _filter_footprint_library_metadata(footprint: KiCadFootprint) -> KiCadFootprint:
    from .kicad_filter_footprint_metadata import fp_filter__remove_schematic_inherited_metadata

    sexp = fp_filter__remove_schematic_inherited_metadata(footprint.to_sexp())
    return KiCadFootprint.from_sexp(sexp)


def strip_footprint_metadata(
    footprint: KiCadFootprint,
    policy: KiCadExtractionMode | str = KiCadExtractionMode.INTERNAL,
) -> KiCadFootprint:
    """Return a copy of ``footprint`` stripped according to the extraction policy."""
    mode = KiCadExtractionMode(policy)
    stripped = _filter_footprint_library_metadata(copy.deepcopy(footprint))
    if mode == KiCadExtractionMode.PROJECT_LOCAL:
        return stripped

    stripped.uuid = None
    stripped.placed = False
    stripped.properties = [
        prop for prop in stripped.properties
        if prop.name in {"Reference", "Value"}
    ]
    stripped.attr = [
        token for token in stripped.attr
        if token not in {"dnp", "exclude_from_bom", "exclude_from_pos_files"}
    ]
    _strip_pad_instance_metadata(stripped)
    return stripped


def _board_footprint_to_standalone(footprint: Footprint) -> KiCadFootprint:
    standalone = KiCadFootprint.from_sexp(footprint.to_sexp())
    _normalise_standalone_footprint_name(standalone, footprint.library_link)
    _normalise_standalone_footprint_for_library_export(standalone, footprint)
    return standalone


def rehydrate_embedded_model_payloads(
    board: KiCadPcb,
    footprint: KiCadFootprint,
) -> KiCadFootprint:
    """Copy board-level embedded model payloads into a standalone footprint."""
    return rehydrate_embedded_model_payloads_from_files(board.embedded_files, footprint)


def rehydrate_embedded_model_payloads_from_files(
    embedded_files: Iterable[EmbeddedFile],
    footprint: KiCadFootprint,
) -> KiCadFootprint:
    """Copy board-level embedded model payloads into a standalone footprint."""
    out = copy.deepcopy(footprint)
    local = _embedded_file_map(out.embedded_files)
    board_files = _embedded_file_map(embedded_files)
    for model in out.models:
        if not model.path.startswith("kicad-embed://"):
            continue
        name = _embedded_name(model.path)
        board_file = board_files.get(name)
        if board_file is None or not _embedded_file_has_payload(board_file):
            continue
        if _embedded_file_has_payload(local.get(name)):
            continue
        replacement = copy.deepcopy(board_file)
        for index, file in enumerate(out.embedded_files):
            if file.name == name:
                out.embedded_files[index] = replacement
                local[name] = replacement
                break
        else:
            out.embedded_files.append(replacement)
            local[name] = replacement
    return out


def extract_symbols(
    project_path: Path | str,
    mode: KiCadExtractionMode | str = KiCadExtractionMode.INTERNAL,
    *,
    dedupe_policy: KiCadExtractionDedupePolicy | str = KiCadExtractionDedupePolicy.NAME,
    skip_power_symbols: bool = False,
) -> tuple[KiCadSymbolExtractionRecord, ...]:
    """Extract embedded schematic library symbols as structured records."""
    policy = KiCadExtractionMode(mode)
    dedupe = KiCadExtractionDedupePolicy(dedupe_policy)
    symbol_items = [
        (schematic_path, symbol)
        for schematic_path in _iter_project_files(project_path, ".kicad_sch")
        for symbol in _iter_schematic_lib_symbols_from_file(schematic_path)
    ]
    footprint_overrides = (
        _project_symbol_footprint_overrides(project_path)
        if policy == KiCadExtractionMode.PROJECT_LOCAL
        else {}
    )
    known_members = {_symbol_member_name(symbol) for _schematic_path, symbol in symbol_items}
    records: list[KiCadSymbolExtractionRecord] = []
    seen: set[str] = set()
    for schematic_path, symbol in symbol_items:
        if _generated_symbol_alias_base_member(symbol, known_members) is not None:
            continue
        working_symbol = copy.deepcopy(symbol)
        footprint_override = (
            footprint_overrides.get(str(getattr(working_symbol, "name", "") or ""))
            or footprint_overrides.get(_symbol_member_name(working_symbol))
        )
        if footprint_override:
            working_symbol.set_property_value(
                StandardPropertyKey.FOOTPRINT,
                footprint_override,
                create=True,
            )
        raw_fields = _symbol_fields(working_symbol)
        if skip_power_symbols and _is_power_symbol(working_symbol, raw_fields):
            continue
        key = _symbol_record_key(
            symbol=working_symbol,
            source_path=schematic_path,
            policy=policy,
            dedupe_policy=dedupe,
        )
        if (policy in {KiCadExtractionMode.INTERNAL, KiCadExtractionMode.PROJECT_LOCAL}) and key in seen:
            continue
        seen.add(key)
        records.append(KiCadSymbolExtractionRecord(
            name=working_symbol.name,
            source_path=str(schematic_path),
            mode=policy.value,
            symbol=strip_symbol_metadata(working_symbol, policy),
            raw_fields=raw_fields,
            canonical_fields=_canonical_identity_fields(raw_fields),
        ))
    return tuple(records)


def _footprint_fingerprint(footprint: KiCadFootprint) -> str:
    return hashlib.sha256(footprint.to_string().encode("utf-8")).hexdigest()


def _common_footprint_body_for_dedupe(footprint: KiCadFootprint) -> KiCadFootprint:
    common = strip_footprint_metadata(footprint, KiCadExtractionMode.INTERNAL)
    common.locked = False
    common.layer = "F.Cu"
    common.properties = []
    return common


def _footprint_record_key(
    footprint: Footprint,
    standalone: KiCadFootprint,
    policy: KiCadExtractionMode,
    dedupe_policy: KiCadExtractionDedupePolicy,
) -> str:
    if dedupe_policy == KiCadExtractionDedupePolicy.LIBRARY_LINK:
        return footprint.library_link
    if dedupe_policy == KiCadExtractionDedupePolicy.FINGERPRINT:
        return _footprint_fingerprint(_common_footprint_body_for_dedupe(standalone))
    if policy == KiCadExtractionMode.PROJECT_LOCAL:
        reference = footprint.get_property_value("Reference")
        return f"{reference}:{footprint.library_link}:{footprint.uuid}"
    return footprint.library_link


def extract_footprints(
    project_path: Path | str,
    mode: KiCadExtractionMode | str = KiCadExtractionMode.INTERNAL,
    *,
    embed_models: bool = True,
    embed_external_models: bool = True,
    env: dict[str, str] | None = None,
    model_search_dirs: Sequence[Path] | None = None,
    use_installed_kicad: bool = True,
    dedupe_policy: KiCadExtractionDedupePolicy | str = KiCadExtractionDedupePolicy.NAME,
) -> tuple[KiCadFootprintExtractionRecord, ...]:
    """Extract PCB footprints as standalone footprint records.

    External 3D models are resolved through the installed-KiCad search ladder so
    ``${KICAD<major>_3DMODEL_DIR}`` references embed even when the variable is not
    set. Pass ``model_search_dirs`` to add explicit roots or
    ``use_installed_kicad=False`` to skip installation auto-detection.
    """
    policy = KiCadExtractionMode(mode)
    dedupe = KiCadExtractionDedupePolicy(dedupe_policy)
    resolved_project = _resolve_project_path(project_path)
    root = resolved_project.parent
    search = _build_model_search(
        env,
        model_search_dirs=model_search_dirs,
        use_installed_kicad=use_installed_kicad,
    )
    records: list[KiCadFootprintExtractionRecord] = []
    seen: set[str] = set()

    for pcb_path in _iter_project_files(resolved_project, ".kicad_pcb"):
        board_embedded_files = tuple(_iter_embedded_files_from_file(pcb_path))
        unique_links = seen if (
            (
                policy == KiCadExtractionMode.INTERNAL
                and dedupe == KiCadExtractionDedupePolicy.NAME
            )
            or dedupe == KiCadExtractionDedupePolicy.LIBRARY_LINK
        ) else None
        for source_fp in _iter_board_footprints_from_file(
            pcb_path,
            unique_library_links=unique_links,
        ):
            standalone = _board_footprint_to_standalone(source_fp)
            if embed_models:
                standalone = rehydrate_embedded_model_payloads_from_files(
                    board_embedded_files,
                    standalone,
                )
            if embed_models and embed_external_models:
                standalone = embed_external_model_payloads(
                    standalone, project_root=root, search=search
                )
            key = _footprint_record_key(source_fp, standalone, policy, dedupe)
            if unique_links is None:
                if (
                    policy == KiCadExtractionMode.INTERNAL
                    or dedupe
                    in {
                        KiCadExtractionDedupePolicy.FINGERPRINT,
                        KiCadExtractionDedupePolicy.LIBRARY_LINK,
                    }
                ) and key in seen:
                    continue
                seen.add(key)
            stripped = strip_footprint_metadata(standalone, policy)
            raw_fields = _footprint_fields(source_fp)
            model_refs = _scan_models_for_owner(
                source_kind="extracted_footprint",
                source_path=pcb_path,
                owner=source_fp.library_link,
                models=stripped.models,
                embedded_files=stripped.embedded_files,
                project_root=root,
                board_embedded_files=board_embedded_files,
                search=search,
            )
            records.append(KiCadFootprintExtractionRecord(
                name=stripped.name,
                library_link=source_fp.library_link,
                source_path=str(pcb_path),
                source_reference=source_fp.get_property_value("Reference"),
                mode=policy.value,
                footprint=stripped,
                raw_fields=raw_fields,
                canonical_fields=_canonical_identity_fields(raw_fields),
                model_references=tuple(model_refs),
            ))
    return tuple(records)


def write_symbol_folder_library(
    records: Iterable[KiCadSymbolExtractionRecord],
    output_dir: Path | str,
    *,
    overwrite: bool = True,
    footprint_library_nickname: str | None = None,
    footprint_name_map: dict[str, str] | None = None,
) -> tuple[Path, ...]:
    """Write one single-symbol ``.kicad_sym`` file per extraction record."""
    out_dir = Path(output_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    written: list[Path] = []
    used: set[str] = set()
    for record in records:
        stem = _unique_stem(_safe_asset_filename(record.name), used)
        path = out_dir / f"{stem}.kicad_sym"
        if path.exists() and not overwrite:
            continue
        symbol = _rename_symbol_for_folder_library(record.symbol, stem)
        symbol = _relink_symbol_footprint_property(
            symbol,
            footprint_library_nickname=footprint_library_nickname,
            footprint_name_map=footprint_name_map,
        )
        lib = KiCadSymbolLib(symbols=[symbol])
        lib.save(path)
        written.append(path)
    return tuple(written)


def write_pretty_library(
    records: Iterable[KiCadFootprintExtractionRecord],
    output_dir: Path | str,
    *,
    overwrite: bool = True,
) -> tuple[Path, ...]:
    """Write one standalone ``.kicad_mod`` file per extraction record."""
    out_dir = Path(output_dir)
    if out_dir.suffix != ".pretty":
        out_dir = out_dir.with_suffix(".pretty")
    out_dir.mkdir(parents=True, exist_ok=True)
    written: list[Path] = []
    used: set[str] = set()
    for record in records:
        stem = _unique_stem(_safe_asset_filename(record.name), used)
        path = out_dir / f"{stem}.kicad_mod"
        if path.exists() and not overwrite:
            continue
        record.footprint.save(path)
        written.append(path)
    return tuple(written)


def build_extraction_metadata_bundle(
    project_path: Path | str,
    *,
    mode: KiCadExtractionMode | str = KiCadExtractionMode.INTERNAL,
    symbol_records: Iterable[KiCadSymbolExtractionRecord] | None = None,
    footprint_records: Iterable[KiCadFootprintExtractionRecord] | None = None,
    include_asset_scan: bool = True,
    skip_power_symbols: bool = False,
    schema: str = "kicad_monkey.library_extraction_bundle.v1",
) -> dict[str, Any]:
    """Build stable JSON metadata for an extracted KiCad library bundle."""
    resolved_project = _resolve_project_path(project_path)
    symbols = (
        tuple(symbol_records)
        if symbol_records is not None
        else extract_symbols(resolved_project, mode, skip_power_symbols=skip_power_symbols)
    )
    footprints = (
        tuple(footprint_records)
        if footprint_records is not None
        else extract_footprints(resolved_project, mode)
    )
    data: dict[str, Any] = {
        "schema": schema,
        "project_path": str(resolved_project),
        "project_root": str(resolved_project.parent),
        "mode": KiCadExtractionMode(mode).value,
        "symbols": [record.metadata_dict() for record in symbols],
        "footprints": [record.metadata_dict() for record in footprints],
    }
    if include_asset_scan:
        data["assets"] = scan_project_assets(resolved_project).to_dict()
    return data


def write_extraction_metadata_bundle(
    project_path: Path | str,
    output_path: Path | str,
    *,
    mode: KiCadExtractionMode | str = KiCadExtractionMode.INTERNAL,
    symbol_records: Iterable[KiCadSymbolExtractionRecord] | None = None,
    footprint_records: Iterable[KiCadFootprintExtractionRecord] | None = None,
    include_asset_scan: bool = True,
    skip_power_symbols: bool = False,
    schema: str = "kicad_monkey.library_extraction_bundle.v1",
) -> Path:
    """Write stable JSON metadata for an extracted KiCad library bundle."""
    path = Path(output_path)
    path.parent.mkdir(parents=True, exist_ok=True)
    data = build_extraction_metadata_bundle(
        project_path,
        mode=mode,
        symbol_records=symbol_records,
        footprint_records=footprint_records,
        include_asset_scan=include_asset_scan,
        skip_power_symbols=skip_power_symbols,
        schema=schema,
    )
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return path


def _append_embedded_file_asset(
    records: list[KiCadEmbeddedAssetRecord],
    file: EmbeddedFile,
    *,
    output_dir: Path,
    source_kind: str,
    source_path: Path,
    owner: str,
    index: int,
    dedupe_payloads: bool,
    written: dict[str, Path],
    used_names_by_dir: dict[Path, set[str]],
) -> None:
    if not file.data:
        return
    data = _embedded_file_payload_bytes(file)
    source_name = file.name or f"{source_kind}_{index}.bin"
    records.append(_write_embedded_asset_payload(
        data=data,
        output_dir=output_dir,
        subdir="embedded_files",
        fallback_stem=f"{source_kind}_{index}",
        kind="embedded_file",
        source_kind=source_kind,
        source_path=source_path,
        owner=owner,
        source_name=source_name,
        file_type=str(file.file_type or ""),
        dedupe_payloads=dedupe_payloads,
        written=written,
        used_names_by_dir=used_names_by_dir,
    ))


def _append_plain_base64_asset(
    records: list[KiCadEmbeddedAssetRecord],
    payload: str,
    *,
    output_dir: Path,
    subdir: str,
    kind: str,
    source_kind: str,
    source_path: Path,
    owner: str,
    source_name: str,
    fallback_stem: str,
    dedupe_payloads: bool,
    written: dict[str, Path],
    used_names_by_dir: dict[Path, set[str]],
) -> None:
    if not payload:
        return
    data = _decode_plain_base64_payload(payload, label=f"{source_path}:{source_name}")
    records.append(_write_embedded_asset_payload(
        data=data,
        output_dir=output_dir,
        subdir=subdir,
        fallback_stem=fallback_stem,
        kind=kind,
        source_kind=source_kind,
        source_path=source_path,
        owner=owner,
        source_name=source_name,
        dedupe_payloads=dedupe_payloads,
        written=written,
        used_names_by_dir=used_names_by_dir,
    ))


def _append_image_assets(
    records: list[KiCadEmbeddedAssetRecord],
    images: Iterable[Any],
    *,
    output_dir: Path,
    subdir: str,
    kind: str,
    source_kind: str,
    source_path: Path,
    owner: str,
    fallback_prefix: str,
    dedupe_payloads: bool,
    written: dict[str, Path],
    used_names_by_dir: dict[Path, set[str]],
) -> None:
    for index, image in enumerate(images, start=1):
        raw_data = getattr(image, "data", "")
        payload = "".join(raw_data) if isinstance(raw_data, list) else str(raw_data or "")
        source_name = str(getattr(image, "uuid", "") or f"{fallback_prefix}_{index}")
        _append_plain_base64_asset(
            records,
            payload,
            output_dir=output_dir,
            subdir=subdir,
            kind=kind,
            source_kind=source_kind,
            source_path=source_path,
            owner=owner,
            source_name=source_name,
            fallback_stem=f"{fallback_prefix}_{index}",
            dedupe_payloads=dedupe_payloads,
            written=written,
            used_names_by_dir=used_names_by_dir,
        )


def _append_embedded_files_from_file(
    records: list[KiCadEmbeddedAssetRecord],
    path: Path,
    *,
    output_dir: Path,
    source_kind: str,
    owner: str,
    dedupe_payloads: bool,
    written: dict[str, Path],
    used_names_by_dir: dict[Path, set[str]],
) -> None:
    for index, file in enumerate(_iter_embedded_files_from_file(path), start=1):
        _append_embedded_file_asset(
            records,
            file,
            output_dir=output_dir,
            source_kind=source_kind,
            source_path=path,
            owner=owner,
            index=index,
            dedupe_payloads=dedupe_payloads,
            written=written,
            used_names_by_dir=used_names_by_dir,
        )


def _append_schematic_image_assets(
    records: list[KiCadEmbeddedAssetRecord],
    schematic_path: Path,
    *,
    output_dir: Path,
    dedupe_payloads: bool,
    written: dict[str, Path],
    used_names_by_dir: dict[Path, set[str]],
) -> None:
    sexp = parse_sexp(_read_kicad_text(schematic_path), source_path=schematic_path)
    images = [SchImage.from_sexp(elem) for elem in find_all_elements(sexp, "image")]
    _append_image_assets(
        records,
        images,
        output_dir=output_dir,
        subdir="schematic_images",
        kind="schematic_image",
        source_kind="schematic",
        source_path=schematic_path,
        owner=schematic_path.stem,
        fallback_prefix=f"{schematic_path.stem}_image",
        dedupe_payloads=dedupe_payloads,
        written=written,
        used_names_by_dir=used_names_by_dir,
    )


def _append_pcb_image_assets(
    records: list[KiCadEmbeddedAssetRecord],
    images: Iterable[Image],
    *,
    output_dir: Path,
    source_kind: str,
    source_path: Path,
    owner: str,
    fallback_prefix: str,
    dedupe_payloads: bool,
    written: dict[str, Path],
    used_names_by_dir: dict[Path, set[str]],
) -> None:
    _append_image_assets(
        records,
        images,
        output_dir=output_dir,
        subdir="pcb_images",
        kind="pcb_image",
        source_kind=source_kind,
        source_path=source_path,
        owner=owner,
        fallback_prefix=fallback_prefix,
        dedupe_payloads=dedupe_payloads,
        written=written,
        used_names_by_dir=used_names_by_dir,
    )


def _append_pcb_assets(
    records: list[KiCadEmbeddedAssetRecord],
    pcb_path: Path,
    *,
    output_dir: Path,
    dedupe_payloads: bool,
    written: dict[str, Path],
    used_names_by_dir: dict[Path, set[str]],
) -> None:
    projection = KiCadPcbProjection.from_file(pcb_path)
    for index, file in enumerate(projection.embedded_files(), start=1):
        _append_embedded_file_asset(
            records,
            file,
            output_dir=output_dir,
            source_kind="pcb",
            source_path=pcb_path,
            owner="board",
            index=index,
            dedupe_payloads=dedupe_payloads,
            written=written,
            used_names_by_dir=used_names_by_dir,
        )
    _append_pcb_image_assets(
        records,
        projection.images(),
        output_dir=output_dir,
        source_kind="pcb",
        source_path=pcb_path,
        owner="board",
        fallback_prefix=f"{pcb_path.stem}_image",
        dedupe_payloads=dedupe_payloads,
        written=written,
        used_names_by_dir=used_names_by_dir,
    )
    for footprint_index, footprint in enumerate(projection.footprints(), start=1):
        owner = (
            str(getattr(footprint, "library_link", "") or "")
            or str(footprint.get_property_value("Reference", "") or "")
            or str(getattr(footprint, "uuid", "") or "")
            or f"footprint_{footprint_index}"
        )
        for file_index, file in enumerate(getattr(footprint, "embedded_files", ()) or (), start=1):
            _append_embedded_file_asset(
                records,
                file,
                output_dir=output_dir,
                source_kind="pcb_footprint",
                source_path=pcb_path,
                owner=owner,
                index=file_index,
                dedupe_payloads=dedupe_payloads,
                written=written,
                used_names_by_dir=used_names_by_dir,
            )
        _append_pcb_image_assets(
            records,
            getattr(footprint, "images", ()) or (),
            output_dir=output_dir,
            source_kind="pcb_footprint",
            source_path=pcb_path,
            owner=owner,
            fallback_prefix=f"{_safe_asset_filename(owner)}_image",
            dedupe_payloads=dedupe_payloads,
            written=written,
            used_names_by_dir=used_names_by_dir,
        )


def _append_footprint_library_image_assets(
    records: list[KiCadEmbeddedAssetRecord],
    footprint_path: Path,
    *,
    output_dir: Path,
    dedupe_payloads: bool,
    written: dict[str, Path],
    used_names_by_dir: dict[Path, set[str]],
) -> None:
    sexp = parse_sexp(_read_kicad_text(footprint_path), source_path=footprint_path)
    images = [Image.from_sexp(elem) for elem in find_all_elements(sexp, "image")]
    _append_pcb_image_assets(
        records,
        images,
        output_dir=output_dir,
        source_kind="footprint_library",
        source_path=footprint_path,
        owner=footprint_path.stem,
        fallback_prefix=f"{footprint_path.stem}_image",
        dedupe_payloads=dedupe_payloads,
        written=written,
        used_names_by_dir=used_names_by_dir,
    )


def _append_worksheet_assets(
    records: list[KiCadEmbeddedAssetRecord],
    worksheet_path: Path,
    *,
    output_dir: Path,
    dedupe_payloads: bool,
    written: dict[str, Path],
    used_names_by_dir: dict[Path, set[str]],
) -> None:
    worksheet = KiCadWorksheet.from_file(worksheet_path)
    for index, bitmap in enumerate(worksheet.bitmaps, start=1):
        source_name = bitmap.name or f"{worksheet_path.stem}_bitmap_{index}"
        _append_plain_base64_asset(
            records,
            bitmap.pngdata,
            output_dir=output_dir,
            subdir="worksheet_bitmaps",
            kind="worksheet_bitmap",
            source_kind="worksheet",
            source_path=worksheet_path,
            owner=worksheet_path.stem,
            source_name=source_name,
            fallback_stem=f"{worksheet_path.stem}_bitmap_{index}",
            dedupe_payloads=dedupe_payloads,
            written=written,
            used_names_by_dir=used_names_by_dir,
        )


def extract_embedded_assets(
    project_path: Path | str,
    output_dir: Path | str,
    *,
    dedupe_payloads: bool = True,
    progress: _ProgressCallback | None = None,
) -> tuple[KiCadEmbeddedAssetRecord, ...]:
    """Extract embedded files and bitmap payloads from the full KiCad project."""
    out_dir = Path(output_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    written: dict[str, Path] = {}
    used_names_by_dir: dict[Path, set[str]] = {}
    records: list[KiCadEmbeddedAssetRecord] = []

    resolved_project = _resolve_project_path(project_path)
    pcb_paths = tuple(_iter_project_files(resolved_project, ".kicad_pcb"))
    schematic_paths = tuple(_iter_project_files(resolved_project, ".kicad_sch"))
    footprint_paths = tuple(_iter_project_files(resolved_project, ".kicad_mod"))
    symbol_library_paths = tuple(_iter_project_files(resolved_project, ".kicad_sym"))
    worksheet_paths = tuple(_iter_project_files(resolved_project, ".kicad_wks"))
    _emit_progress(
        progress,
        "inventory found "
        f"{len(pcb_paths)} PCB, {len(schematic_paths)} schematic, "
        f"{len(footprint_paths)} footprint, {len(symbol_library_paths)} symbol library, "
        f"and {len(worksheet_paths)} worksheet file(s)",
    )
    for index, pcb_path in enumerate(pcb_paths, start=1):
        _emit_progress(
            progress,
            f"scanning PCB assets {index}/{len(pcb_paths)}: {pcb_path.name}",
        )
        _append_pcb_assets(
            records,
            pcb_path,
            output_dir=out_dir,
            dedupe_payloads=dedupe_payloads,
            written=written,
            used_names_by_dir=used_names_by_dir,
        )
    for index, schematic_path in enumerate(schematic_paths, start=1):
        _emit_progress(
            progress,
            f"scanning schematic assets {index}/{len(schematic_paths)}: {schematic_path.name}",
        )
        _append_schematic_image_assets(
            records,
            schematic_path,
            output_dir=out_dir,
            dedupe_payloads=dedupe_payloads,
            written=written,
            used_names_by_dir=used_names_by_dir,
        )
        _append_embedded_files_from_file(
            records,
            schematic_path,
            output_dir=out_dir,
            source_kind="schematic",
            owner=schematic_path.stem,
            dedupe_payloads=dedupe_payloads,
            written=written,
            used_names_by_dir=used_names_by_dir,
        )
    for index, footprint_path in enumerate(footprint_paths, start=1):
        _emit_progress(
            progress,
            f"scanning footprint-library assets {index}/{len(footprint_paths)}: "
            f"{footprint_path.name}",
        )
        _append_embedded_files_from_file(
            records,
            footprint_path,
            output_dir=out_dir,
            source_kind="footprint_library",
            owner=footprint_path.stem,
            dedupe_payloads=dedupe_payloads,
            written=written,
            used_names_by_dir=used_names_by_dir,
        )
        _append_footprint_library_image_assets(
            records,
            footprint_path,
            output_dir=out_dir,
            dedupe_payloads=dedupe_payloads,
            written=written,
            used_names_by_dir=used_names_by_dir,
        )
    for index, symbol_library_path in enumerate(symbol_library_paths, start=1):
        _emit_progress(
            progress,
            f"scanning symbol-library assets {index}/{len(symbol_library_paths)}: "
            f"{symbol_library_path.name}",
        )
        _append_embedded_files_from_file(
            records,
            symbol_library_path,
            output_dir=out_dir,
            source_kind="symbol_library",
            owner=symbol_library_path.stem,
            dedupe_payloads=dedupe_payloads,
            written=written,
            used_names_by_dir=used_names_by_dir,
        )
    for index, worksheet_path in enumerate(worksheet_paths, start=1):
        _emit_progress(
            progress,
            f"scanning worksheet assets {index}/{len(worksheet_paths)}: {worksheet_path.name}",
        )
        _append_worksheet_assets(
            records,
            worksheet_path,
            output_dir=out_dir,
            dedupe_payloads=dedupe_payloads,
            written=written,
            used_names_by_dir=used_names_by_dir,
        )
    return tuple(records)


def _write_embedded_model_payload(
    file: EmbeddedFile,
    output_dir: Path,
    written: dict[str, Path],
    used_names: set[str],
    *,
    dedupe_payloads: bool = True,
) -> None:
    if not _is_step_model_name(file.name):
        return
    data = _embedded_file_payload_bytes(file)
    digest = hashlib.sha256(data).hexdigest()
    safe_name = _safe_asset_filename(file.name)
    key = f"sha256:{digest}" if dedupe_payloads else f"name:{safe_name.lower()}:{digest}"
    if key in written:
        return
    path = output_dir / _unique_file_name(safe_name, used_names)
    path.write_bytes(data)
    written[key] = path


def extract_3d_models(
    project_path: Path | str,
    output_dir: Path | str,
    *,
    dedupe_payloads: bool = True,
) -> tuple[Path, ...]:
    """Extract embedded model payloads from project boards and footprints."""
    out_dir = Path(output_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    written: dict[str, Path] = {}
    used_names: set[str] = set()

    resolved_project = _resolve_project_path(project_path)
    for pcb_path in _iter_project_files(resolved_project, ".kicad_pcb"):
        for file in KiCadPcbProjection.from_file(pcb_path).embedded_files():
            _write_embedded_model_payload(
                file,
                out_dir,
                written,
                used_names,
                dedupe_payloads=dedupe_payloads,
            )
    for fp_path in _iter_project_files(resolved_project, ".kicad_mod"):
        for file in _iter_embedded_files_from_file(fp_path):
            _write_embedded_model_payload(
                file,
                out_dir,
                written,
                used_names,
                dedupe_payloads=dedupe_payloads,
            )
    return tuple(written.values())


def extract_3d_models_from_footprint_records(
    records: Iterable[KiCadFootprintExtractionRecord],
    output_dir: Path | str,
    *,
    dedupe_payloads: bool = True,
) -> tuple[Path, ...]:
    """Extract embedded STEP model payloads from already-extracted footprints."""
    out_dir = Path(output_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    written: dict[str, Path] = {}
    used_names: set[str] = set()
    for record in records:
        for file in record.footprint.embedded_files:
            _write_embedded_model_payload(
                file,
                out_dir,
                written,
                used_names,
                dedupe_payloads=dedupe_payloads,
            )
    return tuple(written.values())


def resolve_kicad_cli(kicad_cli: Path | str | None = None) -> Path | None:
    """Resolve a local KiCad CLI executable, preferring explicit input."""
    if kicad_cli is not None:
        candidate = Path(kicad_cli)
        return candidate if candidate.exists() else None
    path_cli = shutil.which("kicad-cli")
    if path_cli:
        return Path(path_cli)
    installation = KiCadEnvironment().highest_installation()
    if installation is not None and installation.kicad_cli.exists():
        return installation.kicad_cli
    return None


def _run_kicad_cli_validation(
    input_path: Path,
    *,
    command_group: str,
    output_suffix: str,
    kicad_cli: Path | str | None = None,
    timeout: int = 60,
) -> KiCadCliValidationResult:
    cli = resolve_kicad_cli(kicad_cli)
    if cli is None:
        return KiCadCliValidationResult(
            input_path=str(input_path),
            command=(),
            returncode=127,
            stderr="kicad-cli not found",
        )
    with tempfile.TemporaryDirectory() as temp_dir:
        output_path = Path(temp_dir) / f"validated{output_suffix}"
        command = (
            str(cli),
            command_group,
            "upgrade",
            "--force",
            "--output",
            str(output_path),
            str(input_path),
        )
        completed = subprocess.run(
            command,
            capture_output=True,
            text=True,
            timeout=timeout,
            check=False,
        )
        return KiCadCliValidationResult(
            input_path=str(input_path),
            command=command,
            returncode=completed.returncode,
            stdout=completed.stdout,
            stderr=completed.stderr,
            output_path=str(output_path),
        )


def validate_symbol_library_with_kicad_cli(
    symbol_library: Path | str,
    *,
    kicad_cli: Path | str | None = None,
    timeout: int = 60,
) -> KiCadCliValidationResult:
    """Ask KiCad CLI to parse and upgrade a symbol library file or directory."""
    path = Path(symbol_library)
    suffix = ".kicad_sym" if path.is_file() else ""
    return _run_kicad_cli_validation(
        path,
        command_group="sym",
        output_suffix=suffix,
        kicad_cli=kicad_cli,
        timeout=timeout,
    )


def validate_pretty_library_with_kicad_cli(
    pretty_library: Path | str,
    *,
    kicad_cli: Path | str | None = None,
    timeout: int = 60,
) -> KiCadCliValidationResult:
    """Ask KiCad CLI to parse and upgrade a footprint library file or directory."""
    path = Path(pretty_library)
    return _run_kicad_cli_validation(
        path,
        command_group="fp",
        output_suffix=".pretty",
        kicad_cli=kicad_cli,
        timeout=timeout,
    )


def _unique_stem(stem: str, used: set[str]) -> str:
    candidate = stem
    index = 1
    while candidate.lower() in used:
        index += 1
        candidate = f"{stem}_{index}"
    used.add(candidate.lower())
    return candidate


def _unique_file_name(filename: str, used: set[str]) -> str:
    path = Path(filename)
    stem = path.stem or "unnamed"
    suffix = path.suffix
    candidate = f"{stem}{suffix}"
    index = 1
    while candidate.lower() in used:
        index += 1
        candidate = f"{stem}_{index}{suffix}"
    used.add(candidate.lower())
    return candidate


__all__ = [
    "KiCadCliValidationResult",
    "KiCadEmbeddedAssetRecord",
    "KiCadExtractionDedupePolicy",
    "KiCadExtractionMode",
    "KiCadFootprintExtractionRecord",
    "KiCadModelReferenceKind",
    "KiCadModelReferenceScan",
    "KiCadProjectAssetScan",
    "KiCadSymbolExtractionRecord",
    "build_extraction_metadata_bundle",
    "build_footprint_library_link_map",
    "embed_external_model_payloads",
    "extract_3d_models",
    "extract_3d_models_from_footprint_records",
    "extract_embedded_assets",
    "extract_footprints",
    "extract_symbols",
    "rehydrate_embedded_model_payloads",
    "rehydrate_embedded_model_payloads_from_files",
    "resolve_kicad_cli",
    "scan_3d_models",
    "scan_project_assets",
    "strip_footprint_metadata",
    "strip_symbol_metadata",
    "validate_pretty_library_with_kicad_cli",
    "validate_symbol_library_with_kicad_cli",
    "write_extraction_metadata_bundle",
    "write_pretty_library",
    "write_symbol_folder_library",
]
