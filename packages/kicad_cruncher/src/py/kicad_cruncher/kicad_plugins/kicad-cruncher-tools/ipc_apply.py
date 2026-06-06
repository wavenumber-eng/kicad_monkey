from __future__ import annotations

from collections import Counter
from contextlib import suppress
from importlib import import_module
from typing import Protocol, cast

PCB_CLEAN_MUTATION_REQUEST_SCHEMA = "kicad_cruncher.pcb.clean.mutation_request.v0"


class _LayerUserNameSetter(Protocol):
    def set_layer_user_name(self, layer: object, user_name: str | None) -> object: ...


class _LayerNameSetter(Protocol):
    def set_layer_name(self, layer: object, name: str) -> object: ...


class _VisibleField(Protocol):
    visible: bool


class _HideField(Protocol):
    hide: bool


class _ValueFieldOwner(Protocol):
    value_field: object


class _BoardLayerModule(Protocol):
    def canonical_name(self, layer: object) -> str: ...


def apply_pcb_clean_mutation_request(
    board: object,
    mutation_request: dict[str, object],
) -> dict[str, object]:
    """Apply a PCB clean mutation request to a live KiCad IPC board."""
    operations = _operation_list(mutation_request.get("operations"))
    commit_label = str(mutation_request.get("commit_label") or "KiCad Cruncher PCB clean")
    commit = _call_required(board, "begin_commit")
    changed: list[object] = []
    applied: Counter[str] = Counter()
    skipped: Counter[str] = Counter()

    try:
        for operation in operations:
            result = _apply_operation(board, operation)
            op_name = str(operation.get("op") or "unknown")
            if result.changed:
                applied[op_name] += 1
                changed.extend(result.updated_items)
            else:
                skipped[result.reason or op_name] += 1

        updated_items = _unique_objects(changed)
        if updated_items:
            _call_required(board, "update_items", updated_items)
            _call_required(board, "push_commit", commit, commit_label)
            status = "applied"
        else:
            _call_required(board, "drop_commit", commit)
            status = "no_changes"
    except Exception:
        _call_required(board, "drop_commit", commit)
        raise

    return {
        "schema": "kicad_cruncher.pcb.clean.ipc_apply_result.v0",
        "status": status,
        "commit_label": commit_label,
        "updated_items": len(updated_items) if status == "applied" else 0,
        "applied": dict(sorted(applied.items())),
        "skipped": dict(sorted(skipped.items())),
    }


class _OperationResult:
    def __init__(
        self,
        *,
        changed: bool,
        updated_items: list[object] | None = None,
        reason: str = "",
    ) -> None:
        self.changed = changed
        self.updated_items = updated_items or []
        self.reason = reason


def _apply_operation(board: object, operation: dict[str, object]) -> _OperationResult:
    op_name = str(operation.get("op") or "")
    if op_name == "reset_layer_user_name":
        return _reset_layer_user_name(board, operation)
    if op_name == "remove_footprint_item":
        return _remove_footprint_item(board, operation)
    if op_name == "hide_footprint_value_field":
        return _hide_footprint_value_field(board, operation)
    if op_name == "remove_board_item":
        return _remove_board_item(board, operation)
    return _OperationResult(changed=False, reason=f"unsupported_operation:{op_name or 'blank'}")


def _reset_layer_user_name(board: object, operation: dict[str, object]) -> _OperationResult:
    layer = _find_board_layer(board, operation)
    if layer is None:
        return _OperationResult(changed=False, reason="layer_not_found")

    canonical_name = str(operation.get("canonical_name") or "")
    if _has_callable(board, "set_layer_user_name"):
        cast(_LayerUserNameSetter, board).set_layer_user_name(layer, None)
        return _OperationResult(changed=True)
    if _has_callable(board, "set_layer_name"):
        cast(_LayerNameSetter, board).set_layer_name(layer, canonical_name)
        return _OperationResult(changed=True)
    return _OperationResult(changed=False, reason="layer_name_reset_not_supported")


def _remove_footprint_item(board: object, operation: dict[str, object]) -> _OperationResult:
    footprint = _find_footprint(board, operation)
    if footprint is None:
        return _OperationResult(changed=False, reason="footprint_not_found")

    item = _find_footprint_item(footprint, operation)
    if item is None:
        return _OperationResult(changed=False, reason="footprint_item_not_found")
    if not _remove_item_from_owner(footprint, item, operation):
        return _OperationResult(changed=False, reason="footprint_item_remove_not_supported")
    return _OperationResult(changed=True, updated_items=[footprint])


