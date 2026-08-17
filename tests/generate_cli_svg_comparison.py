"""Generate KiCad CLI vs kicad_monkey schematic SVG comparison artifacts.

This is a downstream oracle layer: KiCad exports final SVGs through
``kicad-cli sch export svg`` and kicad_monkey exports the same sheets through
``KiCadDesign -> schematic IR -> render_ir_to_svg``.  The generated report
keeps the raw SVGs, overlay SVGs, and focused metrics for text and image
placement.
"""

from __future__ import annotations

import argparse
import base64
import html
import json
import math
import os
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from datetime import datetime, timezone
from importlib.util import find_spec
from pathlib import Path
from typing import Any
from xml.etree import ElementTree as ET

from kicad_cli_resolver import resolve_kicad_cli
from svg.svg_diff_helpers import compare_svg_bounds, create_overlay_diff


REPO_ROOT = Path(__file__).resolve().parents[1]
LOCAL_KICAD_MONKEY_SRC = REPO_ROOT / "src" / "py"
if LOCAL_KICAD_MONKEY_SRC.exists() and str(LOCAL_KICAD_MONKEY_SRC) not in sys.path:
    sys.path.insert(0, str(LOCAL_KICAD_MONKEY_SRC))

from kicad_monkey.testing.corpus import get_kicad_corpus_root  # noqa: E402

PLOTTED_RE = re.compile(r"Plotted to ['\"]([^'\"]+\.svg)['\"]", re.IGNORECASE)
TRANSFORM_RE = re.compile(r"([A-Za-z]+)\(([^)]*)\)")
NUMBER_RE = re.compile(r"[-+]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][-+]?\d+)?")
VIEWBOX_RE = re.compile(r"<svg\b[^>]*\bviewBox=(['\"])(.*?)\1", re.IGNORECASE | re.DOTALL)
TEXT_MATCH_MAX_DISTANCE_MM = 20.0
# KiCad's SVG plotter emits hidden text anchors and textLength at four
# decimal places, while kicad_monkey computes text anchors in integer
# schematic IUs through Python FreeType/HarfBuzz bindings.  Treat a
# 1-micron axis delta as the same SVG text placement so reports surface
# real layout errors instead of host-font rounding noise.
TEXT_SVG_PRECISION_EQUIV_MM = 0.001
# textLength is serialized by KiCad at four decimals.  A one-quantum
# attribute delta is retained as raw data but is not a semantic mismatch.
TEXT_LENGTH_SVG_PRECISION_EQUIV_MM = 0.0001
SVG_PAN_ZOOM_ASSET = "svg-pan-zoom.min.js"
SVG_PAN_ZOOM_VENDOR_CANDIDATES: tuple[Path, ...] = ()

_PANZOOM_CSS = """
body { font-family: Segoe UI, Arial, sans-serif; margin: 0; color: #1f2933; background: #f7f8fa; }
header { position: sticky; top: 0; z-index: 3; background: #f7f8fa; border-bottom: 1px solid #d9e2ec; padding: 12px 16px 9px; }
main { padding: 0 16px 18px; }
h1 { margin: 0 0 4px; }
.meta { color: #52606d; margin-top: 0; line-height: 1.35; }
table { border-collapse: collapse; width: 100%; background: white; margin: 14px 0 22px; }
th, td { border: 1px solid #d9e2ec; padding: 6px 8px; text-align: left; vertical-align: top; }
th { background: #e4edf7; }
section { background: white; border-top: 1px solid #d9e2ec; border-bottom: 1px solid #d9e2ec; padding: 12px 0; margin: 16px -16px; }
section > h2, section > .meta { padding-left: 16px; padding-right: 16px; }
details.sheet { border: 1px solid #d9e2ec; margin: 10px 16px; background: #fbfcfd; }
details.sheet > summary { cursor: pointer; padding: 8px 10px; font-weight: 600; }
.links { display: flex; flex-wrap: wrap; gap: 10px; font-size: 12px; padding: 0 10px 8px; }
.viewers { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 520px), 1fr)); grid-auto-rows: minmax(0, 1fr); gap: 10px; padding: 10px; height: calc(100vh - 220px); min-height: 560px; overflow: hidden; }
figure { display: flex; flex-direction: column; min-height: 0; margin: 0; background: white; border: 1px solid #d9e2ec; min-width: 0; }
figcaption { display: flex; justify-content: space-between; gap: 8px; align-items: center; font-size: 12px; padding: 5px 7px; border-bottom: 1px solid #d9e2ec; color: #334e68; }
.panzoom-actions { display: inline-flex; gap: 4px; }
.panzoom-actions button { min-width: 28px; height: 24px; border: 1px solid #bcccdc; background: #fff; color: #243b53; cursor: pointer; }
.panzoom-frame { position: relative; flex: 1 1 auto; min-height: 360px; overflow: hidden; background: #fff; touch-action: none; cursor: grab; contain: strict; }
.panzoom-frame.dragging { cursor: grabbing; }
.panzoom-frame object { width: 100%; height: 100%; border: 0; display: block; pointer-events: auto; }
.panzoom-status { position: absolute; inset: auto 8px 8px auto; max-width: calc(100% - 16px); padding: 3px 6px; border: 1px solid #d9e2ec; background: rgba(255, 255, 255, 0.88); color: #52606d; font-size: 11px; pointer-events: none; }
.panzoom-frame.loaded .panzoom-status { display: none; }
.panzoom-frame.failed .panzoom-status { color: #9b1c1c; }
.mini { font-size: 12px; margin: 8px 10px 10px; width: calc(100% - 20px); }
.metric { padding: 0 10px 8px; color: #334e68; font-size: 13px; }
.delta-details { margin: 10px; font-size: 12px; }
.delta-details > summary { cursor: pointer; font-weight: 600; color: #334e68; }
.review-controls { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; margin: 10px 0 0; }
.review-controls select { flex: 1 1 520px; min-width: min(720px, 100%); height: 32px; border: 1px solid #bcccdc; background: #fff; color: #1f2933; }
.review-index { margin: 12px 0; }
.review-index > summary { cursor: pointer; font-weight: 600; }
.review-index table { margin-bottom: 0; }
.sheet-card[hidden], .unit-card[hidden] { display: none; }
.sheet-card { margin-top: 12px; }
code { background: #eef2f7; padding: 1px 4px; border-radius: 3px; }
@media (max-width: 720px) {
  .viewers { grid-template-columns: 1fr; }
  .viewers { height: calc(100vh - 180px); min-height: 380px; }
  .panzoom-frame { min-height: 280px; }
}
"""

