"""PCB cleanup config and planning helpers."""

from __future__ import annotations

from collections import Counter
from dataclasses import dataclass, field
from fnmatch import fnmatchcase
from pathlib import Path
from typing import Protocol, cast

from kicad_monkey import KiCadPcb

from kicad_cruncher.config_json import load_json_config, render_commented_jsonc

PCB_CLEAN_CONFIG_FILENAME = "pcb.clean.config"
PCB_CLEAN_CONFIG_SCHEMA = "kicad_cruncher.pcb.clean.config.v0"
PCB_CLEAN_PLAN_SCHEMA = "kicad_cruncher.pcb.clean.plan.v0"
PCB_CLEAN_MUTATION_REQUEST_SCHEMA = "kicad_cruncher.pcb.clean.mutation_request.v0"

_DEFAULT_INCLUDE_LAYERS = ("*.User", "User.*", "*.Fab", "*.CrtYd")
_DEFAULT_EXCLUDE_LAYERS = ("F.Cu", "B.Cu", "Edge.Cuts")
_BOARD_GRAPHIC_COLLECTIONS = (
    ("gr_texts", "gr_text"),
    ("gr_lines", "gr_line"),
    ("gr_rects", "gr_rect"),
    ("gr_arcs", "gr_arc"),
    ("gr_circles", "gr_circle"),
    ("gr_polys", "gr_poly"),
    ("gr_curves", "gr_curve"),
    ("gr_text_boxes", "gr_text_box"),
)
_FOOTPRINT_GRAPHIC_COLLECTIONS = (
    ("fp_texts", "fp_text"),
    ("fp_text_boxes", "fp_text_box"),
    ("fp_lines", "fp_line"),
    ("fp_arcs", "fp_arc"),
    ("fp_circles", "fp_circle"),
    ("fp_rects", "fp_rect"),
    ("fp_polys", "fp_poly"),
)

_PCB_CLEAN_CONFIG_HEADER = (
    "KiCad Cruncher PCB Clean config.",
    "",
    "This file is JSONC. Comments and trailing commas are accepted.",
    "Apply removes configured documentation-layer graphics and hides visible Value fields.",
    "Copper, pads, models, routing, and Edge.Cuts stay protected.",
    "Protection is enforced through layer exclusion and object selection.",
    "Daemon/plugin usage shares this planner but returns a KiCad IPC mutation request.",
    "Daemon/plugin usage does not edit a board file behind the editor.",
)
_PCB_CLEAN_CONFIG_COMMENTS = {
    ("schema",): "Required config contract id.",
    ("targets",): "Cleanup target groups. Set a group false to keep that class untouched.",
    ("targets", "user_layers"): "Reset user layer names that match the configured layer selection.",
    (
        "targets",
        "generated_graphics",
    ): "Remove generated cleanup metadata graphics owned by this tool.",
    ("targets", "footprint_graphics"): "Remove matching footprint-local documentation graphics.",
    (
        "targets",
        "board_graphics",
    ): "Remove matching board-level documentation graphics; disabled by default.",
    ("targets", "value_fields"): "Hide visible footprint Value fields on selected layers.",
    ("safety",): "Safety gates that prevent cleanup from touching electrical or required objects.",
    ("safety", "protect_pads"): "Never remove pads.",
    ("safety", "protect_models"): "Never remove 3D models.",
    ("safety", "protect_mandatory_fields"): "Never remove mandatory reference/value field objects.",
    (
        "safety",
        "require_explicit_apply",
    ): "Require explicit --apply for file mutation; dry-run remains non-mutating.",
    ("layers",): "Layer selection globs used by cleanup targets.",
    ("layers", "include"): "Glob patterns for documentation layers eligible for cleanup.",
    ("layers", "exclude"): "Glob patterns that are always protected even when included elsewhere.",
    ("metadata",): "Generated-item metadata used to recognize this tool's own cleanup annotations.",
    ("metadata", "field_name"): "KiCad custom field name used for generated cleanup metadata.",
    ("metadata", "schema"): "Metadata schema id stored on generated cleanup annotations.",
}


