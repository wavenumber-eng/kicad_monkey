"""Measure selected large-board KiCad Cruncher call paths.

This is a research/signoff helper, not a product benchmark suite. Keep
before/after comparisons on the same command line, host class, corpus input,
and dependency lock.
"""

from __future__ import annotations

import argparse
import gc
import importlib.metadata
import json
import platform
import statistics
import sys
import time
from collections.abc import Callable
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
SRC = ROOT / "src" / "py"
if str(SRC) not in sys.path:
    sys.path.insert(0, str(SRC))


JsonObject = dict[str, Any]
TimedCallable = Callable[[], JsonObject]


def _resolve_repo_path(raw: str | None) -> Path | None:
    if raw is None:
        return None
    path = Path(raw)
    if not path.is_absolute():
        path = ROOT / path
    return path.resolve()


def _version(name: str) -> str:
    try:
        return importlib.metadata.version(name)
    except importlib.metadata.PackageNotFoundError:
        return "not-installed"


def _time_call(fn: TimedCallable) -> JsonObject:
    gc.collect()
    started = time.perf_counter()
    result = fn()
    elapsed = time.perf_counter() - started
    return {"seconds": elapsed, **result}


def _summary(rounds: list[JsonObject]) -> JsonObject:
    values = [float(item["seconds"]) for item in rounds]
    return {
        "rounds": len(values),
        "min_seconds": min(values),
        "median_seconds": statistics.median(values),
        "max_seconds": max(values),
    }


def _round_workload(name: str, rounds: int, fn: TimedCallable) -> JsonObject:
    measured: list[JsonObject] = []
    for _ in range(rounds):
        measured.append(_time_call(fn))
    return {
        "workload": name,
        "summary": _summary(measured),
        "rounds": measured,
    }


def _pcb_copper_layers(pcb: object) -> list[str]:
    from kicad_cruncher.kicad_cruncher_cmd_design import _pcb_copper_layers as layers

    return layers(pcb)


def _selected_copper_layers(pcb: object, max_layers: int) -> list[str]:
    layers = _pcb_copper_layers(pcb)
    if max_layers > 0:
        return layers[:max_layers]
    return layers


def _workload_pcb_parse(pcb_path: Path) -> TimedCallable:
    def run() -> JsonObject:
        from kicad_monkey import KiCadPcb

        pcb = KiCadPcb.from_file(pcb_path)
        return {
            "pcb": str(pcb_path.relative_to(ROOT)),
            "layers": len(getattr(pcb, "layers", []) or []),
            "footprints": len(getattr(pcb, "footprints", []) or []),
            "segments": len(getattr(pcb, "segments", []) or []),
            "vias": len(getattr(pcb, "vias", []) or []),
            "zones": len(getattr(pcb, "zones", []) or []),
        }

    return run


def _workload_pcb_clean_dry_run(pcb_path: Path) -> TimedCallable:
    def run() -> JsonObject:
        from kicad_cruncher.kicad_cruncher_pcb_clean import plan_pcb_clean

        plan = plan_pcb_clean(board_path=pcb_path, config_path=None, dry_run=True)
        board_report = plan.get("board_report", {})
        inventory = board_report.get("inventory", {}) if isinstance(board_report, dict) else {}
        return {
            "pcb": str(pcb_path.relative_to(ROOT)),
            "inventory": inventory,
        }

    return run


def _workload_design_pcb_review(
    project_path: Path,
    max_layers: int,
    *,
    cached_svg: bool = False,
) -> TimedCallable:
    def run() -> JsonObject:
        from kicad_cruncher.kicad_cruncher_cmd_design import (
            _PCB_TRACE_COLOR,
            _cached_pcb_review_svg_text,
            _style_pcb_review_svg,
        )
        from kicad_cruncher.kicad_cruncher_pcb_svg_compositor import (
            PcbSvgCompositionRenderCache,
        )
        from kicad_monkey import KiCadDesign

        stage_times: dict[str, float] = {}

        started = time.perf_counter()
        design = KiCadDesign.from_file(project_path)
        stage_times["load_design"] = time.perf_counter() - started

        started = time.perf_counter()
        design_payload = design.to_json(include_indexes=True)
        stage_times["design_json"] = time.perf_counter() - started

        started = time.perf_counter()
        netlist_payload = design.to_netlist_json()
        stage_times["netlist_json"] = time.perf_counter() - started

        started = time.perf_counter()
        kicad_netlist = design.to_kicad_netlist_sexpr(tool="kicad_cruncher", date="")
        stage_times["kicad_netlist"] = time.perf_counter() - started

        pcb = design.pcb
        if pcb is None:
            return {
                "project": str(project_path.relative_to(ROOT)),
                "stage_seconds": stage_times,
                "pcb_layers_rendered": [],
                "pcb_svg_bytes": 0,
                "drill_slot_records": 0,
                "design_components": len(design_payload.get("components", []) or []),
                "netlist_nets": len(netlist_payload.get("nets", []) or []),
                "kicad_netlist_bytes": len(kicad_netlist),
            }

        layers = _selected_copper_layers(pcb, max_layers)
        svg_bytes = 0
        drill_slot_records = 0
        render_cache = PcbSvgCompositionRenderCache(pcb) if cached_svg else None
        started = time.perf_counter()
        for layer in layers:
            if render_cache is not None:
                svg_text = _cached_pcb_review_svg_text(pcb, render_cache, layer)
            else:
                svg_text = pcb.to_svg(
                    layers=[layer, "Edge.Cuts"],
                    fill=_PCB_TRACE_COLOR,
                    stroke=_PCB_TRACE_COLOR,
                    black_and_white=False,
                    profile="enriched",
                )
            styled_svg, count = _style_pcb_review_svg(str(svg_text), layer)
            svg_bytes += len(styled_svg)
            drill_slot_records += count
        stage_name = "pcb_review_cached_svg_loop" if cached_svg else "pcb_review_svg_loop"
        stage_times[stage_name] = time.perf_counter() - started

        return {
            "project": str(project_path.relative_to(ROOT)),
            "stage_seconds": stage_times,
            "pcb_layers_rendered": layers,
            "pcb_svg_bytes": svg_bytes,
            "drill_slot_records": drill_slot_records,
            "design_components": len(design_payload.get("components", []) or []),
            "netlist_nets": len(netlist_payload.get("nets", []) or []),
            "kicad_netlist_bytes": len(kicad_netlist),
        }

    return run


