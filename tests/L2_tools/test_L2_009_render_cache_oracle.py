"""Live KiCad render-cache oracle tests."""

from __future__ import annotations

import base64
import hashlib
import json
from pathlib import Path
import subprocess

import pytest

from kicad_cli_resolver import resolve_kicad_cli
from kicad_monkey.kicad_pcb import KiCadPcb
from kicad_monkey.kicad_primitives import RenderCache
from kicad_monkey.kicad_render_cache import (
    RenderCacheResolver,
    render_cache_request_for_board_text,
    render_cache_request_for_dimension_text,
    render_cache_request_for_footprint_property,
    render_cache_request_for_footprint_text,
    render_cache_request_for_footprint_text_box,
    render_cache_request_for_table_cell,
)
from kicad_monkey.kicad_render_cache_oracle import (
    compare_render_caches,
    compare_render_cache_entry_sets,
    run_kicad_pcb_render_cache_save_oracle,
    summarize_render_cache_entries,
)
from kicad_monkey.kicad_sexpr import parse_sexp


_CLI = resolve_kicad_cli(required_capability="pcb_svg")
_CONSOLAS = Path("C:/Windows/Fonts/consola.ttf")
_ARIAL = Path("C:/Windows/Fonts/arial.ttf")


def _find_wavenumber_font() -> Path | None:
    for parent in Path(__file__).resolve().parents:
        candidate = parent / "libz" / "wn-general" / "assets" / "fonts" / "Wavenumber-Regular.ttf"
        if candidate.exists():
            return candidate
    return None


_WAVENUMBER_FONT = _find_wavenumber_font()


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
def test_kicad_pcb_save_oracle_generates_render_cache_from_semantic_text(
    tmp_path: Path,
):
    source = tmp_path / "render_cache_oracle.kicad_pcb"
    source.write_text(
        """(kicad_pcb
\t(version 20241229)
\t(generator "pcbnew")
\t(generator_version "9.0")
\t(general
\t\t(thickness 1.6)
\t\t(legacy_teardrops no)
\t)
\t(paper "A4")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(2 "B.Cu" signal)
\t\t(5 "F.SilkS" user "F.Silkscreen")
\t\t(7 "B.SilkS" user "B.Silkscreen")
\t\t(1 "F.Mask" user)
\t\t(3 "B.Mask" user)
\t\t(25 "Edge.Cuts" user)
\t)
\t(setup (pad_to_mask_clearance 0))
\t(gr_text "Cache Me"
\t\t(at 10 10 0)
\t\t(layer "F.SilkS")
\t\t(uuid "11111111-1111-1111-1111-111111111111")
\t\t(effects (font (face "Arial") (size 2 2) (thickness 0.2)))
\t)
)
""",
        encoding="utf-8",
    )

    result = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / "oracle",
    )

    matching = [
        entry
        for entry in result.entries
        if entry.object_type == "gr_text" and entry.text == "Cache Me"
    ]
    assert matching
    assert matching[0].cache.polygons
    assert "(render_cache" in result.oracle_pcb.read_text(encoding="utf-8")


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
def test_kicad_pcb_save_oracle_covers_promoted_text_object_types(tmp_path: Path):
    source = tmp_path / "render_cache_oracle_comprehensive.kicad_pcb"
    source.write_text(
        """(kicad_pcb
\t(version 20241229)
\t(generator "pcbnew")
\t(generator_version "9.0")
\t(general
\t\t(thickness 1.6)
\t\t(legacy_teardrops no)
\t)
\t(paper "A4")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(2 "B.Cu" signal)
\t\t(5 "F.SilkS" user "F.Silkscreen")
\t\t(7 "B.SilkS" user "B.Silkscreen")
\t\t(1 "F.Mask" user)
\t\t(3 "B.Mask" user)
\t\t(25 "Edge.Cuts" user)
\t)
\t(setup (pad_to_mask_clearance 0))
\t(gr_text "Board Cache"
\t\t(at 10 10 15)
\t\t(layer "F.SilkS")
\t\t(uuid "11111111-1111-1111-1111-111111111111")
\t\t(effects
\t\t\t(font
\t\t\t\t(face "Arial")
\t\t\t\t(size 2 2)
\t\t\t\t(thickness 0.2)
\t\t\t\t(bold yes)
\t\t\t\t(italic yes)
\t\t\t)
\t\t\t(justify left top)
\t\t)
\t)
\t(gr_text_box "Board Box"
\t\t(start 20 10)
\t\t(end 50 22)
\t\t(margins 0.6 0.6 0.6 0.6)
\t\t(layer "F.SilkS")
\t\t(uuid "11111111-1111-1111-1111-111111111112")
\t\t(effects
\t\t\t(font
\t\t\t\t(face "Arial")
\t\t\t\t(size 1.4 1.4)
\t\t\t\t(thickness 0.14)
\t\t\t)
\t\t\t(justify left top)
\t\t)
\t\t(border yes)
\t\t(stroke (width 0.15) (type solid))
\t)
\t(footprint "Test:CacheText"
\t\t(layer "F.Cu")
\t\t(uuid "22222222-2222-2222-2222-222222222222")
\t\t(at 30 40 30)
\t\t(property "Reference" "U_SYN"
\t\t\t(at 0 -5 0)
\t\t\t(layer "F.SilkS")
\t\t\t(uuid "33333333-3333-3333-3333-333333333331")
\t\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t)
\t\t(property "Value" "VAL_SYN"
\t\t\t(at 0 5 0)
\t\t\t(layer "F.SilkS")
\t\t\t(uuid "33333333-3333-3333-3333-333333333332")
\t\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t)
\t\t(property "UserProp" "PROP_SYN"
\t\t\t(at 0 8 10)
\t\t\t(layer "F.SilkS")
\t\t\t(uuid "33333333-3333-3333-3333-333333333333")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 1 1) (thickness 0.1))
\t\t\t\t(justify right bottom)
\t\t\t)
\t\t)
\t\t(fp_text user "USER_SYN"
\t\t\t(at 0 0 45)
\t\t\t(layer "F.SilkS")
\t\t\t(uuid "44444444-4444-4444-4444-444444444443")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 1.2 1.2) (thickness 0.12))
\t\t\t\t(justify left)
\t\t\t)
\t\t)
\t\t(fp_text_box "Footprint Box"
\t\t\t(start -6 -3)
\t\t\t(end 6 3)
\t\t\t(margins 0.4 0.4 0.4 0.4)
\t\t\t(angle 20)
\t\t\t(layer "F.SilkS")
\t\t\t(uuid "55555555-5555-5555-5555-555555555555")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 1 1) (thickness 0.1))
\t\t\t\t(justify left top)
\t\t\t)
\t\t\t(border no)
\t\t\t(stroke (width 0.12) (type default))
\t\t)
\t)
)
""",
        encoding="utf-8",
    )

    result = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / "oracle",
    )
    summary = summarize_render_cache_entries(result.entries)

    required = ["gr_text", "gr_text_box", "property", "fp_text", "fp_text_box"]
    assert summary.missing_object_types(required) == []
    assert summary.object_type_counts == {
        "fp_text": 1,
        "fp_text_box": 1,
        "gr_text": 1,
        "gr_text_box": 1,
        "property": 3,
    }
    assert summary.polygon_count > 0
    assert summary.contour_count >= summary.polygon_count
    assert all(entry.cache.polygons for entry in result.entries)

    regenerated = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=result.oracle_pcb,
        work_dir=tmp_path / "oracle_regenerated",
    )
    comparison = compare_render_cache_entry_sets(result.entries, regenerated.entries)
    assert comparison.matched, comparison