@dataclass
class _SelectionTally:
    total: int = 0
    by_layer: Counter[str] = field(default_factory=Counter)
    by_type: Counter[str] = field(default_factory=Counter)
    protected: int = 0
    protected_by_reason: Counter[str] = field(default_factory=Counter)

    def add_candidate(self, *, item_type: str, layer: str) -> None:
        self.total += 1
        self.by_type[item_type] += 1
        self.by_layer[layer] += 1

    def add_protected(self, *, reason: str) -> None:
        self.protected += 1
        self.protected_by_reason[reason] += 1

    def to_json(self) -> dict[str, object]:
        return {
            "total": self.total,
            "by_type": dict(sorted(self.by_type.items())),
            "by_layer": dict(sorted(self.by_layer.items())),
            "protected": self.protected,
            "protected_by_reason": dict(sorted(self.protected_by_reason.items())),
        }


class _HideableField(Protocol):
    hide: bool


@dataclass
class _MutationReport:
    layer_user_names_reset: int = 0
    footprint_graphics_removed: _SelectionTally = field(default_factory=_SelectionTally)
    board_graphics_removed: _SelectionTally = field(default_factory=_SelectionTally)
    generated_items_removed: _SelectionTally = field(default_factory=_SelectionTally)
    value_fields_hidden: _SelectionTally = field(default_factory=_SelectionTally)

    def to_json(self, saved_board: Path) -> dict[str, object]:
        return {
            "saved_board": str(saved_board),
            "layer_user_names_reset": self.layer_user_names_reset,
            "footprint_graphics_removed": self.footprint_graphics_removed.to_json(),
            "board_graphics_removed": self.board_graphics_removed.to_json(),
            "generated_items_removed": self.generated_items_removed.to_json(),
            "value_fields_hidden": self.value_fields_hidden.to_json(),
        }


def default_pcb_clean_config() -> dict[str, object]:
    """Return the default PCB cleanup config object."""
    return {
        "schema": PCB_CLEAN_CONFIG_SCHEMA,
        "targets": {
            "user_layers": True,
            "generated_graphics": True,
            "footprint_graphics": True,
            "board_graphics": False,
            "value_fields": True,
        },
        "safety": {
            "protect_pads": True,
            "protect_models": True,
            "protect_mandatory_fields": True,
            "require_explicit_apply": True,
        },
        "layers": {
            "include": list(_DEFAULT_INCLUDE_LAYERS),
            "exclude": list(_DEFAULT_EXCLUDE_LAYERS),
        },
        "metadata": {
            "field_name": "ALX_HLR_META",
            "schema": "wavenumber.kicad_cruncher.pcb_clean.metadata.v0",
        },
    }


def write_default_pcb_clean_config(path: Path) -> None:
    """Write the default PCB cleanup JSONC config."""
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(_default_pcb_clean_config_text(), encoding="utf-8")


def load_pcb_clean_config(path: Path) -> dict[str, object]:
    """Load a PCB cleanup JSON or JSONC config."""
    config = load_json_config(path)
    schema = str(config.get("schema", "")).strip()
    if schema != PCB_CLEAN_CONFIG_SCHEMA:
        raise ValueError(f"{path} schema must be {PCB_CLEAN_CONFIG_SCHEMA!r}")
    return config


def plan_pcb_clean(
    *,
    board_path: Path | None,
    config_path: Path | None,
    dry_run: bool,
) -> dict[str, object]:
    """Return a deterministic PCB cleanup plan report."""
    config = (
        load_pcb_clean_config(config_path)
        if config_path is not None
        else default_pcb_clean_config()
    )
    board_report = _plan_board_cleanup(board_path, config) if board_path is not None else {}
    return {
        "schema": PCB_CLEAN_PLAN_SCHEMA,
        "status": "planned",
        "dry_run": dry_run,
        "board": str(board_path) if board_path is not None else None,
        "config": str(config_path) if config_path is not None else None,
        "config_schema": config.get("schema"),
        "layer_selection": _layer_selection(config),
        "planned_operations": _planned_operations(board_report),
        "board_report": board_report,
        "mutation_supported": True,
        "apply_policy": _apply_policy(),
    }


