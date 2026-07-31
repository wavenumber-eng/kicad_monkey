"""Unit tests for project-lib source relinking."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path
from typing import Any, cast

from jsonschema import validate
from kicad_cruncher.kicad_cruncher_cmd_megamaid import _run_project_erc_hygiene_check
from kicad_cruncher.kicad_cruncher_project_lib_relink import relink_project_sources

_PROJECT_ROOT = Path(__file__).resolve().parents[2]
JsonObject = dict[str, Any]


def _json_object(value: object) -> JsonObject:
    return cast(JsonObject, value)


def _json_object_list(value: object) -> list[JsonObject]:
    return cast(list[JsonObject], value)


def _run_cli(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, "-m", "kicad_cruncher", *args],
        cwd=_PROJECT_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


def _write_fake_kicad_cli_with_project_erc(tmp_path: Path) -> tuple[Path, Path]:
    log_path = tmp_path / "fake-kicad-cli.log"
    script_path = tmp_path / "fake-kicad-cli.py"
    script_path.write_text(
        f"""from __future__ import annotations

import json
import sys
from pathlib import Path

log_path = Path({str(log_path)!r})
args = sys.argv[1:]
with log_path.open("a", encoding="utf-8") as log:
    log.write(" ".join(args) + "\\n")

if len(args) >= 2 and args[0] == "sch" and args[1] == "erc":
    output = Path(args[args.index("--output") + 1])
    violations = [
        {{"type": "pin_not_connected", "description": "Ordinary ERC warning"}},
    ]
    if output.name == "project_erc_before.json":
        violations.insert(
            0,
            {{"type": "lib_symbol_mismatch", "description": "Library cache mismatch"}},
        )
    output.write_text(
        json.dumps({{"sheets": [{{"path": "/", "violations": violations}}]}}) + "\\n",
        encoding="utf-8",
    )