def _workload_pcb_svg_composition(pcb_path: Path, max_layers: int) -> TimedCallable:
    def run() -> JsonObject:
        from kicad_cruncher.kicad_cruncher_pcb_svg_compositor import (
            PcbSvgCompositionRenderCache,
            render_pcb_svg_composition,
        )
        from kicad_cruncher.kicad_cruncher_pcb_svg_config import _PcbSvgConfig
        from kicad_monkey import KiCadPcb

        stage_times: dict[str, float] = {}

        started = time.perf_counter()
        pcb = KiCadPcb.from_file(pcb_path)
        stage_times["load_pcb"] = time.perf_counter() - started

        config = _PcbSvgConfig.default()
        render_cache = PcbSvgCompositionRenderCache(pcb)
        layers = _selected_copper_layers(pcb, max_layers)
        svg_bytes = 0

        started = time.perf_counter()
        for layer in layers:
            composition = render_pcb_svg_composition(
                pcb,
                [layer, "Edge.Cuts"],
                styles=config.global_options.styles,
                group_id=f"profile-{layer.replace('.', '-')}",
                config=config,
                render_cache=render_cache,
            )
            svg_bytes += len(composition.svg_text)
        stage_times["compose_physical_layers"] = time.perf_counter() - started

        return {
            "pcb": str(pcb_path.relative_to(ROOT)),
            "stage_seconds": stage_times,
            "pcb_layers_rendered": layers,
            "pcb_svg_bytes": svg_bytes,
        }

    return run


def _required_path(path: Path | None, option: str, workload: str) -> Path:
    if path is None:
        raise SystemExit(f"{option} is required for {workload}")
    return path


def _workloads(args: argparse.Namespace) -> list[tuple[str, TimedCallable]]:
    pcb_path = _resolve_repo_path(args.pcb)
    project_path = _resolve_repo_path(args.project)
    selected = [item.strip() for item in args.workloads.split(",") if item.strip()]
    builders: dict[str, Callable[[], TimedCallable]] = {
        "pcb-parse": lambda: _workload_pcb_parse(
            _required_path(pcb_path, "--pcb", "pcb-parse")
        ),
        "pcb-clean-dry-run": lambda: _workload_pcb_clean_dry_run(
            _required_path(pcb_path, "--pcb", "pcb-clean-dry-run")
        ),
        "design-pcb-review": lambda: _workload_design_pcb_review(
            _required_path(project_path, "--project", "design-pcb-review"),
            args.max_copper_layers,
        ),
        "design-pcb-review-cached": lambda: _workload_design_pcb_review(
            _required_path(project_path, "--project", "design-pcb-review-cached"),
            args.max_copper_layers,
            cached_svg=True,
        ),
        "pcb-svg-composition": lambda: _workload_pcb_svg_composition(
            _required_path(pcb_path, "--pcb", "pcb-svg-composition"),
            args.max_copper_layers,
        ),
    }
    unknown = sorted(set(selected) - set(builders))
    if unknown:
        raise SystemExit(f"Unknown workload(s): {', '.join(unknown)}")
    workloads = [(name, builders[name]()) for name in dict.fromkeys(selected)]
    if not workloads:
        raise SystemExit("No workloads selected")
    return workloads


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project", help="KiCad project path, relative to repo root if needed")
    parser.add_argument("--pcb", help="KiCad PCB path, relative to repo root if needed")
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument(
        "--max-copper-layers",
        type=int,
        default=2,
        help="Copper layers to render; 0 means all copper layers",
    )
    parser.add_argument(
        "--workloads",
        default="pcb-parse,design-pcb-review,pcb-clean-dry-run,pcb-svg-composition",
        help=(
            "Comma-separated workload names: pcb-parse, design-pcb-review, "
            "design-pcb-review-cached, pcb-clean-dry-run, pcb-svg-composition"
        ),
    )
    parser.add_argument("--output", help="Write JSON result to this path")
    args = parser.parse_args(argv)

    if args.rounds < 1:
        raise SystemExit("--rounds must be >= 1")

    result: JsonObject = {
        "schema": "kicad_cruncher.large_board_profile_probe.a0",
        "repo_root": str(ROOT),
        "python": sys.version,
        "platform": platform.platform(),
        "versions": {
            "kicad-cruncher": _version("kicad-cruncher"),
            "kicad-monkey": _version("kicad-monkey"),
            "wn-geometer": _version("wn-geometer"),
        },
        "args": {
            "project": args.project,
            "pcb": args.pcb,
            "rounds": args.rounds,
            "max_copper_layers": args.max_copper_layers,
            "workloads": args.workloads,
        },
        "results": [],
    }

    for name, fn in _workloads(args):
        result["results"].append(_round_workload(name, args.rounds, fn))

    text = json.dumps(result, indent=2)
    if args.output:
        output = _resolve_repo_path(args.output)
        assert output is not None
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(text + "\n", encoding="utf-8")
    else:
        print(text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
