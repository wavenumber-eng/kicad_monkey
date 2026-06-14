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
from typing import Any, Iterable

from .kicad_footprint import KiCadFootprint
from .kicad_model import EmbeddedFile, Model
from .kicad_pcb import KiCadPcb
from .kicad_pcb_footprint import Footprint
from .kicad_pcb_other import NetRef
from .kicad_environment import KiCadEnvironment
from .kicad_project import find_adjacent_kicad_project_path
from .kicad_sch_enums import PropertyId, StandardPropertyKey
from .kicad_schematic import KiCadSchematic
from .kicad_symbol_lib import KiCadSymbolLib

try:
    import zstandard as _zstandard
except ImportError:
    _zstandard = None


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
    diagnostics: tuple[str, ...] = ()

    def to_dict(self) -> dict[str, Any]:
        data = asdict(self)
        data["model_references"] = [ref.to_dict() for ref in self.model_references]
        return data


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


def _library_member_name(name: str) -> str:
    return str(name).split(":", 1)[1] if ":" in str(name) else str(name)


def _safe_asset_filename(name: str) -> str:
    clean = _library_member_name(name)
    for char in r'<>:"/\|?*':
        clean = clean.replace(char, "_")
    for char in " \t\n\r":
        clean = clean.replace(char, "_")
    return clean or "unnamed"


def _symbol_fields(symbol: Any) -> dict[str, str]:
    return {
        str(getattr(prop, "key", "")): str(getattr(prop, "value", ""))
        for prop in getattr(symbol, "properties", ()) or ()
        if getattr(prop, "key", "")
    }


def _footprint_fields(footprint: Any) -> dict[str, str]:
    return {
        str(getattr(prop, "name", "")): str(getattr(prop, "value", ""))
        for prop in getattr(footprint, "properties", ()) or ()
        if getattr(prop, "name", "")
    }


def _canonical_identity_fields(fields: dict[str, str]) -> dict[str, str]:
    candidates = {
        "mpn": ("mpn", "MPN", "manf_pn", "manufacturer part number", "Manufacturer Part Number"),
        "cad-reference": ("cad-reference", "cad_reference", "CAD Reference"),
    }
    out: dict[str, str] = {}
    lower_map = {key.lower(): value for key, value in fields.items()}
    for canonical, aliases in candidates.items():
        for alias in aliases:
            value = lower_map.get(alias.lower())
            if value:
                out[canonical] = value
                break
    return out


def _embedded_name(path: str) -> str:
    return path.removeprefix("kicad-embed://")


def _embedded_file_map(files: Iterable[EmbeddedFile]) -> dict[str, EmbeddedFile]:
    return {file.name: file for file in files if getattr(file, "name", "")}


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


def _effective_env(env: dict[str, str] | None = None) -> dict[str, str]:
    effective = dict(os.environ)
    if env:
        effective.update(env)
    return effective


def _expand_model_path(model_path: str, *, project_root: Path, env: dict[str, str]) -> tuple[Path | None, bool]:
    unresolved = False

    def replace_var(match: re.Match[str]) -> str:
        nonlocal unresolved
        name = next(group for group in match.groups() if group is not None)
        value = env.get(name)
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
    env: dict[str, str] | None = None,
) -> tuple[KiCadModelReferenceKind, str, bool, str, bool, str, tuple[str, ...]]:
    diagnostics: list[str] = []
    if model_path.startswith("kicad-embed://"):
        name = _embedded_name(model_path)
        if name in embedded_files:
            return (
                KiCadModelReferenceKind.EMBEDDED,
                "",
                False,
                name,
                True,
                "footprint",
                (),
            )
        if board_embedded_files and name in board_embedded_files:
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

    resolved, unresolved = _expand_model_path(
        model_path,
        project_root=project_root,
        env=_effective_env(env),
    )
    if unresolved or resolved is None:
        diagnostics.append("unresolved environment variable")
        return kind, model_path, False, "", False, "", tuple(diagnostics)
    exists = resolved.exists()
    if not exists:
        diagnostics.append("model path does not exist")
    return kind, str(resolved), exists, "", False, "", tuple(diagnostics)