sys.exit(0)
""",
        encoding="utf-8",
    )
    if sys.platform == "win32":
        cli_path = tmp_path / "kicad-cli.cmd"
        cli_path.write_text(
            f'@echo off\r\n"{sys.executable}" "{script_path}" %*\r\n',
            encoding="utf-8",
        )
    else:
        cli_path = tmp_path / "kicad-cli"
        cli_path.write_text(
            f'#!/bin/sh\nexec "{sys.executable}" "{script_path}" "$@"\n',
            encoding="utf-8",
        )
        cli_path.chmod(0o755)
    return cli_path, log_path


def _write_fake_failing_kicad_cli(tmp_path: Path) -> Path:
    script_path = tmp_path / "fake-failing-kicad-cli.py"
    script_path.write_text("import sys\nsys.exit(7)\n", encoding="utf-8")
    if sys.platform == "win32":
        cli_path = tmp_path / "failing-kicad-cli.cmd"
        cli_path.write_text(
            f'@echo off\r\n"{sys.executable}" "{script_path}" %*\r\n',
            encoding="utf-8",
        )
        return cli_path

    cli_path = tmp_path / "failing-kicad-cli"
    cli_path.write_text(
        f'#!/bin/sh\nexec "{sys.executable}" "{script_path}" "$@"\n',
        encoding="utf-8",
    )
    cli_path.chmod(0o755)
    return cli_path


def _write_relink_project(tmp_path: Path) -> Path:
    project_path = tmp_path / "demo.kicad_pro"
    project_path.write_text("{}", encoding="utf-8")
    (tmp_path / "demo.kicad_sch").write_text(
        """(kicad_sch
  (version 20250114)
  (generator "kicad_cruncher_test")
  (lib_symbols
    (symbol "oldlib:Part"
      (property "Reference" "U" (id 0) (at 0 0 0))
      (property "Value" "Part" (id 1) (at 0 2.54 0))
      (property "Footprint" "oldfp:Foot" (id 2) (at 0 5.08 0))
    )
  )
  (symbol
    (lib_id "oldlib:Part")
    (at 0 0 0)
    (property "Reference" "U1" (id 0) (at 0 0 0))
    (property "Value" "Part" (id 1) (at 0 2.54 0))
    (property "Footprint" "oldfp:Foot" (id 2) (at 0 5.08 0))
  )
)
""",
        encoding="utf-8",
    )
    (tmp_path / "demo.kicad_pcb").write_text(
        """(kicad_pcb
  (version 20250114)
  (generator "kicad_cruncher_test")
  (footprint "oldfp:Foot"
    (layer "F.Cu")
    (property "Reference" "U1" (at 0 0 0))
  )
)
""",
        encoding="utf-8",
    )
    return project_path


def _write_cache_alias_project(tmp_path: Path, *, duplicate_candidates: bool = False) -> Path:
    project_path = tmp_path / "alias.kicad_pro"
    project_path.write_text("{}", encoding="utf-8")
    cache_symbols = (
        '(symbol "local-a:Part_1")\n    (symbol "local-b:Part_1")'
        if duplicate_candidates
        else '(symbol "local-symbols:Part_1")'
    )
    (tmp_path / "alias.kicad_sch").write_text(
        f"""(kicad_sch
  (version 20250114)
  (generator "kicad_cruncher_test")
  (lib_symbols
    {cache_symbols}
  )
  (symbol
    (lib_name "Part_1")
    (lib_id "oldlib:Part")
    (at 0 0 0)
    (property "Reference" "U1" (id 0) (at 0 0 0))
    (property "Value" "Part" (id 1) (at 0 2.54 0))
    (property "Footprint" "oldfp:Foot" (id 2) (at 0 5.08 0))
  )
)
""",
        encoding="utf-8",
    )
    return project_path


def _write_member_only_cache_alias_project(tmp_path: Path) -> Path:
    project_path = tmp_path / "member-only-alias.kicad_pro"
    project_path.write_text("{}", encoding="utf-8")
    (tmp_path / "member-only-alias.kicad_sch").write_text(
        """(kicad_sch
  (version 20250114)
  (generator "kicad_cruncher_test")
  (lib_symbols
    (symbol "Osc_1"
      (property "Reference" "U" (id 0) (at 0 0 0))
      (property "Value" "Osc" (id 1) (at 0 2.54 0))
      (property "Footprint" "oldfp:Foot" (id 2) (at 0 5.08 0))
    )
  )
  (symbol
    (lib_name "Osc_1")
    (lib_id "oldlib:Osc")
    (at 0 0 0)
    (property "Reference" "U1" (id 0) (at 0 0 0))
    (property "Value" "Osc" (id 1) (at 0 2.54 0))
    (property "Footprint" "oldfp:Foot" (id 2) (at 0 5.08 0))
  )
)
""",
        encoding="utf-8",
    )
    return project_path


def _write_cache_body_override_project(tmp_path: Path) -> Path:
    project_path = tmp_path / "cache-body-override.kicad_pro"
    project_path.write_text("{}", encoding="utf-8")
    (tmp_path / "cache-body-override.kicad_sch").write_text(
        """(kicad_sch
  (version 20250114)
  (generator "kicad_cruncher_test")
  (lib_symbols
    (symbol "oldlib:Part"
      (property "Reference" "U" (id 0) (at 0 0 0))
      (property "Value" "Part" (id 1) (at 0 2.54 0))
      (property "Footprint" "" (id 2) (at 0 5.08 0))
    )
  )
  (symbol
    (lib_id "oldlib:Part")
    (at 0 0 0)
    (property "Reference" "U1" (id 0) (at 0 0 0))
    (property "Value" "Part" (id 1) (at 0 2.54 0))
    (property "Footprint" "oldfp:PlacedFoot" (id 2) (at 0 5.08 0))
  )
)
""",
        encoding="utf-8",
    )
    return project_path


def _write_existing_local_cache_project(tmp_path: Path) -> Path:
    project_path = tmp_path / "existing-local-cache.kicad_pro"
    project_path.write_text("{}", encoding="utf-8")
    (tmp_path / "existing-local-cache.kicad_sch").write_text(
        """(kicad_sch
  (version 20250114)
  (generator "kicad_cruncher_test")
  (lib_symbols
    (symbol "oldlib:Part")
    (symbol "local-symbols:Part")
  )
  (symbol
    (lib_id "oldlib:Part")
    (at 0 0 0)
    (property "Reference" "U1" (id 0) (at 0 0 0))
    (property "Value" "Part" (id 1) (at 0 2.54 0))
    (property "Footprint" "oldfp:Foot" (id 2) (at 0 5.08 0))
  )
)
""",
        encoding="utf-8",
    )
    return project_path


def _write_repair_after_cache_relink_project(tmp_path: Path) -> Path:
    project_path = tmp_path / "repair-after-cache-relink.kicad_pro"
    project_path.write_text("{}", encoding="utf-8")
    (tmp_path / "repair-after-cache-relink.kicad_sch").write_text(
        """(kicad_sch
  (version 20250114)
  (generator "kicad_cruncher_test")
  (lib_symbols
    (symbol "oldlib:Part"
      (property "Reference" "U" (id 0) (at 0 0 0))
      (property "Value" "Part" (id 1) (at 0 2.54 0))
      (property "Footprint" "oldfp:Foot" (id 2) (at 0 5.08 0))
      (symbol "Part_1_0" (pin passive line))
    )
  )
  (symbol
    (lib_id "oldlib:Part")
    (at 0 0 0)
    (property "Reference" "U1" (id 0) (at 0 0 0))
    (property "Value" "Part" (id 1) (at 0 2.54 0))
    (property "Footprint" "oldfp:Foot" (id 2) (at 0 5.08 0))
  )
  (symbol
    (lib_name "oldlib:Part")
    (lib_id "oldlib:Part")
    (at 10 0 0)
    (property "Reference" "U2" (id 0) (at 10 0 0))
    (property "Value" "Part" (id 1) (at 10 2.54 0))
    (property "Footprint" "oldfp:Foot" (id 2) (at 10 5.08 0))
  )
)
""",
        encoding="utf-8",
    )
    return project_path


def _write_cache_unit_project(tmp_path: Path, *, already_invalid: bool = False) -> Path:
    project_path = tmp_path / "cache-unit.kicad_pro"
    project_path.write_text("{}", encoding="utf-8")
    parent_symbol = "local-symbols:Part_Slash" if already_invalid else "oldlib:Part/Slash"
    child_symbol = "Part/Slash_1_0"
    placed_lib_id = "local-symbols:Part_Slash" if already_invalid else "oldlib:Part/Slash"
    (tmp_path / "cache-unit.kicad_sch").write_text(
        f"""(kicad_sch
  (version 20250114)
  (generator "kicad_cruncher_test")
  (lib_symbols
    (symbol "{parent_symbol}"
      (property "Reference" "U" (id 0) (at 0 0 0))
      (property "Value" "Part" (id 1) (at 0 2.54 0))
      (property "Footprint" "oldfp:Foot" (id 2) (at 0 5.08 0))
      (symbol "{child_symbol}" (pin passive line))
    )
  )
  (symbol
    (lib_id "{placed_lib_id}")
    (at 0 0 0)
    (property "Reference" "U1" (id 0) (at 0 0 0))
    (property "Value" "Part" (id 1) (at 0 2.54 0))
    (property "Footprint" "oldfp:Foot" (id 2) (at 0 5.08 0))
  )
)
""",
        encoding="utf-8",
    )
    return project_path


def _relink(project_path: Path, *, dry_run: bool) -> dict[str, object]:
    return relink_project_sources(
        project_path=project_path,
        symbol_library_nickname="local-symbols",
        footprint_library_nickname="local-footprints",
        symbol_member_map={"oldlib:Part": "Part"},
        footprint_member_map={"oldfp:Foot": "Foot"},
        dry_run=dry_run,
    )


def test_relink_project_sources_dry_run_reports_without_editing(tmp_path: Path) -> None:
    """Dry-run mode should report exact source relinks without mutating files."""
    project_path = _write_relink_project(tmp_path)

    report = _relink(project_path, dry_run=True)

    assert report["mode"] == "dry_run"
    assert report["changed"] is True
    assert report["summary"] == {"files_checked": 2, "files_changed": 2, "changes": 5}
    assert _json_object(report["cache_link_validation"])["ok"] is True
    assert _json_object(report["cache_unit_validation"])["ok"] is True
    cache_body = _json_object(report["cache_body_validation"])
    assert cache_body["ok"] is True
    assert cache_body["initial_issue_count"] == 1
    assert cache_body["remaining_issue_count"] == 0
    files = _json_object_list(report["files"])
    changes = _json_object_list(files[0]["changes"])
    assert "schematic_cache_symbol_footprint" in {change["kind"] for change in changes}
    assert '"oldlib:Part"' in (tmp_path / "demo.kicad_sch").read_text(encoding="utf-8")
    assert '"oldfp:Foot"' in (tmp_path / "demo.kicad_pcb").read_text(encoding="utf-8")


def test_relink_project_sources_apply_updates_design_links(tmp_path: Path) -> None:
    """Apply mode should rewrite schematic and PCB links to local nicknames."""
    project_path = _write_relink_project(tmp_path)

    report = _relink(project_path, dry_run=False)

    schematic = (tmp_path / "demo.kicad_sch").read_text(encoding="utf-8")
    pcb = (tmp_path / "demo.kicad_pcb").read_text(encoding="utf-8")
    assert report["mode"] == "apply"
    assert report["summary"] == {"files_checked": 2, "files_changed": 2, "changes": 5}
    assert _json_object(report["cache_body_validation"])["ok"] is True
    assert '(symbol "local-symbols:Part"' in schematic
    assert '(lib_id "local-symbols:Part")' in schematic
    assert schematic.count('(property "Footprint" "local-footprints:Foot"') == 2
    assert "oldfp:Foot" not in schematic
    assert '(footprint "local-footprints:Foot"' in pcb


def test_relink_project_sources_uses_generated_cache_body_footprint(
    tmp_path: Path,
) -> None:
    """Cache body sync should use generated symbol defaults, not only old values."""
    project_path = _write_cache_body_override_project(tmp_path)

    report = relink_project_sources(
        project_path=project_path,
        symbol_library_nickname="local-symbols",
        footprint_library_nickname="local-footprints",
        symbol_member_map={"oldlib:Part": "Part"},
        footprint_member_map={"oldfp:PlacedFoot": "PlacedFoot"},
        symbol_footprint_map={
            "oldlib:Part": "local-footprints:GeneratedFoot",
            "local-symbols:Part": "local-footprints:GeneratedFoot",
        },
        dry_run=False,
        fail_on_cache_link_issues=True,
    )

    schematic = (tmp_path / "cache-body-override.kicad_sch").read_text(encoding="utf-8")
    files = _json_object_list(report["files"])
    changes = _json_object_list(files[0]["changes"])
    cache_body = _json_object(report["cache_body_validation"])
    assert report["blocked"] is False
    assert report["summary"] == {"files_checked": 1, "files_changed": 1, "changes": 4}
    assert cache_body["ok"] is True
    assert cache_body["initial_issue_count"] == 1
    assert cache_body["remaining_issue_count"] == 0
    assert "schematic_cache_symbol_footprint" in {change["kind"] for change in changes}
    assert '(property "Footprint" "local-footprints:GeneratedFoot"' in schematic
    assert '(property "Footprint" "local-footprints:PlacedFoot"' in schematic


def test_relink_project_sources_updates_cache_unit_prefixes(
    tmp_path: Path,
) -> None:
    """Relinking embedded cache parents should keep KiCad unit prefixes valid."""
    project_path = _write_cache_unit_project(tmp_path)

    report = relink_project_sources(
        project_path=project_path,
        symbol_library_nickname="local-symbols",
        footprint_library_nickname="local-footprints",
        symbol_member_map={"oldlib:Part/Slash": "Part_Slash"},
        footprint_member_map={"oldfp:Foot": "Foot"},
        dry_run=False,
        fail_on_cache_link_issues=True,
    )

    schematic = (tmp_path / "cache-unit.kicad_sch").read_text(encoding="utf-8")
    assert report["blocked"] is False
    assert report["applied"] is True
    assert report["summary"] == {"files_checked": 1, "files_changed": 1, "changes": 5}
    assert _json_object(report["cache_link_validation"])["ok"] is True
    assert _json_object(report["cache_unit_validation"])["ok"] is True
    assert _json_object(report["cache_body_validation"])["ok"] is True
    assert '(symbol "local-symbols:Part_Slash"' in schematic
    assert '(symbol "Part_Slash_1_0"' in schematic
    assert '(lib_id "local-symbols:Part_Slash")' in schematic
    assert '(property "Footprint" "local-footprints:Foot"' in schematic


def test_relink_project_sources_reports_invalid_cache_unit_prefixes(
    tmp_path: Path,
) -> None:
    """Dry-run reports embedded cache unit names that KiCad's loader rejects."""
    project_path = _write_cache_unit_project(tmp_path, already_invalid=True)

    report = relink_project_sources(
        project_path=project_path,
        symbol_library_nickname="local-symbols",
        footprint_library_nickname="local-footprints",
        symbol_member_map={},
        footprint_member_map={},
        dry_run=True,
    )

    validation = _json_object(report["cache_unit_validation"])
    assert validation["ok"] is False
    assert validation["initial_issue_count"] == 1
    assert validation["remaining_issue_count"] == 1
    issue = _json_object_list(validation["issues"])[0]
    assert issue["parent_symbol"] == "local-symbols:Part_Slash"
    assert issue["child_symbol"] == "Part/Slash_1_0"
    assert issue["expected_prefix"] == "Part_Slash"
    assert issue["reason"] == "prefix"


