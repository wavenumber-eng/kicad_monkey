"""Public workflow tests for daemon/plugin PCB cleanup routing."""

from __future__ import annotations

import shutil
from pathlib import Path

from kicad_cruncher.kicad_cruncher_daemon import (
    create_app,
    daemon_command_inventory_payload,
    daemon_pcb_layer_cleanup,
)
from kicad_cruncher.kicad_cruncher_plugin_installer import (
    DEFAULT_PLUGIN_NAME,
    install_plugin,
    plugin_identifier,
    plugin_package_root,
)
from kicad_monkey import KiCadPcb

_PROJECT_ROOT = Path(__file__).resolve().parents[2]
_CORPUS_ROOT = _PROJECT_ROOT / "tests" / "corpus" / "kicad"
_HLR_TEST_PCB = _CORPUS_ROOT / "projects" / "hlr_test" / "hlr_test.kicad_pcb"


def _json_object(value: object) -> dict[str, object]:
    assert isinstance(value, dict)
    return value


def _json_list(value: object) -> list[object]:
    assert isinstance(value, list)
    return value


def test_daemon_command_inventory_exposes_pcb_clean() -> None:
    """Verify the daemon advertises the shared PCB clean command."""
    app = create_app()
    paths = {str(getattr(route, "path", "")) for route in getattr(app, "routes", [])}
    payload = daemon_command_inventory_payload()
    commands = _json_list(payload["commands"])
    pcb_clean = next(
        _json_object(item)
        for item in commands
        if _json_object(item)["id"] == "pcb.clean"
    )

    assert "/api/v1/commands" in paths
    assert "/api/v1/pcb/layer-cleanup" in paths
    assert payload["schema"] == "kicad_cruncher.daemon.commands.v0"
    assert pcb_clean["endpoint"] == "/api/v1/pcb/layer-cleanup"
    assert "daemon:kicad-ipc-plan" in _json_list(pcb_clean["adapters"])


def test_plugin_install_copies_apply_adapter(tmp_path: Path) -> None:
    """Verify installed plugin package includes the IPC apply adapter."""
    plugins_dir = tmp_path / "plugins"

    results = install_plugin(plugins_dir=plugins_dir)

    source_dir = plugin_package_root(DEFAULT_PLUGIN_NAME)
    target_dir = plugins_dir / plugin_identifier(source_dir)
    assert len(results) == 1
    assert results[0].target_dir == target_dir
    assert (target_dir / "plugin.json").is_file()
    assert (target_dir / "main.py").is_file()
    assert (target_dir / "ipc_apply.py").is_file()
    assert not list(target_dir.rglob("__pycache__"))


def test_daemon_pcb_clean_kicad_ipc_mode_returns_mutation_request() -> None:
    """Verify plugin-mode requests get IPC operations instead of file mutation."""
    payload = daemon_pcb_layer_cleanup(
        {
            "schema": "kicad_cruncher.daemon.pcb.layer_cleanup.request.v0",
            "mode": "kicad-ipc",
            "board_path": str(_HLR_TEST_PCB),
        }
    )

    result = _json_object(payload["result"])
    operation_counts = _json_object(result["operation_counts"])
    operations = _json_list(result["operations"])
    first_graphic = _json_object(
        next(item for item in operations if _json_object(item)["op"] == "remove_footprint_item")
    )

    assert payload["schema"] == "kicad_cruncher.daemon.pcb.layer_cleanup.response.v0"
    assert payload["mode"] == "kicad-ipc"
    assert payload["applied"] is False
    assert result["schema"] == "kicad_cruncher.pcb.clean.mutation_request.v0"
    assert result["operation_target"] == "kicad-ipc"
    assert result["plugin_apply_required"] is True
    assert operation_counts == {
        "remove_footprint_item": 310,
        "reset_layer_user_name": 6,
    }
    assert first_graphic["collection"] == "fp_lines"
    assert first_graphic["layer"] == "F.Fab"
    assert first_graphic["footprint_reference"] == "U1"


def test_daemon_pcb_clean_file_mode_apply_mutates_copy(tmp_path: Path) -> None:
    """Verify daemon file mode delegates to the same safe direct-file apply."""
    board_copy = tmp_path / "hlr_test.kicad_pcb"
    shutil.copy2(_HLR_TEST_PCB, board_copy)
    pcb = KiCadPcb(board_copy)
    value_property = pcb.footprints[0].get_property_object("Value")
    assert value_property is not None
    value_property.hide = False
    pcb.save(board_copy)

    payload = daemon_pcb_layer_cleanup(
        {
            "schema": "kicad_cruncher.daemon.pcb.layer_cleanup.request.v0",
            "mode": "file",
            "apply": True,
            "board_path": str(board_copy),
        }
    )

    result = _json_object(payload["result"])
    mutation_report = _json_object(result["mutation_report"])
    removed = _json_object(mutation_report["footprint_graphics_removed"])
    after = KiCadPcb(board_copy)
    after_value = after.footprints[0].get_property_object("Value")

    assert payload["applied"] is True
    assert result["status"] == "applied"
    assert removed["total"] == 310
    assert after_value is not None
    assert after_value.hide is True
