"""Generate/check deterministic native plotter-base-a0 SVG snapshots."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any

from kicad_monkey.kicad_native import KiCadNativeError, native_render_svg

ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "tests/parity/native_svg_a0_vectors.json"
SOURCES = (
    ("footprint", "footprint_plotter_a0_vectors.json"),
    ("symbol", "symbol_plotter_a0_vectors.json"),
    ("board", "board_plotter_a0_vectors.json"),
    ("schematic", "schematic_plotter_a0_vectors.json"),
)
REJECTED_CASES = {
    "board:tracks-follow-graphics-with-net-extras": "field width_nm must be nonnegative",
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--executable", type=Path)
    args = parser.parse_args()
    executable = args.executable or _default_executable()
    output = _generate(executable)
    encoded = json.dumps(output, indent=2, ensure_ascii=False) + "\n"
    if args.check:
        if not OUTPUT.is_file() or OUTPUT.read_text(encoding="utf-8") != encoded:
            raise SystemExit("native SVG parity vectors are stale; regenerate them")
        return 0
    OUTPUT.write_text(encoded, encoding="utf-8", newline="\n")
    return 0


def _generate(executable: Path) -> dict[str, object]:
    cases: list[dict[str, object]] = []
    for kind, filename in SOURCES:
        source = json.loads((ROOT / "tests/parity" / filename).read_text(encoding="utf-8"))
        for vector in source["vectors"]:
            document = vector["expected"]
            viewport = _viewport(kind, document)
            case_id = f"{kind}:{vector['id']}"
            common = {
                "id": case_id,
                "producer": kind,
                "source_vector": filename,
                "source_id": vector["id"],
                "document_sha256": _canonical_sha(document),
                "viewport": viewport,
            }
            try:
                result = native_render_svg(
                    document,
                    document_kind=kind,
                    viewport=viewport,
                    executable=executable,
                )
            except KiCadNativeError as error:
                expected = REJECTED_CASES.get(case_id)
                if expected is None or expected not in str(error):
                    raise
                cases.append(
                    {
                        **common,
                        "outcome": "rejected",
                        "error_contains": expected,
                    }
                )
                continue
            if case_id in REJECTED_CASES:
                raise RuntimeError(f"expected native SVG safety rejection for {case_id}")
            svg_root = ET.fromstring(result.svg_utf8)
            tags: dict[str, int] = {}
            for element in svg_root.iter():
                tag = element.tag.rsplit("}", 1)[-1]
                tags[tag] = tags.get(tag, 0) + 1
            cases.append(
                {
                    **common,
                    "outcome": "svg",
                    "svg_bytes": result.svg_bytes,
                    "svg_sha256": result.svg_sha256,
                    "tag_counts": dict(sorted(tags.items())),
                }
            )
    return {
        "schema": "kicad_monkey.native_svg_parity.a0",
        "profile": "plotter-base-a0",
        "case_count": len(cases),
        "svg_case_count": sum(case["outcome"] == "svg" for case in cases),
        "rejected_case_count": sum(case["outcome"] == "rejected" for case in cases),
        "cases": cases,
    }


def _viewport(kind: str, document: dict[str, Any]) -> dict[str, int]:
    if kind == "schematic":
        return {
            "min_x_nm": 0,
            "min_y_nm": 0,
            "width_nm": int(document["canvas"]["width_nm"]),
            "height_nm": int(document["canvas"]["height_nm"]),
        }
    return {
        "min_x_nm": -100_000_000,
        "min_y_nm": -100_000_000,
        "width_nm": 200_000_000,
        "height_nm": 200_000_000,
    }


def _canonical_sha(value: object) -> str:
    encoded = json.dumps(value, sort_keys=True, separators=(",", ":"), ensure_ascii=False).encode()
    return hashlib.sha256(encoded).hexdigest()


def _default_executable() -> Path:
    filename = "kicad-monkey-native.exe" if os.name == "nt" else "kicad-monkey-native"
    return ROOT / "target" / "debug" / filename


if __name__ == "__main__":
    raise SystemExit(main())