def apply_pcb_clean(
    *,
    board_path: Path | None,
    config_path: Path | None,
) -> dict[str, object]:
    """Apply configured PCB cleanup mutations and return a deterministic report."""
    config = (
        load_pcb_clean_config(config_path)
        if config_path is not None
        else default_pcb_clean_config()
    )
    mutation_report = (
        _apply_board_cleanup(board_path, config) if board_path is not None else {}
    )
    return {
        "schema": PCB_CLEAN_PLAN_SCHEMA,
        "status": "applied" if mutation_report.get("status") == "applied" else "not_applied",
        "dry_run": False,
        "board": str(board_path) if board_path is not None else None,
        "config": str(config_path) if config_path is not None else None,
        "config_schema": config.get("schema"),
        "layer_selection": _layer_selection(config),
        "mutation_report": mutation_report,
        "mutation_supported": True,
        "apply_policy": _apply_policy(),
    }


def build_pcb_clean_mutation_request(
    *,
    board_path: Path | None,
    config_path: Path | None,
) -> dict[str, object]:
    """Return a KiCad IPC-friendly mutation request without mutating files."""
    config = (
        load_pcb_clean_config(config_path)
        if config_path is not None
        else default_pcb_clean_config()
    )
    board_report = _plan_board_cleanup(board_path, config) if board_path is not None else {}
    operations = (
        _board_cleanup_operations(board_path, config)
        if board_path is not None and board_report.get("status") == "loaded"
        else []
    )
    status = (
        "planned"
        if operations or board_report.get("status") == "loaded"
        else "not_loaded"
    )
    return {
        "schema": PCB_CLEAN_MUTATION_REQUEST_SCHEMA,
        "status": status,
        "board": str(board_path) if board_path is not None else None,
        "config": str(config_path) if config_path is not None else None,
        "config_schema": config.get("schema"),
        "operation_target": "kicad-ipc",
        "commit_label": "KiCad Cruncher PCB clean",
        "plugin_apply_required": True,
        "daemon_file_apply_allowed": False,
        "operations": operations,
        "operation_counts": _operation_counts(operations),
        "board_report": board_report,
        "apply_policy": _apply_policy(),
    }


def _default_pcb_clean_config_text() -> str:
    return render_commented_jsonc(
        default_pcb_clean_config(),
        comments_by_path=_PCB_CLEAN_CONFIG_COMMENTS,
        header_lines=_PCB_CLEAN_CONFIG_HEADER,
    )


def _plan_board_cleanup(board_path: Path, config: dict[str, object]) -> dict[str, object]:
    resolved_path = _resolve_board_path(board_path)
    if resolved_path is None:
        return {
            "status": "not_loaded",
            "reason": "board file was not found",
            "input": str(board_path),
        }

    pcb = KiCadPcb(resolved_path)
    targets = _section(config, "targets")
    layer_user_resets = _layer_user_reset_candidates(pcb, config)
    footprint_tally = _footprint_graphics_tally(pcb, config)
    board_tally = (
        _board_graphics_tally(pcb, config)
        if _bool_value(targets, "board_graphics", False)
        else None
    )
    generated_tally = (
        _generated_cleanup_tally(pcb, config)
        if _bool_value(targets, "generated_graphics", True)
        else None
    )

    return {
        "status": "loaded",
        "input": str(board_path),
        "resolved_board": str(resolved_path),
        "inventory": {
            "layers": len(pcb.layers),
            "footprints": len(pcb.footprints),
            "generated_items": len(pcb.generated_items),
        },
        "layer_user_name_resets": layer_user_resets,
        "footprint_graphics": footprint_tally.to_json(),
        "footprint_local_items": footprint_tally.to_json(),
        "board_graphics": board_tally.to_json() if board_tally is not None else _disabled_tally(),
        "generated_items": (
            generated_tally.to_json() if generated_tally is not None else _disabled_tally()
        ),
        "value_fields": (
            _value_field_tally(pcb, config).to_json()
            if _bool_value(targets, "value_fields", True)
            else _disabled_tally()
        ),
    }


def _resolve_board_path(input_path: Path) -> Path | None:
    if input_path.is_file() and input_path.suffix == ".kicad_pcb":
        return input_path
    if input_path.is_file() and input_path.suffix == ".kicad_pro":
        sibling_board = input_path.with_suffix(".kicad_pcb")
        if sibling_board.is_file():
            return sibling_board
        candidates = sorted(input_path.parent.glob("*.kicad_pcb"))
        return candidates[0] if len(candidates) == 1 else None
    return None


