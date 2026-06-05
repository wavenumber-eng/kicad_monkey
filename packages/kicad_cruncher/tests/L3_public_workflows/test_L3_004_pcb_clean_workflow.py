"""Public workflow tests for the pcb clean command."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

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
    footprint_items = _json_object(board_report["footprint_local_items"])
    by_type = _json_object(footprint_items["by_type"])
    by_layer = _json_object(footprint_items["by_layer"])
    protected_by_reason = _json_object(footprint_items["protected_by_reason"])

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
    assert footprint_items["total"] == 310
    assert by_type == {"fp_line": 310}
    assert by_layer == {"F.Fab": 310}
    assert footprint_items["protected"] == 4
    assert protected_by_reason == {"mandatory_field": 4}


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
                    "footprint_local_items": False,
                    "board_items": False,
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
    footprint_items = _json_object(board_report["footprint_local_items"])
    generated_items = _json_object(board_report["generated_items"])

    assert board_report["layer_user_name_resets"] == []
    assert footprint_items["total"] == 0
    assert generated_items["disabled"] is True