def _write_outline_text_board(
    source: Path,
    text: str,
    font_style: str = "",
    font_face: str = "Arial",
    angle: float = 0.0,
    justify: str | None = "left top",
) -> None:
    justify_expr = f"\t\t\t(justify {justify})\n" if justify else ""
    source.write_text(
        f"""(kicad_pcb
\t(version 20241229)
\t(generator "pcbnew")
\t(generator_version "9.0")
\t(general
\t\t(thickness 1.6)
\t\t(legacy_teardrops no)
\t)
\t(paper "A4")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(2 "B.Cu" signal)
\t\t(5 "F.SilkS" user "F.Silkscreen")
\t\t(7 "B.SilkS" user "B.Silkscreen")
\t\t(1 "F.Mask" user)
\t\t(3 "B.Mask" user)
\t\t(25 "Edge.Cuts" user)
\t)
\t(setup (pad_to_mask_clearance 0))
\t(gr_text "{text}"
\t\t(at 10 10 {angle})
\t\t(layer "F.SilkS")
\t\t(uuid "11111111-1111-1111-1111-111111111111")
\t\t(effects
\t\t\t(font (face "{font_face}") (size 2 2) (thickness 0.2) {font_style})
{justify_expr}
\t\t)
\t)
)
""",
        encoding="utf-8",
    )


def _compress_embedded_payload(data: bytes) -> str:
    zstandard = pytest.importorskip("zstandard")
    compressed = zstandard.ZstdCompressor().compress(data)
    return base64.b64encode(compressed).decode("ascii")


def _write_embedded_font_text_board(
    source: Path,
    font_path: Path,
) -> None:
    font_data = font_path.read_bytes()
    embedded_font_data = _compress_embedded_payload(font_data)
    checksum = hashlib.sha256(font_data).hexdigest()
    source.write_text(
        f"""(kicad_pcb
\t(version 20241229)
\t(generator "pcbnew")
\t(generator_version "9.0")
\t(general
\t\t(thickness 1.6)
\t\t(legacy_teardrops no)
\t)
\t(paper "A4")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(2 "B.Cu" signal)
\t\t(5 "F.SilkS" user "F.Silkscreen")
\t\t(7 "B.SilkS" user "B.Silkscreen")
\t\t(1 "F.Mask" user)
\t\t(3 "B.Mask" user)
\t\t(25 "Edge.Cuts" user)
\t)
\t(setup (pad_to_mask_clearance 0))
\t(gr_text "A"
\t\t(at 10 10 0)
\t\t(layer "F.SilkS")
\t\t(uuid "11111111-1111-1111-1111-111111111122")
\t\t(effects
\t\t\t(font (face "Wavenumber") (size 2 2) (thickness 0.2))
\t\t\t(justify left top)
\t\t)
\t)
\t(embedded_fonts yes)
\t(embedded_files
\t\t(file
\t\t\t(name "{font_path.name}")
\t\t\t(type font)
\t\t\t(data |{embedded_font_data}|)
\t\t\t(checksum "{checksum}")
\t\t)
\t)
)
""",
        encoding="utf-8",
    )


def _write_footprint_outline_text_board(
    source: Path,
    object_kind: str,
    text: str,
    footprint_at: tuple[float, float, float] = (0.0, 0.0, 0.0),
    footprint_layer: str = "F.Cu",
    text_layer: str = "F.SilkS",
    justify: str = "left top",
) -> None:
    if object_kind == "fp_text":
        text_body = f'''\t\t(fp_text user "{text}"
\t\t\t(at 10 10 0)
\t\t\t(layer "{text_layer}")
\t\t\t(uuid "11111111-1111-1111-1111-111111111111")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t(justify {justify})
\t\t\t)
\t\t)'''
    elif object_kind == "property":
        text_body = f'''\t\t(property "Label" "{text}"
\t\t\t(at 10 10 0)
\t\t\t(layer "{text_layer}")
\t\t\t(uuid "11111111-1111-1111-1111-111111111111")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t(justify {justify})
\t\t\t)
\t\t)'''
    elif object_kind == "fp_text_box":
        text_body = f'''\t\t(fp_text_box "{text}"
\t\t\t(start 10 10)
\t\t\t(end 40 22)
\t\t\t(margins 0.6 0.6 0.6 0.6)
\t\t\t(layer "{text_layer}")
\t\t\t(uuid "11111111-1111-1111-1111-111111111111")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 1.4 1.4) (thickness 0.14))
\t\t\t\t(justify {justify})
\t\t\t)
\t\t\t(border yes)
\t\t\t(stroke (width 0.15) (type solid))
\t\t)'''
    else:
        raise ValueError(object_kind)

    source.write_text(
        f"""(kicad_pcb
\t(version 20241229)
\t(generator "pcbnew")
\t(generator_version "9.0")
\t(general
\t\t(thickness 1.6)
\t\t(legacy_teardrops no)
\t)
\t(paper "A4")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(2 "B.Cu" signal)
\t\t(5 "F.SilkS" user "F.Silkscreen")
\t\t(7 "B.SilkS" user "B.Silkscreen")
\t\t(1 "F.Mask" user)
\t\t(3 "B.Mask" user)
\t\t(25 "Edge.Cuts" user)
\t)
\t(setup (pad_to_mask_clearance 0))
\t(footprint "TEST:One"
\t\t(layer "{footprint_layer}")
\t\t(uuid "22222222-2222-2222-2222-222222222222")
\t\t(at {footprint_at[0]} {footprint_at[1]} {footprint_at[2]})
{text_body}
\t)
)
""",
        encoding="utf-8",
    )