def _layer_selection(config: dict[str, object]) -> dict[str, object]:
    layers = _section(config, "layers")
    return {
        "include": _string_list(layers.get("include"), _DEFAULT_INCLUDE_LAYERS),
        "exclude": _string_list(layers.get("exclude"), _DEFAULT_EXCLUDE_LAYERS),
    }


def _layer_user_reset_candidates(
    pcb: KiCadPcb,
    config: dict[str, object],
) -> list[dict[str, object]]:
    targets = _section(config, "targets")
    if not _bool_value(targets, "user_layers", True):
        return []

    resets: list[dict[str, object]] = []
    for layer in pcb.layers:
        user_name = getattr(layer, "user_name", None)
        canonical_name = str(getattr(layer, "canonical_name", "") or "")
        if user_name and _layer_allowed(canonical_name, config):
            resets.append(
                {
                    "ordinal": int(getattr(layer, "ordinal", -1)),
                    "canonical_name": canonical_name,
                    "user_name": str(user_name),
                }
            )
    return resets


def _footprint_graphics_tally(pcb: KiCadPcb, config: dict[str, object]) -> _SelectionTally:
    targets = _section(config, "targets")
    if not _bool_value(targets, "footprint_graphics", True):
        return _SelectionTally()

    tally = _SelectionTally()
    for footprint in pcb.footprints:
        for collection_name, item_type in _FOOTPRINT_GRAPHIC_COLLECTIONS:
            for item in getattr(footprint, collection_name, ()) or ():
                if _is_value_text(item, item_type):
                    continue
                _count_layer_item(tally, item=item, item_type=item_type, config=config)
    return tally


def _board_graphics_tally(pcb: KiCadPcb, config: dict[str, object]) -> _SelectionTally:
    tally = _SelectionTally()
    for collection_name, item_type in _BOARD_GRAPHIC_COLLECTIONS:
        for item in getattr(pcb, collection_name, ()) or ():
            _count_layer_item(tally, item=item, item_type=item_type, config=config)
    return tally


def _generated_cleanup_tally(pcb: KiCadPcb, config: dict[str, object]) -> _SelectionTally:
    tally = _SelectionTally()
    for item in pcb.generated_items:
        layer = _item_layer(item)
        if _layer_allowed(layer, config):
            tally.add_candidate(item_type="generated", layer=layer)
    return tally


def _count_layer_item(
    tally: _SelectionTally,
    *,
    item: object,
    item_type: str,
    config: dict[str, object],
) -> None:
    layer = _item_layer(item)
    if not _layer_allowed(layer, config):
        return
    tally.add_candidate(item_type=item_type, layer=layer)


def _value_field_tally(pcb: KiCadPcb, config: dict[str, object]) -> _SelectionTally:
    tally = _SelectionTally()
    for footprint in pcb.footprints:
        for item in getattr(footprint, "properties", ()) or ():
            _count_value_field(tally, item=item, item_type="property", config=config)
        for item in getattr(footprint, "fp_texts", ()) or ():
            _count_value_field(tally, item=item, item_type="fp_text", config=config)
    return tally


def _count_value_field(
    tally: _SelectionTally,
    *,
    item: object,
    item_type: str,
    config: dict[str, object],
) -> None:
    if not _is_value_field(item, item_type):
        return
    layer = _item_layer(item)
    if not _layer_allowed(layer, config):
        return
    if not _is_graphical_value_field(item, item_type):
        return
    if bool(getattr(item, "hide", False)):
        tally.add_protected(reason="already_hidden")
        return
    tally.add_candidate(item_type=item_type, layer=layer)


def _apply_board_cleanup(board_path: Path, config: dict[str, object]) -> dict[str, object]:
    resolved_path = _resolve_board_path(board_path)
    if resolved_path is None:
        return {
            "status": "not_loaded",
            "reason": "board file was not found",
            "input": str(board_path),
        }

    pcb = KiCadPcb(resolved_path)
    report = _MutationReport()
    targets = _section(config, "targets")
    if _bool_value(targets, "user_layers", True):
        report.layer_user_names_reset = _apply_layer_user_name_resets(pcb, config)
    if _bool_value(targets, "footprint_graphics", True):
        _apply_footprint_graphic_removals(pcb, config, report.footprint_graphics_removed)
    if _bool_value(targets, "board_graphics", False):
        _apply_board_graphic_removals(pcb, config, report.board_graphics_removed)
    if _bool_value(targets, "generated_graphics", True):
        _apply_generated_item_removals(pcb, config, report.generated_items_removed)
    if _bool_value(targets, "value_fields", True):
        _apply_value_field_hides(pcb, config, report.value_fields_hidden)

    pcb.save(resolved_path)
    payload = report.to_json(resolved_path)
    payload["status"] = "applied"
    payload["input"] = str(board_path)
    return payload


