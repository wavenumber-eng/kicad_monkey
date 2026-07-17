"""Run repeatable PCB parser and projection performance probes.

This script is intentionally advisory. It is for optimization research and
should not be treated as a stable performance gate without reviewer approval.
"""

from __future__ import annotations

import argparse
import gc
import json
import platform
import statistics
import sys
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import Any

from kicad_monkey import KiCadPcb, KiCadPcbProjection, __version__


@dataclass(frozen=True)
class ProbeCase:
    """One PCB text or file path to measure."""

    name: str
    source: str | Path
    kind: str

    def read_text(self) -> str:
        if isinstance(self.source, Path):
            return self.source.read_text(encoding="utf-8")
        return self.source

    @property
    def size_bytes(self) -> int:
        if isinstance(self.source, Path):
            return self.source.stat().st_size
        return len(self.source.encode("utf-8"))


def _net_name(index: int) -> str:
    return "" if index == 0 else f"/N{index}"


def make_net_dense_board(
    *,
    nets: int,
    footprints: int,
    pads_per_footprint: int,
    segments: int,
    vias: int,
) -> str:
    """Return a synthetic board stressing net-bound object resolution."""
    lines = [
        "(kicad_pcb",
        "  (version 20240108)",
        '  (generator "kicad")',
        '  (generator_version "10.0.3")',
        '  (paper "A4")',
        "  (layers",
        '    (0 "F.Cu" signal)',
        '    (31 "B.Cu" signal))',
    ]
    for net in range(nets):
        lines.append(f'  (net {net} "{_net_name(net)}")')
    usable_nets = max(1, nets - 1)
    for fp_index in range(footprints):
        lines.extend(
            [
                f'  (footprint "Device:R" (layer "F.Cu") (at {fp_index % 200} {fp_index // 200} 0)',
                f'    (property "Reference" "R{fp_index}" (at 0 0 0) (layer "F.SilkS"))',
                '    (property "Value" "10k" (at 0 1 0) (layer "F.Fab"))',
            ]
        )
        for pad_index in range(pads_per_footprint):
            net = ((fp_index * pads_per_footprint + pad_index) % usable_nets) + 1
            lines.append(
                f'    (pad "{pad_index + 1}" smd rect (at {pad_index} 0) '
                f'(size 1 1) (layers "F.Cu") (net {net} "{_net_name(net)}"))'
            )
        lines.append("  )")
    for index in range(segments):
        net = (index % usable_nets) + 1
        lines.append(
            f'  (segment (start {index % 100} 0) (end {(index % 100) + 1} 0) '
            f'(width 0.1) (layer "F.Cu") (net {net}) (uuid "seg{index}"))'
        )
    for index in range(vias):
        net = (index % usable_nets) + 1
        lines.append(
            f'  (via (at {index % 100} 1) (size 0.4) (drill 0.2) '
            f'(layers "F.Cu" "B.Cu") (net {net}) (uuid "via{index}"))'
        )
    lines.append(")")
    return "\n".join(lines) + "\n"


def make_nested_span_board(*, footprints: int, pads_per_footprint: int) -> str:
    """Return a synthetic board stressing nested pad/model source spans."""
    lines = [
        "(kicad_pcb",
        "  (version 20240108)",
        '  (generator "kicad")',
        '  (generator_version "10.0.3")',
        '  (paper "A4")',
        "  (layers",
        '    (0 "F.Cu" signal)',
        '    (31 "B.Cu" signal))',
        '  (net 0 "")',
        '  (net 1 "/N1")',
    ]
    for fp_index in range(footprints):
        lines.extend(
            [
                f'  (footprint "Device:R" (layer "F.Cu") (at {fp_index % 200} {fp_index // 200} 0)',
                f'    (property "Reference" "R{fp_index}" (at 0 0 0) (layer "F.SilkS"))',
                '    (property "Value" "10k" (at 0 1 0) (layer "F.Fab"))',
            ]
        )
        for pad_index in range(pads_per_footprint):
            lines.append(
                f'    (pad "{pad_index + 1}" smd rect (at {pad_index} 0) '
                '(size 1 1) (layers "F.Cu") (net 1 "/N1"))'
            )
        lines.extend(
            [
                '    (model "${KICAD10_3DMODEL_DIR}/Resistor.3dshapes/R.step"',
                "      (offset (xyz 0 0 0))",
                "      (scale (xyz 1 1 1))",
                "      (rotate (xyz 0 0 0))))",
            ]
        )
    lines.append(")")
    return "\n".join(lines) + "\n"