_PANZOOM_JS = """
(() => {
  function clamp(value, min, max) {
    return Math.max(min, Math.min(max, value));
  }

  function isVisible(element) {
    return !!(element.offsetWidth || element.offsetHeight || element.getClientRects().length);
  }

  function loadSvgPanZoomLibrary() {
    if (window.svgPanZoom) return Promise.resolve(window.svgPanZoom);
    return Promise.reject(new Error('svg-pan-zoom is not loaded'));
  }

  function createObjectPanZoomFallback(frame, object, loadSvg) {
    let scale = 1;
    let panX = 0;
    let panY = 0;
    let drag = null;

    object.style.position = 'absolute';
    object.style.inset = 'auto';
    object.style.pointerEvents = 'none';

    function apply() {
      const width = Math.max(frame.clientWidth, 1);
      const height = Math.max(frame.clientHeight, 1);
      object.style.width = `${width * scale}px`;
      object.style.height = `${height * scale}px`;
      object.style.left = `${panX}px`;
      object.style.top = `${panY}px`;
      frame.classList.add('loaded', 'fallback');
    }

    function zoomAt(nextScale, x, y) {
      loadSvg();
      nextScale = clamp(nextScale, 0.08, 80);
      const ratio = nextScale / scale;
      panX = x - (x - panX) * ratio;
      panY = y - (y - panY) * ratio;
      scale = nextScale;
      apply();
    }

    frame.addEventListener('wheel', (event) => {
      event.preventDefault();
      const rect = frame.getBoundingClientRect();
      const factor = event.deltaY < 0 ? 1.18 : 1 / 1.18;
      zoomAt(scale * factor, event.clientX - rect.left, event.clientY - rect.top);
    }, { passive: false });
    frame.addEventListener('pointerdown', (event) => {
      if (event.target.closest('button')) return;
      loadSvg();
      drag = { id: event.pointerId, x: event.clientX, y: event.clientY, panX, panY };
      frame.classList.add('dragging');
      frame.setPointerCapture(event.pointerId);
    });
    frame.addEventListener('pointermove', (event) => {
      if (!drag || drag.id !== event.pointerId) return;
      panX = drag.panX + event.clientX - drag.x;
      panY = drag.panY + event.clientY - drag.y;
      apply();
    });
    function endDrag(event) {
      if (!drag || drag.id !== event.pointerId) return;
      drag = null;
      frame.classList.remove('dragging');
    }
    frame.addEventListener('pointerup', endDrag);
    frame.addEventListener('pointercancel', endDrag);
    apply();
    return {
      zoomIn: () => zoomAt(scale * 1.25, frame.clientWidth / 2, frame.clientHeight / 2),
      zoomOut: () => zoomAt(scale / 1.25, frame.clientWidth / 2, frame.clientHeight / 2),
      reset: () => {
        scale = 1;
        panX = 0;
        panY = 0;
        apply();
      },
      resize: apply,
    };
  }

  function setupPanZoom(frame) {
    const object = frame.querySelector('object');
    const status = frame.querySelector('.panzoom-status');
    const src = frame.dataset.svgSrc || object.getAttribute('data-src');
    let instance = null;
    let fallback = null;

    function setStatus(text, failed = false) {
      if (!status) return;
      status.textContent = text;
      frame.classList.toggle('failed', failed);
    }

    function initPanZoom() {
      if (instance || fallback || !object.getAttribute('data')) return;
      loadSvgPanZoomLibrary()
        .then((svgPanZoom) => {
          instance = svgPanZoom(object, {
            zoomEnabled: true,
            controlIconsEnabled: false,
            fit: true,
            center: true,
            minZoom: 0.02,
            maxZoom: 200,
            dblClickZoomEnabled: true,
            mouseWheelZoomEnabled: true,
            preventMouseEventsDefault: true,
          });
          instance.resize();
          instance.fit();
          instance.center();
          frame._svgPanZoomInstance = instance;
          frame.classList.add('loaded');
        })
        .catch(() => {
          fallback = fallback || createObjectPanZoomFallback(frame, object, loadSvg);
          frame._objectPanZoomFallback = fallback;
          setStatus('File-mode fallback pan/zoom active');
        });
    }

    function reset() {
      loadSvg();
      if (instance) {
        instance.resize();
        instance.fit();
        instance.center();
      } else if (fallback) {
        fallback.reset();
      }
      frame.classList.add('loaded');
    }

    function loadSvg() {
      if (!src || object.getAttribute('data-loaded') === '1') return;
      setStatus('Loading SVG...');
      object.setAttribute('data-loaded', '1');
      object.setAttribute('data', src);
    }

    object.addEventListener('load', initPanZoom);
    const figure = frame.closest('figure');
    figure.querySelector('[data-panzoom-action="in"]').addEventListener('click', () => {
      loadSvg();
      if (instance) instance.zoomIn();
      else if (fallback) fallback.zoomIn();
    });
    figure.querySelector('[data-panzoom-action="out"]').addEventListener('click', () => {
      loadSvg();
      if (instance) instance.zoomOut();
      else if (fallback) fallback.zoomOut();
    });
    figure.querySelector('[data-panzoom-action="reset"]').addEventListener('click', () => {
      reset();
    });
    if (isVisible(frame)) {
      loadSvg();
    }
  }
  window.kicadReviewLoadVisibleSvgs = () => {
    document.querySelectorAll('.panzoom-frame').forEach((frame) => {
      if (isVisible(frame)) {
        const object = frame.querySelector('object');
        const src = frame.dataset.svgSrc || object.getAttribute('data-src');
        if (src && object.getAttribute('data-loaded') !== '1') {
          object.setAttribute('data-loaded', '1');
          object.setAttribute('data', src);
        } else if (object.getAttribute('data-loaded') === '1' && frame._svgPanZoomInstance) {
          frame._svgPanZoomInstance.resize();
        } else if (object.getAttribute('data-loaded') === '1' && frame._objectPanZoomFallback) {
          frame._objectPanZoomFallback.resize();
        }
      }
    });
  };
  window.addEventListener('DOMContentLoaded', () => {
    document.querySelectorAll('.panzoom-frame').forEach(setupPanZoom);
    window.kicadReviewLoadVisibleSvgs();
  });
})();
"""


def _require_text_metric_dependencies() -> None:
    missing = [
        module
        for module in ("freetype", "uharfbuzz")
        if find_spec(module) is None
    ]
    if find_spec("zstd") is None and find_spec("zstandard") is None:
        missing.append("zstandard")
    if missing:
        raise RuntimeError(
            "KiCad SVG comparison requires kicad_monkey's text-metric "
            "dependencies: "
            + ", ".join(missing)
            + ". Run this with the package project environment, for example: "
            "`uv run python tests\\generate_cli_svg_comparison.py ...`."
        )


def _default_kicad_root() -> Path:
    return get_kicad_corpus_root()


def _html(value: object) -> str:
    return html.escape(str(value), quote=True)


def _slug(value: str) -> str:
    return re.sub(r"[^A-Za-z0-9_.-]+", "_", value).strip("_") or "sheet"


def _rel_href(path: Path, *, from_dir: Path) -> str:
    return os.path.relpath(path, start=from_dir).replace(os.sep, "/")


def _svg_pan_zoom_script() -> str:
    for candidate in SVG_PAN_ZOOM_VENDOR_CANDIDATES:
        if candidate.exists() and candidate.stat().st_size > 10_000:
            script = candidate.read_text(encoding="utf-8")
            if "svgPanZoom" not in script:
                raise RuntimeError(f"Vendored {SVG_PAN_ZOOM_ASSET} did not look valid: {candidate}")
            return script
    return ""


def _inline_script_block(script: str) -> str:
    return "<script>\n" + script.replace("</script", "<\\/script") + "\n</script>"


def _load_manifest(kicad_root: Path) -> dict[str, Any]:
    manifest_path = kicad_root / "manifest.json"
    return json.loads(manifest_path.read_text(encoding="utf-8-sig"))


def _iter_real_world_schematic_cases(kicad_root: Path) -> list[dict[str, Any]]:
    manifest = _load_manifest(kicad_root)
    out: list[dict[str, Any]] = []
    for case in manifest.get("cases") or []:
        if not isinstance(case, dict):
            continue
        if case.get("status") != "active":
            continue
        if case.get("origin") != "real_world":
            continue
        if "schematic_ir" not in (case.get("domains") or []):
            continue
        if not case.get("project_file"):
            continue
        out.append(case)
    return sorted(out, key=lambda item: str(item.get("name") or item.get("id") or ""))


def _select_cases(
    cases: list[dict[str, Any]],
    *,
    names: set[str] | None,
    max_cases: int | None,
) -> list[dict[str, Any]]:
    selected: list[dict[str, Any]] = []
    for case in cases:
        case_name = str(case.get("name") or "")
        case_id = str(case.get("id") or "")
        if names and case_name not in names and case_id not in names:
            continue
        selected.append(case)
        if max_cases is not None and len(selected) >= max_cases:
            break
    return selected


def _resolve_case_path(kicad_root: Path, case: dict[str, Any], key: str) -> Path | None:
    value = case.get(key)
    if value in (None, ""):
        return None
    return kicad_root / str(value)


def _clear_svg_dir(path: Path) -> None:
    path.mkdir(parents=True, exist_ok=True)
    for svg_path in path.glob("*.svg"):
        svg_path.unlink()


def _stage_project_for_cli(
    *,
    project_file: Path,
    root_schematic: Path,
    stage_dir: Path,
) -> Path:
    """Copy a project to scratch before invoking KiCad CLI.

    KiCad writes `.kicad_prl` sidecar state next to the project.  The
    comparison oracle should never dirty corpus inputs, so the CLI always
    runs against a disposable staged tree.
    """

    project_root = project_file.parent.resolve()
    root_schematic = root_schematic.resolve()
    try:
        root_rel = root_schematic.relative_to(project_root)
    except ValueError as exc:
        raise RuntimeError(
            f"Root schematic {root_schematic} is not under project root {project_root}"
        ) from exc

    if stage_dir.exists():
        shutil.rmtree(stage_dir)

    def ignore(_directory: str, names: list[str]) -> set[str]:
        ignored: set[str] = set()
        for name in names:
            lower = name.casefold()
            if lower.endswith(".kicad_prl") or lower in {
                "_stage",
                "output",
                "reference_output",
                "__pycache__",
            }:
                ignored.add(name)
        return ignored

    shutil.copytree(project_root, stage_dir, ignore=ignore)
    return stage_dir / root_rel


def _stage_cli_color_theme(
    *,
    preferences_dir: Path,
    theme_name: str,
    stage_dir: Path,
) -> Path:
    """Stage only KiCad color theme files for CLI reference plotting.

    Pointing ``KICAD_CONFIG_HOME`` at a full user preferences tree also makes
    KiCad consume editor defaults such as the default font, which changes
    placement.  The SVG reference only needs the color theme, so keep this
    scratch config intentionally narrow.
    """

    if stage_dir.exists():
        shutil.rmtree(stage_dir)
    colors_dir = stage_dir / "colors"
    colors_dir.mkdir(parents=True, exist_ok=True)
    for name in {theme_name, "user"}:
        source = preferences_dir / "colors" / f"{name}.json"
        if source.exists():
            shutil.copyfile(source, colors_dir / source.name)
    return stage_dir