def _apply_layer_user_name_resets(pcb: KiCadPcb, config: dict[str, object]) -> int:
    reset_count = 0
    for layer in pcb.layers:
        user_name = getattr(layer, "user_name", None)
        canonical_name = str(getattr(layer, "canonical_name", "") or "")
        if user_name and _layer_allowed(canonical_name, config):
            layer.user_name = None
            reset_count += 1
    return reset_count


def _apply_footprint_graphic_removals(
    pcb: KiCadPcb,
    config: dict[str, object],
    tally: _SelectionTally,
) -> None:
    for footprint in pcb.footprints:
        for collection_name, item_type in _FOOTPRINT_GRAPHIC_COLLECTIONS:
            collection = getattr(footprint, collection_name, None)
            if not isinstance(collection, list):
                continue
            for item in list(collection):
                if _is_value_text(item, item_type) or not _layer_allowed(_item_layer(item), config):
                    continue
                collection.remove(item)
                tally.add_candidate(item_type=item_type, layer=_item_layer(item))


def _apply_board_graphic_removals(
    pcb: KiCadPcb,
    config: dict[str, object],
    tally: _SelectionTally,
) -> None:
    for collection_name, item_type in _BOARD_GRAPHIC_COLLECTIONS:
        collection = getattr(pcb, collection_name, None)
        if not isinstance(collection, list):
            continue
        for item in list(collection):
            if not _layer_allowed(_item_layer(item), config):
                continue
            collection.remove(item)
            tally.add_candidate(item_type=item_type, layer=_item_layer(item))


def _apply_generated_item_removals(
    pcb: KiCadPcb,
    config: dict[str, object],
    tally: _SelectionTally,
) -> None:
    for item in list(pcb.generated_items):
        layer = _item_layer(item)
        if _layer_allowed(layer, config) and pcb.remove_object(item):
            tally.add_candidate(item_type="generated", layer=layer)


def _apply_value_field_hides(
    pcb: KiCadPcb,
    config: dict[str, object],
    tally: _SelectionTally,
) -> None:
    for footprint in pcb.footprints:
        for item in getattr(footprint, "properties", ()) or ():
            _apply_value_field_hide(tally, item=item, item_type="property", config=config)
        for item in getattr(footprint, "fp_texts", ()) or ():
            _apply_value_field_hide(tally, item=item, item_type="fp_text", config=config)


def _apply_value_field_hide(
    tally: _SelectionTally,
    *,
    item: object,
    item_type: str,
    config: dict[str, object],
) -> None:
    if not _is_value_field(item, item_type):
        return
    layer = _item_layer(item)
    if not _layer_allowed(layer, config) or not _is_graphical_value_field(item, item_type):
        return
    if bool(getattr(item, "hide", False)):
        tally.add_protected(reason="already_hidden")
        return
    cast(_HideableField, item).hide = True
    tally.add_candidate(item_type=item_type, layer=layer)


def _board_cleanup_operations(
    board_path: Path,
    config: dict[str, object],
) -> list[dict[str, object]]:
    resolved_path = _resolve_board_path(board_path)
    if resolved_path is None:
        return []

    pcb = KiCadPcb(resolved_path)
    targets = _section(config, "targets")
    operations: list[dict[str, object]] = []
    if _bool_value(targets, "user_layers", True):
        operations.extend(_layer_user_name_reset_operations(pcb, config))
    if _bool_value(targets, "footprint_graphics", True):
        operations.extend(_footprint_graphic_removal_operations(pcb, config))
    if _bool_value(targets, "board_graphics", False):
        operations.extend(_board_graphic_removal_operations(pcb, config))
    if _bool_value(targets, "generated_graphics", True):
        operations.extend(_generated_item_removal_operations(pcb, config))
    if _bool_value(targets, "value_fields", True):
        operations.extend(_value_field_hide_operations(pcb, config))
    return operations