def make_top_level_scan_board(*, nets: int, gr_lines: int, gr_texts: int) -> str:
    """Return a synthetic board stressing repeated top-level family scans."""
    lines = [
        "(kicad_pcb",
        "  (version 20240108)",
        '  (generator "kicad")',
        '  (generator_version "10.0.3")',
        '  (paper "A4")',
        "  (layers",
        '    (0 "F.Cu" signal)',
        '    (31 "B.Cu" signal))',
    ]
    for net in range(nets):
        lines.append(f'  (net {net} "{_net_name(net)}")')
    for index in range(gr_lines):
        lines.append(
            f'  (gr_line (start {index % 100} 0) (end {(index % 100) + 1} 0) '
            '(stroke (width 0.1) (type solid)) (layer "F.SilkS"))'
        )
    for index in range(gr_texts):
        lines.append(
            f'  (gr_text "T{index}" (at {index % 100} 2 0) '
            '(effects (font (size 1 1) (thickness 0.15))) (layer "F.SilkS"))'
        )
    lines.append(")")
    return "\n".join(lines) + "\n"


def synthetic_cases(scale: int) -> list[ProbeCase]:
    """Return deterministic synthetic benchmark cases."""
    return [
        ProbeCase(
            name="synthetic-net-dense",
            kind="synthetic",
            source=make_net_dense_board(
                nets=300 * scale,
                footprints=200 * scale,
                pads_per_footprint=4,
                segments=900 * scale,
                vias=900 * scale,
            ),
        ),
        ProbeCase(
            name="synthetic-nested-spans",
            kind="synthetic",
            source=make_nested_span_board(
                footprints=600 * scale,
                pads_per_footprint=2,
            ),
        ),
        ProbeCase(
            name="synthetic-top-level-scan",
            kind="synthetic",
            source=make_top_level_scan_board(
                nets=200 * scale,
                gr_lines=1000 * scale,
                gr_texts=250 * scale,
            ),
        ),
    ]


def discover_corpus_cases(repo_root: Path, *, limit: int) -> list[ProbeCase]:
    """Return the largest local corpus project boards, if available."""
    projects = repo_root / "tests" / "corpus" / ".unpacked" / "kicad" / "projects"
    if not projects.exists():
        return []
    boards = sorted(projects.rglob("*.kicad_pcb"), key=lambda path: path.stat().st_size, reverse=True)
    return [
        ProbeCase(name=f"corpus:{path.parent.parent.name}/{path.name}", kind="corpus", source=path)
        for path in boards[:limit]
    ]


def _full_parse(text: str) -> dict[str, int]:
    pcb = KiCadPcb.from_string(text)
    return {
        "footprints": len(pcb.footprints),
        "pads": sum(len(getattr(footprint, "pads", ()) or ()) for footprint in pcb.footprints),
        "nets": len(pcb.nets),
        "segments": len(pcb.segments),
        "vias": len(pcb.vias),
        "arcs": len(pcb.arcs),
        "zones": len(pcb.zones),
    }


def _projection_routes(text: str) -> dict[str, int]:
    projection = KiCadPcbProjection(source_text=text)
    return {
        "nets": len(projection.nets()),
        "segments": len(projection.segments()),
        "vias": len(projection.vias()),
        "arcs": len(projection.arcs()),
        "zones": len(projection.zones()),
    }