def _hide_footprint_value_field(board: object, operation: dict[str, object]) -> _OperationResult:
    footprint = _find_footprint(board, operation)
    if footprint is None:
        return _OperationResult(changed=False, reason="footprint_not_found")

    field = _find_footprint_field(footprint, operation)
    if field is None:
        return _OperationResult(changed=False, reason="footprint_value_field_not_found")
    if hasattr(field, "visible"):
        visible_field = cast(_VisibleField, field)
        if visible_field.visible is False:
            return _OperationResult(changed=False, reason="value_field_already_hidden")
        visible_field.visible = False
    elif hasattr(field, "hide"):
        hide_field = cast(_HideField, field)
        if hide_field.hide is True:
            return _OperationResult(changed=False, reason="value_field_already_hidden")
        hide_field.hide = True
    else:
        return _OperationResult(changed=False, reason="value_field_visibility_not_supported")

    if hasattr(footprint, "value_field"):
        with suppress(Exception):
            cast(_ValueFieldOwner, footprint).value_field = field
    return _OperationResult(changed=True, updated_items=[footprint])


def _remove_board_item(board: object, operation: dict[str, object]) -> _OperationResult:
    collection_name = str(operation.get("collection") or "")
    item = _find_item_in_collection(getattr(board, collection_name, None), operation)
    if item is None:
        return _OperationResult(changed=False, reason="board_item_not_found")
    collection = getattr(board, collection_name, None)
    if isinstance(collection, list):
        collection.remove(item)
        return _OperationResult(changed=True, updated_items=[board])
    return _OperationResult(changed=False, reason="board_item_remove_not_supported")


def _find_board_layer(board: object, operation: dict[str, object]) -> object | None:
    layer_ordinal = _optional_int(operation.get("layer_ordinal"))
    canonical_name = str(operation.get("canonical_name") or "")
    layers = _call_optional(board, "get_enabled_layers")
    if not isinstance(layers, list | tuple):
        layers = getattr(board, "layers", ())
    for layer in layers or ():
        if layer_ordinal is not None and _optional_int(layer) == layer_ordinal:
            return layer
        if canonical_name and _canonical_layer_name(layer) == canonical_name:
            return layer
    return None


def _find_footprint(board: object, operation: dict[str, object]) -> object | None:
    footprints = _call_optional(board, "get_footprints")
    if not isinstance(footprints, list | tuple):
        footprints = getattr(board, "footprints", ())
    target_uuid = str(operation.get("footprint_uuid") or "")
    target_index = _optional_int(operation.get("footprint_index"))
    for index, footprint in enumerate(footprints or ()):
        if target_uuid and _object_id_text(footprint) == target_uuid:
            return footprint
        if target_index is not None and index == target_index:
            return footprint
    return None


def _find_footprint_item(footprint: object, operation: dict[str, object]) -> object | None:
    definition = getattr(footprint, "definition", None)
    item = _find_item_in_collection(getattr(definition, "items", None), operation)
    if item is not None:
        return item
    collection_name = str(operation.get("collection") or "")
    return _find_item_in_collection(getattr(footprint, collection_name, None), operation)


def _find_footprint_field(footprint: object, operation: dict[str, object]) -> object | None:
    value_field = getattr(footprint, "value_field", None)
    if value_field is not None and _item_matches(value_field, operation):
        return value_field
    for collection_name in ("properties", "fp_texts"):
        item = _find_item_in_collection(getattr(footprint, collection_name, None), operation)
        if item is not None:
            return item
    definition = getattr(footprint, "definition", None)
    return _find_item_in_collection(getattr(definition, "items", None), operation)


def _find_item_in_collection(collection: object, operation: dict[str, object]) -> object | None:
    if not isinstance(collection, list | tuple):
        return None
    target_uuid = str(operation.get("item_uuid") or "")
    if target_uuid:
        return _find_item_by_uuid(collection, target_uuid)

    target_index = _optional_int(operation.get("item_index"))
    target_layer = str(operation.get("layer") or "")
    item_type = str(operation.get("item_type") or "")
    for index, item in enumerate(collection):
        if _item_matches_index_layer(item, index, target_index, target_layer):
            return item
        if _item_matches_type_layer(item, item_type, target_layer):
            return item
    return None