def _scan_models_for_owner(
    *,
    source_kind: str,
    source_path: Path,
    owner: str,
    models: Iterable[Model],
    embedded_files: Iterable[EmbeddedFile],
    project_root: Path,
    board_embedded_files: Iterable[EmbeddedFile] = (),
    env: dict[str, str] | None = None,
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
            env=env,
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


def _scan_3d_models_with_diagnostics(
    project_path: Path | str,
) -> tuple[tuple[KiCadModelReferenceScan, ...], tuple[str, ...]]:
    resolved_project = _resolve_project_path(project_path)
    root = resolved_project.parent
    records: list[KiCadModelReferenceScan] = []
    diagnostics: list[str] = []

    for pcb_path in _iter_project_files(resolved_project, ".kicad_pcb"):
        try:
            pcb = KiCadPcb.from_file(pcb_path)
        except Exception as exc:
            diagnostics.append(f"failed to parse PCB {pcb_path}: {exc}")
            continue
        for footprint in pcb.footprints:
            owner = getattr(footprint, "library_link", "") or getattr(footprint, "reference", "")
            records.extend(_scan_models_for_owner(
                source_kind="pcb_footprint",
                source_path=pcb_path,
                owner=owner,
                models=getattr(footprint, "models", ()) or (),
                embedded_files=getattr(footprint, "embedded_files", ()) or (),
                project_root=root,
                board_embedded_files=getattr(pcb, "embedded_files", ()) or (),
            ))

    for fp_path in _iter_project_files(resolved_project, ".kicad_mod"):
        try:
            footprint = KiCadFootprint.from_file(fp_path)
        except Exception as exc:
            diagnostics.append(f"failed to parse footprint {fp_path}: {exc}")
            continue
        records.extend(_scan_models_for_owner(
            source_kind="footprint_library",
            source_path=fp_path,
            owner=footprint.name,
            models=footprint.models,
            embedded_files=footprint.embedded_files,
            project_root=root,
        ))
    return tuple(records), tuple(diagnostics)


def scan_3d_models(project_path: Path | str) -> tuple[KiCadModelReferenceScan, ...]:
    """Scan PCB and local footprint-library model references."""
    records, _diagnostics = _scan_3d_models_with_diagnostics(project_path)
    return records


def scan_project_assets(project_path: Path | str) -> KiCadProjectAssetScan:
    """Return a structured inventory of a KiCad project's local assets."""
    resolved_project = _resolve_project_path(project_path)
    root = resolved_project.parent
    schematics = tuple(str(path) for path in _iter_project_files(resolved_project, ".kicad_sch"))
    pcbs = tuple(str(path) for path in _iter_project_files(resolved_project, ".kicad_pcb"))
    symbol_libraries = tuple(str(path) for path in _iter_project_files(resolved_project, ".kicad_sym"))
    pretty_libraries = tuple(str(path) for path in sorted(root.rglob("*.pretty")) if path.is_dir())
    footprint_files = tuple(str(path) for path in _iter_project_files(resolved_project, ".kicad_mod"))
    model_references, diagnostics = _scan_3d_models_with_diagnostics(resolved_project)
    return KiCadProjectAssetScan(
        project_path=str(resolved_project),
        project_root=str(root),
        schematics=schematics,
        pcbs=pcbs,
        symbol_libraries=symbol_libraries,
        pretty_libraries=pretty_libraries,
        footprint_files=footprint_files,
        model_references=model_references,
        diagnostics=diagnostics,
    )


def embed_external_model_payloads(
    footprint: KiCadFootprint,
    *,
    project_root: Path | str,
    env: dict[str, str] | None = None,
) -> KiCadFootprint:
    """Return a footprint with resolvable external STEP/STP models embedded.

    Existing ``kicad-embed://`` model references are preserved. Resolvable
    external STEP/STP paths are copied into the footprint's ``embedded_files``
    block and their model paths are rewritten to ``kicad-embed://...``.
    """
    out = copy.deepcopy(footprint)
    local = _embedded_file_map(out.embedded_files)
    existing_names = set(local)
    effective_env = _effective_env(env)
    root = Path(project_root)

    for model in out.models:
        if model.path.startswith("kicad-embed://"):
            continue
        path, unresolved = _expand_model_path(model.path, project_root=root, env=effective_env)
        if unresolved or path is None or not path.exists() or not _is_step_model_name(path.name):
            continue
        embedded_name = _unique_embedded_name(path.name, existing_names)
        out.embedded_files.append(_embedded_file_from_path(path, name=embedded_name))
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
        return f"{source_path}:{getattr(symbol, 'name', '')}"
    if dedupe_policy == KiCadExtractionDedupePolicy.FINGERPRINT:
        return _symbol_fingerprint(strip_symbol_metadata(symbol, policy))
    return str(getattr(symbol, "name", ""))


def _normalise_standalone_footprint_name(footprint: KiCadFootprint, name: str) -> None:
    footprint.name = _library_member_name(name)


def _strip_pad_instance_metadata(footprint: KiCadFootprint) -> None:
    for pad in footprint.pads:
        pad.net = NetRef()
        pad.uuid = None


def strip_footprint_metadata(
    footprint: KiCadFootprint,
    policy: KiCadExtractionMode | str = KiCadExtractionMode.INTERNAL,
) -> KiCadFootprint:
    """Return a copy of ``footprint`` stripped according to the extraction policy."""
    mode = KiCadExtractionMode(policy)
    stripped = copy.deepcopy(footprint)
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
    return standalone


def rehydrate_embedded_model_payloads(
    board: KiCadPcb,
    footprint: KiCadFootprint,
) -> KiCadFootprint:
    """Copy board-level embedded model payloads into a standalone footprint."""
    out = copy.deepcopy(footprint)
    local = _embedded_file_map(out.embedded_files)
    board_files = _embedded_file_map(board.embedded_files)
    for model in out.models:
        if not model.path.startswith("kicad-embed://"):
            continue
        name = _embedded_name(model.path)
        if name not in local and name in board_files:
            out.embedded_files.append(copy.deepcopy(board_files[name]))
            local[name] = out.embedded_files[-1]
    return out


def extract_symbols(
    project_path: Path | str,
    mode: KiCadExtractionMode | str = KiCadExtractionMode.INTERNAL,
    *,
    dedupe_policy: KiCadExtractionDedupePolicy | str = KiCadExtractionDedupePolicy.NAME,
) -> tuple[KiCadSymbolExtractionRecord, ...]:
    """Extract embedded schematic library symbols as structured records."""
    policy = KiCadExtractionMode(mode)
    dedupe = KiCadExtractionDedupePolicy(dedupe_policy)
    records: list[KiCadSymbolExtractionRecord] = []
    seen: set[str] = set()
    for schematic_path in _iter_project_files(project_path, ".kicad_sch"):
        schematic = KiCadSchematic.from_file(schematic_path)
        for symbol in schematic.lib_symbols:
            raw_fields = _symbol_fields(symbol)
            key = _symbol_record_key(
                symbol=symbol,
                source_path=schematic_path,
                policy=policy,
                dedupe_policy=dedupe,
            )
            if policy == KiCadExtractionMode.INTERNAL and key in seen:
                continue
            seen.add(key)
            records.append(KiCadSymbolExtractionRecord(
                name=symbol.name,
                source_path=str(schematic_path),
                mode=policy.value,
                symbol=strip_symbol_metadata(symbol, policy),
                raw_fields=raw_fields,
                canonical_fields=_canonical_identity_fields(raw_fields),
            ))
    return tuple(records)


def _footprint_fingerprint(footprint: KiCadFootprint) -> str:
    return hashlib.sha256(footprint.to_string().encode("utf-8")).hexdigest()


def _footprint_record_key(
    footprint: Footprint,
    standalone: KiCadFootprint,
    policy: KiCadExtractionMode,
    dedupe_policy: KiCadExtractionDedupePolicy,
) -> str:
    if policy == KiCadExtractionMode.PROJECT_LOCAL:
        reference = footprint.get_property_value("Reference")
        return f"{reference}:{footprint.library_link}:{footprint.uuid}"
    if dedupe_policy == KiCadExtractionDedupePolicy.FINGERPRINT:
        return _footprint_fingerprint(strip_footprint_metadata(standalone, policy))
    return footprint.library_link


def extract_footprints(
    project_path: Path | str,
    mode: KiCadExtractionMode | str = KiCadExtractionMode.INTERNAL,
    *,
    embed_models: bool = True,
    embed_external_models: bool = True,
    env: dict[str, str] | None = None,
    dedupe_policy: KiCadExtractionDedupePolicy | str = KiCadExtractionDedupePolicy.NAME,
) -> tuple[KiCadFootprintExtractionRecord, ...]:
    """Extract PCB footprints as standalone footprint records."""
    policy = KiCadExtractionMode(mode)
    dedupe = KiCadExtractionDedupePolicy(dedupe_policy)
    resolved_project = _resolve_project_path(project_path)
    root = resolved_project.parent
    records: list[KiCadFootprintExtractionRecord] = []
    seen: set[str] = set()

    for pcb_path in _iter_project_files(resolved_project, ".kicad_pcb"):
        board = KiCadPcb.from_file(pcb_path)
        for source_fp in board.footprints:
            standalone = _board_footprint_to_standalone(source_fp)
            if embed_models:
                standalone = rehydrate_embedded_model_payloads(board, standalone)
            if embed_models and embed_external_models:
                standalone = embed_external_model_payloads(standalone, project_root=root, env=env)
            key = _footprint_record_key(source_fp, standalone, policy, dedupe)
            if policy == KiCadExtractionMode.INTERNAL and key in seen:
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
                board_embedded_files=board.embedded_files,
                env=env,
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
        lib = KiCadSymbolLib(symbols=[copy.deepcopy(record.symbol)])
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
) -> dict[str, Any]:
    """Build stable JSON metadata for an extracted KiCad library bundle."""
    resolved_project = _resolve_project_path(project_path)
    symbols = tuple(symbol_records) if symbol_records is not None else extract_symbols(resolved_project, mode)
    footprints = (
        tuple(footprint_records)
        if footprint_records is not None
        else extract_footprints(resolved_project, mode)
    )
    data: dict[str, Any] = {
        "schema": "kicad_monkey.library_extraction_bundle.v1",
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
    )
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return path