def test_relink_project_sources_blocks_apply_on_invalid_cache_unit_prefixes(
    tmp_path: Path,
) -> None:
    """Apply mode blocks when embedded cache unit names still violate KiCad parsing."""
    project_path = _write_cache_unit_project(tmp_path, already_invalid=True)

    report = relink_project_sources(
        project_path=project_path,
        symbol_library_nickname="local-symbols",
        footprint_library_nickname="local-footprints",
        symbol_member_map={},
        footprint_member_map={},
        dry_run=False,
        fail_on_cache_link_issues=True,
    )

    schematic = (tmp_path / "cache-unit.kicad_sch").read_text(encoding="utf-8")
    validation = _json_object(report["cache_unit_validation"])
    assert report["blocked"] is True
    assert report["applied"] is False
    assert validation["ok"] is False
    assert validation["remaining_issue_count"] == 1
    assert '(symbol "local-symbols:Part_Slash"' in schematic
    assert '(symbol "Part/Slash_1_0"' in schematic


def test_relink_project_sources_skips_cache_symbol_duplicate_targets(
    tmp_path: Path,
) -> None:
    """Relinking should not create duplicate embedded cache symbol names."""
    project_path = _write_existing_local_cache_project(tmp_path)

    report = _relink(project_path, dry_run=False)

    schematic = (tmp_path / "existing-local-cache.kicad_sch").read_text(encoding="utf-8")
    files = _json_object_list(report["files"])
    changes = _json_object_list(files[0]["changes"])
    validation = _json_object(report["cache_link_validation"])
    assert validation["ok"] is True
    assert {change["kind"] for change in changes} == {
        "schematic_symbol_lib_id",
        "schematic_symbol_footprint",
    }
    assert schematic.count('(symbol "local-symbols:Part"') == 1
    assert '(symbol "oldlib:Part"' in schematic
    assert '(lib_id "local-symbols:Part")' in schematic