def _projection_nested(text: str) -> dict[str, int]:
    projection = KiCadPcbProjection(source_text=text)
    models = projection.model_references()
    spans = [item.model_span for item in models if item.model_span is not None]
    return {
        "footprints": len(projection.footprints()),
        "pads": len(projection.pads()),
        "models": len(models),
        "model_spans": len(spans),
    }


def _projection_common_families(text: str) -> dict[str, int]:
    projection = KiCadPcbProjection(source_text=text)
    return {
        "layers": len(projection.layers()),
        "nets": len(projection.nets()),
        "properties": len(projection.properties()),
        "footprints": len(projection.footprints()),
        "gr_lines": len(projection.gr_lines()),
        "gr_texts": len(projection.gr_texts()),
        "segments": len(projection.segments()),
        "vias": len(projection.vias()),
        "zones": len(projection.zones()),
        "unknown": len(projection.unknown_elements()),
    }


OPERATIONS: dict[str, Callable[[str], dict[str, int]]] = {
    "full_parse": _full_parse,
    "projection_routes": _projection_routes,
    "projection_nested": _projection_nested,
    "projection_common_families": _projection_common_families,
}


def run_timed(operation: Callable[[str], dict[str, int]], text: str, rounds: int) -> dict[str, Any]:
    """Run one operation for ``rounds`` and return timing statistics."""
    timings: list[float] = []
    result: dict[str, int] | None = None
    for _ in range(rounds):
        gc.collect()
        start = time.perf_counter()
        result = operation(text)
        timings.append(time.perf_counter() - start)
    return {
        "rounds": rounds,
        "best_s": min(timings),
        "median_s": statistics.median(timings),
        "max_s": max(timings),
        "result": result,
    }


def run_case(case: ProbeCase, *, rounds: int) -> dict[str, Any]:
    """Run all probe operations for one case."""
    text = case.read_text()
    operations: dict[str, Any] = {}
    for name, operation in OPERATIONS.items():
        try:
            operations[name] = run_timed(operation, text, rounds)
        except Exception as exc:  # pragma: no cover - diagnostic script
            operations[name] = {
                "error": f"{type(exc).__name__}: {exc}",
                "rounds": rounds,
            }
    return {
        "name": case.name,
        "kind": case.kind,
        "size_bytes": case.size_bytes,
        "operations": operations,
    }


def main() -> None:
    """Parse arguments and run the performance probes."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--rounds", type=int, default=3, help="Timing rounds per operation.")
    parser.add_argument("--synthetic-scale", type=int, default=1, help="Synthetic case scale factor.")
    parser.add_argument("--corpus-limit", type=int, default=4, help="Largest corpus project boards to probe.")
    parser.add_argument("--json-out", type=Path, default=None, help="Optional JSON output path.")
    parser.add_argument(
        "--synthetic-only",
        action="store_true",
        help="Skip local corpus discovery and only run synthetic cases.",
    )
    args = parser.parse_args()

    repo_root = Path(__file__).resolve().parents[2]
    cases = synthetic_cases(max(1, args.synthetic_scale))
    if not args.synthetic_only:
        cases.extend(discover_corpus_cases(repo_root, limit=max(0, args.corpus_limit)))

    payload = {
        "metadata": {
            "python": sys.version,
            "platform": platform.platform(),
            "kicad_monkey_version": __version__,
            "rounds": args.rounds,
            "synthetic_scale": args.synthetic_scale,
            "corpus_limit": args.corpus_limit if not args.synthetic_only else 0,
        },
        "cases": [run_case(case, rounds=max(1, args.rounds)) for case in cases],
    }

    text = json.dumps(payload, indent=2, sort_keys=True)
    if args.json_out is not None:
        args.json_out.parent.mkdir(parents=True, exist_ok=True)
        args.json_out.write_text(text + "\n", encoding="utf-8")
    print(text)


if __name__ == "__main__":
    main()
