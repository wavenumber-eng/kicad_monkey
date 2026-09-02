"""Browser-visible Rust SVG regression coverage for reopened issue #78."""

from __future__ import annotations

import json
import os
import platform
from pathlib import Path
import subprocess
import sys

from kicad_monkey import KiCadSymbolLib, SymbolRenderOptions
from kicad_monkey.testing.corpus import (
    get_kicad_corpus_case,
    resolve_kicad_manifest_path,
)


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
PROBE = PACKAGE_ROOT / "tests" / "support_scripts" / "svg_browser_probe.mjs"
CASE_ID = "internal_library/symbol_svg/MIMXRT685SFVKB"


def _run(command: list[str], *, timeout: int = 600) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=timeout,
        check=False,
    )
    assert completed.returncode == 0, (
        f"command failed: {' '.join(command)}\n"
        f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
    )
    return completed


def _browser_facts(svg: Path, screenshot: Path, browser: str = "chromium") -> dict:
    completed = _run(["node", str(PROBE), str(svg), str(screenshot), browser])
    facts = json.loads(completed.stdout)
    assert screenshot.is_file() and screenshot.stat().st_size > 1_000
    return facts


def _artifact_root(tmp_path: Path) -> Path:
    configured = os.environ.get("KM_SVG_BROWSER_ARTIFACTS")
    root = Path(configured) if configured else tmp_path
    root.mkdir(parents=True, exist_ok=True)
    return root


def _assert_browser_scaled(facts: dict) -> None:
    view_box = [float(value) for value in facts["view_box"].split()]
    assert len(view_box) == 4
    assert 0.0 < view_box[2] < 10_000.0
    assert 0.0 < view_box[3] < 10_000.0
    assert facts["width"].endswith("mm") and facts["height"].endswith("mm")
    # The 800px border box has 10px padding on each side.
    assert facts["rendered_width_px"] == 780
    assert facts["rendered_height_px"] == 780
    assert facts["image_embedding"]["rendered_width_px"] == 800
    assert facts["image_embedding"]["rendered_height_px"] == 800
    assert facts["image_embedding"]["natural_width_px"] > 0
    assert facts["image_embedding"]["natural_height_px"] > 0


def _label(facts: dict, value: str) -> dict:
    matches = [entry for entry in facts["texts"] if entry["text"] == value]
    assert matches, f"missing semantic label {value!r}"
    return matches[0]


def test_mimx_direct_rust_svg_is_legible_in_pinned_chromium(tmp_path: Path) -> None:
    artifacts = _artifact_root(tmp_path)
    case = get_kicad_corpus_case(CASE_ID)
    source = resolve_kicad_manifest_path(case, "input_file")
    reference_root = resolve_kicad_manifest_path(case, "reference_output_root")
    assert source is not None and source.is_file()
    assert reference_root is not None and reference_root.is_dir()
    reference = reference_root / "MIMXRT685SFVKB_unit1.svg"
    assert reference.is_file()

    rust_svg = artifacts / "mimx-rust.svg"
    python_svg = artifacts / "mimx-python.svg"
    _run(
        [
            "cargo",
            "run",
            "--locked",
            "-p",
            "kicad-monkey-svg",
            "--example",
            "render_symbol_svg",
            "--",
            str(source),
            "MIMXRT685SFVKB",
            "1",
            str(rust_svg),
        ]
    )
    library = KiCadSymbolLib.from_file(source)
    python_svg.write_text(
        library.symbol_to_svg(
            "MIMXRT685SFVKB",
            options=SymbolRenderOptions(unit=1),
        ),
        encoding="utf-8",
    )

    rust = _browser_facts(rust_svg, artifacts / "mimx-rust.png")
    rust_webkit = _browser_facts(
        rust_svg, artifacts / "mimx-rust-webkit.png", "webkit"
    )
    python = _browser_facts(python_svg, artifacts / "mimx-python.png")
    kicad = _browser_facts(reference, artifacts / "mimx-kicad.png")
    (artifacts / "mimx-browser-facts.json").write_text(
        json.dumps(
            {"rust": rust, "rust_webkit": rust_webkit, "python": python, "kicad": kicad},
            indent=2,
        ),
        encoding="utf-8",
    )
    (artifacts / "svg-browser-environment.json").write_text(
        json.dumps(
            {
                "commit": _run(["git", "rev-parse", "HEAD"]).stdout.strip(),
                "platform": platform.platform(),
                "python": sys.version,
                "node": _run(["node", "--version"]).stdout.strip(),
                "playwright": _run(
                    ["node", "-p", "require('playwright/package.json').version"]
                ).stdout.strip(),
                "browsers": {
                    "chromium": rust["user_agent"],
                    "webkit": rust_webkit["user_agent"],
                },
            },
            indent=2,
        ),
        encoding="utf-8",
    )

    _assert_browser_scaled(rust)
    _assert_browser_scaled(rust_webkit)
    assert rust["browser_logs"] == []
    assert rust_webkit["browser_logs"] == []
    assert 0.0 < rust["max_font_size_px"] < 100.0
    assert rust["painted_text_count"] >= 40
    assert rust["painted_element_count"] >= 60
    assert 0.05 < rust["raster"]["occupancy"] < 0.95
    assert rust["raster"]["painted_bounds"] is not None
    body = max(
        (item for item in rust["painted"] if item["tag"] != "text"),
        key=lambda item: item["width_px"] * item["height_px"],
    )
    assert body["intersects_viewport"] and not body["clipped"]
    assert body["intersects_view_box"] and not body["clipped_view_box"]

    for expected in ("A13", "VDD1V8_1"):
        rust_label = _label(rust, expected)
        assert rust_label["visible"]
        assert rust_label["intersects_viewport"]
        assert not rust_label["clipped"]
        assert rust_label["intersects_view_box"]
        assert not rust_label["clipped_view_box"]
        assert 5.0 < rust_label["height_px"] < 100.0
        assert rust_label["width_px"] > 5.0
        assert _label(python, expected)
        assert _label(kicad, expected)
        webkit_label = _label(rust_webkit, expected)
        assert webkit_label["intersects_viewport"]
        assert not webkit_label["clipped"]
        assert webkit_label["intersects_view_box"]
        assert not webkit_label["clipped_view_box"]

    # KiCad CLI and Python paint retained glyph geometry and keep their
    # matching semantic text hidden. Rust currently has no symbol render cache,
    # so its correctly scaled browser-font fallback is intentionally visible.
    assert python["painted_element_count"] > rust["painted_element_count"]
    assert kicad["painted_element_count"] > rust["painted_element_count"]
    assert python["painted_text_count"] == 0
    assert kicad["painted_text_count"] == 0
    assert python["raster"]["non_white_pixels"] > 1_000
    assert kicad["raster"]["non_white_pixels"] > 1_000