def _write_outline_text_box_board(
    source: Path,
    text: str,
    *,
    angle: float = 0.0,
    justify: str | None = "left top",
    end_x: float = 50.0,
    end_y: float = 22.0,
) -> None:
    justify_expr = f"\t\t\t(justify {justify})\n" if justify else ""
    angle_expr = f"\t\t(angle {angle})\n" if angle else ""
    source.write_text(
        f"""(kicad_pcb
\t(version 20241229)
\t(generator "pcbnew")
\t(generator_version "9.0")
\t(general
\t\t(thickness 1.6)
\t\t(legacy_teardrops no)
\t)
\t(paper "A4")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(2 "B.Cu" signal)
\t\t(5 "F.SilkS" user "F.Silkscreen")
\t\t(7 "B.SilkS" user "B.Silkscreen")
\t\t(1 "F.Mask" user)
\t\t(3 "B.Mask" user)
\t\t(25 "Edge.Cuts" user)
\t)
\t(setup (pad_to_mask_clearance 0))
\t(gr_text_box "{text}"
\t\t(start 20 10)
\t\t(end {end_x} {end_y})
\t\t(margins 0.6 0.6 0.6 0.6)
{angle_expr}\t\t(layer "F.SilkS")
\t\t(uuid "11111111-1111-1111-1111-111111111112")
\t\t(effects
\t\t\t(font (face "Arial") (size 1.4 1.4) (thickness 0.14))
{justify_expr}\t\t)
\t\t(border yes)
\t\t(stroke (width 0.15) (type solid))
\t)
)
""",
        encoding="utf-8",
    )


def _write_polygon_text_box_board(
    source: Path,
    text: str,
) -> None:
    source.write_text(
        f"""(kicad_pcb
\t(version 20241229)
\t(generator "pcbnew")
\t(generator_version "9.0")
\t(general
\t\t(thickness 1.6)
\t\t(legacy_teardrops no)
\t)
\t(paper "A4")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(2 "B.Cu" signal)
\t\t(5 "F.SilkS" user "F.Silkscreen")
\t\t(7 "B.SilkS" user "B.Silkscreen")
\t\t(1 "F.Mask" user)
\t\t(3 "B.Mask" user)
\t\t(25 "Edge.Cuts" user)
\t)
\t(setup (pad_to_mask_clearance 0))
\t(gr_text_box "{text}"
\t\t(pts
\t\t\t(xy 20 30)
\t\t\t(xy 45.980762 15)
\t\t\t(xy 51.980762 25.392305)
\t\t\t(xy 26 40.392305)
\t\t)
\t\t(margins 0.6 0.6 0.6 0.6)
\t\t(angle 30)
\t\t(layer "F.SilkS")
\t\t(uuid "11111111-1111-1111-1111-111111111114")
\t\t(effects
\t\t\t(font (face "Arial") (size 1.4 1.4) (thickness 0.14))
\t\t\t(justify left top)
\t\t)
\t\t(border yes)
\t\t(stroke (width 0.15) (type solid))
\t)
)
""",
        encoding="utf-8",
    )


def _write_polygon_footprint_text_box_board(
    source: Path,
    text: str,
) -> None:
    source.write_text(
        f"""(kicad_pcb
\t(version 20241229)
\t(generator "pcbnew")
\t(generator_version "9.0")
\t(general
\t\t(thickness 1.6)
\t\t(legacy_teardrops no)
\t)
\t(paper "A4")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(2 "B.Cu" signal)
\t\t(5 "F.SilkS" user "F.Silkscreen")
\t\t(7 "B.SilkS" user "B.Silkscreen")
\t\t(1 "F.Mask" user)
\t\t(3 "B.Mask" user)
\t\t(25 "Edge.Cuts" user)
\t)
\t(setup (pad_to_mask_clearance 0))
\t(footprint "TEST:PolyBox"
\t\t(layer "F.Cu")
\t\t(uuid "22222222-2222-2222-2222-222222222224")
\t\t(at 30 40 30)
\t\t(fp_text_box "{text}"
\t\t\t(pts
\t\t\t\t(xy 10 10)
\t\t\t\t(xy 40 10)
\t\t\t\t(xy 40 22)
\t\t\t\t(xy 10 22)
\t\t\t)
\t\t\t(margins 0.6 0.6 0.6 0.6)
\t\t\t(layer "F.SilkS")
\t\t\t(uuid "11111111-1111-1111-1111-111111111115")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 1.4 1.4) (thickness 0.14))
\t\t\t\t(justify left top)
\t\t\t)
\t\t\t(border yes)
\t\t\t(stroke (width 0.15) (type solid))
\t\t)
\t)
)
""",
        encoding="utf-8",
    )


def _write_table_cell_board(
    source: Path,
    text: str,
) -> None:
    source.write_text(
        f"""(kicad_pcb
\t(version 20241229)
\t(generator "pcbnew")
\t(generator_version "9.0")
\t(general
\t\t(thickness 1.6)
\t\t(legacy_teardrops no)
\t)
\t(paper "A4")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(2 "B.Cu" signal)
\t\t(5 "F.SilkS" user "F.Silkscreen")
\t\t(7 "B.SilkS" user "B.Silkscreen")
\t\t(1 "F.Mask" user)
\t\t(3 "B.Mask" user)
\t\t(25 "Edge.Cuts" user)
\t)
\t(setup (pad_to_mask_clearance 0))
\t(table
\t\t(column_count 1)
\t\t(layer "F.SilkS")
\t\t(border (external no) (header no))
\t\t(separators (rows no) (cols no))
\t\t(column_widths 30)
\t\t(row_heights 12)
\t\t(cells
\t\t\t(table_cell "{text}"
\t\t\t\t(start 20 10)
\t\t\t\t(end 50 22)
\t\t\t\t(margins 0.6 0.6 0.6 0.6)
\t\t\t\t(span 1 1)
\t\t\t\t(layer "F.SilkS")
\t\t\t\t(uuid "11111111-1111-1111-1111-111111111116")
\t\t\t\t(effects
\t\t\t\t\t(font (face "Arial") (size 1.4 1.4) (thickness 0.14))
\t\t\t\t\t(justify left top)
\t\t\t\t)
\t\t\t)
\t\t)
\t)
)
""",
        encoding="utf-8",
    )


