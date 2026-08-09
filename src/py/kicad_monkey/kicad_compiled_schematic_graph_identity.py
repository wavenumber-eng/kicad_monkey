"""Stable UUIDv7 identities for the compiled schematic graph.

This is the package-local copy of the governed generic allocator. Keep the
address shape and normalization semantics byte-for-byte compatible while the
producer remains independently installable.
"""

from __future__ import annotations

import hashlib
import json
import uuid
from collections.abc import Mapping, Sequence

SCH_COMPILED_SCHEMATIC_GRAPH_IDENTITY_NAMESPACE = "sch.compiled_schematic_graph.a0"
SCH_COMPILED_SCHEMATIC_GRAPH_IDENTITY_EPOCH_MS = 1_786_060_800_000


class SchCompiledSchematicGraphIdentityAllocator:
    """Allocate stable UUIDv7 identities from governed source addresses."""

    def __init__(self, *, design_scope: Mapping[str, object]) -> None:
        scope = _normalized_mapping(design_scope)
        if not scope:
            raise ValueError("schematic identity design_scope must not be empty")
        self._design_scope = scope
        self._address_by_id: dict[str, str] = {}
        self._allocated_addresses: set[str] = set()

    def allocate_source(
        self,
        *,
        object_type: str,
        source_identity: Mapping[str, object],
        owner_refs: Sequence[str] = (),
    ) -> str:
        source = _stable_source_selector(
            object_type=object_type,
            source_identity=source_identity,
        )
        if not source:
            raise ValueError(
                f"{object_type} requires governed source identity for stable allocation"
            )
        return self._allocate(
            object_type=object_type,
            identity={
                "source_identity": source,
                "owner_refs": _normalized_refs(owner_refs),
            },
        )

    def allocate_derived(
        self,
        *,
        object_type: str,
        identity: Mapping[str, object],
    ) -> str:
        normalized = _normalized_mapping(identity)
        if not normalized:
            raise ValueError(f"{object_type} derived identity must not be empty")
        return self._allocate(object_type=object_type, identity=normalized)

    def _allocate(
        self,
        *,
        object_type: str,
        identity: Mapping[str, object],
    ) -> str:
        normalized_type = str(object_type or "").strip()
        if not normalized_type.startswith("sch."):
            raise ValueError(
                "schematic occurrence identity object_type must use the sch. namespace"
            )
        address = _canonical_json(
            {
                "namespace": SCH_COMPILED_SCHEMATIC_GRAPH_IDENTITY_NAMESPACE,
                "design_scope": self._design_scope,
                "object_type": normalized_type,
                "identity": identity,
            }
        )
        if address in self._allocated_addresses:
            raise ValueError(
                f"duplicate stable schematic identity address for {normalized_type}"
            )
        object_id = _deterministic_uuidv7(address)
        previous = self._address_by_id.get(object_id)
        if previous is not None and previous != address:
            raise ValueError(
                f"stable schematic identity collision for {normalized_type}: {object_id}"
            )
        self._allocated_addresses.add(address)
        self._address_by_id[object_id] = address
        return object_id


def compiled_schematic_graph_design_scope(
    *, source_cad: object, project: object
) -> dict[str, str]:
    """Return the portable Design namespace used by occurrence allocation."""

    project_row = project if isinstance(project, Mapping) else {}
    project_file = str(
        project_row.get("filename") or project_row.get("name") or ""
    ).strip()
    if not project_file:
        raise ValueError(
            "compiled schematic identity requires a source project filename or name"
        )
    return {
        "source_cad": str(source_cad or "unknown").strip().casefold(),
        "project_file": project_file.replace("\\", "/").casefold(),
    }


def _stable_source_selector(
    *, object_type: str, source_identity: Mapping[str, object]
) -> dict[str, object]:
    normalized = _normalized_mapping(source_identity)
    source_uuid = normalized.get("sch.source_key.source_uuid")
    source_path = normalized.get("sch.source_key.source_path")
    source_record = normalized.get("sch.source_key.source_record")
    source_subobject = normalized.get("sch.source_key.source_subobject")
    compiled_net = normalized.get("sch.source_key.compiled_net")
    artifact_element = normalized.get("sch.source_key.artifact_element")

    if object_type in {"sch.unit_definition", "sch.page_definition"}:
        definition_selector = {
            key: value
            for key, value in (
                ("sch.source_key.source_path", source_path),
                ("sch.source_key.source_uuid", source_uuid),
            )
            if value
        }
        if definition_selector:
            return definition_selector

    if object_type == "sch.terminal_occurrence":
        if source_uuid:
            terminal_selector = {"sch.source_key.source_uuid": source_uuid}
            if source_subobject:
                terminal_selector["sch.source_key.source_subobject"] = source_subobject
            return terminal_selector
        return {}

    if object_type == "sch.local_net_occurrence":
        return {}

    for key, value in (
        ("sch.source_key.source_uuid", source_uuid),
        ("sch.source_key.compiled_net", compiled_net),
        ("sch.source_key.source_path", source_path),
        ("sch.source_key.artifact_element", artifact_element),
        ("sch.source_key.source_record", source_record),
    ):
        if value:
            return {key: value}
    return {}


def _deterministic_uuidv7(address: str) -> str:
    digest = hashlib.sha256(address.encode("utf-8")).digest()
    value = bytearray(16)
    value[:6] = SCH_COMPILED_SCHEMATIC_GRAPH_IDENTITY_EPOCH_MS.to_bytes(6, "big")
    value[6] = 0x70 | (digest[0] & 0x0F)
    value[7] = digest[1]
    value[8] = 0x80 | (digest[2] & 0x3F)
    value[9:] = digest[3:10]
    return str(uuid.UUID(bytes=bytes(value)))


def _normalized_mapping(value: Mapping[str, object]) -> dict[str, object]:
    normalized: dict[str, object] = {}
    for key in sorted(value, key=str):
        text_key = str(key or "").strip()
        if not text_key:
            continue
        item = value[key]
        if item is None:
            continue
        if isinstance(item, Mapping):
            nested = _normalized_mapping(item)
            if nested:
                normalized[text_key] = nested
            continue
        if isinstance(item, Sequence) and not isinstance(item, str | bytes):
            sequence = [str(entry or "").strip() for entry in item]
            normalized[text_key] = [entry for entry in sequence if entry]
            continue
        if isinstance(item, str):
            text = item.strip()
            if text:
                normalized[text_key] = text
            continue
        if isinstance(item, bool | int | float):
            normalized[text_key] = item
    return normalized


def _normalized_refs(values: Sequence[str]) -> list[str]:
    refs = [str(value or "").strip() for value in values]
    return [value for value in refs if value]


def _canonical_json(value: Mapping[str, object]) -> str:
    return json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=True)


__all__ = [
    "SCH_COMPILED_SCHEMATIC_GRAPH_IDENTITY_EPOCH_MS",
    "SCH_COMPILED_SCHEMATIC_GRAPH_IDENTITY_NAMESPACE",
    "SchCompiledSchematicGraphIdentityAllocator",
    "compiled_schematic_graph_design_scope",
]