def _layer_user_name_reset_operations(
    pcb: KiCadPcb,
    config: dict[str, object],
) -> list[dict[str, object]]:
    operations: list[dict[str, object]] = []
    for candidate in _layer_user_reset_candidates(pcb, config):
        operations.append(
            {
                "op": "reset_layer_user_name",
                "target": "board.layer",
                "layer_ordinal": candidate["ordinal"],
                "canonical_name": candidate["canonical_name"],
                "previous_user_name": candidate["user_name"],
            }
        )
    return operations


def _footprint_graphic_removal_operations(
    pcb: KiCadPcb,
    config: dict[str, object],
) -> list[dict[str, object]]:
    operations: list[dict[str, object]] = []
    for footprint_index, footprint in enumerate(pcb.footprints):
        footprint_payload = _footprint_selector(footprint, footprint_index)
        for collection_name, item_type in _FOOTPRINT_GRAPHIC_COLLECTIONS:
            for item_index, item in enumerate(getattr(footprint, collection_name, ()) or ()):
                if _is_value_text(item, item_type) or not _layer_allowed(_item_layer(item), config):
                    continue
                operations.append(
                    {
                        "op": "remove_footprint_item",
                        "target": "footprint.definition.item",
                        "collection": collection_name,
                        "item_type": item_type,
                        "item_index": item_index,
                        "item_uuid": _optional_text(getattr(item, "uuid", None)),
                        "layer": _item_layer(item),
                        **footprint_payload,
                    }
                )
    return operations


def _board_graphic_removal_operations(
    pcb: KiCadPcb,
    config: dict[str, object],
) -> list[dict[str, object]]:
    operations: list[dict[str, object]] = []
    for collection_name, item_type in _BOARD_GRAPHIC_COLLECTIONS:
        for item_index, item in enumerate(getattr(pcb, collection_name, ()) or ()):
            if not _layer_allowed(_item_layer(item), config):
                continue
            operations.append(
                {
                    "op": "remove_board_item",
                    "target": "board.item",
                    "collection": collection_name,
                    "item_type": item_type,
                    "item_index": item_index,
                    "item_uuid": _optional_text(getattr(item, "uuid", None)),
                    "layer": _item_layer(item),
                }
            )
    return operations


def _generated_item_removal_operations(
    pcb: KiCadPcb,
    config: dict[str, object],
) -> list[dict[str, object]]:
    operations: list[dict[str, object]] = []
    for item_index, item in enumerate(pcb.generated_items):
        layer = _item_layer(item)
        if not _layer_allowed(layer, config):
            continue
        operations.append(
            {
                "op": "remove_board_item",
                "target": "board.generated_item",
                "collection": "generated_items",
                "item_type": "generated",
                "item_index": item_index,
                "item_uuid": _optional_text(getattr(item, "uuid", None)),
                "layer": layer,
            }
        )
    return operations


def _value_field_hide_operations(
    pcb: KiCadPcb,
    config: dict[str, object],
) -> list[dict[str, object]]:
    operations: list[dict[str, object]] = []
    for footprint_index, footprint in enumerate(pcb.footprints):
        footprint_payload = _footprint_selector(footprint, footprint_index)
        for item_index, item in enumerate(getattr(footprint, "properties", ()) or ()):
            operation = _value_field_hide_operation(
                item=item,
                item_type="property",
                collection_name="properties",
                item_index=item_index,
                config=config,
            )
            if operation:
                operations.append({**operation, **footprint_payload})
        for item_index, item in enumerate(getattr(footprint, "fp_texts", ()) or ()):
            operation = _value_field_hide_operation(
                item=item,
                item_type="fp_text",
                collection_name="fp_texts",
                item_index=item_index,
                config=config,
            )
            if operation:
                operations.append({**operation, **footprint_payload})
    return operations


