"""Unit tests for project-lib source relinking."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from jsonschema import validate
from kicad_cruncher.kicad_cruncher_project_lib_relink import relink_project_sources

_PROJECT_ROOT = Path(__file__).resolve().parents[2]


def _run_cli(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, "-m", "kicad_cruncher", *args],
        cwd=_PROJECT_ROOT,
        capture_output=True,
        text=True,
        check=False,
    )


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
    assert report["summary"] == {"files_checked": 2, "files_changed": 2, "changes": 3}
    assert '"oldlib:Part"' in (tmp_path / "demo.kicad_sch").read_text(encoding="utf-8")
    assert '"oldfp:Foot"' in (tmp_path / "demo.kicad_pcb").read_text(encoding="utf-8")


def test_relink_project_sources_apply_updates_design_links(tmp_path: Path) -> None:
    """Apply mode should rewrite schematic and PCB links to local nicknames."""
    project_path = _write_relink_project(tmp_path)

    report = _relink(project_path, dry_run=False)

    schematic = (tmp_path / "demo.kicad_sch").read_text(encoding="utf-8")
    pcb = (tmp_path / "demo.kicad_pcb").read_text(encoding="utf-8")
    assert report["mode"] == "apply"
    assert report["summary"] == {"files_checked": 2, "files_changed": 2, "changes": 3}
    assert '(lib_id "local-symbols:Part")' in schematic
    assert '(property "Footprint" "local-footprints:Foot"' in schematic
    assert '(footprint "local-footprints:Foot"' in pcb


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
    assert relink_report["summary"] == {"files_checked": 2, "files_changed": 2, "changes": 3}
    validate(relink_report, relink_schema)
    assert '"oldlib:Part"' in (tmp_path / "demo.kicad_sch").read_text(encoding="utf-8")


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