def _write_dimension_text_board(
    source: Path,
    text: str,
    *,
    resolved_nested_text: bool = True,
) -> None:
    nested_text = f"{text} mm" if resolved_nested_text else "STALE"
    nested_x = 5.0 if resolved_nested_text else 20.0
    nested_y = 0.46 if resolved_nested_text else 10.0
    source.write_text(
        f"""(kicad_pcb
\t(version 20241229)
\t(generator "pcbnew")
\t(generator_version "9.0")
\t(general
\t\t(thickness 1.6)
\t\t(legacy_teardrops no)
\t)
\t(paper "A4")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(2 "B.Cu" signal)
\t\t(5 "F.SilkS" user "F.Silkscreen")
\t\t(7 "B.SilkS" user "B.Silkscreen")
\t\t(1 "F.Mask" user)
\t\t(3 "B.Mask" user)
\t\t(25 "Edge.Cuts" user)
\t)
\t(setup (pad_to_mask_clearance 0))
\t(dimension
\t\t(type aligned)
\t\t(layer "F.SilkS")
\t\t(uuid "22222222-2222-2222-2222-222222222226")
\t\t(pts (xy 0 0) (xy 10 0))
\t\t(height 2)
\t\t(format
\t\t\t(prefix "")
\t\t\t(suffix "")
\t\t\t(units 2)
\t\t\t(units_format 1)
\t\t\t(precision 4)
\t\t\t(override_value "{text}")
\t\t)
\t\t(style
\t\t\t(thickness 0.15)
\t\t\t(arrow_length 1.27)
\t\t\t(text_position_mode 0)
\t\t\t(arrow_direction outward)
\t\t\t(extension_height 0.6)
\t\t\t(extension_offset 0)
\t\t)
\t\t(gr_text "{nested_text}"
\t\t\t(at {nested_x:g} {nested_y:g} 0)
\t\t\t(layer "F.SilkS")
\t\t\t(uuid "11111111-1111-1111-1111-111111111117")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 1.4 1.4) (thickness 0.14))
\t\t\t\t(justify left top)
\t\t\t)
\t\t)
\t)
)
""",
        encoding="utf-8",
    )


def _write_auto_thickness_dimension_text_board(
    source: Path,
    text: str,
) -> None:
    source.write_text(
        f"""(kicad_pcb
\t(version 20241229)
\t(generator "pcbnew")
\t(generator_version "9.0")
\t(general
\t\t(thickness 1.6)
\t\t(legacy_teardrops no)
\t)
\t(paper "A4")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(2 "B.Cu" signal)
\t\t(5 "F.SilkS" user "F.Silkscreen")
\t\t(7 "B.SilkS" user "B.Silkscreen")
\t\t(1 "F.Mask" user)
\t\t(3 "B.Mask" user)
\t\t(25 "Edge.Cuts" user)
\t)
\t(setup (pad_to_mask_clearance 0))
\t(dimension
\t\t(type aligned)
\t\t(layer "F.SilkS")
\t\t(uuid "22222222-2222-2222-2222-222222222230")
\t\t(pts (xy 0 0) (xy 10 0))
\t\t(height 2)
\t\t(format
\t\t\t(prefix "")
\t\t\t(suffix "")
\t\t\t(units 2)
\t\t\t(units_format 1)
\t\t\t(precision 4)
\t\t\t(override_value "{text}")
\t\t)
\t\t(style
\t\t\t(thickness 0.15)
\t\t\t(arrow_length 1.27)
\t\t\t(text_position_mode 0)
\t\t\t(arrow_direction outward)
\t\t\t(extension_height 0.6)
\t\t\t(extension_offset 0)
\t\t)
\t\t(gr_text "STALE"
\t\t\t(at 20 10 0)
\t\t\t(layer "F.SilkS")
\t\t\t(uuid "11111111-1111-1111-1111-111111111121")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 1.27 1.27))
\t\t\t\t(justify left top)
\t\t\t)
\t\t)
\t)
)
""",
        encoding="utf-8",
    )


def _write_orthogonal_dimension_text_board(
    source: Path,
    text: str,
) -> None:
    source.write_text(
        f"""(kicad_pcb
\t(version 20241229)
\t(generator "pcbnew")
\t(generator_version "9.0")
\t(general
\t\t(thickness 1.6)
\t\t(legacy_teardrops no)
\t)
\t(paper "A4")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(2 "B.Cu" signal)
\t\t(5 "F.SilkS" user "F.Silkscreen")
\t\t(7 "B.SilkS" user "B.Silkscreen")
\t\t(1 "F.Mask" user)
\t\t(3 "B.Mask" user)
\t\t(25 "Edge.Cuts" user)
\t)
\t(setup (pad_to_mask_clearance 0))
\t(dimension
\t\t(type orthogonal)
\t\t(layer "F.SilkS")
\t\t(uuid "22222222-2222-2222-2222-222222222227")
\t\t(pts (xy 0 0) (xy 10 10))
\t\t(height 2)
\t\t(orientation 1)
\t\t(format
\t\t\t(prefix "")
\t\t\t(suffix "")
\t\t\t(units 2)
\t\t\t(units_format 1)
\t\t\t(precision 4)
\t\t\t(override_value "{text}")
\t\t)
\t\t(style
\t\t\t(thickness 0.15)
\t\t\t(arrow_length 1.27)
\t\t\t(text_position_mode 0)
\t\t\t(arrow_direction outward)
\t\t\t(extension_height 0.6)
\t\t\t(extension_offset 0)
\t\t\t(keep_text_aligned yes)
\t\t)
\t\t(gr_text "STALE"
\t\t\t(at 20 10 0)
\t\t\t(layer "F.SilkS")
\t\t\t(uuid "11111111-1111-1111-1111-111111111118")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 1.4 1.4) (thickness 0.14))
\t\t\t\t(justify left top)
\t\t\t)
\t\t)
\t)
)
""",
        encoding="utf-8",
    )