def _find_item_by_uuid(
    collection: list[object] | tuple[object, ...],
    target_uuid: str,
) -> object | None:
    for item in collection:
        if _object_id_text(item) == target_uuid:
            return item
    return None


def _item_matches_index_layer(
    item: object,
    index: int,
    target_index: int | None,
    target_layer: str,
) -> bool:
    return (
        target_index is not None
        and index == target_index
        and (not target_layer or _item_layer_name(item) == target_layer)
    )


def _item_matches_type_layer(item: object, item_type: str, target_layer: str) -> bool:
    return bool(
        item_type
        and _item_type_name(item) == item_type
        and _item_layer_name(item) == target_layer
    )


def _remove_item_from_owner(
    footprint: object,
    item: object,
    operation: dict[str, object],
) -> bool:
    definition = getattr(footprint, "definition", None)
    if definition is not None and _remove_from_collection(definition, "items", item):
        return True
    collection_name = str(operation.get("collection") or "")
    return bool(collection_name and _remove_from_collection(footprint, collection_name, item))


def _remove_from_collection(owner: object, collection_name: str, item: object) -> bool:
    collection = getattr(owner, collection_name, None)
    if isinstance(collection, list) and item in collection:
        collection.remove(item)
        return True
    return False


def _item_matches(item: object, operation: dict[str, object]) -> bool:
    target_uuid = str(operation.get("item_uuid") or "")
    if target_uuid:
        return _object_id_text(item) == target_uuid
    field_name = str(operation.get("field_name") or "")
    if field_name and str(getattr(item, "name", "") or "") != field_name:
        return False
    layer = str(operation.get("layer") or "")
    return not layer or _item_layer_name(item) == layer


def _operation_list(value: object) -> list[dict[str, object]]:
    if not isinstance(value, list):
        return []
    operations: list[dict[str, object]] = []
    for item in value:
        if isinstance(item, dict):
            operations.append(dict(item))
    return operations


def _unique_objects(items: list[object]) -> list[object]:
    unique: list[object] = []
    seen: set[int] = set()
    for item in items:
        identity = id(item)
        if identity in seen:
            continue
        seen.add(identity)
        unique.append(item)
    return unique


def _object_id_text(item: object) -> str:
    for name in ("id", "uuid", "kiid"):
        value = getattr(item, name, None)
        text = _text_id(value)
        if text:
            return text
    proto = getattr(item, "proto", None)
    text = _text_id(getattr(proto, "id", None))
    return text


def _text_id(value: object) -> str:
    if value is None:
        return ""
    nested_value = getattr(value, "value", None)
    if nested_value is not None:
        return str(nested_value or "").strip()
    nested_id = getattr(value, "id", None)
    if nested_id is not None and nested_id is not value:
        return _text_id(nested_id)
    return str(value or "").strip()


def _canonical_layer_name(layer: object) -> str:
    direct = getattr(layer, "canonical_name", None)
    if direct:
        return str(direct)
    try:
        board_layer = cast(_BoardLayerModule, import_module("kipy.util.board_layer"))
        return str(board_layer.canonical_name(layer))
    except Exception:
        return str(layer or "")


def _item_layer_name(item: object) -> str:
    value = getattr(item, "layer", "")
    return _canonical_layer_name(value) if value is not None else ""


def _item_type_name(item: object) -> str:
    class_name = item.__class__.__name__
    if class_name.startswith("Fp"):
        return "fp_" + class_name[2:].lower()
    if class_name.startswith("Gr"):
        return "gr_" + class_name[2:].lower()
    return class_name.lower()


def _optional_int(value: object) -> int | None:
    if value is None:
        return None
    try:
        return int(str(value))
    except (TypeError, ValueError):
        return None


def _has_callable(target: object, name: str) -> bool:
    return callable(getattr(target, name, None))


def _call_optional(target: object, name: str) -> object:
    method = getattr(target, name, None)
    if not callable(method):
        return None
    return method()


def _call_required(target: object, name: str, *args: object) -> object:
    method = getattr(target, name, None)
    if not callable(method):
        raise RuntimeError(f"KiCad IPC board does not expose {name}()")
    return method(*args)