def _iter_board_embedded_files(board: KiCadPcb) -> Iterable[EmbeddedFile]:
    yield from board.embedded_files
    for footprint in board.footprints:
        yield from footprint.embedded_files


def _write_embedded_model_payload(
    file: EmbeddedFile,
    output_dir: Path,
    written_by_hash: dict[str, Path],
    used_names: set[str],
) -> None:
    if not _is_step_model_name(file.name):
        return
    data = _embedded_file_payload_bytes(file)
    digest = hashlib.sha256(data).hexdigest()
    if digest in written_by_hash:
        return
    path = output_dir / _unique_file_name(_safe_asset_filename(file.name), used_names)
    path.write_bytes(data)
    written_by_hash[digest] = path


def extract_3d_models(
    project_path: Path | str,
    output_dir: Path | str,
) -> tuple[Path, ...]:
    """Extract embedded model payloads from project boards and footprints."""
    out_dir = Path(output_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    written_by_hash: dict[str, Path] = {}
    used_names: set[str] = set()

    resolved_project = _resolve_project_path(project_path)
    for pcb_path in _iter_project_files(resolved_project, ".kicad_pcb"):
        try:
            board = KiCadPcb.from_file(pcb_path)
        except Exception:
            continue
        for file in _iter_board_embedded_files(board):
            _write_embedded_model_payload(file, out_dir, written_by_hash, used_names)
    for fp_path in _iter_project_files(resolved_project, ".kicad_mod"):
        try:
            footprint = KiCadFootprint.from_file(fp_path)
        except Exception:
            continue
        for file in footprint.embedded_files:
            _write_embedded_model_payload(file, out_dir, written_by_hash, used_names)
    return tuple(written_by_hash.values())


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
    "KiCadExtractionDedupePolicy",
    "KiCadExtractionMode",
    "KiCadFootprintExtractionRecord",
    "KiCadModelReferenceKind",
    "KiCadModelReferenceScan",
    "KiCadProjectAssetScan",
    "KiCadSymbolExtractionRecord",
    "build_extraction_metadata_bundle",
    "embed_external_model_payloads",
    "extract_3d_models",
    "extract_footprints",
    "extract_symbols",
    "rehydrate_embedded_model_payloads",
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
