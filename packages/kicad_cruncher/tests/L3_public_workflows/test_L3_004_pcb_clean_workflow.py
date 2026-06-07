"""Public workflow tests for the pcb clean command."""

from __future__ import annotations

import json
import shutil
import subprocess
import sys
from pathlib import Path

from kicad_monkey import KiCadPcb

_PROJECT_ROOT = Path(__file__).resolve().parents[2]
_CORPUS_ROOT = _PROJECT_ROOT / "tests" / "corpus" / "kicad"
_HLR_TEST_PROJECT = _CORPUS_ROOT / "projects" / "hlr_test" / "hlr_test.kicad_pro"
_HLR_TEST_PCB = _CORPUS_ROOT / "projects" / "hlr_test" / "hlr_test.kicad_pcb"


def _run_cli(*args: str) -> subprocess.CompletedProcess[str]:
    """Run the current checkout's CLI through the active Python environment."""
    return subprocess.run(
        [sys.executable, "-m", "kicad_cruncher", *args],
        cwd=_PROJECT_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


def _json_object(value: object) -> dict[str, object]:
    assert isinstance(value, dict)
    return value


def _json_list(value: object) -> list[object]:
    assert isinstance(value, list)
    return value


def test_pcb_clean_dry_run_reports_fixture_cleanup_candidates() -> None:
    """Verify PCB clean reports deterministic candidates from a real public board."""
    result = _run_cli("pcb", "clean", str(_HLR_TEST_PCB), "--dry-run")

    assert result.returncode == 0, result.stderr
    payload = _json_object(json.loads(result.stdout))
    board_report = _json_object(payload["board_report"])
    inventory = _json_object(board_report["inventory"])
    layer_resets = _json_list(board_report["layer_user_name_resets"])
    footprint_graphics = _json_object(board_report["footprint_graphics"])
    value_fields = _json_object(board_report["value_fields"])
    by_type = _json_object(footprint_graphics["by_type"])
    by_layer = _json_object(footprint_graphics["by_layer"])
    protected_value_reason = _json_object(value_fields["protected_by_reason"])

    assert payload["schema"] == "kicad_cruncher.pcb.clean.plan.v0"
    assert board_report["status"] == "loaded"
    assert inventory["layers"] == 24
    assert inventory["footprints"] == 1
    canonical_names: set[str] = set()
    for item in layer_resets:
        canonical_names.add(str(_json_object(item)["canonical_name"]))

    assert canonical_names == {
        "B.CrtYd",
        "Cmts.User",
        "Dwgs.User",
        "Eco1.User",
        "Eco2.User",
        "F.CrtYd",
    }
    assert footprint_graphics["total"] == 310
    assert by_type == {"fp_line": 310}
    assert by_layer == {"F.Fab": 310}
    assert value_fields["total"] == 0
    assert value_fields["protected"] == 1
    assert protected_value_reason == {"already_hidden": 1}


def test_pcb_clean_resolves_project_input_to_sibling_board() -> None:
    """Verify .kicad_pro input resolves to the sibling board for CLI/plugin parity."""
    result = _run_cli("pcb", "clean", str(_HLR_TEST_PROJECT), "--dry-run")

    assert result.returncode == 0, result.stderr
    payload = _json_object(json.loads(result.stdout))
    board_report = _json_object(payload["board_report"])

    assert board_report["status"] == "loaded"
    assert Path(str(board_report["resolved_board"])).name == "hlr_test.kicad_pcb"


def test_pcb_clean_config_can_disable_cleanup_target_classes(tmp_path: Path) -> None:
    """Verify config switches suppress layer resets and footprint candidate selection."""
    config_path = tmp_path / "pcb.clean.config"
    config_path.write_text(
        json.dumps(
            {
                "schema": "kicad_cruncher.pcb.clean.config.v0",
                "targets": {
                    "user_layers": False,
                    "generated_graphics": False,
                    "footprint_graphics": False,
                    "board_graphics": False,
                    "value_fields": False,
                },
                "safety": {
                    "protect_pads": True,
                    "protect_models": True,
                    "protect_mandatory_fields": True,
                    "require_explicit_apply": True,
                },
                "layers": {
                    "include": ["*.User", "User.*", "*.Fab", "*.CrtYd"],
                    "exclude": ["F.Cu", "B.Cu", "Edge.Cuts"],
                },
            },
            indent=2,
        ),
        encoding="utf-8",
    )

    result = _run_cli(
        "pcb",
        "clean",
        str(_HLR_TEST_PCB),
        "--config",
        str(config_path),
        "--dry-run",
    )

    assert result.returncode == 0, result.stderr
    payload = _json_object(json.loads(result.stdout))
    board_report = _json_object(payload["board_report"])
    footprint_graphics = _json_object(board_report["footprint_graphics"])
    value_fields = _json_object(board_report["value_fields"])
    generated_items = _json_object(board_report["generated_items"])

    assert board_report["layer_user_name_resets"] == []
    assert footprint_graphics["total"] == 0
    assert value_fields["disabled"] is True
    assert generated_items["disabled"] is True


def test_pcb_clean_apply_mutates_copied_fixture_without_touching_copper(
    tmp_path: Path,
) -> None:
    """Verify apply removes docs-layer graphics and hides Value fields only."""
    board_copy = tmp_path / "hlr_test.kicad_pcb"
    shutil.copy2(_HLR_TEST_PCB, board_copy)
    before = KiCadPcb(board_copy)
    before_footprint = before.footprints[0]
    before_value = before_footprint.get_property_object("Value")
    before_pad_count = len(before_footprint.pads)
    assert before_value is not None
    before_value.hide = False
    before.save(board_copy)

    result = _run_cli("pcb", "clean", str(board_copy), "--apply")

    assert result.returncode == 0, result.stderr
    payload = _json_object(json.loads(result.stdout))
    mutation_report = _json_object(payload["mutation_report"])
    removed = _json_object(mutation_report["footprint_graphics_removed"])
    hidden = _json_object(mutation_report["value_fields_hidden"])
    after = KiCadPcb(board_copy)
    after_footprint = after.footprints[0]
    value_property = after_footprint.get_property_object("Value")

    assert payload["status"] == "applied"
    assert mutation_report["layer_user_names_reset"] == 6
    assert removed["total"] == 310
    assert hidden["total"] == 1
    assert all(
        layer.user_name is None
        for layer in after.layers
        if layer.canonical_name.endswith("User")
    )
    assert len(after_footprint.fp_lines) == 4
    assert not any(line.layer == "F.Fab" for line in after_footprint.fp_lines)
    assert len(after_footprint.pads) == before_pad_count
    assert value_property is not None
    assert value_property.value == "TPS7A2018PDBVR"
    assert value_property.hide is True