def test_relink_project_sources_repairs_lib_name_after_cache_parent_relink(
    tmp_path: Path,
) -> None:
    """Cache-link repair should use planned cache names after parent relinks."""
    project_path = _write_repair_after_cache_relink_project(tmp_path)

    report = relink_project_sources(
        project_path=project_path,
        symbol_library_nickname="local-symbols",
        footprint_library_nickname="local-footprints",
        symbol_member_map={"oldlib:Part": "Part"},
        footprint_member_map={"oldfp:Foot": "Foot"},
        dry_run=False,
        repair_cache_links=True,
        fail_on_cache_link_issues=True,
    )

    schematic = (tmp_path / "repair-after-cache-relink.kicad_sch").read_text(encoding="utf-8")
    validation = _json_object(report["cache_link_validation"])
    files = _json_object_list(report["files"])
    changes = _json_object_list(files[0]["changes"])
    assert report["blocked"] is False
    assert report["applied"] is True
    assert validation["ok"] is True
    assert validation["remaining_issue_count"] == 0
    assert report["summary"] == {"files_checked": 1, "files_changed": 1, "changes": 7}
    assert _json_object(report["cache_body_validation"])["ok"] is True
    assert [change["kind"] for change in changes].count("schematic_symbol_lib_name") == 1
    assert [change["kind"] for change in changes].count("schematic_cache_symbol_footprint") == 1
    assert '(symbol "local-symbols:Part"' in schematic
    assert '(lib_name "local-symbols:Part")' in schematic
    assert schematic.count('(lib_id "local-symbols:Part")') == 2