def _write_radial_dimension_text_board(
    source: Path,
    text: str,
) -> None:
    source.write_text(
        f"""(kicad_pcb
\t(version 20241229)
\t(generator "pcbnew")
\t(generator_version "9.0")
\t(general
\t\t(thickness 1.6)
\t\t(legacy_teardrops no)
\t)
\t(paper "A4")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(2 "B.Cu" signal)
\t\t(5 "F.SilkS" user "F.Silkscreen")
\t\t(7 "B.SilkS" user "B.Silkscreen")
\t\t(1 "F.Mask" user)
\t\t(3 "B.Mask" user)
\t\t(25 "Edge.Cuts" user)
\t)
\t(setup (pad_to_mask_clearance 0))
\t(dimension
\t\t(type radial)
\t\t(layer "F.SilkS")
\t\t(uuid "22222222-2222-2222-2222-222222222228")
\t\t(pts (xy 0 0) (xy 10 0))
\t\t(leader_length 4)
\t\t(format
\t\t\t(prefix "")
\t\t\t(suffix "")
\t\t\t(units 2)
\t\t\t(units_format 0)
\t\t\t(precision 4)
\t\t\t(override_value "{text}")
\t\t)
\t\t(style
\t\t\t(thickness 0.15)
\t\t\t(arrow_length 1.27)
\t\t\t(text_position_mode 0)
\t\t\t(extension_offset 0)
\t\t\t(keep_text_aligned yes)
\t\t)
\t\t(gr_text "STALE"
\t\t\t(at 14 6 0)
\t\t\t(layer "F.SilkS")
\t\t\t(uuid "11111111-1111-1111-1111-111111111119")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 1.4 1.4) (thickness 0.14))
\t\t\t\t(justify left top)
\t\t\t)
\t\t)
\t)
)
""",
        encoding="utf-8",
    )


def _write_leader_dimension_text_board(
    source: Path,
    text: str,
) -> None:
    source.write_text(
        f"""(kicad_pcb
\t(version 20241229)
\t(generator "pcbnew")
\t(generator_version "9.0")
\t(general
\t\t(thickness 1.6)
\t\t(legacy_teardrops no)
\t)
\t(paper "A4")
\t(layers
\t\t(0 "F.Cu" signal)
\t\t(2 "B.Cu" signal)
\t\t(5 "F.SilkS" user "F.Silkscreen")
\t\t(7 "B.SilkS" user "B.Silkscreen")
\t\t(1 "F.Mask" user)
\t\t(3 "B.Mask" user)
\t\t(25 "Edge.Cuts" user)
\t)
\t(setup (pad_to_mask_clearance 0))
\t(dimension
\t\t(type leader)
\t\t(layer "F.SilkS")
\t\t(uuid "22222222-2222-2222-2222-222222222229")
\t\t(pts (xy 0 0) (xy 5 5))
\t\t(format
\t\t\t(prefix "")
\t\t\t(suffix "")
\t\t\t(units 2)
\t\t\t(units_format 0)
\t\t\t(precision 4)
\t\t\t(override_value "{text}")
\t\t)
\t\t(style
\t\t\t(thickness 0.15)
\t\t\t(arrow_length 1.27)
\t\t\t(text_position_mode 0)
\t\t\t(extension_offset 0)
\t\t\t(text_frame 0)
\t\t)
\t\t(gr_text "STALE"
\t\t\t(at 10 5 0)
\t\t\t(layer "F.SilkS")
\t\t\t(uuid "11111111-1111-1111-1111-111111111120")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 1.4 1.4) (thickness 0.14))
\t\t\t\t(justify left top)
\t\t\t)
\t\t)
\t)
)
""",
        encoding="utf-8",
    )


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
@pytest.mark.parametrize(
    ("text", "case_name"),
    [
        ("TE", "straight"),
        ("S", "curved"),
        ("O", "holed"),
    ],
)
def test_python_render_cache_generator_matches_kicad_oracle_for_outline_glyphs(
    tmp_path: Path,
    text: str,
    case_name: str,
):
    source = tmp_path / f"render_cache_python_generator_{case_name}.kicad_pcb"
    _write_outline_text_board(source, text)

    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / "oracle",
    )
    pcb = KiCadPcb.from_file(source)
    request = render_cache_request_for_board_text(
        pcb.gr_texts[0],
        pcb,
        include_text_params=True,
    )
    generated = RenderCacheResolver().ensure_cache(request)

    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
