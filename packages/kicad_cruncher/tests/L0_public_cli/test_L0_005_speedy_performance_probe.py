"""Safety and parity checks for the Speedy performance probe."""

from __future__ import annotations

import importlib.util
import json
import xml.etree.ElementTree as ET
import zipfile
from pathlib import Path
from types import ModuleType

import pytest

_PACKAGE_ROOT = Path(__file__).resolve().parents[2]
_PROBE_PATH = _PACKAGE_ROOT / "tests/support_scripts/speedy_dr_performance_probe.py"


def _probe() -> ModuleType:
    spec = importlib.util.spec_from_file_location("speedy_dr_performance_probe", _PROBE_PATH)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _write_schematic_svg(
    path: Path,
    *,
    scale: int,
    identity_attribute: str,
    include_source_kind: bool,
) -> None:
    root = ET.Element(
        "svg",
        {
            "width": "1mm",
            "height": "1mm",
            "viewBox": f"0 0 {scale} {scale}",
            "data-review-theme": "kicad_cruncher.design_review.schematic_svg.a0",
        },
    )
    metadata = ET.SubElement(root, "metadata", {"id": "schematic-enrichment-a0"})
    metadata.text = json.dumps({"schema": "fixture.a0"})
    attrs = {
        "id": "record-a",
        "data-ref": "wire",
        "data-primitive": "wire",
        "data-element-key": "record-a",
        identity_attribute: "record-a",
    }
    if include_source_kind:
        attrs["data-source-kind"] = "schematic"
    record = ET.SubElement(root, "g", attrs)
    ET.SubElement(
        record,
        "line",
        {
            "x1": "0",
            "y1": "0",
            "x2": str(scale),
            "y2": str(scale),
            "stroke": "#000000" if scale > 1 else "#000000FF",
        },
    )
    ET.ElementTree(root).write(path, encoding="unicode")


def test_speedy_probe_rejects_windows_archive_escape_members(tmp_path: Path) -> None:
    probe = _probe()
    archive = tmp_path / "kicad.zip"
    prefix = probe.SPEEDY_PREFIX.as_posix()
    with zipfile.ZipFile(archive, "w") as fixture:
        fixture.writestr(f"{prefix}/{probe.SPEEDY_PROJECT}", "(kicad_pro)")
        fixture.writestr(f"{prefix}/..\\escaped.txt", "escape")

    destination = tmp_path / "selected"
    with pytest.raises(AssertionError, match="unsafe reviewed corpus member"):
        probe._extract_speedy(archive, destination)

    assert not (tmp_path / "escaped.txt").exists()


def test_speedy_probe_semantically_validates_schematic_svg_bodies(tmp_path: Path) -> None:
    probe = _probe()
    python_svg = tmp_path / "python.svg"
    rust_svg = tmp_path / "rust.svg"
    _write_schematic_svg(
        python_svg,
        scale=1,
        identity_attribute="data-uuid",
        include_source_kind=False,
    )
    _write_schematic_svg(
        rust_svg,
        scale=1_000_000,
        identity_attribute="data-object-id",
        include_source_kind=True,
    )

    assert probe._assert_schematic_svg_parity(python_svg, rust_svg) == 1

    rust_root = ET.parse(rust_svg).getroot()
    record = next(element for element in rust_root.iter() if element.attrib.get("data-ref"))
    record.clear()
    record.attrib.update(
        {
            "id": "record-a",
            "data-ref": "wire",
            "data-primitive": "wire",
            "data-element-key": "record-a",
            "data-object-id": "record-a",
            "data-source-kind": "schematic",
        }
    )
    ET.SubElement(
        record,
        "line",
        {"x1": "0", "y1": "0", "x2": "0", "y2": "0", "stroke": "#000000"},
    )
    ET.ElementTree(rust_root).write(rust_svg, encoding="unicode")

    with pytest.raises(AssertionError, match="geometry"):
        probe._assert_schematic_svg_parity(python_svg, rust_svg)


def test_speedy_probe_requires_a_bundle_bound_rust_profile() -> None:
    probe = _probe()
    stages = [{"name": name, "elapsed_ns": 1} for name in probe._RUST_PROFILE_STAGES]
    profile = {
        "schema": probe._RUST_PROFILE_SCHEMA,
        "total_elapsed_ns": len(stages) + 5,
        "accounted_elapsed_ns": len(stages),
        "unattributed_elapsed_ns": 5,
        "artifact_count": 35,
        "artifact_bytes": 100,
        "stages": stages,
    }
    line = probe._RUST_PROFILE_PREFIX + json.dumps(profile)
    signature = {"file_count": 35, "total_bytes": 100}

    assert probe._performance_profile([line], expected=True, signature=signature) == profile
    with pytest.raises(AssertionError, match="expected 1"):
        probe._performance_profile([], expected=True, signature=signature)
    with pytest.raises(AssertionError, match="expected 0"):
        probe._performance_profile([line], expected=False, signature=signature)

    invalid_profiles = []
    invalid = dict(profile)
    invalid["schema"] = "wrong.a0"
    invalid_profiles.append((invalid, "schema"))
    invalid = dict(profile)
    invalid["total_elapsed_ns"] += 1
    invalid_profiles.append((invalid, "arithmetic"))
    invalid = dict(profile)
    invalid["stages"] = stages[:-1]
    invalid_profiles.append((invalid, "stage inventory"))
    invalid = dict(profile)
    invalid["artifact_count"] = 34
    invalid_profiles.append((invalid, "artifact count"))
    invalid = dict(profile)
    invalid["artifact_bytes"] = 99
    invalid_profiles.append((invalid, "artifact bytes"))
    for invalid, message in invalid_profiles:
        with pytest.raises(AssertionError, match=message):
            probe._performance_profile(
                [probe._RUST_PROFILE_PREFIX + json.dumps(invalid)],
                expected=True,
                signature=signature,
            )
