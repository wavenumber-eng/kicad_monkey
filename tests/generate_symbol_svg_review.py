"""Generate a pan/zoom SVG review page for KiCad schematic library symbols."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import shutil
import sys
from typing import Any

from generate_cli_svg_comparison import (
    _PANZOOM_CSS,
    _PANZOOM_JS,
    _default_kicad_root,
    _html,
    _inline_script_block,
    _load_manifest,
    _panzoom_figure,
    _rel_href,
    _slug,
    _svg_pan_zoom_script,
)


REPO_ROOT = Path(__file__).resolve().parents[1]
LOCAL_KICAD_MONKEY_SRC = REPO_ROOT / "src" / "py"
if LOCAL_KICAD_MONKEY_SRC.exists() and str(LOCAL_KICAD_MONKEY_SRC) not in sys.path:
    sys.path.insert(0, str(LOCAL_KICAD_MONKEY_SRC))


def _iter_symbol_cases(kicad_root: Path) -> list[dict[str, Any]]:
    manifest = _load_manifest(kicad_root)
    cases: list[dict[str, Any]] = []
    for case in manifest.get("cases") or []:
        if not isinstance(case, dict):
            continue
        if case.get("status") != "active":
            continue
        if "symbol_svg" not in (case.get("domains") or []):
            continue
        if not case.get("input_file"):
            continue
        cases.append(case)
    return sorted(
        cases,
        key=lambda item: (
            0 if "MIMXRT685" in str(item.get("name") or item.get("id") or "") else 1,
            str(item.get("name") or item.get("id") or ""),
        ),
    )


def _select_cases(cases: list[dict[str, Any]], names: set[str] | None) -> list[dict[str, Any]]:
    if not names:
        return cases
    out: list[dict[str, Any]] = []
    for case in cases:
        case_name = str(case.get("name") or "")
        case_id = str(case.get("id") or "")
        symbol_name = str(case.get("symbol_name") or "")
        if case_name in names or case_id in names or symbol_name in names:
            out.append(case)
    return out


def _symbol_theme(preferences_dir: Path | None):
    if preferences_dir is None:
        return None
    from kicad_monkey.kicad_svg_preferences import symbol_theme_from_preferences

    return symbol_theme_from_preferences(preferences_dir)


def _reset_generated_assets(path: Path, *, review_dir: Path) -> None:
    resolved_path = path.resolve()
    resolved_review = review_dir.resolve()
    try:
        resolved_path.relative_to(resolved_review)
    except ValueError as exc:
        raise RuntimeError(f"Refusing to clear assets outside review dir: {path}") from exc
    if resolved_path.exists():
        shutil.rmtree(resolved_path)
    resolved_path.mkdir(parents=True, exist_ok=True)


def _case_symbol_names(lib: Any, case: dict[str, Any], input_file: Path) -> list[str]:
    explicit = case.get("symbol_name")
    if explicit:
        symbol_name = str(explicit)
        if lib.get_symbol(symbol_name) is None:
            raise RuntimeError(
                f"Manifest symbol_name {symbol_name!r} not found in {input_file}. "
                f"Available symbols: {lib.symbol_names()}"
            )
        return [symbol_name]

    candidates = [
        str(value)
        for value in (case.get("name"), input_file.stem)
        if value not in (None, "")
    ]
    for candidate in candidates:
        if lib.get_symbol(candidate) is not None:
            return [candidate]

    names = [
        symbol.name
        for symbol in lib.symbols
        if not getattr(symbol, "extends", None) or getattr(symbol, "subsymbols", None)
    ]
    return names or lib.symbol_names()


def _render_symbol(
    *,
    case: dict[str, Any],
    input_file: Path,
    lib: Any,
    symbol_name: str,
    assets_dir: Path,
    preferences_dir: Path | None,
) -> dict[str, Any]:
    from kicad_monkey import SymbolTheme

    symbol = lib.get_symbol(symbol_name)
    if symbol is None:
        raise RuntimeError(f"Symbol {symbol_name!r} not found in {input_file}")

    case_key = str(case.get("id") or case.get("name") or input_file.stem)
    symbol_dir = assets_dir / _slug(case_key) / _slug(symbol_name)
    default_dir = symbol_dir / "default"
    theme_dir = symbol_dir / "wavenumber"
    default_dir.mkdir(parents=True, exist_ok=True)
    theme_dir.mkdir(parents=True, exist_ok=True)
    for directory in (default_dir, theme_dir):
        for svg_path in directory.glob("*.svg"):
            svg_path.unlink()

    themed = _symbol_theme(preferences_dir)
    units: list[dict[str, Any]] = []
    for unit in range(1, int(symbol.unit_count) + 1):
        stem = f"{_slug(symbol_name)}_unit{unit}"
        default_svg = default_dir / f"{stem}.svg"
        themed_svg = theme_dir / f"{stem}.svg"
        default_svg.write_text(
            lib.symbol_to_svg(symbol_name, unit=unit, theme=SymbolTheme()),
            encoding="utf-8",
        )
        themed_svg.write_text(
            lib.symbol_to_svg(symbol_name, unit=unit, theme=themed or SymbolTheme()),
            encoding="utf-8",
        )
        units.append(
            {
                "unit": unit,
                "default_svg": str(default_svg),
                "themed_svg": str(themed_svg),
                "default_bytes": default_svg.stat().st_size,
                "themed_bytes": themed_svg.stat().st_size,
            }
        )

    return {
        "case": {
            "id": case.get("id"),
            "name": case.get("name"),
            "origin": case.get("origin"),
            "input_file": str(input_file),
            "symbol_name": symbol_name,
            "unit_count": int(symbol.unit_count),
        },
        "units": units,
    }


def _render_case(
    *,
    kicad_root: Path,
    case: dict[str, Any],
    assets_dir: Path,
    preferences_dir: Path | None,
) -> list[dict[str, Any]]:
    from kicad_monkey import KiCadSymbolLib

    input_file = kicad_root / str(case["input_file"])
    lib = KiCadSymbolLib.from_file(input_file)
    return [
        _render_symbol(
            case=case,
            input_file=input_file,
            lib=lib,
            symbol_name=symbol_name,
            assets_dir=assets_dir,
            preferences_dir=preferences_dir,
        )
        for symbol_name in _case_symbol_names(lib, case, input_file)
    ]


def _render_html(
    *,
    summaries: list[dict[str, Any]],
    kicad_root: Path,
    output_path: Path,
    preferences_dir: Path | None,
) -> str:
    generated = datetime.now(timezone.utc).strftime("%Y-%m-%d %H:%M:%SZ")
    review_dir = output_path.parent
    options: list[str] = []
    cards: list[str] = []
    rows: list[str] = []
    first_card = True

    for summary in summaries:
        case = summary["case"]
        case_id = str(case.get("id") or case.get("name") or case["symbol_name"])
        symbol_name = str(case["symbol_name"])
        rows.append(
            "<tr>"
            f"<td>{_html(case.get('name') or case.get('id') or '')}</td>"
            f"<td>{_html(symbol_name)}</td>"
            f"<td>{_html(case['unit_count'])}</td>"
            f"<td><code>{_html(case['input_file'])}</code></td>"
            "</tr>"
        )
        for unit in summary["units"]:
            key = _slug(f"{case_id}__{symbol_name}__unit{unit['unit']}")
            label = f"{symbol_name} :: unit {unit['unit']}"
            options.append(
                f"<option value=\"{_html(key)}\">{_html(label)}</option>"
            )
            default_href = _rel_href(Path(unit["default_svg"]), from_dir=review_dir)
            themed_href = _rel_href(Path(unit["themed_svg"]), from_dir=review_dir)
            hidden_attr = "" if first_card else " hidden"
            first_card = False
            cards.append(
                f"<section class=\"unit-card\" data-unit-card=\"{_html(key)}\"{hidden_attr}>"
                f"<h2>{_html(label)}</h2>"
                f"<p class=\"meta\">Case: {_html(case.get('name') or case.get('id') or '')}. "
                f"Input: <code>{_html(case['input_file'])}</code>.</p>"
                "<div class=\"links\">"
                f"<a href=\"{_html(default_href)}\" target=\"_blank\" rel=\"noreferrer\">Default SVG</a>"
                f"<a href=\"{_html(themed_href)}\" target=\"_blank\" rel=\"noreferrer\">Wavenumber SVG</a>"
                f"<span>bytes default/theme: {_html(unit['default_bytes'])}/{_html(unit['themed_bytes'])}</span>"
                "</div>"
                "<div class=\"viewers\">"
                f"{_panzoom_figure('Default', default_href)}"
                f"{_panzoom_figure('Wavenumber Theme', themed_href)}"
                "</div>"
                "</section>"
            )

    selector_js = """
