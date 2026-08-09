"""Regenerate ``kicad_stroke_font_data.json`` from KiCad's Newstroke source.

The bundled JSON is a flat list of Hershey-encoded glyph strings indexed by
``codepoint - 0x20``, exactly mirroring KiCad's ``newstroke_font[]`` array in
``common/newstroke_font.cpp``. This script parses that C++ source and writes
the leading non-CJK portion of the array (U+0020 through U+2BFF by default).

Provenance and licensing:

- The vendored range is the original Newstroke stroke font by Vladimir
  Uryvaev, which is dedicated to the public domain under CC0-1.0.
- The CJK sections that KiCad appends after U+2BFF carry separate licenses
  (MIT-style conversion code and SIL OFL Source Han Sans outlines) and are
  deliberately excluded.
- The source file must match the KiCad release used as the rendering
  reference (``tools/kicad-cli``); stroke order differs between KiCad
  branches and the SVG renderers depend on the exact order.

Usage:

    uv run python scripts/generate_kicad_stroke_font_data.py --download
    uv run python scripts/generate_kicad_stroke_font_data.py --source temp/newstroke_font.cpp
    uv run python scripts/generate_kicad_stroke_font_data.py --download --check
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import urllib.request
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUTPUT = (
    REPO_ROOT / "src" / "py" / "kicad_monkey" / "kicad_stroke_font_data.json"
)
DEFAULT_CACHE = REPO_ROOT / "temp" / "newstroke_font.cpp"

# Pin to the KiCad branch matching the bundled kicad-cli rendering reference.
KICAD_BRANCH = "10.0"
SOURCE_URL = f"https://gitlab.com/kicad/code/kicad/-/raw/{KICAD_BRANCH}/common/newstroke_font.cpp"

# First codepoint excluded from the vendored range. Everything below this is
# the original CC0 Newstroke set; the differently licensed CJK-oriented
# sections start at the "Skip to CJK" marker after U+2BFF.
DEFAULT_END_CODEPOINT = 0x2C00

FIRST_CODEPOINT = 0x20
ARRAY_START_MARKER = "const char* const newstroke_font[] ="
ARRAY_END_MARKER = "const int newstroke_font_bufsize"

_STRING_LITERAL = re.compile(r'"((?:\\.|[^"\\])*)"')
_BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.DOTALL)


def parse_newstroke_glyphs(cpp_text: str) -> list[str]:
    """Extract the glyph strings from ``newstroke_font.cpp`` in array order."""
    start = cpp_text.index(ARRAY_START_MARKER)
    end = cpp_text.index(ARRAY_END_MARKER)
    body = _BLOCK_COMMENT.sub("", cpp_text[start:end])
    glyphs: list[str] = []
    for match in _STRING_LITERAL.finditer(body):
        literal = match.group(1)
        glyphs.append(literal.replace('\\"', '"').replace("\\\\", "\\"))
    return glyphs


def _validate_glyphs(glyphs: list[str], end_codepoint: int) -> None:
    needed = end_codepoint - FIRST_CODEPOINT
    if len(glyphs) < needed:
        raise SystemExit(
            f"source array has {len(glyphs)} glyphs; need {needed} "
            f"for U+{FIRST_CODEPOINT:04X}..U+{end_codepoint - 1:04X}"
        )
    if glyphs[0] != "JZ":
        raise SystemExit(f"unexpected space glyph {glyphs[0]!r}; wrong source file?")


def _fetch_source(cache_path: Path) -> str:
    cache_path.parent.mkdir(parents=True, exist_ok=True)
    print(f"downloading {SOURCE_URL}")
    with urllib.request.urlopen(SOURCE_URL) as response:
        text = response.read().decode("utf-8")
    if ARRAY_START_MARKER not in text:
        raise SystemExit("downloaded file does not look like newstroke_font.cpp")
    cache_path.write_text(text, encoding="utf-8", newline="\n")
    return text


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--source",
        type=Path,
        default=None,
        help=f"Path to a local newstroke_font.cpp (default: {DEFAULT_CACHE})",
    )
    parser.add_argument(
        "--download",
        action="store_true",
        help=f"Download newstroke_font.cpp from the KiCad {KICAD_BRANCH} branch first",
    )
    parser.add_argument("--output", type=Path, default=DEFAULT_OUTPUT)
    parser.add_argument(
        "--end-codepoint",
        type=lambda value: int(value, 0),
        default=DEFAULT_END_CODEPOINT,
        help="First codepoint excluded from the output (default 0x2C00)",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Fail if the existing output file does not match the regeneration",
    )
    return parser.parse_args(argv)


def main(argv: list[str]) -> int:
    args = parse_args(argv)
    source_path = args.source if args.source is not None else DEFAULT_CACHE
    if args.download:
        text = _fetch_source(source_path)
    else:
        if not source_path.is_file():
            raise SystemExit(
                f"missing source {source_path}; pass --download or --source"
            )
        text = source_path.read_text(encoding="utf-8")

    glyphs = parse_newstroke_glyphs(text)
    _validate_glyphs(glyphs, args.end_codepoint)
    vendored = glyphs[: args.end_codepoint - FIRST_CODEPOINT]
    payload = json.dumps(vendored, ensure_ascii=True)

    if args.check:
        current = (
            args.output.read_text(encoding="utf-8") if args.output.is_file() else ""
        )
        if current != payload:
            print(f"stale stroke font data: {args.output}", file=sys.stderr)
            return 1
        print(f"{args.output.name} matches source ({len(vendored)} glyphs)")
        return 0

    args.output.write_text(payload, encoding="utf-8", newline="\n")
    print(
        f"wrote {args.output} with {len(vendored)} glyphs "
        f"(U+{FIRST_CODEPOINT:04X}..U+{args.end_codepoint - 1:04X})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