def test_all_four_direct_renderers_emit_browser_safe_dimensional_tokens(
    tmp_path: Path,
) -> None:
    artifacts = _artifact_root(tmp_path)
    cases = [
        (
            "footprint",
            "footprint_plotter_a0_vectors.json",
            "standalone-properties-text-and-text-box",
        ),
        ("symbol", "symbol_plotter_a0_vectors.json", "styled-body-and-pin-text"),
        ("board", "board_plotter_a0_vectors.json", "board-text-follows-python-serializer"),
        (
            "schematic",
            "schematic_plotter_a0_vectors.json",
            "explicit-font-metrics-for-schematic-annotations",
        ),
    ]
    for family, filename, vector_id in cases:
        svg = artifacts / f"{family}.svg"
        _run(
            [
                "cargo",
                "run",
                "--locked",
                "-p",
                "kicad-monkey-svg",
                "--example",
                "render_plot_vector_svg",
                "--",
                family,
                str(PACKAGE_ROOT / "tests" / "parity" / filename),
                vector_id,
                str(svg),
            ]
        )
        svg_text = svg.read_text(encoding="utf-8")
        assert "font-size=\"1000000\"" not in svg_text
        assert "stroke-width=\"100000\"" not in svg_text
        facts = _browser_facts(svg, artifacts / f"{family}.png")
        _assert_browser_scaled(facts)
        assert facts["painted_element_count"] > 0
        assert facts["raster"]["non_white_pixels"] > 10
        assert facts["raster"]["painted_bounds"] is not None
        if facts["texts"]:
            assert 0.0 < facts["max_font_size_px"] < 100.0
            assert all(
                item["intersects_viewport"] and not item["clipped"]
                and item["intersects_view_box"] and not item["clipped_view_box"]
                for item in facts["texts"]
                if item["visible"] and (item["width_px"] > 0 or item["height_px"] > 0)
            )
        if family == "board":
            # The source's styled label has unequal nominal X/Y text sizes,
            # italic glyphs, a descender, right/top alignment, and rotation.
            assert "scale(-0.75 1)" in svg_text
            styled = _label(facts, "styled")
            assert styled["intersects_viewport"] and not styled["clipped"]
            assert styled["intersects_view_box"] and not styled["clipped_view_box"]


def test_actual_cruncher_pcb_and_schematic_artifacts_are_browser_safe(
    tmp_path: Path,
) -> None:
    artifacts = _artifact_root(tmp_path)
    cruncher = artifacts / "cruncher"
    project = (
        PACKAGE_ROOT
        / "packages"
        / "kicad_cruncher"
        / "tests"
        / "corpus"
        / "kicad"
        / "projects"
        / "hlr_test"
        / "hlr_test.kicad_pro"
    )
    _run(
        [
            "cargo",
            "run",
            "--locked",
            "-p",
            "kicad-cruncher-cli",
            "--example",
            "render_review_svgs",
            "--",
            str(project),
            str(cruncher),
        ]
    )
    all_facts = {}
    for family in ("pcb", "schematic"):
        svg = cruncher / f"{family}-review.svg"
        text = svg.read_text(encoding="utf-8")
        assert 'viewBox="0 0 ' in text
        assert "scale(0.000001)" not in text
        assert 'font-size="1270000"' not in text
        facts = _browser_facts(svg, artifacts / f"cruncher-{family}.png")
        _assert_browser_scaled(facts)
        minimum_elements = 5 if family == "pcb" else 10
        assert facts["painted_element_count"] >= minimum_elements
        assert facts["raster"]["non_white_pixels"] >= 100
        if facts["texts"]:
            assert facts["max_font_size_px"] < 100.0
        all_facts[family] = facts
    (artifacts / "cruncher-browser-facts.json").write_text(
        json.dumps(all_facts, indent=2), encoding="utf-8"
    )