def test_relink_project_sources_uses_lib_name_member_for_unmapped_lib_id(
    tmp_path: Path,
) -> None:
    """Placed lib_id relink should fall back to a mapped lib_name cache member."""
    project_path = _write_member_only_cache_alias_project(tmp_path)

    report = relink_project_sources(
        project_path=project_path,
        symbol_library_nickname="local-symbols",
        footprint_library_nickname="local-footprints",
        symbol_member_map={"Osc_1": "Osc_1"},
        footprint_member_map={"oldfp:Foot": "Foot"},
        dry_run=False,
        repair_cache_links=True,
        fail_on_cache_link_issues=True,
    )

    schematic = (tmp_path / "member-only-alias.kicad_sch").read_text(encoding="utf-8")
    files = _json_object_list(report["files"])
    changes = _json_object_list(files[0]["changes"])
    validation = _json_object(report["cache_link_validation"])
    assert report["blocked"] is False
    assert report["applied"] is True
    assert validation["ok"] is True
    assert report["summary"] == {"files_checked": 1, "files_changed": 1, "changes": 3}
    assert _json_object(report["cache_body_validation"])["ok"] is True
    assert {change["kind"] for change in changes} == {
        "schematic_cache_symbol_footprint",
        "schematic_symbol_lib_id",
        "schematic_symbol_footprint",
    }
    assert '(symbol "Osc_1"' in schematic
    assert '(lib_name "Osc_1")' in schematic
    assert '(lib_id "local-symbols:Osc_1")' in schematic
    assert '(lib_id "oldlib:Osc")' not in schematic