@pytest.mark.skipif(not _ARIAL.exists(), reason="Arial font not installed")
@pytest.mark.parametrize(
    (
        "board_text",
        "semantic_text",
        "font_style",
        "case_name",
        "line_spacing",
        "angle",
        "justify",
        "horizontal_alignment",
        "vertical_alignment",
        "mirrored",
    ),
    [
        ("TE", "TE", "", "straight", 1.0, 0.0, "left top", "left", "top", False),
        ("S", "S", "", "curved", 1.0, 0.0, "left top", "left", "top", False),
        ("O", "O", "", "holed", 1.0, 0.0, "left top", "left", "top", False),
        (
            "S\\nS",
            "S\nS",
            "",
            "multiline",
            1.0,
            0.0,
            "left top",
            "left",
            "top",
            False,
        ),
        ("S\\tS", "S\tS", "", "tab", 1.0, 0.0, "left top", "left", "top", False),
        (
            "S\\nS",
            "S\nS",
            "(line_spacing 2.0)",
            "line_spacing_2",
            2.0,
            0.0,
            "left top",
            "left",
            "top",
            False,
        ),
        (
            "S\\nSS",
            "S\nSS",
            "",
            "multiline_center",
            1.0,
            0.0,
            None,
            "center",
            "center",
            False,
        ),
        (
            "S\\nS",
            "S\nS",
            "",
            "multiline_rotated_mirror",
            1.0,
            37.0,
            "left top mirror",
            "left",
            "top",
            True,
        ),
        (
            "S\\nSS",
            "S\nSS",
            "",
            "multiline_right_bottom",
            1.0,
            0.0,
            "right bottom",
            "right",
            "bottom",
            False,
        ),
        (
            "AV To",
            "AV To",
            "",
            "kerning_right",
            1.0,
            0.0,
            "right top",
            "right",
            "top",
            False,
        ),
        (
            "S S",
            "S S",
            "",
            "space_right",
            1.0,
            0.0,
            "right top",
            "right",
            "top",
            False,
        ),
        (
            "V_{IN}",
            "V_{IN}",
            "",
            "markup_subscript",
            1.0,
            0.0,
            "left top",
            "left",
            "top",
            False,
        ),
        (
            "X^{2}",
            "X^{2}",
            "",
            "markup_superscript",
            1.0,
            0.0,
            "left top",
            "left",
            "top",
            False,
        ),
        # "AV" keeps this case scoped to overbar markup: double-hole glyphs
        # such as "B" expose a separate, pre-existing fracture-order gap.
        (
            "~{AV}",
            "~{AV}",
            "",
            "markup_overbar",
            1.0,
            0.0,
            "left top",
            "left",
            "top",
            False,
        ),
        (
            "~{S}",
            "~{S}",
            "",
            "markup_overbar_rotated_mirror",
            1.0,
            37.0,
            "left top mirror",
            "left",
            "top",
            True,
        ),
    ],
)
def test_native_render_cache_matches_kicad_save_oracle_for_outline_glyphs(
    tmp_path: Path,
    board_text: str,
    semantic_text: str,
    font_style: str,
    case_name: str,
    line_spacing: float,
    angle: float,
    justify: str | None,
    horizontal_alignment: str,
    vertical_alignment: str,
    mirrored: bool,
):
    source = tmp_path / f"render_cache_native_{case_name}.kicad_pcb"
    _write_outline_text_board(
        source,
        board_text,
        font_style=font_style,
        angle=angle,
        justify=justify,
    )
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / "oracle_native",
    )

    font_bytes = _ARIAL.read_bytes()
    # Arial's OpenType head table uses 2048 units per em. Keeping this explicit
    # makes the live system-font precondition and shaping scale independently visible.
    units_per_em = 2048
    request_path = tmp_path / "native_request.json"
    request_path.write_text(
        json.dumps(
            {
                "shaping": {
                    "font_id": "windows_arial_regular",
                    "font_sha256": hashlib.sha256(font_bytes).hexdigest(),
                    "face_index": 0,
                    "variations": [],
                    "text": semantic_text,
                    "text_index_unit": "utf8_byte_offset",
                    "scale_x": units_per_em,
                    "scale_y": units_per_em,
                    "direction": "left_to_right",
                    "script": "Latn",
                    "language": "en",
                    "features": [],
                    "buffer_properties": {
                        "cluster_level": "monotone_graphemes",
                        "beginning_of_text": True,
                        "end_of_text": True,
                        "default_ignorables": "normal",
                        "do_not_insert_dotted_circle": False,
                        "produce_unsafe_to_concat": False,
                        "produce_safe_to_insert_tatweel": False,
                    },
                },
                "size_x": 2.0,
                "size_y": 2.0,
                "position_x": 10.0,
                "position_y": 10.0,
                "angle_degrees": angle,
                "mirrored": mirrored,
                "horizontal_alignment": horizontal_alignment,
                "vertical_alignment": vertical_alignment,
                "line_spacing": line_spacing,
                # The authored board font thickness; it strokes overbar markup.
                "stroke_width": 0.2,
                "max_error": 2.0,
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    completed = subprocess.run(
        [
            "cargo",
            "run",
            "--quiet",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--example",
            "text_render_cache_oracle_gate",
            "--",
            str(_ARIAL),
            str(request_path),
        ],
        cwd=Path(__file__).resolve().parents[2],
        capture_output=True,
        encoding="utf-8",
        timeout=180,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr
    native_cache = RenderCache.from_sexp(parse_sexp(f"(holder {completed.stdout})"))
    assert native_cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        native_cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
@pytest.mark.skipif(not _CONSOLAS.exists(), reason="Consolas font not installed")
def test_python_render_cache_generator_matches_system_font_lookup_oracle(
    tmp_path: Path,
):
    source = tmp_path / "render_cache_python_generator_system_font.kicad_pcb"
    _write_outline_text_board(source, "S", font_face="Consolas")

    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / "oracle_system_font",
    )
    pcb = KiCadPcb.from_file(source)
    request = render_cache_request_for_board_text(
        pcb.gr_texts[0],
        pcb,
        include_text_params=True,
    )
    generated = RenderCacheResolver().ensure_cache(request)

    assert request.font_face == "Consolas"
    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
@pytest.mark.skipif(_WAVENUMBER_FONT is None, reason="Wavenumber font asset not present")
def test_python_render_cache_generator_matches_embedded_font_oracle(
    tmp_path: Path,
):
    assert _WAVENUMBER_FONT is not None
    source = tmp_path / "render_cache_python_generator_embedded_font.kicad_pcb"
    _write_embedded_font_text_board(source, _WAVENUMBER_FONT)

    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / "oracle_embedded_font",
    )
    pcb = KiCadPcb.from_file(source)
    request = render_cache_request_for_board_text(
        pcb.gr_texts[0],
        pcb,
        include_text_params=True,
    )
    generated = RenderCacheResolver().ensure_cache(request)

    assert request.font_face == "Wavenumber"
    assert request.embedded_fonts
    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
@pytest.mark.parametrize(
    ("justify", "angle", "case_name"),
    [
        ("left top", 0.0, "left_top"),
        (None, 0.0, "default_center"),
        ("right bottom", 0.0, "right_bottom"),
        ("left top", 90.0, "rotate_90"),
    ],
)
def test_python_render_cache_generator_matches_board_text_box_oracle(
    tmp_path: Path,
    justify: str | None,
    angle: float,
    case_name: str,
):
    source = tmp_path / f"render_cache_python_generator_text_box_{case_name}.kicad_pcb"
    _write_outline_text_box_board(source, "S", angle=angle, justify=justify)
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / f"oracle_text_box_{case_name}",
    )
    pcb = KiCadPcb.from_file(source)
    request = render_cache_request_for_board_text(
        pcb.gr_text_boxes[0],
        pcb,
        include_text_params=True,
    )
    generated = RenderCacheResolver().ensure_cache(request)

    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
def test_python_render_cache_generator_matches_markup_text_box_wrap_oracle(
    tmp_path: Path,
):
    source = tmp_path / "render_cache_python_generator_text_box_markup_wrap.kicad_pcb"
    _write_outline_text_box_board(source, "~{S S}", end_x=23.7)
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / "oracle_text_box_markup_wrap",
    )
    pcb = KiCadPcb.from_file(source)
    request = render_cache_request_for_board_text(
        pcb.gr_text_boxes[0],
        pcb,
        include_text_params=True,
    )
    generated = RenderCacheResolver().ensure_cache(request)

    assert request.text == "~{S S}"
    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
