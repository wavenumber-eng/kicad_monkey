"""Release signoff for the checked-in KiCad Stroke webfont bundle."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

from fontTools.ttLib import TTFont

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
BUNDLE_ROOT = PACKAGE_ROOT / "assets" / "fonts"


def test_webfont_asset_bundle_is_current() -> None:
    result = subprocess.run(
        [
            sys.executable,
            str(PACKAGE_ROOT / "tools" / "package_kicad_stroke_webfont_assets.py"),
            "--check",
        ],
        cwd=PACKAGE_ROOT,
        text=True,
        capture_output=True,
        encoding="utf-8",
        check=False,
    )
    assert result.returncode == 0, result.stdout + result.stderr


def test_webfont_manifest_and_faces_are_complete() -> None:
    manifest = json.loads(
        (BUNDLE_ROOT / "kicad-stroke-font-package.json").read_text(encoding="utf-8")
    )
    assert manifest["type"] == "monkey_font_package"
    assert manifest["version"] == 1
    assert manifest["id"] == "kicad-newstroke"
    assert manifest["family"] == "KiCad Stroke"
    assert manifest["license"] == "CC0-1.0"
    assert len(manifest["faces"]) == 6
    assert {face["weight"] for face in manifest["faces"]} == {300, 400, 700}
    assert {face["italic"] for face in manifest["faces"]} == {False, True}
    assert {face["glyph_count"] for face in manifest["faces"]} == {11191}
    assert len(manifest["files"]) == 26

    for face in manifest["faces"]:
        parsed = TTFont(BUNDLE_ROOT / f"{face['stem']}.woff2")
        assert len(parsed.getBestCmap()) == 11191
        assert parsed["name"].getDebugName(1) == "KiCad Stroke"
