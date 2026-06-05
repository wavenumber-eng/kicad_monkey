"""PCB cleanup config and planning helpers."""

from __future__ import annotations

import json
from collections import Counter
from dataclasses import dataclass, field
from fnmatch import fnmatchcase
from pathlib import Path
from typing import cast

from kicad_monkey import KiCadPcb

from kicad_cruncher.config_json import load_json_config

PCB_CLEAN_CONFIG_FILENAME = "pcb.clean.config"
PCB_CLEAN_CONFIG_SCHEMA = "kicad_cruncher.pcb.clean.config.v0"

_DEFAULT_INCLUDE_LAYERS = ("*.User", "User.*", "*.Fab", "*.CrtYd")
_DEFAULT_EXCLUDE_LAYERS = ("F.Cu", "B.Cu", "Edge.Cuts")
_MANDATORY_FOOTPRINT_FIELDS = frozenset(("Reference", "Value", "Datasheet", "Description"))
_BOARD_ITEM_COLLECTIONS = (
    ("gr_texts", "gr_text"),
    ("gr_lines", "gr_line"),
    ("gr_rects", "gr_rect"),
    ("gr_arcs", "gr_arc"),
    ("gr_circles", "gr_circle"),
    ("gr_polys", "gr_poly"),
    ("gr_curves", "gr_curve"),
    ("gr_text_boxes", "gr_text_box"),
    ("images", "image"),
    ("barcodes", "barcode"),
    ("tables", "table"),
    ("dimensions", "dimension"),
)
_FOOTPRINT_ITEM_COLLECTIONS = (
    ("properties", "property"),
    ("fp_texts", "fp_text"),
    ("fp_text_boxes", "fp_text_box"),
    ("fp_lines", "fp_line"),
    ("fp_arcs", "fp_arc"),
    ("fp_circles", "fp_circle"),
    ("fp_rects", "fp_rect"),
    ("fp_polys", "fp_poly"),
    ("images", "image"),
    ("tables", "table"),
    ("barcodes", "barcode"),
    ("dimensions", "dimension"),
    ("zones", "zone"),
)


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


def default_pcb_clean_config() -> dict[str, object]:
    """Return the default PCB cleanup config object."""
    return {
        "schema": PCB_CLEAN_CONFIG_SCHEMA,
        "targets": {
            "user_layers": True,
            "generated_graphics": True,
            "footprint_local_items": True,
            "board_items": False,
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
        "schema": "kicad_cruncher.pcb.clean.plan.v0",
        "status": "planned",
        "dry_run": dry_run,
        "board": str(board_path) if board_path is not None else None,
        "config": str(config_path) if config_path is not None else None,
        "config_schema": config.get("schema"),
        "layer_selection": _layer_selection(config),
        "planned_operations": _planned_operations(board_report),
        "board_report": board_report,
        "mutation_supported": False,
    }


def _default_pcb_clean_config_text() -> str:
    payload = json.dumps(default_pcb_clean_config(), indent=2)
    return (
        "/*\n"
        "  KiCad Cruncher PCB Clean config.\n"
        "  The first implementation targets existing user/generated layers and\n"
        "  keeps apply behavior explicit. Dry-run reports should be reviewed\n"
        "  before mutation is enabled.\n"
        "*/\n"
        f"{payload}\n"
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
    footprint_tally = _footprint_cleanup_tally(pcb, config)
    board_tally = (
        _board_cleanup_tally(pcb, config)
        if _bool_value(targets, "board_items", False)
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
        "footprint_local_items": footprint_tally.to_json(),
        "board_items": board_tally.to_json() if board_tally is not None else _disabled_tally(),
        "generated_items": (
            generated_tally.to_json() if generated_tally is not None else _disabled_tally()
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


def _footprint_cleanup_tally(pcb: KiCadPcb, config: dict[str, object]) -> _SelectionTally:
    targets = _section(config, "targets")
    if not _bool_value(targets, "footprint_local_items", True):
        return _SelectionTally()

    tally = _SelectionTally()
    safety = _section(config, "safety")
    for footprint in pcb.footprints:
        for collection_name, item_type in _FOOTPRINT_ITEM_COLLECTIONS:
            for item in getattr(footprint, collection_name, ()) or ():
                _count_layer_item(
                    tally,
                    item=item,
                    item_type=item_type,
                    config=config,
                    safety=safety,
                )
    return tally


def _board_cleanup_tally(pcb: KiCadPcb, config: dict[str, object]) -> _SelectionTally:
    tally = _SelectionTally()
    safety = _section(config, "safety")
    for collection_name, item_type in _BOARD_ITEM_COLLECTIONS:
        for item in getattr(pcb, collection_name, ()) or ():
            _count_layer_item(tally, item=item, item_type=item_type, config=config, safety=safety)
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
    safety: dict[str, object],
) -> None:
    layer = _item_layer(item)
    if not _layer_allowed(layer, config):
        return
    reason = _protection_reason(item, item_type, safety)
    if reason:
        tally.add_protected(reason=reason)
        return
    tally.add_candidate(item_type=item_type, layer=layer)


def _protection_reason(item: object, item_type: str, safety: dict[str, object]) -> str:
    if item_type == "property" and _bool_value(safety, "protect_mandatory_fields", True):
        name = str(getattr(item, "name", "") or "")
        if name in _MANDATORY_FOOTPRINT_FIELDS:
            return "mandatory_field"
    return ""


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
        "protect pads, models, mandatory fields, and excluded layers",
        "report generated/user graphics before mutation",
    ]
    if board_report:
        operations.append("load board and summarize cleanup candidates")
    return operations