def _run_kicad_cli_export(
    *,
    kicad_cli: Path,
    root_schematic: Path,
    output_dir: Path,
    timeout_s: int,
    theme_name: str | None = None,
    default_font: str | None = None,
    config_home: Path | None = None,
) -> tuple[list[Path], str]:
    _clear_svg_dir(output_dir)
    cmd = [
        str(kicad_cli),
        "sch",
        "export",
        "svg",
        "--output",
        str(output_dir),
        str(root_schematic),
    ]
    if theme_name:
        cmd[4:4] = ["--theme", theme_name]
    if default_font:
        cmd[4:4] = ["--default-font", default_font]
    env = os.environ.copy()
    if config_home is not None:
        env["KICAD_CONFIG_HOME"] = str(config_home)
    result = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        timeout=timeout_s,
        check=False,
        env=env,
    )
    combined_output = (result.stdout or "") + (result.stderr or "")
    if result.returncode != 0:
        raise RuntimeError(
            f"kicad-cli export failed for {root_schematic} with exit "
            f"{result.returncode}\n{combined_output}"
        )

    plotted = [Path(match.group(1)) for match in PLOTTED_RE.finditer(combined_output)]
    plotted = [path for path in plotted if path.exists()]
    if not plotted:
        plotted = sorted(output_dir.glob("*.svg"), key=lambda p: p.stat().st_mtime)
    return plotted, combined_output


@dataclass(frozen=True)
class MonkeySheet:
    index: int
    source_path: Path
    sheet_name: str
    cli_sheet_key: str
    sheet_path: str
    sheet_instance_path: str | None
    sheet_number: int
    svg_path: Path


@dataclass(frozen=True)
class SchematicEntry:
    schematic: Any
    sheet_name: str
    cli_sheet_key: str
    sheet_path: str
    sheet_instance_path: str | None
    sheet_number: int


def _join_sheet_path(parent: str, child: str) -> str:
    parent = parent if parent.endswith("/") else parent + "/"
    return f"{parent}{child.strip('/')}/"


def _sheet_path_cli_key(sheet_path: str, fallback: str) -> str:
    parts = [part for part in sheet_path.strip("/").split("/") if part]
    return "-".join(parts) if parts else fallback


def _sheet_page_number(sheet: Any, parent_instance_path: str | None = None) -> int | None:
    target_path = str(parent_instance_path or "").rstrip("/")
    fallback: int | None = None
    for inst in getattr(sheet, "instances", ()) or ():
        page = str(getattr(inst, "page", "") or "")
        if not page.isdigit():
            continue
        page_number = int(page)
        if fallback is None:
            fallback = page_number
        inst_path = str(getattr(inst, "path", "") or "").rstrip("/")
        if target_path and inst_path == target_path:
            return page_number
    return fallback


