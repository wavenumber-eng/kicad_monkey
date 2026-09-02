"""Pinned-Git external Rust consumer gate for all four direct SVG families."""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
CONSUMER = PACKAGE_ROOT / "tests" / "support_scripts" / "pinned_git_svg_consumer.py"
PROBE = PACKAGE_ROOT / "tests" / "support_scripts" / "svg_browser_probe.mjs"


def _run(command: list[str], *, timeout: int = 1_200) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(command, cwd=PACKAGE_ROOT, capture_output=True, text=True, encoding="utf-8", timeout=timeout, check=False)
    assert completed.returncode == 0, (
        f"command failed: {' '.join(command)}\nstdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
    )
    return completed


def test_exact_git_revision_consumer_renders_all_families_without_adapters(tmp_path: Path) -> None:
    configured = os.environ.get("KM_SVG_BROWSER_ARTIFACTS")
    retained = (Path(configured) / "pinned-git") if configured else tmp_path / "retained"
    retained.mkdir(parents=True, exist_ok=True)
    root = tmp_path / "pinned-git"
    root.mkdir(parents=True, exist_ok=True)
    result = json.loads(_run([
        "python", str(CONSUMER), "--repository", str(PACKAGE_ROOT), "--output", str(root),
    ]).stdout)
    assert len(result["revision"]) == 40
    expected_ci_revision = os.environ.get("GITHUB_SHA")
    if expected_ci_revision:
        assert not result["synthetic"]
        assert result["revision"] == expected_ci_revision
    assert "kicad-monkey-wasm" not in result["runtime_tree"]
    assert "kicad-monkey-native" not in result["runtime_tree"]
    shutil.copy2(root / "result.json", retained / "result.json")
    for family, svg_name in result["artifacts"].items():
        svg = Path(svg_name)
        shutil.copy2(svg, retained / f"{family}.svg")
        text = svg.read_text(encoding="utf-8")
        assert "#2468AC" in text
        assert "data-ref=" not in text
        assert "scale(0.000001)" not in text
        assert 'font-size="1000000"' not in text
        if family == "footprint":
            assert "scale(-0.5 1)" in text
        screenshot = retained / f"{family}.png"
        facts = json.loads(_run(["node", str(PROBE), str(svg), str(screenshot)]).stdout)
        (retained / f"{family}-browser-facts.json").write_text(
            json.dumps(facts, indent=2), encoding="utf-8"
        )
        view_box = [float(value) for value in facts["view_box"].split()]
        assert 0 < view_box[2] < 10_000
        assert 0 < view_box[3] < 10_000
        assert facts["painted_element_count"] > 0
        assert facts["raster"]["non_white_pixels"] > 10
        assert facts["raster"]["painted_bounds"] is not None
        assert all(item["intersects_viewport"] for item in facts["painted"])
        assert all(not item["clipped"] for item in facts["texts"] if item["visible"])
        assert all(
            item["intersects_view_box"] and not item["clipped_view_box"]
            for item in facts["texts"]
            if item["visible"]
        )
