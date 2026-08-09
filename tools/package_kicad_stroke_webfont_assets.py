"""Build or verify the authoritative KiCad Stroke webfont asset bundle.

The checked-in ``assets/fonts`` directory is produced only through this tool.
Its manifest pins every generated file plus the generator, Newstroke table,
theme, and Monkey Kit mark inputs that define the bundle.

Usage:

    uv run python tools/package_kicad_stroke_webfont_assets.py
    uv run python tools/package_kicad_stroke_webfont_assets.py --check
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Final

from fontTools.ttLib import TTFont

import generate_kicad_stroke_webfont as webfont

PROJECT_ROOT: Final = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT_DIR: Final = PROJECT_ROOT / "assets" / "fonts"
MANIFEST_NAME: Final = "kicad-stroke-font-package.json"
PACKAGE_ID: Final = "kicad-newstroke"
FONT_FORMATS: Final = ("ttf", "otf", "woff", "woff2")
FONT_STEMS: Final = (
    "kicad-stroke-light",
    "kicad-stroke-light-italic",
    "kicad-stroke",
    "kicad-stroke-italic",
    "kicad-stroke-bold",
    "kicad-stroke-bold-italic",
)
INPUT_PATHS: Final = {
    "generator": PROJECT_ROOT / "tools" / "generate_kicad_stroke_webfont.py",
    "packager": Path(__file__).resolve(),
    "newstroke_data": (
        PROJECT_ROOT / "src" / "py" / "kicad_monkey" / "kicad_stroke_font_data.json"
    ),
    "monkey_art": PROJECT_ROOT / "assets" / "monkey" / "kicad-monkey.txt",
    "theme_css": PROJECT_ROOT / "assets" / "kicad-monkey-theme.css",
}


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _font_file_names() -> tuple[str, ...]:
    return tuple(
        f"{stem}.{extension}" for stem in FONT_STEMS for extension in FONT_FORMATS
    )


def _bundle_file_names() -> tuple[str, ...]:
    return (
        *_font_file_names(),
        webfont.CSS_FILE_NAME,
        webfont.DEMO_FILE_NAME,
    )


def _face_records(output_dir: Path) -> list[dict[str, object]]:
    records: list[dict[str, object]] = []
    for stem in FONT_STEMS:
        font = TTFont(output_dir / f"{stem}.ttf")
        records.append(
            {
                "stem": stem,
                "family": font["name"].getDebugName(1),
                "style": font["name"].getDebugName(2),
                "weight": font["OS/2"].usWeightClass,
                "italic": bool(
                    font["OS/2"].fsSelection & webfont.OS2_FS_SELECTION_ITALIC
                ),
                "glyph_count": len(font.getBestCmap()),
                "formats": list(FONT_FORMATS),
            }
        )
    return records


def _manifest(output_dir: Path) -> dict[str, object]:
    return {
        "type": "monkey_font_package",
        "version": 1,
        "id": PACKAGE_ID,
        "display_name": "KiCad Newstroke",
        "family": webfont.FAMILY_NAME,
        "description": (
            "KiCad Newstroke Light, Regular, and Bold faces with upright and "
            "KiCad-compatible italic variants."
        ),
        "source_project": "kicad-monkey",
        "source_version": webfont.FONT_VERSION,
        "license": "CC0-1.0",
        "provenance": (
            "Generated from the non-CJK CC0 Newstroke range vendored from the "
            "KiCad 10.0 branch."
        ),
        "stylesheet": webfont.CSS_FILE_NAME,
        "demo": webfont.DEMO_FILE_NAME,
        "default_face": "kicad-stroke",
        "faces": _face_records(output_dir),
        "inputs": {
            name: {
                "path": path.relative_to(PROJECT_ROOT).as_posix(),
                "sha256": _sha256(path),
            }
            for name, path in INPUT_PATHS.items()
        },
        "files": {
            name: _sha256(output_dir / name) for name in sorted(_bundle_file_names())
        },
    }


def build_bundle(output_dir: Path) -> int:
    output_dir.mkdir(parents=True, exist_ok=True)
    result = webfont.generate_fonts(
        output_dir=output_dir,
        weights=("light", "regular", "bold"),
        italic=True,
        formats=FONT_FORMATS,
    )
    if result.css_path is None:
        raise RuntimeError("bundle generation did not produce CSS")
    demo_path = output_dir / webfont.DEMO_FILE_NAME
    demo_path.write_text(
        webfont.build_demo_html(
            INPUT_PATHS["monkey_art"].read_text(encoding="utf-8"),
            INPUT_PATHS["theme_css"].read_text(encoding="utf-8"),
            result.css_path.name,
        ),
        encoding="utf-8",
        newline="\n",
    )
    manifest_path = output_dir / MANIFEST_NAME
    manifest_path.write_text(
        json.dumps(_manifest(output_dir), indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    return check_bundle(output_dir)


def check_bundle(output_dir: Path) -> int:
    findings: list[str] = []
    manifest_path = output_dir / MANIFEST_NAME
    if not manifest_path.is_file():
        print(f"missing font-package manifest: {manifest_path}", file=sys.stderr)
        return 1
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    expected_names = {*_bundle_file_names(), MANIFEST_NAME}
    actual_names = {path.name for path in output_dir.iterdir() if path.is_file()}
    for name in sorted(expected_names - actual_names):
        findings.append(f"missing bundle file: {name}")
    for name in sorted(actual_names - expected_names):
        findings.append(f"unexpected bundle file: {name}")

    recorded_inputs = manifest.get("inputs", {})
    if not isinstance(recorded_inputs, dict):
        findings.append("manifest inputs must be an object")
        recorded_inputs = {}
    for name, path in INPUT_PATHS.items():
        record = recorded_inputs.get(name, {})
        actual_hash = _sha256(path)
        if not isinstance(record, dict) or record.get("sha256") != actual_hash:
            findings.append(f"stale generator input hash: {name}")

    recorded_files = manifest.get("files", {})
    if not isinstance(recorded_files, dict):
        findings.append("manifest files must be an object")
        recorded_files = {}
    for name in _bundle_file_names():
        path = output_dir / name
        if path.is_file() and recorded_files.get(name) != _sha256(path):
            findings.append(f"stale bundle hash: {name}")

    if not findings:
        expected_faces = _face_records(output_dir)
        if manifest.get("faces") != expected_faces:
            findings.append("font face metadata does not match the manifest")
        css = (output_dir / webfont.CSS_FILE_NAME).read_text(encoding="utf-8")
        missing_css = [name for name in _font_file_names() if name not in css]
        if missing_css:
            findings.append(f"CSS does not reference {len(missing_css)} font files")
        demo = (output_dir / webfont.DEMO_FILE_NAME).read_text(encoding="utf-8")
        if f'href="{webfont.CSS_FILE_NAME}"' not in demo:
            findings.append("demo does not reference the package stylesheet")

    if findings:
        print("\n".join(findings), file=sys.stderr)
        return 1
    print(
        f"verified {PACKAGE_ID}: {len(FONT_STEMS)} faces, "
        f"{len(_font_file_names())} font files"
    )
    return 0


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--check", action="store_true", help="Verify without rebuilding"
    )
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> int:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    output_dir = args.output_dir.resolve()
    return check_bundle(output_dir) if args.check else build_bundle(output_dir)


if __name__ == "__main__":
    raise SystemExit(main())