def _schematic_source_counts(entries: list["SchematicEntry"]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for entry in entries:
        schematic = entry.schematic
        source = getattr(schematic, "source_path", None)
        key = str(Path(source).resolve() if source else id(schematic))
        counts[key] = counts.get(key, 0) + 1
    return counts


def _schematic_output_stem(
    entry: "SchematicEntry",
    source_path: Path,
    counts: dict[str, int],
) -> str:
    key = str(source_path.resolve()) if source_path else str(id(entry.schematic))
    if counts.get(key, 0) > 1 and entry.sheet_name:
        return entry.sheet_name
    return source_path.stem


def _walk_design_schematics(design: Any) -> list[SchematicEntry]:
    top = design.top_schematic
    if top is None:
        return []
    top_instance_path = f"/{top.uuid}" if getattr(top, "uuid", "") else None
    top_name = (
        Path(top.source_path).stem
        if getattr(top, "source_path", None)
        else str(getattr(top, "uuid", "") or "root")
    )
    entries = [SchematicEntry(top, top_name, top_name, "/", top_instance_path, 1)]

    def walk(parent: Any, parent_path: str, parent_instance_path: str | None) -> None:
        child_sheets = list(enumerate(getattr(parent, "sheets", ()) or ()))

        def sort_key(item: tuple[int, Any]) -> int:
            page = _sheet_page_number(item[1], parent_instance_path)
            return page if page is not None else 1_000_000 + item[0]

        child_sheets.sort(key=sort_key)
        for _ordinal, sheet in child_sheets:
            child = getattr(parent, "sub_schematics", {}).get(sheet.sheet_file)
            if child is None:
                continue
            sheet_name = sheet.sheet_name or Path(sheet.sheet_file).stem
            child_path = _join_sheet_path(parent_path, sheet_name)
            cli_sheet_key = _sheet_path_cli_key(child_path, sheet_name)
            sheet_number = _sheet_page_number(sheet, parent_instance_path) or (
                len(entries) + 1
            )
            child_instance_path = (
                _join_sheet_path(parent_instance_path, sheet.uuid).rstrip("/")
                if parent_instance_path and getattr(sheet, "uuid", "")
                else None
            )
            entries.append(
                SchematicEntry(
                    child,
                    sheet_name,
                    cli_sheet_key,
                    child_path,
                    child_instance_path,
                    sheet_number,
                )
            )
            walk(child, child_path, child_instance_path)

    walk(top, "/", top_instance_path)
    return entries


def _render_monkey_svgs(
    *,
    project_file: Path,
    output_dir: Path,
    max_sheets: int | None,
    options: Any | None = None,
    filename_suffix: str = "monkey",
) -> list[MonkeySheet]:
    from kicad_monkey import KiCadDesign, render_ir_to_svg
    from kicad_monkey.kicad_sch_svg_renderer import KiCadSvgRenderOptions

    _clear_svg_dir(output_dir)
    design = KiCadDesign.from_project_file(project_file)
    schematics = _walk_design_schematics(design)
    if max_sheets is not None:
        schematics = schematics[:max_sheets]

    opts = options or KiCadSvgRenderOptions.kicad_native()
    opts.include_metadata = True
    sheets: list[MonkeySheet] = []
    sheet_count = len(schematics)
    source_counts = _schematic_source_counts(schematics)
    for index, entry in enumerate(schematics, start=1):
        schematic = entry.schematic
        source_path = (
            Path(schematic.source_path)
            if getattr(schematic, "source_path", None)
            else Path(f"sheet_{index}.kicad_sch")
        )
        source_stem = source_path.stem
        sheet_name = entry.sheet_name or source_stem
        document_id = schematic.uuid or f"{source_stem}_{index}"
        doc = design.to_schematic_ir(
            schematic=schematic,
            sheet_index=entry.sheet_number,
            sheet_count=sheet_count,
            sheet_path=entry.sheet_path,
            sheet_instance_path=entry.sheet_instance_path,
            sheet_name=sheet_name,
            document_id=document_id,
        )
        svg = render_ir_to_svg(doc, options=opts)
        output_stem = _schematic_output_stem(entry, source_path, source_counts)
        svg_path = output_dir / f"{index:02d}_{_slug(output_stem)}__{filename_suffix}.svg"
        svg_path.write_text(svg, encoding="utf-8")
        sheets.append(
            MonkeySheet(
                index=index,
                source_path=source_path,
                sheet_name=sheet_name,
                cli_sheet_key=entry.cli_sheet_key,
                sheet_path=entry.sheet_path,
                sheet_instance_path=entry.sheet_instance_path,
                sheet_number=entry.sheet_number,
                svg_path=svg_path,
            )
        )
    return sheets


def _local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


Matrix = tuple[float, float, float, float, float, float]
IDENTITY: Matrix = (1.0, 0.0, 0.0, 1.0, 0.0, 0.0)


def _mat_mul(left: Matrix, right: Matrix) -> Matrix:
    la, lb, lc, ld, le, lf = left
    ra, rb, rc, rd, re, rf = right
    return (
        la * ra + lc * rb,
        lb * ra + ld * rb,
        la * rc + lc * rd,
        lb * rc + ld * rd,
        la * re + lc * rf + le,
        lb * re + ld * rf + lf,
    )


def _apply_matrix(matrix: Matrix, x: float, y: float) -> tuple[float, float]:
    a, b, c, d, e, f = matrix
    return a * x + c * y + e, b * x + d * y + f


def _numbers(value: str) -> list[float]:
    return [float(match.group(0)) for match in NUMBER_RE.finditer(value)]


def _translate(tx: float, ty: float = 0.0) -> Matrix:
    return (1.0, 0.0, 0.0, 1.0, tx, ty)


def _scale(sx: float, sy: float | None = None) -> Matrix:
    return (sx, 0.0, 0.0, sx if sy is None else sy, 0.0, 0.0)


def _rotate(angle_deg: float, cx: float = 0.0, cy: float = 0.0) -> Matrix:
    rad = math.radians(angle_deg)
    c = math.cos(rad)
    s = math.sin(rad)
    rot = (c, s, -s, c, 0.0, 0.0)
    if cx or cy:
        return _mat_mul(_mat_mul(_translate(cx, cy), rot), _translate(-cx, -cy))
    return rot


def _parse_transform(value: str | None) -> Matrix:
    if not value:
        return IDENTITY
    matrix = IDENTITY
    for match in TRANSFORM_RE.finditer(value):
        name = match.group(1).lower()
        nums = _numbers(match.group(2))
        op = IDENTITY
        if name == "matrix" and len(nums) >= 6:
            op = (nums[0], nums[1], nums[2], nums[3], nums[4], nums[5])
        elif name == "translate" and nums:
            op = _translate(nums[0], nums[1] if len(nums) > 1 else 0.0)
        elif name == "scale" and nums:
            op = _scale(nums[0], nums[1] if len(nums) > 1 else None)
        elif name == "rotate" and nums:
            if len(nums) >= 3:
                op = _rotate(nums[0], nums[1], nums[2])
            else:
                op = _rotate(nums[0])
        matrix = _mat_mul(matrix, op)
    return matrix


def _float_attr(elem: ET.Element, name: str, default: float = 0.0) -> float:
    value = elem.attrib.get(name)
    if value is None:
        return default
    nums = _numbers(value)
    return nums[0] if nums else default


def _href_attr(elem: ET.Element) -> str:
    for key, value in elem.attrib.items():
        if key == "href" or key.endswith("}href"):
            return value
    return ""


def _image_intrinsic_px(href: str) -> tuple[int, int] | None:
    if "base64," not in href:
        return None
    payload = href.split("base64,", 1)[1]
    payload = re.sub(r"\s+", "", payload)
    head = payload[:8192]
    if len(head) % 4:
        head += "=" * (4 - len(head) % 4)
    try:
        raw = base64.b64decode(head, validate=False)
    except Exception:
        return None
    if raw.startswith(b"\x89PNG\r\n\x1a\n") and len(raw) >= 24:
        return int.from_bytes(raw[16:20], "big"), int.from_bytes(raw[20:24], "big")
    if raw.startswith(b"BM") and len(raw) >= 26:
        dib_header_size = int.from_bytes(raw[14:18], "little")
        if dib_header_size == 12 and len(raw) >= 26:
            width = int.from_bytes(raw[18:20], "little")
            height = int.from_bytes(raw[20:22], "little")
            return width, height
        if dib_header_size >= 40 and len(raw) >= 26:
            width = int.from_bytes(raw[18:22], "little", signed=True)
            height = int.from_bytes(raw[22:26], "little", signed=True)
            return abs(width), abs(height)
    if raw.startswith(b"\xff\xd8"):
        idx = 2
        sof_markers = {
            0xC0,
            0xC1,
            0xC2,
            0xC3,
            0xC5,
            0xC6,
            0xC7,
            0xC9,
            0xCA,
            0xCB,
            0xCD,
            0xCE,
            0xCF,
        }
        while idx + 9 < len(raw):
            if raw[idx] != 0xFF:
                idx += 1
                continue
            while idx < len(raw) and raw[idx] == 0xFF:
                idx += 1
            if idx >= len(raw):
                break
            marker = raw[idx]
            idx += 1
            if marker in {0xD8, 0xD9}:
                continue
            if idx + 2 > len(raw):
                break
            segment_length = int.from_bytes(raw[idx : idx + 2], "big")
            if segment_length < 2 or idx + segment_length > len(raw):
                break
            if marker in sof_markers and segment_length >= 7:
                height = int.from_bytes(raw[idx + 3 : idx + 5], "big")
                width = int.from_bytes(raw[idx + 5 : idx + 7], "big")
                return width, height
            idx += segment_length
    return None


def _parse_svg_root(svg_path: Path) -> ET.Element:
    text = svg_path.read_text(encoding="utf-8", errors="replace")
    return ET.fromstring(text)


def _extract_text_items(svg_path: Path) -> list[dict[str, Any]]:
    root = _parse_svg_root(svg_path)
    items: list[dict[str, Any]] = []

    def walk(elem: ET.Element, inherited: Matrix) -> None:
        matrix = _mat_mul(inherited, _parse_transform(elem.attrib.get("transform")))
        if _local_name(elem.tag) == "text":
            text = "".join(elem.itertext())
            normalized = " ".join(text.split())
            if normalized:
                x = _float_attr(elem, "x")
                y = _float_attr(elem, "y")
                ax, ay = _apply_matrix(matrix, x, y)
                items.append(
                    {
                        "text": normalized,
                        "x": round(ax, 6),
                        "y": round(ay, 6),
                        "font_size": _float_attr(elem, "font-size", 0.0),
                        "text_length": _float_attr(elem, "textLength", 0.0)
                        if "textLength" in elem.attrib
                        else None,
                        "text_anchor": elem.attrib.get("text-anchor", ""),
                        "transform": elem.attrib.get("transform", ""),
                    }
                )
        for child in elem:
            walk(child, matrix)

    walk(root, IDENTITY)
    return items


def _extract_image_items(svg_path: Path) -> list[dict[str, Any]]:
    root = _parse_svg_root(svg_path)
    items: list[dict[str, Any]] = []

    def walk(elem: ET.Element, inherited: Matrix) -> None:
        matrix = _mat_mul(inherited, _parse_transform(elem.attrib.get("transform")))
        if _local_name(elem.tag) == "image":
            x = _float_attr(elem, "x")
            y = _float_attr(elem, "y")
            width = _float_attr(elem, "width")
            height = _float_attr(elem, "height")
            x1, y1 = _apply_matrix(matrix, x, y)
            x2, y2 = _apply_matrix(matrix, x + width, y + height)
            href = _href_attr(elem)
            intrinsic_px = _image_intrinsic_px(href)
            items.append(
                {
                    "x": round(min(x1, x2), 6),
                    "y": round(min(y1, y2), 6),
                    "width": round(abs(x2 - x1), 6),
                    "height": round(abs(y2 - y1), 6),
                    "href_chars": len(href),
                    "intrinsic_px": intrinsic_px,
                    "preserve_aspect_ratio": elem.attrib.get("preserveAspectRatio", ""),
                }
            )
        for child in elem:
            walk(child, matrix)

    walk(root, IDENTITY)
    return items


def _distance(a: dict[str, Any], b: dict[str, Any]) -> float:
    return math.hypot(float(a["x"]) - float(b["x"]), float(a["y"]) - float(b["y"]))


def _semantic_axis_delta_mm(delta: float) -> float:
    if abs(delta) <= TEXT_SVG_PRECISION_EQUIV_MM:
        return 0.0
    return delta


def _semantic_distance(a: dict[str, Any], b: dict[str, Any]) -> float:
    dx = _semantic_axis_delta_mm(float(b["x"]) - float(a["x"]))
    dy = _semantic_axis_delta_mm(float(b["y"]) - float(a["y"]))
    return math.hypot(dx, dy)


def _semantic_text_length_delta_mm(delta: float) -> float:
    if abs(delta) <= TEXT_LENGTH_SVG_PRECISION_EQUIV_MM + 1e-9:
        return 0.0
    return abs(delta)


def _short_text(value: str, limit: int = 80) -> str:
    if len(value) <= limit:
        return value
    return value[: limit - 1] + "..."


def _text_semantic_key(item: dict[str, Any]) -> tuple[Any, ...]:
    return (
        str(item.get("text", "")),
        round(float(item.get("x") or 0.0), 4),
        round(float(item.get("y") or 0.0), 4),
        round(float(item.get("font_size") or 0.0), 4),
        str(item.get("text_anchor", "")),
        str(item.get("transform", "")),
    )


def _dedupe_semantic_text_overdraw(
    items: list[dict[str, Any]],
) -> tuple[list[dict[str, Any]], int]:
    """Collapse exact opaque duplicate text overdraw.

    KiCad's schematic plotter intentionally overplots symbol text when
    symbols overlap.  A second identical text item at the same transform
    is visually idempotent, so the comparison layer treats it the same
    way the IR equivalence layer treats duplicate opaque overdraw.
    """
    out: list[dict[str, Any]] = []
    seen: set[tuple[Any, ...]] = set()
    duplicate_count = 0
    for item in items:
        key = _text_semantic_key(item)
        if key in seen:
            duplicate_count += 1
            continue
        seen.add(key)
        out.append(item)
    return out, duplicate_count


def _compare_text(reference_svg: Path, generated_svg: Path) -> dict[str, Any]:
    raw_refs = _extract_text_items(reference_svg)
    raw_gens = _extract_text_items(generated_svg)
    refs, ref_duplicate_count = _dedupe_semantic_text_overdraw(raw_refs)
    gens, gen_duplicate_count = _dedupe_semantic_text_overdraw(raw_gens)
    refs_by_text: dict[str, list[tuple[int, dict[str, Any]]]] = {}
    gens_by_text: dict[str, list[tuple[int, dict[str, Any]]]] = {}
    for idx, item in enumerate(refs):
        refs_by_text.setdefault(str(item["text"]), []).append((idx, item))
    for idx, item in enumerate(gens):
        gens_by_text.setdefault(str(item["text"]), []).append((idx, item))

    matches: list[dict[str, Any]] = []
    unmatched_refs: list[dict[str, Any]] = []
    unmatched_gens: list[dict[str, Any]] = []

    for text in sorted(set(refs_by_text) | set(gens_by_text)):
        ref_group = refs_by_text.get(text, [])
        gen_group = gens_by_text.get(text, [])
        if not ref_group:
            unmatched_gens.extend(gen for _idx, gen in gen_group)
            continue
        if not gen_group:
            unmatched_refs.extend(ref for _idx, ref in ref_group)
            continue

        candidate_pairs = sorted(
            (
                (_semantic_distance(ref, gen), ref_idx, gen_idx, ref, gen)
                for ref_idx, ref in ref_group
                for gen_idx, gen in gen_group
                if _distance(ref, gen) <= TEXT_MATCH_MAX_DISTANCE_MM
            ),
            key=lambda item: item[0],
        )
        used_refs: set[int] = set()
        used_gens: set[int] = set()
        for dist, ref_idx, gen_idx, ref, gen in candidate_pairs:
            if ref_idx in used_refs or gen_idx in used_gens:
                continue
            used_refs.add(ref_idx)
            used_gens.add(gen_idx)
            raw_dx = float(gen["x"]) - float(ref["x"])
            raw_dy = float(gen["y"]) - float(ref["y"])
            dx = _semantic_axis_delta_mm(float(gen["x"]) - float(ref["x"]))
            dy = _semantic_axis_delta_mm(float(gen["y"]) - float(ref["y"]))
            ref_len = ref.get("text_length")
            gen_len = gen.get("text_length")
            raw_length_delta = (
                abs(float(ref_len) - float(gen_len))
                if ref_len is not None and gen_len is not None
                else None
            )
            length_delta = (
                _semantic_text_length_delta_mm(raw_length_delta)
                if raw_length_delta is not None
                else None
            )
            matches.append(
                {
                    "text": ref["text"],
                    "dx_mm": round(dx, 6),
                    "dy_mm": round(dy, 6),
                    "distance_mm": round(dist, 6),
                    "raw_dx_mm": round(raw_dx, 6),
                    "raw_dy_mm": round(raw_dy, 6),
                    "raw_distance_mm": round(math.hypot(raw_dx, raw_dy), 6),
                    "font_size_delta_mm": round(
                        float(gen.get("font_size") or 0.0) - float(ref.get("font_size") or 0.0),
                        6,
                    ),
                    "raw_text_length_delta_mm": round(raw_length_delta, 6)
                    if raw_length_delta is not None
                    else None,
                    "text_length_delta_mm": round(length_delta, 6)
                    if length_delta is not None
                    else None,
                    "reference": ref,
                    "generated": gen,
                }
            )
        unmatched_refs.extend(ref for ref_idx, ref in ref_group if ref_idx not in used_refs)
        unmatched_gens.extend(gen for gen_idx, gen in gen_group if gen_idx not in used_gens)

    distances = [float(match["distance_mm"]) for match in matches]
    raw_distances = [float(match["raw_distance_mm"]) for match in matches]
    font_deltas = [abs(float(match["font_size_delta_mm"])) for match in matches]
    length_deltas = [
        float(match["text_length_delta_mm"])
        for match in matches
        if match["text_length_delta_mm"] is not None
    ]
    raw_length_deltas = [
        float(match["raw_text_length_delta_mm"])
        for match in matches
        if match["raw_text_length_delta_mm"] is not None
    ]
    missing_gen_lengths = sum(
        1
        for match in matches
        if match["reference"].get("text_length") is not None
        and match["generated"].get("text_length") is None
    )

    return {
        "reference_count": len(refs),
        "generated_count": len(gens),
        "reference_raw_count": len(raw_refs),
        "generated_raw_count": len(raw_gens),
        "reference_duplicate_count": ref_duplicate_count,
        "generated_duplicate_count": gen_duplicate_count,
        "matched_count": len(matches),
        "reference_only_count": len(unmatched_refs),
        "generated_only_count": len(unmatched_gens),
        "max_distance_mm": round(max(distances), 6) if distances else 0.0,
        "mean_distance_mm": round(sum(distances) / len(distances), 6) if distances else 0.0,
        "max_raw_distance_mm": round(max(raw_distances), 6) if raw_distances else 0.0,
        "max_font_size_delta_mm": round(max(font_deltas), 6) if font_deltas else 0.0,
        "max_text_length_delta_mm": round(max(length_deltas), 6) if length_deltas else None,
        "max_raw_text_length_delta_mm": round(max(raw_length_deltas), 6)
        if raw_length_deltas
        else None,
        "reference_text_length_missing_in_generated": missing_gen_lengths,
        "worst_matches": sorted(
            matches,
            key=lambda item: (float(item["distance_mm"]), float(item["raw_distance_mm"])),
            reverse=True,
        )[:12],
        "reference_only_samples": [
            {
                "text": _short_text(item["text"]),
                "x": item["x"],
                "y": item["y"],
            }
            for item in unmatched_refs[:12]
        ],
        "generated_only_samples": [
            {
                "text": _short_text(item["text"]),
                "x": item["x"],
                "y": item["y"],
            }
            for item in unmatched_gens[:12]
        ],
    }


def _compare_images(reference_svg: Path, generated_svg: Path) -> dict[str, Any]:
    refs = _extract_image_items(reference_svg)
    gens = _extract_image_items(generated_svg)
    candidate_pairs: list[tuple[float, int, int, float, float, dict[str, Any], dict[str, Any]]] = []
    for ref_idx, ref in enumerate(refs):
        for gen_idx, gen in enumerate(gens):
            ref_px = ref.get("intrinsic_px")
            gen_px = gen.get("intrinsic_px")
            if ref_px is not None and gen_px is not None and tuple(ref_px) != tuple(gen_px):
                continue
            pos = _distance(ref, gen)
            size = math.hypot(
                float(ref["width"]) - float(gen["width"]),
                float(ref["height"]) - float(gen["height"]),
            )
            candidate_pairs.append((pos + size, ref_idx, gen_idx, pos, size, ref, gen))
    candidate_pairs.sort(key=lambda item: item[0])

    matches: list[dict[str, Any]] = []
    used_refs: set[int] = set()
    used_gens: set[int] = set()

    for _score, ref_idx, gen_idx, pos_delta, size_delta, ref, gen in candidate_pairs:
        if ref_idx in used_refs or gen_idx in used_gens:
            continue
        used_refs.add(ref_idx)
        used_gens.add(gen_idx)
        matches.append(
            {
                "position_delta_mm": round(pos_delta, 6),
                "size_delta_mm": round(size_delta, 6),
                "dx_mm": round(float(gen["x"]) - float(ref["x"]), 6),
                "dy_mm": round(float(gen["y"]) - float(ref["y"]), 6),
                "dwidth_mm": round(float(gen["width"]) - float(ref["width"]), 6),
                "dheight_mm": round(float(gen["height"]) - float(ref["height"]), 6),
                "reference": ref,
                "generated": gen,
            }
        )

    unmatched_refs = [ref for idx, ref in enumerate(refs) if idx not in used_refs]
    unmatched_gens = [gen for idx, gen in enumerate(gens) if idx not in used_gens]
    pos_deltas = [float(match["position_delta_mm"]) for match in matches]
    size_deltas = [float(match["size_delta_mm"]) for match in matches]
    return {
        "reference_count": len(refs),
        "generated_count": len(gens),
        "matched_count": len(matches),
        "reference_only_count": len(unmatched_refs),
        "generated_only_count": len(unmatched_gens),
        "max_position_delta_mm": round(max(pos_deltas), 6) if pos_deltas else 0.0,
        "max_size_delta_mm": round(max(size_deltas), 6) if size_deltas else 0.0,
        "worst_matches": sorted(
            matches,
            key=lambda item: (float(item["position_delta_mm"]), float(item["size_delta_mm"])),
            reverse=True,
        )[:12],
        "reference_only_samples": unmatched_refs[:12],
        "generated_only_samples": unmatched_gens[:12],
    }


def _element_hist(svg_path: Path) -> dict[str, int]:
    root = _parse_svg_root(svg_path)
    hist: dict[str, int] = {}
    for elem in root.iter():
        tag = _local_name(elem.tag)
        if tag in {"path", "polyline", "polygon", "rect", "circle", "ellipse", "line", "text", "image"}:
            hist[tag] = hist.get(tag, 0) + 1
    return dict(sorted(hist.items()))


def _viewbox(svg_path: Path) -> str | None:
    text = svg_path.read_text(encoding="utf-8", errors="replace")
    match = VIEWBOX_RE.search(text[:8192])
    if not match:
        return None
    return " ".join(match.group(2).split())


def _pair_metrics(reference_svg: Path, generated_svg: Path) -> dict[str, Any]:
    bounds_passed, bounds = compare_svg_bounds(reference_svg, generated_svg, tolerance=0.01)
    return {
        "bounds_passed": bounds_passed,
        "bounds": bounds,
        "viewbox": {
            "reference": _viewbox(reference_svg),
            "generated": _viewbox(generated_svg),
        },
        "elements": {
            "reference": _element_hist(reference_svg),
            "generated": _element_hist(generated_svg),
        },
        "text": _compare_text(reference_svg, generated_svg),
        "images": _compare_images(reference_svg, generated_svg),
    }


def _copy_cli_svg(cli_svg: Path, target: Path) -> Path:
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(cli_svg, target)
    return target


def _cli_sheet_key(cli_svg: Path, *, root_stem: str, sheet_names: set[str]) -> str:
    stem = cli_svg.stem
    if stem == root_stem:
        return root_stem
    prefix = f"{root_stem}-"
    if stem.startswith(prefix):
        stem = stem[len(prefix) :]
    if stem in sheet_names:
        return stem
    stripped = re.sub(r"\d+$", "", stem)
    if stripped in sheet_names:
        return stripped
    return stem


def _pair_cli_svgs(
    *,
    monkey_sheets: list[MonkeySheet],
    cli_svgs: list[Path],
    root_schematic: Path,
) -> tuple[list[tuple[MonkeySheet, Path]], list[Path], list[MonkeySheet]]:
    sheet_names = {sheet.cli_sheet_key for sheet in monkey_sheets}
    by_key: dict[str, list[Path]] = {}
    for cli_svg in cli_svgs:
        key = _cli_sheet_key(cli_svg, root_stem=root_schematic.stem, sheet_names=sheet_names)
        by_key.setdefault(key, []).append(cli_svg)

    used_cli: set[Path] = set()
    pairs: list[tuple[MonkeySheet, Path]] = []
    unpaired_monkey: list[MonkeySheet] = []

    for sheet in monkey_sheets:
        candidates = by_key.get(sheet.cli_sheet_key) or []
        cli_svg: Path | None = None
        while candidates:
            candidate = candidates.pop(0)
            if candidate not in used_cli:
                cli_svg = candidate
                break
        if cli_svg is None:
            for candidate in cli_svgs:
                if candidate not in used_cli:
                    cli_svg = candidate
                    break
        if cli_svg is None:
            unpaired_monkey.append(sheet)
            continue
        used_cli.add(cli_svg)
        pairs.append((sheet, cli_svg))

    unpaired_cli = [path for path in cli_svgs if path not in used_cli]
    return pairs, unpaired_cli, unpaired_monkey


def _case_name(case: dict[str, Any]) -> str:
    return str(case.get("name") or case.get("id") or "unnamed")


def _compare_case(
    *,
    case: dict[str, Any],
    kicad_root: Path,
    kicad_cli: Path,
    assets_dir: Path,
    max_sheets: int | None,
    timeout_s: int,
    preferences_dir: Path | None = None,
    use_cli_theme: bool = False,
    compare_themed: bool = False,
) -> dict[str, Any]:
    case_name = _case_name(case)
    project_file = _resolve_case_path(kicad_root, case, "project_file")
    if project_file is None:
        raise RuntimeError(f"Case has no project_file: {case_name}")
    schematics = case.get("schematics") or []
    project_root_schematic = project_file.with_suffix(".kicad_sch")
    root_schematic = (
        project_root_schematic
        if project_root_schematic.exists()
        else kicad_root / str(schematics[0])
        if schematics
        else project_root_schematic
    )

    case_dir = assets_dir / _slug(case_name)
    cli_stage_dir = case_dir / "_cli_stage"
    cli_config_dir = case_dir / "_cli_config"
    cli_raw_dir = case_dir / "_cli_raw"
    cli_dir = case_dir / "kicad_cli"
    monkey_dir = case_dir / "monkey_ir"
    themed_dir = case_dir / "monkey_wavenumber"
    overlay_dir = case_dir / "overlay"
    for directory in (cli_dir, monkey_dir, themed_dir, overlay_dir):
        _clear_svg_dir(directory)

    preference_theme = None
    cli_config_home: Path | None = None
    if preferences_dir is not None:
        from kicad_monkey.kicad_svg_preferences import load_kicad_svg_preference_theme

        preference_theme = load_kicad_svg_preference_theme(preferences_dir)
        if use_cli_theme:
            cli_config_home = _stage_cli_color_theme(
                preferences_dir=preferences_dir,
                theme_name=preference_theme.name,
                stage_dir=cli_config_dir,
            )

    staged_root_schematic = _stage_project_for_cli(
        project_file=project_file,
        root_schematic=root_schematic,
        stage_dir=cli_stage_dir,
    )
    cli_exported, cli_output = _run_kicad_cli_export(
        kicad_cli=kicad_cli,
        root_schematic=staged_root_schematic,
        output_dir=cli_raw_dir,
        timeout_s=timeout_s,
        theme_name=preference_theme.name if use_cli_theme and preference_theme is not None else None,
        config_home=cli_config_home,
    )
    monkey_sheets = _render_monkey_svgs(
        project_file=project_file,
        output_dir=monkey_dir,
        max_sheets=max_sheets,
    )
    themed_by_index: dict[int, Path] = {}
    if preferences_dir is not None:
        from kicad_monkey.kicad_sch_svg_renderer import KiCadSvgRenderOptions
        from kicad_monkey.kicad_svg_preferences import schematic_svg_options_from_preferences

        themed_options = KiCadSvgRenderOptions.kicad_native()
        themed_options = schematic_svg_options_from_preferences(
            preferences_dir,
            base=themed_options,
        )
        themed_sheets = _render_monkey_svgs(
            project_file=project_file,
            output_dir=themed_dir,
            max_sheets=max_sheets,
            options=themed_options,
            filename_suffix="wavenumber",
        )
        themed_by_index = {sheet.index: sheet.svg_path for sheet in themed_sheets}
    if max_sheets is not None:
        cli_exported = cli_exported[:max_sheets]

    sheet_cli_pairs, unpaired_cli, unpaired_monkey = _pair_cli_svgs(
        monkey_sheets=monkey_sheets,
        cli_svgs=cli_exported,
        root_schematic=root_schematic,
    )

    pairs: list[dict[str, Any]] = []
    for sheet, cli_svg in sheet_cli_pairs:
        stem = f"{sheet.index:02d}_{_slug(sheet.source_path.stem)}"
        reference_svg = _copy_cli_svg(cli_svg, cli_dir / f"{stem}__kicad.svg")
        generated_svg = sheet.svg_path
        comparison_svg = (
            themed_by_index.get(sheet.index, generated_svg)
            if compare_themed
            else generated_svg
        )
        overlay_svg = overlay_dir / f"{stem}__overlay.svg"
        create_overlay_diff(reference_svg, comparison_svg, overlay_svg, opacity=0.38)
        metrics = _pair_metrics(reference_svg, comparison_svg)
        pairs.append(
            {
                "index": sheet.index,
                "sheet_name": sheet.sheet_name,
                "source_path": str(sheet.source_path),
                "kicad_cli_svg": str(reference_svg),
                "monkey_svg": str(generated_svg),
                "comparison_svg": str(comparison_svg),
                "themed_svg": str(themed_by_index[sheet.index])
                if sheet.index in themed_by_index
                else None,
                "overlay_svg": str(overlay_svg),
                "kicad_cli_bytes": reference_svg.stat().st_size,
                "monkey_bytes": generated_svg.stat().st_size,
                "comparison_bytes": comparison_svg.stat().st_size,
                "overlay_bytes": overlay_svg.stat().st_size,
                "metrics": metrics,
            }
        )

    summary = {
        "case": {
            "name": case_name,
            "id": case.get("id"),
            "project_file": str(project_file),
            "root_schematic": str(root_schematic),
            "cli_stage_root_schematic": str(staged_root_schematic),
        },
        "cli_svg_count": len(cli_exported),
        "monkey_svg_count": len(monkey_sheets),
        "paired_count": len(pairs),
        "unpaired_cli_svgs": [str(path) for path in unpaired_cli],
        "unpaired_monkey_svgs": [str(sheet.svg_path) for sheet in unpaired_monkey],
        "cli_output_tail": "\n".join(cli_output.splitlines()[-20:]),
        "pairs": pairs,
    }
    (case_dir / "summary.json").write_text(json.dumps(summary, indent=2) + "\n", encoding="utf-8")
    return summary


def _aggregate(summaries: list[dict[str, Any]]) -> dict[str, Any]:
    pair_count = sum(len(summary["pairs"]) for summary in summaries)
    max_text = 0.0
    max_raw_text = 0.0
    max_text_length_delta = 0.0
    max_raw_text_length_delta = 0.0
    text_length_missing = 0
    max_image_pos = 0.0
    max_image_size = 0.0
    text_ref_only = 0
    text_gen_only = 0
    image_ref_only = 0
    image_gen_only = 0
    bounds_failures = 0
    for summary in summaries:
        for pair in summary["pairs"]:
            metrics = pair["metrics"]
            text = metrics["text"]
            images = metrics["images"]
            max_text = max(max_text, float(text["max_distance_mm"]))
            max_raw_text = max(max_raw_text, float(text.get("max_raw_distance_mm", 0.0)))
            if text.get("max_text_length_delta_mm") is not None:
                max_text_length_delta = max(
                    max_text_length_delta,
                    float(text["max_text_length_delta_mm"]),
                )
            if text.get("max_raw_text_length_delta_mm") is not None:
                max_raw_text_length_delta = max(
                    max_raw_text_length_delta,
                    float(text["max_raw_text_length_delta_mm"]),
                )
            text_length_missing += int(text.get("reference_text_length_missing_in_generated", 0))
            text_ref_only += int(text["reference_only_count"])
            text_gen_only += int(text["generated_only_count"])
            max_image_pos = max(max_image_pos, float(images["max_position_delta_mm"]))
            max_image_size = max(max_image_size, float(images["max_size_delta_mm"]))
            image_ref_only += int(images["reference_only_count"])
            image_gen_only += int(images["generated_only_count"])
            if not metrics["bounds_passed"]:
                bounds_failures += 1
    return {
        "case_count": len(summaries),
        "pair_count": pair_count,
        "max_text_distance_mm": round(max_text, 6),
        "max_raw_text_distance_mm": round(max_raw_text, 6),
        "max_text_length_delta_mm": round(max_text_length_delta, 6),
        "max_raw_text_length_delta_mm": round(max_raw_text_length_delta, 6),
        "reference_text_length_missing_in_generated": text_length_missing,
        "text_reference_only_total": text_ref_only,
        "text_generated_only_total": text_gen_only,
        "max_image_position_delta_mm": round(max_image_pos, 6),
        "max_image_size_delta_mm": round(max_image_size, 6),
        "image_reference_only_total": image_ref_only,
        "image_generated_only_total": image_gen_only,
        "bounds_failure_count": bounds_failures,
    }


def _metric_cell(pair: dict[str, Any]) -> str:
    metrics = pair["metrics"]
    text = metrics["text"]
    images = metrics["images"]
    return (
        f"text max {_html(text['max_distance_mm'])} mm, "
        f"raw {_html(text.get('max_raw_distance_mm', 0.0))} mm, "
        f"textLength {_html(text.get('max_text_length_delta_mm'))} mm, "
        f"raw textLength {_html(text.get('max_raw_text_length_delta_mm'))} mm, "
        f"text ref/gen only {_html(text['reference_only_count'])}/"
        f"{_html(text['generated_only_count'])}, "
        f"image pos/size max {_html(images['max_position_delta_mm'])}/"
        f"{_html(images['max_size_delta_mm'])} mm"
    )


def _render_worst_text(metrics: dict[str, Any]) -> str:
    rows = []
    for match in metrics["text"]["worst_matches"][:6]:
        rows.append(
            "<tr>"
            f"<td>{_html(_short_text(match['text'], 48))}</td>"
            f"<td>{_html(match['dx_mm'])}</td>"
            f"<td>{_html(match['dy_mm'])}</td>"
            f"<td>{_html(match['distance_mm'])}</td>"
            f"<td>{_html(match.get('raw_distance_mm', 0.0))}</td>"
            f"<td>{_html(match['font_size_delta_mm'])}</td>"
            "</tr>"
        )
    if not rows:
        return ""
    return (
        "<table class=\"mini\"><thead><tr><th>Text</th><th>dx</th><th>dy</th>"
        "<th>dist</th><th>raw dist</th><th>font d</th></tr></thead><tbody>"
        f"{''.join(rows)}</tbody></table>"
    )


def _render_worst_images(metrics: dict[str, Any]) -> str:
    rows = []
    for match in metrics["images"]["worst_matches"][:6]:
        rows.append(
            "<tr>"
            f"<td>{_html(match['dx_mm'])}</td>"
            f"<td>{_html(match['dy_mm'])}</td>"
            f"<td>{_html(match['dwidth_mm'])}</td>"
            f"<td>{_html(match['dheight_mm'])}</td>"
            f"<td>{_html(match['position_delta_mm'])}</td>"
            f"<td>{_html(match['size_delta_mm'])}</td>"
            "</tr>"
        )
    if not rows:
        return ""
    return (
        "<table class=\"mini\"><thead><tr><th>dx</th><th>dy</th><th>dw</th>"
        "<th>dh</th><th>pos</th><th>size</th></tr></thead><tbody>"
        f"{''.join(rows)}</tbody></table>"
    )


def _panzoom_figure(title: str, href: str) -> str:
    return (
        "<figure>"
        "<figcaption>"
        f"<span>{_html(title)}</span>"
        "<span class=\"panzoom-actions\">"
        "<button type=\"button\" data-panzoom-action=\"out\" title=\"Zoom out\">-</button>"
        "<button type=\"button\" data-panzoom-action=\"reset\" title=\"Reset pan and zoom\">1:1</button>"
        "<button type=\"button\" data-panzoom-action=\"in\" title=\"Zoom in\">+</button>"
        "</span>"
        "</figcaption>"
        f"<div class=\"panzoom-frame\" data-svg-src=\"{_html(href)}\">"
        f"<object data-src=\"{_html(href)}\" type=\"image/svg+xml\" aria-label=\"{_html(title)}\"></object>"
        "<div class=\"panzoom-status\">Loading SVG...</div>"
        "</div>"
        "</figure>"
    )


def _render_html(
    *,
    summaries: list[dict[str, Any]],
    aggregate: dict[str, Any],
    kicad_root: Path,
    kicad_cli: Path,
    review_dir: Path,
) -> str:
    generated = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%SZ")
    rows = []
    options = []
    cards = []
    first_card = True
    for summary in summaries:
        case = summary["case"]
        worst_text = 0.0
        worst_image_pos = 0.0
        worst_image_size = 0.0
        for pair in summary["pairs"]:
            metrics = pair["metrics"]
            worst_text = max(worst_text, float(metrics["text"]["max_distance_mm"]))
            worst_image_pos = max(worst_image_pos, float(metrics["images"]["max_position_delta_mm"]))
            worst_image_size = max(worst_image_size, float(metrics["images"]["max_size_delta_mm"]))
        rows.append(
            "<tr>"
            f"<td><a href=\"#{_html(_slug(case['name']))}\">{_html(case['name'])}</a></td>"
            f"<td>{_html(summary['paired_count'])}</td>"
            f"<td>{_html(worst_text)}</td>"
            f"<td>{_html(worst_image_pos)}</td>"
            f"<td>{_html(worst_image_size)}</td>"
            f"<td>{_html(summary['cli_svg_count'])}/{_html(summary['monkey_svg_count'])}</td>"
            "</tr>"
        )

        for pair in summary["pairs"]:
            key = _slug(f"{case['name']}__{pair['index']}__{pair['sheet_name']}")
            label = f"{case['name']} :: {pair['index']}. {pair['sheet_name']}"
            options.append(f"<option value=\"{_html(key)}\">{_html(label)}</option>")
            ref_href = _rel_href(Path(pair["kicad_cli_svg"]), from_dir=review_dir)
            gen_href = _rel_href(Path(pair["monkey_svg"]), from_dir=review_dir)
            themed_href = (
                _rel_href(Path(pair["themed_svg"]), from_dir=review_dir)
                if pair.get("themed_svg")
                else None
            )
            overlay_href = _rel_href(Path(pair["overlay_svg"]), from_dir=review_dir)
            metrics = pair["metrics"]
            hidden_attr = "" if first_card else " hidden"
            first_card = False
            cards.append(
                f"<section class=\"sheet-card\" data-sheet-card=\"{_html(key)}\"{hidden_attr}>"
            )
            cards.append(f"<h2>{_html(label)}</h2>")
            cards.append(
                f"<p class=\"meta\">{_html(_metric_cell(pair))}. "
                f"Project: <code>{_html(case['project_file'])}</code></p>"
            )
            cards.append(
                "<div class=\"links\">"
                f"<a href=\"{_html(ref_href)}\" target=\"_blank\" rel=\"noreferrer\">KiCad CLI SVG</a>"
                f"<a href=\"{_html(gen_href)}\" target=\"_blank\" rel=\"noreferrer\">Monkey SVG</a>"
                + (
                    f"<a href=\"{_html(themed_href)}\" target=\"_blank\" rel=\"noreferrer\">Wavenumber SVG</a>"
                    if themed_href
                    else ""
                )
                + (
                    f"<a href=\"{_html(overlay_href)}\" target=\"_blank\" rel=\"noreferrer\">Overlay SVG</a>"
                    f"<span>viewBox ref/gen: {_html(metrics['viewbox']['reference'])} / "
                    f"{_html(metrics['viewbox']['generated'])}</span>"
                    "</div>"
                )
            )
            cards.append("<div class=\"viewers\">")
            for title, href in (
                ("KiCad CLI", ref_href),
                ("Monkey IR SVG", gen_href),
                *((("Wavenumber Theme", themed_href),) if themed_href else ()),
                ("Overlay", overlay_href),
            ):
                cards.append(_panzoom_figure(title, href))
            cards.append("</div>")
            cards.append("<details class=\"delta-details\"><summary>Delta details</summary>")
            cards.append(
                f"<div class=\"metric\">Elements ref/gen: "
                f"{_html(metrics['elements']['reference'])} / {_html(metrics['elements']['generated'])}</div>"
            )
            cards.append(_render_worst_text(metrics))
            cards.append(_render_worst_images(metrics))
            cards.append("</details></section>")

    selector_js = """
(() => {
  window.addEventListener('DOMContentLoaded', () => {
    const select = document.getElementById('sheet-select');
    const cards = Array.from(document.querySelectorAll('[data-sheet-card]'));
    const storageKey = 'kicad-cli-svg-review-sheet';
    function optionExists(value) {
      return Array.from(select.options).some((option) => option.value === value);
    }
    const stored = window.localStorage ? window.localStorage.getItem(storageKey) : null;
    if (stored && optionExists(stored)) {
      select.value = stored;
    }
    function showSelected() {
      const value = select.value;
      cards.forEach((card) => { card.hidden = card.dataset.sheetCard !== value; });
      if (window.localStorage) {
        window.localStorage.setItem(storageKey, value);
      }
      if (window.kicadReviewLoadVisibleSvgs) {
        window.kicadReviewLoadVisibleSvgs();
      }
    }
    select.addEventListener('change', showSelected);
    showSelected();
  });
})();
"""

    body = [
        "<!doctype html>",
        '<html lang="en">',
        "<head>",
        '<meta charset="utf-8">',
        "<title>KiCad CLI SVG Comparison</title>",
        "<style>",
        _PANZOOM_CSS,
        "</style>",
        "</head>",
        "<body>",
        "<header>",
        "<h1>KiCad CLI SVG Comparison</h1>",
        (
            f"<p class=\"meta\">Generated {_html(generated)} from <code>{_html(kicad_root)}</code>. "
            f"KiCad CLI: <code>{_html(kicad_cli)}</code>. "
            f"{_html(aggregate['case_count'])} cases, {_html(aggregate['pair_count'])} paired sheets. "
            f"Semantic text max {_html(aggregate['max_text_distance_mm'])} mm; "
            f"raw text max {_html(aggregate['max_raw_text_distance_mm'])} mm; "
            f"textLength max {_html(aggregate['max_text_length_delta_mm'])} mm; "
            f"raw textLength max {_html(aggregate.get('max_raw_text_length_delta_mm', 0.0))} mm. "
            f"Comparison target: {_html(aggregate.get('comparison_target', 'Monkey IR SVG'))}.</p>"
        ),
        "<div class=\"review-controls\">",
        "<label for=\"sheet-select\">Schematic</label>",
        f"<select id=\"sheet-select\">{''.join(options)}</select>",
        "</div>",
        "</header>",
        "<main>",
        "<details class=\"review-index\"><summary>Case index</summary>",
        "<table><thead><tr><th>Case</th><th>Pairs</th><th>Worst text dist mm</th>"
        "<th>Worst image pos mm</th><th>Worst image size mm</th><th>CLI/monkey count</th>"
        f"</tr></thead><tbody>{''.join(rows)}</tbody></table>",
        "</details>",
        *cards,
    ]

    body.extend(
        [
            "</main>",
            _inline_script_block(_svg_pan_zoom_script()),
            _inline_script_block(f"{_PANZOOM_JS}\n{selector_js}"),
            "</body>",
            "</html>",
        ]
    )
    return "\n".join(body) + "\n"


def generate_comparison(
    *,
    kicad_root: Path,
    output_path: Path | None = None,
    cases: list[str] | None = None,
    max_cases: int | None = None,
    max_sheets_per_case: int | None = None,
    timeout_s: int = 240,
    preferences_dir: Path | None = None,
    use_cli_theme: bool = False,
    compare_themed: bool = False,
) -> Path:
    kicad_root = kicad_root.resolve()
    default_review_dir = REPO_ROOT / "tests" / "L3_rendering" / "output" / "cli_svg"
    review_dir = (output_path.parent if output_path else default_review_dir).resolve()
    output_path = output_path or (review_dir / "cli_svg_compare.html")
    assets_dir = review_dir / "cli_svg_compare"
    review_dir.mkdir(parents=True, exist_ok=True)
    assets_dir.mkdir(parents=True, exist_ok=True)

    kicad_cli = resolve_kicad_cli()
    if kicad_cli is None:
        raise RuntimeError("Unable to resolve kicad-cli for SVG comparison")

    all_cases = _iter_real_world_schematic_cases(kicad_root)
    selected = _select_cases(
        all_cases,
        names=set(cases) if cases else None,
        max_cases=max_cases,
    )
    if not selected:
        raise RuntimeError("No matching active real-world schematic cases found")

    summaries = [
        _compare_case(
            case=case,
            kicad_root=kicad_root,
            kicad_cli=kicad_cli,
            assets_dir=assets_dir,
            max_sheets=max_sheets_per_case,
            timeout_s=timeout_s,
            preferences_dir=preferences_dir,
            use_cli_theme=use_cli_theme,
            compare_themed=compare_themed,
        )
        for case in selected
    ]
    aggregate = _aggregate(summaries)
    if use_cli_theme and compare_themed:
        aggregate["comparison_target"] = "KiCad CLI --theme vs kicad_monkey Wavenumber Theme"
    elif compare_themed:
        aggregate["comparison_target"] = "KiCad CLI vs kicad_monkey Wavenumber Theme"
    elif use_cli_theme:
        aggregate["comparison_target"] = "KiCad CLI --theme vs kicad_monkey IR SVG"
    else:
        aggregate["comparison_target"] = "KiCad CLI vs kicad_monkey IR SVG"
    payload = {
        "generated_utc": datetime.now(timezone.utc).isoformat(),
        "kicad_root": str(kicad_root),
        "kicad_cli": str(kicad_cli),
        "preferences_dir": str(preferences_dir) if preferences_dir else None,
        "use_cli_theme": use_cli_theme,
        "compare_themed": compare_themed,
        "aggregate": aggregate,
        "cases": summaries,
    }
    (review_dir / "cli_svg_compare.json").write_text(
        json.dumps(payload, indent=2) + "\n",
        encoding="utf-8",
    )
    output_path.write_text(
        _render_html(
            summaries=summaries,
            aggregate=aggregate,
            kicad_root=kicad_root,
            kicad_cli=kicad_cli,
            review_dir=review_dir,
        ),
        encoding="utf-8",
    )
    return output_path


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--kicad-root",
        type=Path,
        default=None,
        help="KiCad corpus root. Defaults to the resolved KM_CORPUS archive.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=None,
        help="HTML output path. Defaults to tests/L3_rendering/output/cli_svg/.",
    )
    parser.add_argument(
        "--case",
        action="append",
        dest="cases",
        default=None,
        help="Case name or id to include. May be repeated. Defaults to all active real-world schematic cases.",
    )
    parser.add_argument("--max-cases", type=int, default=None)
    parser.add_argument("--max-sheets-per-case", type=int, default=None)
    parser.add_argument("--timeout-s", type=int, default=240)
    parser.add_argument(
        "--preferences",
        type=Path,
        default=None,
        help="Optional KiCad preferences directory to render an extra themed monkey SVG viewer.",
    )
    parser.add_argument(
        "--use-cli-theme",
        action="store_true",
        help="Pass the selected preference color theme to kicad-cli with --theme.",
    )
    parser.add_argument(
        "--compare-themed",
        action="store_true",
        help="Compare KiCad output against the themed monkey SVG instead of the default monkey SVG.",
    )
    args = parser.parse_args()

    _require_text_metric_dependencies()
    kicad_root = args.kicad_root or _default_kicad_root()
    output_path = generate_comparison(
        kicad_root=kicad_root,
        output_path=args.output,
        cases=args.cases,
        max_cases=args.max_cases,
        max_sheets_per_case=args.max_sheets_per_case,
        timeout_s=args.timeout_s,
        preferences_dir=args.preferences,
        use_cli_theme=args.use_cli_theme,
        compare_themed=args.compare_themed,
    )
    print(output_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
