"""Rack-owned native worksheet parity and exact-write gate."""

from __future__ import annotations

import json
import math
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any

from _suite_paths import KICAD_PACKAGE_ROOT
from kicad_monkey import KiCadWorksheet

PACKAGE_ROOT = KICAD_PACKAGE_ROOT
WORKSHEET_INPUTS = PACKAGE_ROOT / "tests/L1_parsing/cases/worksheets/input"


def _run(command: list[str], *, timeout: int = 900) -> subprocess.CompletedProcess[str]:
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
        f"Command failed: {' '.join(command)}\n"
        f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
    )
    return completed


def _worksheet_executable() -> Path:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the Rust worksheet gate"
    _run(
        [
            cargo,
            "build",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--example",
            "worksheet_gate",
        ]
    )
    return PACKAGE_ROOT / "target/debug/examples" / (
        "worksheet_gate.exe" if os.name == "nt" else "worksheet_gate"
    )


def _worksheet_inputs() -> list[Path]:
    paths = sorted(WORKSHEET_INPUTS.glob("*.kicad_wks"))
    assert len(paths) == 5, f"expected all five durable worksheets: {WORKSHEET_INPUTS}"
    return paths


def test_native_worksheet_model_matches_python_and_writes_exactly() -> None:
    paths = _worksheet_inputs()
    evidence = json.loads(
        _run([str(_worksheet_executable()), *(str(path) for path in paths)]).stdout
    )
    assert evidence["schema"] == "kicad_monkey.worksheet_gate_evidence.a0"
    assert evidence["file_count"] == 5
    assert [Path(item["path"]).resolve() for item in evidence["files"]] == [
        path.resolve() for path in paths
    ]
    for path, actual in zip(paths, evidence["files"]):
        assert actual["source_bytes"] == path.stat().st_size
        assert actual["exact_first_write"]
        assert actual["stable_second_write"]
        assert actual == _python_projection(path)


def test_native_worksheet_resource_mutation_and_io_oracles_are_rack_owned() -> None:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the Rust worksheet gate"
    _run(
        [
            cargo,
            "test",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--test",
            "worksheet",
        ]
    )


def test_native_bitmap_mixed_children_match_python(tmp_path: Path) -> None:
    cases = {
        "modern.kicad_wks": """(kicad_wks (version 20231118)
  (bitmap (data \"a\" 2 (nested \"ignored\") bare \"b\")
    (pngdata \"unused\")))""",
        "legacy.kicad_wks": """(page_layout
  (bitmap (pngdata \"first\" \"second\" 3)))""",
    }
    paths = []
    for name, source in cases.items():
        path = tmp_path / name
        path.write_text(source, encoding="utf-8")
        paths.append(path)
    evidence = json.loads(
        _run([str(_worksheet_executable()), *(str(path) for path in paths)]).stdout
    )
    assert evidence["file_count"] == 2
    for path, actual in zip(paths, evidence["files"]):
        assert actual == _python_projection(path)


def _python_projection(path: Path) -> dict[str, Any]:
    worksheet = KiCadWorksheet.from_file(path)
    setup = worksheet.setup
    return {
        "path": str(path),
        "source_bytes": path.stat().st_size,
        "format": "kicad_wks" if worksheet.version > 0 else "page_layout",
        "version": worksheet.version,
        "generator": worksheet.generator,
        "generator_version": worksheet.generator_version,
        "setup": {
            "text_size_x": setup.text_size_x,
            "text_size_y": setup.text_size_y,
            "linewidth": setup.linewidth,
            "textlinewidth": setup.textlinewidth,
            "left_margin": setup.left_margin,
            "right_margin": setup.right_margin,
            "top_margin": setup.top_margin,
            "bottom_margin": setup.bottom_margin,
        },
        "items": [_python_item(kind, item) for kind, item in worksheet._ordered_items],
        "exact_first_write": True,
        "stable_second_write": True,
    }


def _python_item(kind: str, item: Any) -> dict[str, Any]:
    common = {
        "kind": kind,
        "name": item.name,
        "comment": item.comment,
        "option": item.option,
        "repeat": _repeat(item.repeat),
    }
    if kind in {"line", "rect"}:
        return {
            **common,
            "start": _point(item.start),
            "end": _point(item.end),
            "linewidth": _optional_float(item.linewidth),
        }
    if kind == "polygon":
        return {
            **common,
            "position": _point(item.pos),
            "rotate": item.rotate,
            "linewidth": _optional_float(item.linewidth),
            "point_sets": [[list(point) for point in points] for points in item.point_sets],
        }
    if kind == "tbtext":
        font = item.font
        return {
            **common,
            "text": item.text,
            "position": _point(item.pos),
            "rotate": item.rotate,
            "justify": item.justify,
            "max_length": item.max_len,
            "max_height": item.max_height,
            "font": {
                "size_x": font.size_x,
                "size_y": font.size_y,
                "linewidth": _optional_float(font.linewidth),
                "bold": font.bold,
                "italic": font.italic,
                "face": font.face,
                "color": list(font.color) if font.color is not None else None,
            },
        }
    assert kind == "bitmap"
    return {
        **common,
        "position": _point(item.pos),
        "scale": item.scale,
        "data_parts": item.data_chunks,
    }


def _point(value: Any) -> dict[str, Any]:
    return {"x": value.x, "y": value.y, "corner": value.corner.value}


def _repeat(value: Any) -> dict[str, Any]:
    return {
        "count": value.count,
        "increment_x": value.incr_x,
        "increment_y": value.incr_y,
        "increment_label": value.incr_label,
    }


def _optional_float(value: float) -> float | None:
    return None if math.isnan(value) else value
