"""Generate a manifest-driven KiCad SVG review page.

The page links generated project SVGs from the resolved ``KM_CORPUS`` and uses
``svg-pan-zoom`` for interactive inspection of selected previews.
"""

from __future__ import annotations

import argparse
import html
import json
import os
import re
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

from corpus_output_paths import resolve_test_corpus_output_path
from kicad_monkey.testing.corpus import get_kicad_corpus_root

PAN_ZOOM_SCRIPT = "https://cdn.jsdelivr.net/npm/svg-pan-zoom@3.6.1/dist/svg-pan-zoom.min.js"
VIEWBOX_RE = re.compile(r"<svg\b[^>]*\bviewBox=(['\"])(.*?)\1", re.IGNORECASE | re.DOTALL)


@dataclass(frozen=True)
class SvgInfo:
    path: Path
    href: str
    size_bytes: int
    view_box: str | None

    @property
    def name(self) -> str:
        return self.path.name


@dataclass(frozen=True)
class ProjectReview:
    case: dict
    schematic_svgs: list[SvgInfo]
    board_svgs: list[SvgInfo]

    @property
    def name(self) -> str:
        return str(self.case.get("name") or self.case.get("id") or "unnamed")


def _default_kicad_root() -> Path:
    return get_kicad_corpus_root()


def _html(value: object) -> str:
    return html.escape(str(value), quote=True)


def _rel_href(path: Path, *, from_dir: Path) -> str:
    return os.path.relpath(path, start=from_dir).replace(os.sep, "/")


def _read_view_box(path: Path) -> str | None:
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None
    match = VIEWBOX_RE.search(text[:8192])
    if not match:
        return None
    return " ".join(match.group(2).split())


def _svg_info(path: Path, *, review_dir: Path) -> SvgInfo:
    return SvgInfo(
        path=path,
        href=_rel_href(path, from_dir=review_dir),
        size_bytes=path.stat().st_size,
        view_box=_read_view_box(path),
    )


def _svg_meta(info: SvgInfo) -> str:
    parts = [f"{info.size_bytes:,} bytes"]
    if info.view_box:
        parts.append(f"viewBox {info.view_box}")
    return ", ".join(parts)


def _discover_projects(kicad_root: Path, *, review_dir: Path) -> list[ProjectReview]:
    manifest_path = kicad_root / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8-sig"))
    projects: list[ProjectReview] = []

    for case in manifest.get("cases") or []:
        if not isinstance(case, dict):
            continue
        if case.get("status") != "active":
            continue
        if case.get("origin") != "real_world":
            continue
        domains = set(case.get("domains") or [])
        if not {"schematic_svg", "board_svg"} & domains:
            continue

        output_root_value = case.get("output_root")
        if not output_root_value:
            continue
        output_root = resolve_test_corpus_output_path(case)
        assert output_root is not None
        schematic_svgs = [
            _svg_info(path, review_dir=review_dir)
            for path in sorted((output_root / "schematic_svg").glob("*.svg"))
        ]
        board_svgs = [
            _svg_info(path, review_dir=review_dir)
            for path in sorted((output_root / "board_svg").glob("*.svg"))
        ]
        if schematic_svgs or board_svgs:
            projects.append(ProjectReview(case=case, schematic_svgs=schematic_svgs, board_svgs=board_svgs))

    def sort_key(project: ProjectReview) -> tuple[int, str]:
        provenance = project.case.get("provenance") or {}
        is_internal = provenance.get("source_kind") == "internal_project_copy"
        return (0 if is_internal else 1, project.name.lower())

    return sorted(projects, key=sort_key)


def _selected_previews(project: ProjectReview) -> list[SvgInfo]:
    previews: list[SvgInfo] = []
    if project.schematic_svgs:
        previews.append(project.schematic_svgs[0])
        midpoint = len(project.schematic_svgs) // 2
        if midpoint != 0:
            previews.append(project.schematic_svgs[midpoint])
    if project.board_svgs:
        previews.append(project.board_svgs[0])
    return previews


def _policy_text(case: dict) -> str:
    policy = case.get("oracle_policy") or {}
    if not isinstance(policy, dict) or not policy:
        return ""
    return ", ".join(f"{key}={value}" for key, value in sorted(policy.items()))


def _render_svg_list(title: str, svgs: list[SvgInfo], *, open_by_default: bool = False) -> str:
    open_attr = " open" if open_by_default else ""
    rows = []
    for info in svgs:
        rows.append(
            f'<li><a href="{_html(info.href)}">{_html(info.name)}</a> '
            f'<span>{_html(_svg_meta(info))}</span></li>'
        )
    return f"<details{open_attr}><summary>{_html(title)}</summary><ul>{''.join(rows)}</ul></details>"