def _value_field_hide_operation(
    *,
    item: object,
    item_type: str,
    collection_name: str,
    item_index: int,
    config: dict[str, object],
) -> dict[str, object] | None:
    if not _is_value_field(item, item_type):
        return None
    layer = _item_layer(item)
    if not _layer_allowed(layer, config) or not _is_graphical_value_field(item, item_type):
        return None
    if bool(getattr(item, "hide", False)):
        return None
    return {
        "op": "hide_footprint_value_field",
        "target": "footprint.field",
        "collection": collection_name,
        "field_type": item_type,
        "field_name": "Value",
        "item_index": item_index,
        "item_uuid": _optional_text(getattr(item, "uuid", None)),
        "layer": layer,
    }


def _footprint_selector(footprint: object, footprint_index: int) -> dict[str, object]:
    return {
        "footprint_index": footprint_index,
        "footprint_uuid": _optional_text(getattr(footprint, "uuid", None)),
        "footprint_reference": _footprint_property_value(footprint, "Reference"),
        "footprint_library_link": str(getattr(footprint, "library_link", "") or ""),
    }


def _footprint_property_value(footprint: object, name: str) -> str:
    for prop in getattr(footprint, "properties", ()) or ():
        if str(getattr(prop, "name", "") or "") == name:
            return str(getattr(prop, "value", "") or "")
    return ""


def _optional_text(value: object) -> str | None:
    if value is None:
        return None
    text = str(value).strip()
    return text or None


def _operation_counts(operations: list[dict[str, object]]) -> dict[str, int]:
    counts: Counter[str] = Counter()
    for operation in operations:
        operation_name = str(operation.get("op", "") or "")
        if operation_name:
            counts[operation_name] += 1
    return dict(sorted(counts.items()))


def _is_value_text(item: object, item_type: str) -> bool:
    return item_type == "fp_text" and str(getattr(item, "text_type", "") or "") == "value"


def _is_value_field(item: object, item_type: str) -> bool:
    if item_type == "property":
        return str(getattr(item, "name", "") or "") == "Value"
    return _is_value_text(item, item_type)


def _is_graphical_value_field(item: object, item_type: str) -> bool:
    if item_type == "property":
        return bool(getattr(item, "graphical", True))
    return True


def _item_layer(item: object) -> str:
    return str(getattr(item, "layer", "") or "").strip()


def _layer_allowed(layer: str, config: dict[str, object]) -> bool:
    selection = _layer_selection(config)
    include = cast(list[str], selection["include"])
    exclude = cast(list[str], selection["exclude"])
    return _matches_any(layer, include) and not _matches_any(layer, exclude)


def _matches_any(layer: str, patterns: list[str]) -> bool:
    return any(fnmatchcase(layer, pattern) for pattern in patterns)


def _section(config: dict[str, object], key: str) -> dict[str, object]:
    value = config.get(key)
    return cast(dict[str, object], value) if isinstance(value, dict) else {}


def _string_list(value: object, fallback: tuple[str, ...]) -> list[str]:
    if not isinstance(value, list):
        return list(fallback)
    return [str(item) for item in value if str(item).strip()]


def _bool_value(section: dict[str, object], key: str, default: bool) -> bool:
    value = section.get(key)
    return value if isinstance(value, bool) else default


def _disabled_tally() -> dict[str, object]:
    return {
        "total": 0,
        "by_type": {},
        "by_layer": {},
        "protected": 0,
        "protected_by_reason": {},
        "disabled": True,
    }


def _planned_operations(board_report: dict[str, object]) -> list[str]:
    operations = [
        "select configured PCB cleanup layers",
        "protect copper, pads, models, routing, and excluded layers",
        "remove configured documentation-layer graphics",
        "hide visible Value fields on configured documentation layers",
    ]
    if board_report:
        operations.append("load board and summarize cleanup candidates")
    return operations


def _apply_policy() -> dict[str, object]:
    return {
        "default_layers": {
            "include": list(_DEFAULT_INCLUDE_LAYERS),
            "exclude": list(_DEFAULT_EXCLUDE_LAYERS),
        },
        "removes": [
            "footprint graphical primitives on selected layers",
            "board graphical primitives on selected layers when board_graphics is enabled",
            "generated board items on selected layers when generated_graphics is enabled",
            "custom user names from selected layer definitions",
        ],
        "hides": ["visible Value fields on selected layers"],
        "never_removes_by_default": [
            "copper",
            "Edge.Cuts",
            "pads",
            "models",
            "routing",
            "zones",
            "footprint properties",
        ],
        "silkscreen": "opt-in through layers.include; not selected by default",
    }