def test_relink_project_sources_reports_schematic_cache_link_mismatches(
    tmp_path: Path,
) -> None:
    """Dry-run reports placed lib_name aliases that do not exactly match cache symbols."""
    project_path = _write_cache_alias_project(tmp_path)

    report = _relink(project_path, dry_run=True)

    validation = _json_object(report["cache_link_validation"])
    assert validation["ok"] is False
    assert validation["initial_issue_count"] == 1
    assert validation["remaining_issue_count"] == 1
    assert validation["repairable_issue_count"] == 1
    issue = _json_object_list(validation["issues"])[0]
    assert issue["reference"] == "U1"
    assert issue["cache_lookup_source"] == "lib_name"
    assert issue["cache_lookup"] == "Part_1"
    assert issue["lib_name"] == "Part_1"
    assert issue["candidate_cache_names"] == ["local-symbols:Part_1"]
    assert issue["repair_candidate"] == "local-symbols:Part_1"


def test_relink_project_sources_repairs_unique_schematic_cache_links(
    tmp_path: Path,
) -> None:
    """Apply mode can repair a unique placed lib_name to embedded cache alias mismatch."""
    project_path = _write_cache_alias_project(tmp_path)

    report = relink_project_sources(
        project_path=project_path,
        symbol_library_nickname="local-symbols",
        footprint_library_nickname="local-footprints",
        symbol_member_map={"oldlib:Part": "Part"},
        footprint_member_map={"oldfp:Foot": "Foot"},
        dry_run=False,
        repair_cache_links=True,
        fail_on_cache_link_issues=True,
    )

    schematic = (tmp_path / "alias.kicad_sch").read_text(encoding="utf-8")
    files = _json_object_list(report["files"])
    changes = _json_object_list(files[0]["changes"])
    validation = _json_object(report["cache_link_validation"])
    assert report["blocked"] is False
    assert report["applied"] is True
    assert validation["ok"] is True
    assert validation["initial_issue_count"] == 1
    assert validation["remaining_issue_count"] == 0
    assert {change["kind"] for change in changes} == {
        "schematic_symbol_lib_name",
        "schematic_symbol_lib_id",
        "schematic_symbol_footprint",
    }
    assert '(lib_name "local-symbols:Part_1")' in schematic
    assert '(lib_id "local-symbols:Part")' in schematic
    assert '(property "Footprint" "local-footprints:Foot"' in schematic


