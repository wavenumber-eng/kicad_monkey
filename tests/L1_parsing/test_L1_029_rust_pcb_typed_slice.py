"""Rack ownership for the native Rust PCB reader/writer vertical slice."""

from __future__ import annotations

import json
import os
from pathlib import Path
import shutil
import subprocess

from _suite_paths import TEST_CORPUS_ROOT
from kicad_monkey import KiCadPcb


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
CORPUS_ROOT = Path(os.environ.get("WN_TEST_CORPUS", TEST_CORPUS_ROOT))
CORPUS_BOARDS = (
    CORPUS_ROOT
    / "kicad/projects/4-ch-backplane/input/4-ch-backplane.kicad_pcb",
    CORPUS_ROOT
    / "kicad/projects/speedy_processing_module/input/11-10084__speedy_processing_module__B.kicad_pcb",
)


def _run(command: list[str], *, timeout: int = 180) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
    )
    assert completed.returncode == 0, (
        f"Command failed: {' '.join(command)}\n"
        f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
    )
    return completed


def test_rack_runs_native_pcb_reader_writer_correctness_gate() -> None:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the Rust PCB gate"
    _run(
        [
            cargo,
            "test",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--test",
            "pcb_typed_slice",
        ]
    )


def test_native_pcb_projection_matches_python_on_promoted_corpus() -> None:
    missing = [str(path) for path in CORPUS_BOARDS if not path.is_file()]
    assert not missing, (
        "required promoted PCB corpus evidence is unavailable; missing: "
        + ", ".join(missing)
    )
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the Rust PCB gate"
    _run(
        [
            cargo,
            "build",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--example",
            "pcb_projection_gate",
        ]
    )
    executable = (
        PACKAGE_ROOT
        / "target/debug/examples"
        / ("pcb_projection_gate.exe" if os.name == "nt" else "pcb_projection_gate")
    )
    summaries = json.loads(
        _run([str(executable), *(str(path) for path in CORPUS_BOARDS)]).stdout
    )
    assert len(summaries) == len(CORPUS_BOARDS)
    for board_path, summary in zip(CORPUS_BOARDS, summaries, strict=True):
        _assert_summary_matches_python(board_path, summary)


def _assert_summary_matches_python(board_path: Path, summary: dict) -> None:
    board = KiCadPcb.from_file(board_path)
    assert summary["source_bytes"] == board_path.stat().st_size
    assert summary["counts"] == {
        "layers": len(board.layers),
        "nets": len(board.nets),
        "properties": len(board.properties),
        "footprints": len(board.footprints),
        "pads": sum(len(footprint.pads) for footprint in board.footprints),
        "models": sum(len(footprint.models) for footprint in board.footprints),
        "segments": len(board.segments),
        "vias": len(board.vias),
        "zones": len(board.zones),
    }
    if board.footprints:
        assert summary["first_footprint"] == {
            "library_link": board.footprints[0].library_link,
            "reference": board.footprints[0].get_property_value("Reference"),
        }
    if board.segments:
        assert summary["first_segment"] == {
            "start_x": board.segments[0].start_x,
            "end_x": board.segments[0].end_x,
            "net": {
                "ordinal": board.segments[0].net.ordinal,
                "name": board.segments[0].net.name or None,
            },
        }
    if board.vias:
        assert summary["first_via"] == {
            "at_x": board.vias[0].at_x,
            "at_y": board.vias[0].at_y,
            "net": {
                "ordinal": board.vias[0].net.ordinal,
                "name": board.vias[0].net.name or None,
            },
        }
    pads = [pad for footprint in board.footprints for pad in footprint.pads]
    if pads:
        pad = pads[0]
        assert summary["first_pad"] == {
            "number": pad.number,
            "kind": pad.pad_type.value,
            "shape": pad.shape.value,
            "at_x": pad.at_x,
            "at_y": pad.at_y,
            "size_x": pad.size_x,
            "size_y": pad.size_y,
            "layers": pad.layers,
            "net": {
                "ordinal": pad.net.ordinal,
                "name": pad.net.name or None,
            },
        }
    models = [model for footprint in board.footprints for model in footprint.models]
    if models:
        model = models[0]
        assert summary["first_model"] == {
            "path": model.path,
            "offset": list(model.offset),
            "scale": list(model.scale),
            "rotate": list(model.rotate),
        }
