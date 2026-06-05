"""PCB cleanup config and planning helpers."""

from __future__ import annotations

import json
from pathlib import Path

from kicad_cruncher.config_json import load_json_config

PCB_CLEAN_CONFIG_FILENAME = "pcb.clean.config"
PCB_CLEAN_CONFIG_SCHEMA = "kicad_cruncher.pcb.clean.config.v0"


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
            "include": ["User.*", "*.Fab", "*.CrtYd"],
            "exclude": ["F.Cu", "B.Cu", "Edge.Cuts"],
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
    return {
        "schema": "kicad_cruncher.pcb.clean.plan.v0",
        "status": "planned",
        "dry_run": dry_run,
        "board": str(board_path) if board_path is not None else None,
        "config": str(config_path) if config_path is not None else None,
        "config_schema": config.get("schema"),
        "planned_operations": [
            "select configured PCB cleanup layers",
            "protect pads, models, mandatory fields, and excluded layers",
            "report generated/user graphics before mutation",
        ],
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