def test_relink_project_sources_blocks_apply_when_cache_link_repair_is_ambiguous(
    tmp_path: Path,
) -> None:
    """Apply mode must not partially rewrite source files when cache links stay invalid."""
    project_path = _write_cache_alias_project(tmp_path, duplicate_candidates=True)

    report = relink_project_sources(
        project_path=project_path,
        symbol_library_nickname="local-symbols",
        footprint_library_nickname="local-footprints",
        symbol_member_map={"oldlib:Part": "Part"},
        footprint_member_map={"oldfp:Foot": "Foot"},
        dry_run=False,
        repair_cache_links=True,
        fail_on_cache_link_issues=True,
    )

    schematic = (tmp_path / "alias.kicad_sch").read_text(encoding="utf-8")
    validation = _json_object(report["cache_link_validation"])
    assert report["blocked"] is True
    assert report["applied"] is False
    assert validation["ok"] is False
    assert validation["initial_issue_count"] == 1
    assert validation["remaining_issue_count"] == 1
    assert validation["repairable_issue_count"] == 0
    remaining_issues = _json_object_list(validation["remaining_issues"])
    assert remaining_issues[0]["candidate_cache_names"] == [
        "local-a:Part_1",
        "local-b:Part_1",
    ]
    assert '(lib_name "Part_1")' in schematic
    assert '(lib_id "oldlib:Part")' in schematic
    assert '(property "Footprint" "oldfp:Foot"' in schematic


def test_relink_project_sources_apply_preserves_lf_newlines(tmp_path: Path) -> None:
    """Apply mode should preserve existing LF-only source newlines on Windows."""
    project_path = _write_relink_project(tmp_path)
    for source_path in (tmp_path / "demo.kicad_sch", tmp_path / "demo.kicad_pcb"):
        source_path.write_bytes(source_path.read_bytes().replace(b"\r\n", b"\n"))

    report = _relink(project_path, dry_run=False)

    assert report["mode"] == "apply"
    assert b"\r\n" not in (tmp_path / "demo.kicad_sch").read_bytes()
    assert b"\r\n" not in (tmp_path / "demo.kicad_pcb").read_bytes()
    assert b'(lib_id "local-symbols:Part")' in (tmp_path / "demo.kicad_sch").read_bytes()


def test_relink_project_sources_cache_body_relink_is_idempotent(tmp_path: Path) -> None:
    """Rerunning after cache body localization should not create more edits."""
    project_path = _write_relink_project(tmp_path)

    first = _relink(project_path, dry_run=False)
    second = _relink(project_path, dry_run=False)

    assert first["summary"] == {"files_checked": 2, "files_changed": 2, "changes": 5}
    assert second["summary"] == {"files_checked": 2, "files_changed": 0, "changes": 0}
    assert _json_object(second["cache_link_validation"])["ok"] is True
    assert _json_object(second["cache_unit_validation"])["ok"] is True
    assert _json_object(second["cache_body_validation"])["ok"] is True


def test_project_lib_relink_dry_run_writes_report_and_manifest(tmp_path: Path) -> None:
    """The public command should expose source relinking as a reviewable dry-run."""
    project_path = _write_relink_project(tmp_path)
    output_dir = tmp_path / "local-library"

    result = _run_cli(
        "project-lib",
        str(project_path),
        "--output",
        str(output_dir),
        "--no-update-library-tables",
        "--no-embed-models",
        "--no-embed-external-models",
        "--relink-dry-run",
    )

    assert result.returncode == 0, result.stderr
    manifest = json.loads((output_dir / "project_lib_manifest.json").read_text(encoding="utf-8"))
    relink_report = json.loads((output_dir / "source_relink.json").read_text(encoding="utf-8"))
    relink_schema = json.loads(
        (_PROJECT_ROOT / "docs" / "contracts" / "source_relink.a0.schema.json").read_text(
            encoding="utf-8"
        )
    )
    assert manifest["source_relink"]["mode"] == "dry_run"
    assert manifest["source_relink"]["report"] == "source_relink.json"
    assert relink_report["summary"] == {"files_checked": 2, "files_changed": 2, "changes": 5}
    assert relink_report["cache_body_validation"]["ok"] is True
    validate(relink_report, relink_schema)
    assert '"oldlib:Part"' in (tmp_path / "demo.kicad_sch").read_text(encoding="utf-8")