def test_python_render_cache_generator_matches_polygon_board_text_box_oracle(
    tmp_path: Path,
):
    source = tmp_path / "render_cache_python_generator_polygon_text_box.kicad_pcb"
    _write_polygon_text_box_board(source, "S")
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / "oracle_polygon_text_box",
    )
    pcb = KiCadPcb.from_file(source)
    request = render_cache_request_for_board_text(
        pcb.gr_text_boxes[0],
        pcb,
        include_text_params=True,
    )
    generated = RenderCacheResolver().ensure_cache(request)

    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
def test_python_render_cache_generator_matches_table_cell_oracle(
    tmp_path: Path,
):
    source = tmp_path / "render_cache_python_generator_table_cell.kicad_pcb"
    _write_table_cell_board(source, "S")
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / "oracle_table_cell",
    )
    pcb = KiCadPcb.from_file(source)
    table = pcb.tables[0]
    request = render_cache_request_for_table_cell(
        table.cells[0],
        table,
        pcb,
        include_text_params=True,
    )
    generated = RenderCacheResolver().ensure_cache(request)

    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
def test_python_render_cache_generator_matches_dimension_text_oracle(
    tmp_path: Path,
):
    source = tmp_path / "render_cache_python_generator_dimension_text.kicad_pcb"
    _write_dimension_text_board(source, "S")
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / "oracle_dimension_text",
    )
    pcb = KiCadPcb.from_file(source)
    request = render_cache_request_for_dimension_text(
        pcb.dimensions[0],
        pcb,
        include_text_params=True,
    )
    generated = RenderCacheResolver().ensure_cache(request)

    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
def test_python_render_cache_generator_matches_dimension_auto_text_oracle(
    tmp_path: Path,
):
    source = tmp_path / "render_cache_python_generator_dimension_auto_text.kicad_pcb"
    _write_dimension_text_board(source, "S", resolved_nested_text=False)
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / "oracle_dimension_auto_text",
    )
    pcb = KiCadPcb.from_file(source)
    request = render_cache_request_for_dimension_text(
        pcb.dimensions[0],
        pcb,
        include_text_params=True,
    )
    generated = RenderCacheResolver().ensure_cache(request)

    assert request.text == "S mm"
    assert request.text_params is not None
    assert request.text_params.position_x == pytest.approx(5.0)
    assert request.text_params.position_y == pytest.approx(0.46)
    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
def test_python_render_cache_generator_matches_dimension_auto_thickness_oracle(
    tmp_path: Path,
):
    source = tmp_path / "render_cache_python_generator_dimension_auto_thickness.kicad_pcb"
    _write_auto_thickness_dimension_text_board(source, "S")
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / "oracle_dimension_auto_thickness",
    )
    pcb = KiCadPcb.from_file(source)
    request = render_cache_request_for_dimension_text(
        pcb.dimensions[0],
        pcb,
        include_text_params=True,
    )
    generated = RenderCacheResolver().ensure_cache(request)

    assert request.text == "S mm"
    assert request.text_params is not None
    assert request.text_params.stroke_width == pytest.approx(0.15875)
    assert request.text_params.position_x == pytest.approx(5.0)
    assert request.text_params.position_y == pytest.approx(0.57125)
    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
def test_python_render_cache_generator_matches_orthogonal_dimension_oracle(
    tmp_path: Path,
):
    source = tmp_path / "render_cache_python_generator_dimension_orthogonal.kicad_pcb"
    _write_orthogonal_dimension_text_board(source, "S")
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / "oracle_dimension_orthogonal",
    )
    pcb = KiCadPcb.from_file(source)
    request = render_cache_request_for_dimension_text(
        pcb.dimensions[0],
        pcb,
        include_text_params=True,
    )
    generated = RenderCacheResolver().ensure_cache(request)

    assert request.text == "S mm"
    assert request.text_params is not None
    assert request.text_params.position_x == pytest.approx(0.46)
    assert request.text_params.position_y == pytest.approx(5.0)
    assert request.text_params.angle == pytest.approx(90.0)
    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
def test_python_render_cache_generator_matches_radial_dimension_oracle(
    tmp_path: Path,
):
    source = tmp_path / "render_cache_python_generator_dimension_radial.kicad_pcb"
    _write_radial_dimension_text_board(source, "S")
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / "oracle_dimension_radial",
    )
    pcb = KiCadPcb.from_file(source)
    request = render_cache_request_for_dimension_text(
        pcb.dimensions[0],
        pcb,
        include_text_params=True,
    )
    generated = RenderCacheResolver().ensure_cache(request)

    assert request.text == "S"
    assert request.text_params is not None
    assert request.text_params.position_x == pytest.approx(14.0)
    assert request.text_params.position_y == pytest.approx(6.0)
    assert request.text_params.angle == pytest.approx(90.0)
    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
def test_python_render_cache_generator_matches_leader_dimension_oracle(
    tmp_path: Path,
):
    source = tmp_path / "render_cache_python_generator_dimension_leader.kicad_pcb"
    _write_leader_dimension_text_board(source, "NOTE")
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / "oracle_dimension_leader",
    )
    pcb = KiCadPcb.from_file(source)
    request = render_cache_request_for_dimension_text(
        pcb.dimensions[0],
        pcb,
        include_text_params=True,
    )
    generated = RenderCacheResolver().ensure_cache(request)

    assert request.text == "NOTE"
    assert request.text_params is not None
    assert request.text_params.position_x == pytest.approx(10.0)
    assert request.text_params.position_y == pytest.approx(5.0)
    assert request.text_params.angle == pytest.approx(0.0)
    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