def _render_preview(info: SvgInfo, preview_id: str) -> str:
    href = _html(info.href)
    name = _html(info.name)
    meta = _html(_svg_meta(info))
    return (
        "<figure>"
        '<div class="viewer-toolbar">'
        f'<a href="{href}" target="_blank" rel="noreferrer">open svg</a>'
        f'<button type="button" data-action="reset" data-target="{_html(preview_id)}">reset</button>'
        '<span class="zoom-note">mouse wheel or +/- to zoom, drag to pan</span>'
        "</div>"
        f'<figcaption><a href="{href}">{name}</a><br><span>{meta}</span></figcaption>'
        f'<object id="{_html(preview_id)}" class="svg-preview" data="{href}" type="image/svg+xml"></object>'
        "</figure>"
    )


def _render_project(project: ProjectReview, *, preview_start: int) -> tuple[str, int]:
    case = project.case
    case_id = _html(case.get("id", ""))
    source = _html(case.get("provenance", {}).get("source_kind") or case.get("origin", ""))
    policy = _policy_text(case)
    policy_html = f" &nbsp; <b>policy:</b> {_html(policy)}" if policy else ""
    pieces = [
        f'<section class="project"><h2>{_html(project.name)}</h2>',
        (
            f"<p><b>ID:</b> {case_id} &nbsp; <b>source:</b> {source}"
            f"{policy_html}</p>"
        ),
        '<div class="previews">',
    ]

    preview_id = preview_start
    for info in _selected_previews(project):
        pieces.append(_render_preview(info, f"preview-{preview_id}"))
        preview_id += 1

    pieces.append("</div>")
    if project.schematic_svgs:
        pieces.append(_render_svg_list("Schematic SVG outputs", project.schematic_svgs, open_by_default=True))
    if project.board_svgs:
        pieces.append(_render_svg_list("Board SVG outputs", project.board_svgs))
    pieces.append("</section>")
    return "".join(pieces), preview_id


def _render_table(projects: list[ProjectReview], *, review_dir: Path, kicad_root: Path) -> str:
    rows = []
    for project in projects:
        case = project.case
        source = case.get("provenance", {}).get("source_kind") or case.get("origin", "")
        output_root_value = case.get("output_root")
        output_href = ""
        if output_root_value:
            output_root = resolve_test_corpus_output_path(case)
            assert output_root is not None
            output_href = _rel_href(output_root, from_dir=review_dir)
        rows.append(
            "<tr>"
            f"<td>{_html(project.name)}</td>"
            f"<td>{_html(source)}</td>"
            f"<td>{len(case.get('schematics') or [])}</td>"
            f"<td>{len(project.schematic_svgs)}</td>"
            f"<td>{len(project.board_svgs)}</td>"
            f'<td><a href="{_html(output_href)}">output</a></td>'
            "</tr>"
        )
    return (
        "<table>"
        "<thead><tr><th>Project</th><th>Source</th><th>Manifest sheets</th>"
        "<th>Schematic SVGs</th><th>Board SVGs</th><th>Output</th></tr></thead>"
        f"<tbody>{''.join(rows)}</tbody>"
        "</table>"
    )