def test_project_lib_relink_sources_validate_kicad_cli_runs_erc_gate(
    tmp_path: Path,
) -> None:
    """Project-lib apply validation should compare ERC hygiene before and after relink."""
    project_path = _write_relink_project(tmp_path)
    fake_cli, fake_cli_log = _write_fake_kicad_cli_with_project_erc(tmp_path)
    output_dir = tmp_path / "local-library"

    result = _run_cli(
        "project-lib",
        str(project_path),
        "--output",
        str(output_dir),
        "--no-embed-models",
        "--no-embed-external-models",
        "--relink-sources",
        "--repair-cache-links",
        "--validate-kicad-cli",
        "--kicad-cli",
        str(fake_cli),
    )

    assert result.returncode == 0, result.stderr
    manifest = json.loads((output_dir / "project_lib_manifest.json").read_text(encoding="utf-8"))
    validation = _json_object(manifest["validation"])
    project_erc = _json_object(validation["project_erc"])
    before = _json_object(project_erc["before"])
    after = _json_object(project_erc["after"])
    before_summary = _json_object(before["summary"])
    after_summary = _json_object(after["summary"])
    assert validation["ok"] is True
    assert project_erc["ok"] is True
    assert project_erc["ordinary_delta"] == 0
    assert before_summary["library_hygiene"] == 1
    assert before_summary["ordinary"] == 1
    assert after_summary["library_hygiene"] == 0
    assert after_summary["ordinary"] == 1
    assert (output_dir / "project_erc_before.json").is_file()
    assert (output_dir / "project_erc_after.json").is_file()
    cli_calls = fake_cli_log.read_text(encoding="utf-8").splitlines()
    assert any("sch erc" in call and "project_erc_before.json" in call for call in cli_calls)
    assert any("sch erc" in call and "project_erc_after.json" in call for call in cli_calls)


def test_project_erc_hygiene_check_rejects_stale_json_when_cli_fails(
    tmp_path: Path,
) -> None:
    """A failed KiCad CLI run must not pass by parsing a stale ERC JSON file."""
    project_path = _write_relink_project(tmp_path)
    fake_cli = _write_fake_failing_kicad_cli(tmp_path)
    output_json = tmp_path / "project_erc_after.json"
    output_json.write_text(
        json.dumps({"sheets": [{"path": "/", "violations": []}]}) + "\n",
        encoding="utf-8",
    )

    report = _run_project_erc_hygiene_check(
        project_path=project_path,
        output_json=output_json,
        kicad_cli=fake_cli,
    )

    assert report["ok"] is False
    assert report["returncode"] == 7
    assert not output_json.exists()
    assert "KiCad CLI ERC failed with exit code 7" in str(report["parse_error"])
    assert "did not write an ERC JSON output file" in str(report["parse_error"])


def test_project_lib_relink_sources_rejects_disabled_table_updates(tmp_path: Path) -> None:
    """Apply mode must not create local links without registering local libraries."""
    result = _run_cli(
        "project-lib",
        str(tmp_path / "missing.kicad_pro"),
        "--no-update-library-tables",
        "--relink-sources",
    )

    assert result.returncode == 2
    assert "--relink-sources requires project library table updates" in (
        result.stdout + result.stderr
    )


def test_project_lib_repair_cache_links_requires_relink_mode(tmp_path: Path) -> None:
    """The cache-link repair option only runs inside an explicit relink report workflow."""
    result = _run_cli(
        "project-lib",
        str(tmp_path / "missing.kicad_pro"),
        "--repair-cache-links",
    )

    assert result.returncode == 2
    assert "--repair-cache-links requires --relink-dry-run or --relink-sources" in (
        result.stdout + result.stderr
    )