@pytest.mark.parametrize(
    ("angle", "justify", "case_name"),
    [
        (0.0, None, "center_center"),
        (0.0, "right bottom", "right_bottom"),
        (90.0, "left top", "rotate_90"),
        (37.0, "left top", "rotate_37"),
        (0.0, "left top mirror", "mirror"),
    ],
)
def test_python_render_cache_generator_matches_kicad_oracle_for_transforms(
    tmp_path: Path,
    angle: float,
    justify: str | None,
    case_name: str,
):
    source = tmp_path / f"render_cache_python_generator_transform_{case_name}.kicad_pcb"
    _write_outline_text_board(source, "S", angle=angle, justify=justify)
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / f"oracle_transform_{case_name}",
    )
    pcb = KiCadPcb.from_file(source)
    request = render_cache_request_for_board_text(
        pcb.gr_texts[0],
        pcb,
        include_text_params=True,
    )
    generated = RenderCacheResolver().ensure_cache(request)

    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
@pytest.mark.parametrize(
    ("text", "case_name", "font_style"),
    [
        ("S\\nS", "multiline", ""),
        ("S\\tS", "tab", ""),
        ("S\\nS", "line_spacing_2", "(line_spacing 2.0)"),
        ("word word word word word word", "space_delimited_words", ""),
        ("S S S S S S S S S", "repeated_space_cursor", ""),
        ("AV To", "kerning_pairs", ""),
        ("A^{2}", "superscript", ""),
        ("H_{2}", "subscript", ""),
        ("~{S}", "overbar", ""),
    ],
)
def test_python_render_cache_generator_matches_kicad_oracle_for_text_runs(
    tmp_path: Path,
    text: str,
    case_name: str,
    font_style: str,
):
    source = tmp_path / f"render_cache_python_generator_runs_{case_name}.kicad_pcb"
    _write_outline_text_board(source, text, font_style=font_style)
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / f"oracle_runs_{case_name}",
    )
    pcb = KiCadPcb.from_file(source)
    request = render_cache_request_for_board_text(
        pcb.gr_texts[0],
        pcb,
        include_text_params=True,
    )
    generated = RenderCacheResolver().ensure_cache(request)

    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
@pytest.mark.parametrize(
    ("font_style", "case_name"),
    [
        ("(bold yes)", "bold"),
        ("(italic yes)", "italic"),
        ("(bold yes) (italic yes)", "bold_italic"),
    ],
)
def test_python_render_cache_generator_matches_kicad_oracle_for_font_styles(
    tmp_path: Path,
    font_style: str,
    case_name: str,
):
    source = tmp_path / f"render_cache_python_generator_style_{case_name}.kicad_pcb"
    _write_outline_text_board(source, "S", font_style)
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / f"oracle_style_{case_name}",
    )
    pcb = KiCadPcb.from_file(source)
    request = render_cache_request_for_board_text(
        pcb.gr_texts[0],
        pcb,
        include_text_params=True,
    )
    generated = RenderCacheResolver().ensure_cache(request)

    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
@pytest.mark.parametrize("object_kind", ["fp_text", "property", "fp_text_box"])
@pytest.mark.parametrize(
    ("footprint_at", "footprint_layer", "text_layer", "justify", "case_name"),
    [
        ((0.0, 0.0, 0.0), "F.Cu", "F.SilkS", "left top", "origin"),
        ((30.0, 40.0, 30.0), "F.Cu", "F.SilkS", "left top", "rotated_footprint"),
        ((30.0, 40.0, -30.0), "B.Cu", "B.SilkS", "left top mirror", "back_mirrored"),
    ],
)
def test_python_render_cache_generator_matches_footprint_text_oracle(
    tmp_path: Path,
    object_kind: str,
    footprint_at: tuple[float, float, float],
    footprint_layer: str,
    text_layer: str,
    justify: str,
    case_name: str,
):
    source = tmp_path / f"render_cache_python_generator_{object_kind}_{case_name}.kicad_pcb"
    _write_footprint_outline_text_board(
        source,
        object_kind,
        "S",
        footprint_at=footprint_at,
        footprint_layer=footprint_layer,
        text_layer=text_layer,
        justify=justify,
    )
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / f"oracle_{object_kind}",
    )
    pcb = KiCadPcb.from_file(source)
    footprint = pcb.footprints[0]

    if object_kind == "fp_text":
        request = render_cache_request_for_footprint_text(
            footprint.fp_texts[0],
            footprint,
            include_text_params=True,
        )
    elif object_kind == "property":
        request = render_cache_request_for_footprint_property(
            footprint.properties[0],
            footprint,
            include_text_params=True,
        )
    else:
        request = render_cache_request_for_footprint_text_box(
            footprint.fp_text_boxes[0],
            footprint,
            include_text_params=True,
        )

    generated = RenderCacheResolver().ensure_cache(request)

    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison


@pytest.mark.skipif(_CLI is None, reason="PCB-capable kicad-cli not resolvable")
def test_python_render_cache_generator_matches_polygon_footprint_text_box_oracle(
    tmp_path: Path,
):
    source = tmp_path / "render_cache_python_generator_polygon_fp_text_box.kicad_pcb"
    _write_polygon_footprint_text_box_board(source, "S")
    oracle = run_kicad_pcb_render_cache_save_oracle(
        kicad_cli=_CLI,
        source_pcb=source,
        work_dir=tmp_path / "oracle_polygon_fp_text_box",
    )
    pcb = KiCadPcb.from_file(source)
    footprint = pcb.footprints[0]
    request = render_cache_request_for_footprint_text_box(
        footprint.fp_text_boxes[0],
        footprint,
        include_text_params=True,
    )
    generated = RenderCacheResolver().ensure_cache(request)

    assert generated.usable
    assert generated.cache is not None
    comparison = compare_render_caches(
        oracle.entries[0].cache,
        generated.cache,
        tolerance=0.002,
    )
    assert comparison.matched, comparison