(() => {
  window.addEventListener('DOMContentLoaded', () => {
    const select = document.getElementById('unit-select');
    const cards = Array.from(document.querySelectorAll('[data-unit-card]'));
    function showSelected() {
      const value = select.value;
      cards.forEach((card) => { card.hidden = card.dataset.unitCard !== value; });
      if (window.kicadReviewLoadVisibleSvgs) {
        window.kicadReviewLoadVisibleSvgs();
      }
    }
    select.addEventListener('change', showSelected);
    showSelected();
  });
})();
"""
    return "\n".join(
        [
            "<!doctype html>",
            '<html lang="en">',
            "<head>",
            '<meta charset="utf-8">',
            "<title>KiCad Symbol SVG Review</title>",
            "<style>",
            _PANZOOM_CSS,
            ".review-controls { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; margin: 10px 0 0; }",
            ".review-controls select { min-width: min(680px, 100%); height: 32px; border: 1px solid #bcccdc; background: #fff; }",
            ".unit-card[hidden] { display: none; }",
            ".symbol-index { margin: 12px 0; }",
            ".symbol-index > summary { cursor: pointer; font-weight: 600; }",
            ".symbol-index table { margin-bottom: 0; }",
            "</style>",
            "</head>",
            "<body>",
            "<header>",
            "<h1>KiCad Symbol SVG Review</h1>",
            (
                f"<p class=\"meta\">Generated {_html(generated)} from <code>{_html(kicad_root)}</code>. "
                f"Preferences: <code>{_html(preferences_dir or '')}</code>.</p>"
            ),
            "<div class=\"review-controls\">",
            "<label for=\"unit-select\">Symbol unit</label>",
            f"<select id=\"unit-select\">{''.join(options)}</select>",
            "</div>",
            "</header>",
            "<main>",
            "<details class=\"symbol-index\"><summary>Symbol index</summary>"
            "<table><thead><tr><th>Case</th><th>Symbol</th><th>Units</th><th>Input</th></tr></thead>"
            f"<tbody>{''.join(rows)}</tbody></table></details>",
            *cards,
            "</main>",
            _inline_script_block(_svg_pan_zoom_script()),
            _inline_script_block(f"{_PANZOOM_JS}\n{selector_js}"),
            "</body>",
            "</html>",
        ]
    ) + "\n"


def generate_symbol_review(
    *,
    kicad_root: Path,
    output_path: Path | None = None,
    cases: list[str] | None = None,
    preferences_dir: Path | None = None,
) -> Path:
    kicad_root = kicad_root.resolve()
    default_review_dir = (
        Path(__file__).parent / "L3_rendering" / "output" / "symbol_svg"
    )
    review_dir = (output_path.parent if output_path else default_review_dir).resolve()
    output_path = output_path or (review_dir / "symbol_svg_review.html")
    assets_dir = review_dir / "symbol_svg_review"
    review_dir.mkdir(parents=True, exist_ok=True)
    _reset_generated_assets(assets_dir, review_dir=review_dir)

    selected = _select_cases(
        _iter_symbol_cases(kicad_root),
        names=set(cases) if cases else None,
    )
    if not selected:
        raise RuntimeError("No matching active symbol SVG cases found")

    summaries = [
        summary
        for case in selected
        for summary in _render_case(
            kicad_root=kicad_root,
            case=case,
            assets_dir=assets_dir,
            preferences_dir=preferences_dir,
        )
    ]
    (review_dir / "symbol_svg_review.json").write_text(
        json.dumps(
            {
                "generated_utc": datetime.now(timezone.utc).isoformat(),
                "kicad_root": str(kicad_root),
                "preferences_dir": str(preferences_dir) if preferences_dir else None,
                "cases": summaries,
            },
            indent=2,
        )
        + "\n",
        encoding="utf-8",
    )
    output_path.write_text(
        _render_html(
            summaries=summaries,
            kicad_root=kicad_root,
            output_path=output_path,
            preferences_dir=preferences_dir,
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
        help="HTML output path. Defaults to tests/L3_rendering/output/symbol_svg/.",
    )
    parser.add_argument(
        "--case",
        action="append",
        dest="cases",
        default=None,
        help="Case name, id, or symbol name to include. May be repeated. Defaults to all active symbol SVG cases.",
    )
    parser.add_argument("--preferences", type=Path, default=None)
    args = parser.parse_args()
    kicad_root = args.kicad_root or _default_kicad_root()
    output = generate_symbol_review(
        kicad_root=kicad_root,
        output_path=args.output,
        cases=args.cases,
        preferences_dir=args.preferences,
    )
    print(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