def _render_page(projects: list[ProjectReview], *, kicad_root: Path, review_dir: Path) -> str:
    generated = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%SZ")
    total_schematic = sum(len(project.schematic_svgs) for project in projects)
    total_board = sum(len(project.board_svgs) for project in projects)
    total_sheets = sum(len(project.case.get("schematics") or []) for project in projects)

    body_parts = [
        "<!doctype html>",
        '<html lang="en">',
        "<head>",
        '<meta charset="utf-8">',
        "<title>KiCad Monkey SVG Review</title>",
        f'<script src="{_html(PAN_ZOOM_SCRIPT)}"></script>',
        "<style>",
        "body { font-family: Segoe UI, Arial, sans-serif; margin: 24px; color: #1f2933; background: #f7f8fa; }",
        "h1 { margin-bottom: 4px; }",
        ".meta { color: #52606d; margin-top: 0; }",
        "table { border-collapse: collapse; width: 100%; background: white; margin: 18px 0 28px; }",
        "th, td { border: 1px solid #d9e2ec; padding: 6px 8px; text-align: left; }",
        "th { background: #e4edf7; }",
        ".project { background: white; border: 1px solid #d9e2ec; border-radius: 6px; padding: 16px; margin: 18px 0; }",
        ".project h2 { margin-top: 0; }",
        ".previews { display: grid; grid-template-columns: repeat(auto-fit, minmax(360px, 1fr)); gap: 12px; }",
        "figure { margin: 0; border: 1px solid #d9e2ec; background: #fff; min-width: 0; }",
        "figcaption { font-size: 12px; padding: 6px 8px; border-bottom: 1px solid #d9e2ec; color: #334e68; }",
        ".viewer-toolbar { display: flex; align-items: center; gap: 8px; padding: 6px 8px; background: #f0f4f8; border-bottom: 1px solid #d9e2ec; font-size: 12px; }",
        ".viewer-toolbar button { border: 1px solid #bcccdc; background: white; color: #243b53; border-radius: 4px; padding: 2px 8px; cursor: pointer; }",
        ".zoom-note { color: #66788a; }",
        "object.svg-preview { width: 100%; height: 460px; background: white; display: block; }",
        "span { color: #66788a; font-size: 12px; }",
        "ul { columns: 2; }",
        "li { break-inside: avoid; margin: 3px 0; }",
        "</style>",
        "</head>",
        "<body>",
        "<h1>KiCad Monkey SVG Review</h1>",
        (
            f'<p class="meta">Generated {_html(generated)} from manifest-driven outputs under '
            f"<code>{_html(kicad_root)}</code>. "
            f"{len(projects)} projects, {total_sheets} manifest sheets, "
            f"{total_schematic} schematic SVGs, {total_board} board SVGs. "
            "Previews use <code>svg-pan-zoom</code>; direct SVG links remain available if the CDN is unavailable.</p>"
        ),
        _render_table(projects, review_dir=review_dir, kicad_root=kicad_root),
    ]

    preview_id = 1
    for project in projects:
        section, preview_id = _render_project(project, preview_start=preview_id)
        body_parts.append(section)

    body_parts.extend(
        [
            "<script>",
            "(function () {",
            "  var panZoomOptions = {",
            "    zoomEnabled: true,",
            "    controlIconsEnabled: true,",
            "    fit: true,",
            "    center: true,",
            "    minZoom: 0.02,",
            "    maxZoom: 80,",
            "    mouseWheelZoomEnabled: true,",
            "    dblClickZoomEnabled: true",
            "  };",
            "",
            "  function createPanZoom(target) {",
            "    return svgPanZoom(target, Object.assign({}, panZoomOptions));",
            "  }",
            "",
            "  function installPanZoom(objectEl) {",
            "    var init = function () {",
            "      if (!window.svgPanZoom) return;",
            "      try {",
            "        objectEl.__panZoom = createPanZoom(objectEl);",
            "      } catch (firstError) {",
            "        try {",
            "          var svg = objectEl.contentDocument && objectEl.contentDocument.querySelector('svg');",
            "          if (!svg) return;",
            "          svg.removeAttribute('width');",
            "          svg.removeAttribute('height');",
            "          objectEl.__panZoom = createPanZoom(svg);",
            "        } catch (secondError) {",
            "          console.warn('svg-pan-zoom init failed for', objectEl.id, firstError, secondError);",
            "        }",
            "      }",
            "    };",
            "    objectEl.addEventListener('load', init);",
            "    if (objectEl.contentDocument) init();",
            "  }",
            "",
            "  document.querySelectorAll('object.svg-preview').forEach(installPanZoom);",
            "  document.querySelectorAll('[data-action=\"reset\"]').forEach(function (button) {",
            "    button.addEventListener('click', function () {",
            "      var target = document.getElementById(button.getAttribute('data-target'));",
            "      if (target && target.__panZoom) {",
            "        target.__panZoom.resetZoom();",
            "        target.__panZoom.center();",
            "        target.__panZoom.fit();",
            "      }",
            "    });",
            "  });",
            "  window.addEventListener('resize', function () {",
            "    document.querySelectorAll('object.svg-preview').forEach(function (objectEl) {",
            "      if (objectEl.__panZoom) {",
            "        objectEl.__panZoom.resize();",
            "        objectEl.__panZoom.fit();",
            "      }",
            "    });",
            "  });",
            "}());",
            "</script>",
            "</body>",
            "</html>",
        ]
    )
    return "\n".join(body_parts) + "\n"


def generate_review(kicad_root: Path, output_path: Path | None = None) -> Path:
    kicad_root = kicad_root.resolve()
    default_review_dir = (
        Path(__file__).parent / "L3_rendering" / "output" / "manifest_svg"
    )
    review_dir = (output_path.parent if output_path else default_review_dir).resolve()
    output_path = output_path or (review_dir / "svg_review.html")
    review_dir.mkdir(parents=True, exist_ok=True)

    projects = _discover_projects(kicad_root, review_dir=review_dir)
    if not projects:
        raise RuntimeError(f"No active real-world project SVG outputs found under {kicad_root}")

    output_path.write_text(
        _render_page(projects, kicad_root=kicad_root, review_dir=review_dir),
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
        help="HTML output path. Defaults to tests/L3_rendering/output/manifest_svg/.",
    )
    args = parser.parse_args()

    output_path = generate_review(args.kicad_root or _default_kicad_root(), args.output)
    print(output_path)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
