"""Render-cache resolver and validation tests."""

from __future__ import annotations

from pathlib import Path
import re

import pytest

from kicad_monkey.kicad_primitives import (
    RenderCache,
    RenderCacheContour,
    RenderCachePolygon,
)
from kicad_monkey.kicad_pcb import KiCadPcb
from kicad_monkey.kicad_render_cache import (
    RenderCacheRequest,
    RenderCacheResolver,
    RenderCacheSource,
    generate_render_cache_from_text_params,
    render_cache_exterior_polygons,
    render_cache_request_for_board_text,
    render_cache_request_for_dimension_text,
    render_cache_request_for_footprint_property,
    render_cache_request_for_footprint_text,
    render_cache_request_for_footprint_text_box,
    render_cache_request_for_table_cell,
)
from kicad_monkey.kicad_render_cache_oracle import (
    RenderCacheOracleEntry,
    build_render_cache_coverage_report_from_pcb,
    compare_render_cache_entry_sets,
    compare_render_caches,
    collect_render_cache_requests_from_pcb,
    extract_render_cache_entries_from_pcb,
    summarize_render_cache_entries,
    summarize_render_cache_requests,
    strip_render_cache_blocks,
)


def _uses_windows_arial_for_generated_cache() -> bool:
    from kicad_monkey.kicad_text import KiCadTextRenderer

    path = KiCadTextRenderer()._find_font_file("Arial")
    return path is not None and Path(path).name.casefold() == "arial.ttf"


def _cache(text: str = "TXT", angle: float = 0.0) -> RenderCache:
    return RenderCache(
        text=text,
        angle=angle,
        polygons=[
            RenderCachePolygon(
                contours=[
                    RenderCacheContour(
                        points=[
                            (0.0, 0.0),
                            (1.0, 0.0),
                            (1.0, 1.0),
                            (0.0, 1.0),
                        ]
                    ),
                    RenderCacheContour(
                        points=[
                            (0.25, 0.25),
                            (0.75, 0.25),
                            (0.75, 0.75),
                            (0.25, 0.75),
                        ]
                    ),
                ]
            )
        ],
    )


def test_existing_render_cache_is_usable_when_text_and_angle_match():
    resolver = RenderCacheResolver()
    result = resolver.ensure_cache(
        RenderCacheRequest(text="TXT", angle=0.0, render_cache=_cache())
    )

    assert result.usable
    assert result.exact
    assert result.source == RenderCacheSource.EXISTING_FILE_CACHE
    assert result.validation.reasons == []


def test_existing_render_cache_is_rejected_when_resolved_text_differs():
    resolver = RenderCacheResolver()
    result = resolver.ensure_cache(RenderCacheRequest(text="OTHER", render_cache=_cache()))

    assert not result.usable
    assert result.cache is None
    assert result.source == RenderCacheSource.INVALID_EXISTING_CACHE
    assert result.validation.reasons == ["resolved_text_mismatch"]


def test_stale_render_cache_regenerates_when_text_params_are_available():
    from kicad_monkey.kicad_geometry import HAlign, TextParams, VAlign

    resolver = RenderCacheResolver()
    result = resolver.ensure_cache(
        RenderCacheRequest(
            text="OK",
            angle=0.0,
            render_cache=_cache(text="STALE"),
            text_params=TextParams(
                text="OK",
                font_name="Arial",
                size_x=1.0,
                size_y=1.0,
                position_x=0.0,
                position_y=0.0,
                angle=0.0,
                h_align=HAlign.LEFT,
                v_align=VAlign.TOP,
            ),
        )
    )

    assert result.usable
    assert result.source == RenderCacheSource.PYTHON_GENERATED_CACHE
    assert result.cache is not None
    assert result.cache.text == "OK"


def test_existing_render_cache_is_rejected_when_angle_differs():
    resolver = RenderCacheResolver()
    result = resolver.ensure_cache(
        RenderCacheRequest(text="TXT", angle=90.0, render_cache=_cache())
    )

    assert not result.usable
    assert "angle_mismatch" in result.validation.reasons


def test_existing_render_cache_is_usable_but_not_exact_when_angle_is_unknown():
    resolver = RenderCacheResolver()
    result = resolver.ensure_cache(
        RenderCacheRequest(text="TXT", render_cache=_cache(angle=45.0))
    )

    assert result.usable
    assert not result.exact
    assert result.validation.reasons == []
    assert result.validation.warnings == ["angle_context_not_provided"]


def test_missing_render_cache_is_reported_explicitly():
    resolver = RenderCacheResolver()
    result = resolver.ensure_cache(RenderCacheRequest(text="TXT"))

    assert not result.usable
    assert result.source == RenderCacheSource.MISSING
    assert result.validation.reasons == ["missing_cache"]


def test_flat_svg_consumer_gets_exterior_contours_from_usable_cache():
    polygons = render_cache_exterior_polygons(text="TXT", render_cache=_cache(angle=45.0))

    assert polygons == [[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]]


def test_flat_svg_consumer_rejects_explicit_angle_mismatch():
    polygons = render_cache_exterior_polygons(
        text="TXT",
        angle=0.0,
        render_cache=_cache(angle=45.0),
    )

    assert polygons == []


def test_flat_svg_consumer_rejects_stale_cache():
    polygons = render_cache_exterior_polygons(text="OTHER", render_cache=_cache())

    assert polygons == []


def test_strip_render_cache_blocks_removes_nested_caches():
    text = """(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(gr_text "TXT"
\t\t(at 1 2 0)
\t\t(layer "F.SilkS")
\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t(render_cache "TXT" 0
\t\t\t(polygon (pts (xy 0 0) (xy 1 0) (xy 1 1)))
\t\t)
\t)
)
"""

    stripped = strip_render_cache_blocks(text)

    assert "render_cache" not in stripped
    assert "gr_text" in stripped
    assert "TXT" in stripped


def test_extract_render_cache_entries_collects_text_and_text_box_caches():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(gr_text "TXT"
\t\t(at 1 2 0)
\t\t(layer "F.SilkS")
\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t(render_cache "TXT" 0
\t\t\t(polygon (pts (xy 0 0) (xy 1 0) (xy 1 1)))
\t\t)
\t)
\t(gr_text_box "BOX"
\t\t(start 1 1)
\t\t(end 3 2)
\t\t(margins 0.1 0.1 0.1 0.1)
\t\t(layer "F.SilkS")
\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t(render_cache "BOX" 0
\t\t\t(polygon (pts (xy 2 2) (xy 3 2) (xy 3 3)))
\t\t)
\t)
\t(table
\t\t(column_count 1)
\t\t(layer "F.SilkS")
\t\t(border (external no) (header no))
\t\t(separators (rows no) (cols no))
\t\t(column_widths 10)
\t\t(row_heights 4)
\t\t(cells
\t\t\t(table_cell "CELL"
\t\t\t\t(start 5 5)
\t\t\t\t(end 15 9)
\t\t\t\t(margins 0.1 0.1 0.1 0.1)
\t\t\t\t(span 1 1)
\t\t\t\t(layer "F.SilkS")
\t\t\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t\t\t(render_cache "CELL" 0
\t\t\t\t\t(polygon (pts (xy 4 4) (xy 5 4) (xy 5 5)))
\t\t\t\t)
\t\t\t)
\t\t)
\t)
\t(dimension
\t\t(type aligned)
\t\t(layer "F.SilkS")
\t\t(uuid "dimension-uuid")
\t\t(pts (xy 0 0) (xy 10 0))
\t\t(height 2)
\t\t(format (prefix "") (suffix "") (units 2) (units_format 1) (precision 4))
\t\t(style
\t\t\t(thickness 0.15)
\t\t\t(arrow_length 1.27)
\t\t\t(text_position_mode 0)
\t\t\t(arrow_direction outward)
\t\t\t(extension_height 0.6)
\t\t\t(extension_offset 0)
\t\t)
\t\t(gr_text "DIM"
\t\t\t(at 5 2 0)
\t\t\t(layer "F.SilkS")
\t\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t\t(render_cache "DIM" 0
\t\t\t\t(polygon (pts (xy 6 6) (xy 7 6) (xy 7 7)))
\t\t\t)
\t\t)
\t)
)
""")

    entries = extract_render_cache_entries_from_pcb(pcb)

    assert [(entry.object_type, entry.text) for entry in entries] == [
        ("gr_text", "TXT"),
        ("gr_text_box", "BOX"),
        ("table_cell", "CELL"),
        ("dimension", "DIM"),
    ]


def test_summarize_render_cache_entries_reports_object_and_topology_histograms():
    entries = [
        RenderCacheOracleEntry(
            object_path="gr_text[0]",
            object_type="gr_text",
            text="TXT",
            layer="F.SilkS",
            cache=_cache("TXT"),
        ),
        RenderCacheOracleEntry(
            object_path="property[0:Reference]",
            object_type="property",
            text="REF",
            layer="F.Fab",
            cache=_cache("REF"),
        ),
    ]

    summary = summarize_render_cache_entries(entries)

    assert summary.entry_count == 2
    assert summary.object_type_counts == {"gr_text": 1, "property": 1}
    assert summary.layer_counts == {"F.Fab": 1, "F.SilkS": 1}
    assert summary.polygon_count == 2
    assert summary.contour_count == 4
    assert summary.hole_polygon_count == 2
    assert summary.max_contours_per_polygon == 2
    assert summary.missing_object_types(["gr_text", "fp_text"]) == ["fp_text"]


def test_render_cache_coverage_report_tracks_states_and_gaps():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(gr_text "STROKE"
\t\t(at 1 1 0)
\t\t(layer "F.SilkS")
\t\t(effects (font (size 1 1) (thickness 0.1)))
\t)
\t(gr_text "GENERATE"
\t\t(at 4 1 0)
\t\t(layer "F.SilkS")
\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t)
\t(gr_text "FRESH"
\t\t(at 8 1 0)
\t\t(layer "F.SilkS")
\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t(render_cache "STALE" 0
\t\t\t(polygon (pts (xy 99 99) (xy 100 99) (xy 100 100)))
\t\t)
\t)
)
""")

    requests = collect_render_cache_requests_from_pcb(pcb)
    summary = summarize_render_cache_requests(requests)
    report = build_render_cache_coverage_report_from_pcb(pcb, source_path="synthetic.kicad_pcb")

    assert summary["object_count"] == 3
    assert summary["histograms"]["font_kind"] == {"outline_font": 2, "stroke_default": 1}
    assert summary["histograms"]["existing_cache_state"] == {
        "missing": 2,
        "present_invalid": 1,
    }
    assert summary["histograms"]["resolved_cache_source"] == {
        "missing_cache": 1,
        "python_generated_cache": 2,
    }
    assert summary["gap_count"] == 0
    assert report["source_path"] == "synthetic.kicad_pcb"
    assert report["histograms"]["object_type"] == {"gr_text": 3}


def test_empty_text_objects_collect_requests_and_accept_empty_caches():
    # KiCad saves a polygon-less render_cache for empty outline-font texts
    # (blank fp_text placeholders and Datasheet properties in the corpus),
    # so empty texts still yield requests, an existing empty file cache
    # validates, and the generator emits a matching empty cache.
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(footprint "Test:Empty"
\t\t(layer "F.Cu")
\t\t(at 0 0)
\t\t(fp_text user ""
\t\t\t(at 1 1 90)
\t\t\t(layer "F.SilkS")
\t\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t\t(render_cache "" 90)
\t\t)
\t)
)
""")

    requests = [
        request
        for request in collect_render_cache_requests_from_pcb(pcb)
        if request.object_path.endswith("/fp_text[0]")
    ]
    assert len(requests) == 1
    request = requests[0]
    assert request.text == ""

    existing = RenderCacheResolver().ensure_cache(request)
    assert existing.usable
    assert existing.source == RenderCacheSource.EXISTING_FILE_CACHE
    assert existing.cache is not None
    assert not existing.cache.polygons

    generated = RenderCacheResolver().generate_cache(request)
    assert generated.usable
    assert generated.cache is not None
    assert generated.cache.text == ""
    assert generated.cache.angle == 90.0
    assert not generated.cache.polygons


def test_board_text_request_resolves_board_variables_and_known_angle():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(property "PCB_PART_NUMBER" "11-10043")
\t(gr_text "${PCB_PART_NUMBER}"
\t\t(at 1 2 90)
\t\t(layer "F.SilkS")
\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t(render_cache "11-10043" 90
\t\t\t(polygon (pts (xy 0 0) (xy 1 0) (xy 1 1)))
\t\t)
\t)
)
""")

    request = render_cache_request_for_board_text(
        pcb.gr_texts[0],
        pcb,
        object_path="gr_text[0]",
    )
    result = RenderCacheResolver().ensure_cache(request)

    assert request.text == "11-10043"
    assert request.angle == 90.0
    assert request.object_type == "GrText"
    assert request.object_path == "gr_text[0]"
    assert result.usable
    assert not result.exact
    assert result.validation.warnings == ["font_context_not_serialized_in_kicad_cache"]


def test_footprint_text_requests_resolve_display_text_without_claiming_angle_exactness():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(footprint "Test:CacheText"
\t\t(layer "F.Cu")
\t\t(at 10 20 180)
\t\t(property "Reference" "D5"
\t\t\t(at 0 0 180)
\t\t\t(layer "F.SilkS")
\t\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t)
\t\t(property "Value" "LED"
\t\t\t(at 0 1 0)
\t\t\t(layer "F.SilkS")
\t\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t)
\t\t(property "UserLabel" "${Reference}-${Value}"
\t\t\t(at 0 2 0)
\t\t\t(layer "F.SilkS")
\t\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t\t(render_cache "D5-LED" 0
\t\t\t\t(polygon (pts (xy 0 0) (xy 1 0) (xy 1 1)))
\t\t\t)
\t\t)
\t\t(fp_text reference "REF**"
\t\t\t(at 0 3 180)
\t\t\t(layer "F.SilkS")
\t\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t\t(render_cache "D5" 0
\t\t\t\t(polygon (pts (xy 0 0) (xy 1 0) (xy 1 1)))
\t\t\t)
\t\t)
\t\t(fp_text_box "${Reference}"
\t\t\t(start 0 0)
\t\t\t(end 5 2)
\t\t\t(margins 0 0 0 0)
\t\t\t(layer "F.SilkS")
\t\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t\t(render_cache "D5" 0
\t\t\t\t(polygon (pts (xy 0 0) (xy 1 0) (xy 1 1)))
\t\t\t)
\t\t)
\t)
)
""")
    footprint = pcb.footprints[0]
    resolver = RenderCacheResolver()

    text_request = render_cache_request_for_footprint_text(
        footprint.fp_texts[0],
        footprint,
        object_path="footprint[0]/fp_text[0]",
    )
    property_request = render_cache_request_for_footprint_property(
        footprint.properties[2],
        footprint,
        object_path="footprint[0]/property[2]",
    )
    text_box_request = render_cache_request_for_footprint_text_box(
        footprint.fp_text_boxes[0],
        footprint,
        object_path="footprint[0]/fp_text_box[0]",
    )

    assert text_request.text == "D5"
    assert property_request.text == "D5-LED"
    assert text_box_request.text == "D5"
    assert text_request.angle is None
    assert property_request.angle is None
    assert text_box_request.angle is None

    for request in [text_request, property_request, text_box_request]:
        result = resolver.ensure_cache(request)
        assert result.usable
        assert not result.exact
        assert result.validation.warnings == [
            "angle_context_not_provided",
            "font_context_not_serialized_in_kicad_cache",
        ]


def test_compare_render_caches_reports_geometry_deltas_and_mismatches():
    expected = _cache("TXT", angle=0.0)
    actual = _cache("TXT", angle=0.0)
    actual.polygons[0].contours[0].points[1] = (1.00001, 0.0)

    within = compare_render_caches(expected, actual, tolerance=0.0001)
    outside = compare_render_caches(expected, actual, tolerance=0.000001)
    text_mismatch = compare_render_caches(expected, _cache("OTHER", angle=0.0))
    angle_mismatch = compare_render_caches(expected, _cache("TXT", angle=90.0))

    assert within.matched
    assert within.compared_points == 8
    assert within.max_point_delta > 0.0
    assert not outside.matched
    assert "point_delta_exceeds_tolerance:polygon=0:contour=0" in outside.reasons
    assert text_mismatch.reasons == ["cache_text_mismatch"]
    assert angle_mismatch.reasons == ["cache_angle_mismatch"]


def test_compare_render_cache_entry_sets_matches_by_uuid_and_reports_gaps():
    expected = [
        RenderCacheOracleEntry(
            object_path="gr_text[0]",
            object_type="gr_text",
            text="TXT",
            layer="F.SilkS",
            uuid="11111111-1111-1111-1111-111111111111",
            cache=_cache("TXT"),
        ),
        RenderCacheOracleEntry(
            object_path="gr_text[1]",
            object_type="gr_text",
            text="MISSING",
            layer="F.SilkS",
            uuid="22222222-2222-2222-2222-222222222222",
            cache=_cache("MISSING"),
        ),
    ]
    actual = [
        RenderCacheOracleEntry(
            object_path="gr_text[0]",
            object_type="gr_text",
            text="TXT",
            layer="F.SilkS",
            uuid="11111111-1111-1111-1111-111111111111",
            cache=_cache("TXT"),
        ),
        RenderCacheOracleEntry(
            object_path="gr_text[2]",
            object_type="gr_text",
            text="EXTRA",
            layer="F.SilkS",
            uuid="33333333-3333-3333-3333-333333333333",
            cache=_cache("EXTRA"),
        ),
    ]

    comparison = compare_render_cache_entry_sets(expected, actual)

    assert not comparison.matched
    assert comparison.missing_keys == ["22222222-2222-2222-2222-222222222222"]
    assert comparison.extra_keys == ["33333333-3333-3333-3333-333333333333"]
    assert comparison.entry_results["11111111-1111-1111-1111-111111111111"].matched


def test_python_render_cache_generator_trims_duplicate_contour_closure():
    from kicad_monkey.kicad_geometry import HAlign, TextParams, VAlign

    params = TextParams(
        text="TE",
        font_name="Arial",
        size_x=2.0,
        size_y=2.0,
        position_x=10.0,
        position_y=10.0,
        angle=0.0,
        h_align=HAlign.LEFT,
        v_align=VAlign.TOP,
        layer="F.SilkS",
    )

    cache = generate_render_cache_from_text_params(params)

    assert cache.text == "TE"
    assert cache.angle == 0.0
    polygon_sizes = [len(poly.points) for poly in cache.polygons]
    if _uses_windows_arial_for_generated_cache():
        assert polygon_sizes == [8, 12]
    else:
        assert polygon_sizes
        assert all(size > 3 for size in polygon_sizes)
    assert all(poly.points[0] != poly.points[-1] for poly in cache.polygons)


def test_python_render_cache_generator_fractures_holed_outline_glyphs():
    from kicad_monkey.kicad_geometry import HAlign, TextParams, VAlign

    params = TextParams(
        text="O",
        font_name="Arial",
        size_x=2.0,
        size_y=2.0,
        position_x=10.0,
        position_y=10.0,
        angle=0.0,
        h_align=HAlign.LEFT,
        v_align=VAlign.TOP,
        layer="F.SilkS",
    )

    cache = generate_render_cache_from_text_params(params)

    assert cache.text == "O"
    if _uses_windows_arial_for_generated_cache():
        assert len(cache.polygons) == 1
        assert len(cache.polygons[0].points) == 79
    else:
        assert cache.polygons
        assert any(len(poly.points) > 8 for poly in cache.polygons)


def test_python_render_cache_generator_emits_overbar_stroke_polygon():
    from kicad_monkey.kicad_geometry import HAlign, TextParams, VAlign

    params = TextParams(
        text="~{S}",
        font_name="Arial",
        size_x=2.0,
        size_y=2.0,
        position_x=10.0,
        position_y=10.0,
        angle=0.0,
        h_align=HAlign.LEFT,
        v_align=VAlign.TOP,
        stroke_width=0.2,
        layer="F.SilkS",
    )

    cache = generate_render_cache_from_text_params(params)

    assert cache.text == "~{S}"
    if _uses_windows_arial_for_generated_cache():
        assert len(cache.polygons) == 2
        assert len(cache.polygons[-1].points) == 28
    else:
        assert len(cache.polygons) >= 2
        assert len(cache.polygons[-1].points) > 3


def test_text_box_linebreaker_keeps_marked_runs_as_single_words():
    from kicad_monkey.kicad_geometry import HAlign, TextParams, VAlign
    from kicad_monkey.kicad_text import KiCadTextRenderer

    params = TextParams(
        text="~{S S}",
        font_name="Arial",
        size_x=1.4,
        size_y=1.4,
        stroke_width=0.14,
        h_align=HAlign.LEFT,
        v_align=VAlign.TOP,
    )

    wrapped = KiCadTextRenderer().linebreak_text(params, column_width=2.5)

    assert wrapped == "~{S S}"


def test_resolver_generates_python_cache_when_text_params_are_available():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(property "PART" "TE")
\t(gr_text "${PART}"
\t\t(at 10 10 0)
\t\t(layer "F.SilkS")
\t\t(effects
\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t(justify left top)
\t\t)
\t)
)
""")
    request = render_cache_request_for_board_text(
        pcb.gr_texts[0],
        pcb,
        include_text_params=True,
    )

    result = RenderCacheResolver().ensure_cache(request)

    assert result.usable
    assert not result.exact
    assert result.source == RenderCacheSource.PYTHON_GENERATED_CACHE
    assert result.cache is not None
    assert result.cache.text == "TE"
    assert result.validation.warnings == ["python_generated_cache_not_kicad_exact"]


def test_pcb_svg_board_text_uses_generated_cache_instead_of_stale_file_cache():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(property "PART" "OK")
\t(gr_text "${PART}"
\t\t(at 10 10 0)
\t\t(layer "F.SilkS")
\t\t(effects
\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t(justify left top)
\t\t)
\t\t(render_cache "STALE" 0
\t\t\t(polygon (pts (xy 500 500) (xy 501 500) (xy 501 501)))
\t\t)
\t)
)
""")

    svg = pcb.to_svg(layers=["F.SilkS"])
    path_data = " ".join(re.findall(r'd="([^"]+)"', svg))
    coords = [float(value) for value in re.findall(r"-?\d+(?:\.\d+)?", path_data)]

    assert coords
    assert max(coords) < 50.0


def test_pcb_svg_board_text_box_uses_generated_cache_instead_of_stale_file_cache():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(property "PART" "OK")
\t(gr_text_box "${PART}"
\t\t(start 10 10)
\t\t(end 30 20)
\t\t(margins 0.5 0.5 0.5 0.5)
\t\t(layer "F.SilkS")
\t\t(effects
\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t(justify left top)
\t\t)
\t\t(border yes)
\t\t(stroke (width 0.15) (type solid))
\t\t(render_cache "STALE" 0
\t\t\t(polygon (pts (xy 500 500) (xy 501 500) (xy 501 501)))
\t\t)
\t)
)
""")

    svg = pcb.to_svg(layers=["F.SilkS"])
    path_data = " ".join(re.findall(r'd="([^"]+)"', svg))
    coords = [float(value) for value in re.findall(r"-?\d+(?:\.\d+)?", path_data)]

    assert coords
    assert max(coords) < 50.0


def test_pcb_svg_footprint_text_uses_generated_cache_instead_of_stale_file_cache():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(footprint "Test:SvgCache"
\t\t(layer "F.Cu")
\t\t(at 10 10 0)
\t\t(property "Reference" "D1"
\t\t\t(at 0 0 0)
\t\t\t(layer "F.SilkS")
\t\t\t(hide yes)
\t\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t)
\t\t(property "UserText" "${Reference}-P"
\t\t\t(at 0 4 0)
\t\t\t(layer "F.SilkS")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t(justify left top)
\t\t\t)
\t\t\t(render_cache "STALE" 0
\t\t\t\t(polygon (pts (xy 500 500) (xy 501 500) (xy 501 501)))
\t\t\t)
\t\t)
\t\t(fp_text reference "REF**"
\t\t\t(at 0 0 0)
\t\t\t(layer "F.SilkS")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t(justify left top)
\t\t\t)
\t\t\t(render_cache "STALE" 0
\t\t\t\t(polygon (pts (xy 500 500) (xy 501 500) (xy 501 501)))
\t\t\t)
\t\t)
\t\t(fp_text_box "${Reference}"
\t\t\t(start 0 8)
\t\t\t(end 20 16)
\t\t\t(margins 0.5 0.5 0.5 0.5)
\t\t\t(layer "F.SilkS")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t(justify left top)
\t\t\t)
\t\t\t(border yes)
\t\t\t(stroke (width 0.15) (type solid))
\t\t\t(render_cache "STALE" 0
\t\t\t\t(polygon (pts (xy 500 500) (xy 501 500) (xy 501 501)))
\t\t\t)
\t\t)
\t)
)
""")

    svg = pcb.to_svg(layers=["F.SilkS"])
    path_data = " ".join(re.findall(r'd="([^"]+)"', svg))
    coords = [float(value) for value in re.findall(r"-?\d+(?:\.\d+)?", path_data)]

    assert coords
    assert max(coords) < 50.0


def test_pcb_svg_table_cell_uses_generated_cache_instead_of_stale_file_cache():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(table
\t\t(column_count 1)
\t\t(layer "F.SilkS")
\t\t(border (external no) (header no))
\t\t(separators (rows no) (cols no))
\t\t(column_widths 20)
\t\t(row_heights 10)
\t\t(cells
\t\t\t(table_cell "${ADDR}:T"
\t\t\t\t(start 10 10)
\t\t\t\t(end 30 20)
\t\t\t\t(margins 0.5 0.5 0.5 0.5)
\t\t\t\t(span 1 1)
\t\t\t\t(layer "F.SilkS")
\t\t\t\t(effects
\t\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t\t(justify left top)
\t\t\t\t)
\t\t\t\t(render_cache "STALE" 0
\t\t\t\t\t(polygon (pts (xy 500 500) (xy 501 500) (xy 501 501)))
\t\t\t\t)
\t\t\t)
\t\t)
\t)
)
""")

    svg = pcb.to_svg(layers=["F.SilkS"])
    path_data = " ".join(re.findall(r'd="([^"]+)"', svg))
    coords = [float(value) for value in re.findall(r"-?\d+(?:\.\d+)?", path_data)]

    assert coords
    assert max(coords) < 50.0


def test_pcb_svg_table_cell_layer_can_render_without_table_layer():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user) (38 "B.SilkS" user))
\t(table
\t\t(column_count 1)
\t\t(layer "F.SilkS")
\t\t(border (external yes))
\t\t(separators (rows no) (cols no))
\t\t(column_widths 20)
\t\t(row_heights 10)
\t\t(cells
\t\t\t(table_cell "BACK"
\t\t\t\t(start 10 10)
\t\t\t\t(end 30 20)
\t\t\t\t(margins 0.5 0.5 0.5 0.5)
\t\t\t\t(span 1 1)
\t\t\t\t(layer "B.SilkS")
\t\t\t\t(effects
\t\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t\t(justify left top)
\t\t\t\t)
\t\t\t)
\t\t)
\t)
)
""")

    svg = pcb.to_svg(layers=["B.SilkS"])

    assert "<path" in svg
    assert ">BACK<" not in svg


def test_pcb_svg_dimension_text_uses_generated_cache_instead_of_stale_file_cache():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(gr_line (start 0 0) (end 1 0) (stroke (width 0.1) (type solid)) (layer "F.SilkS"))
\t(dimension
\t\t(type aligned)
\t\t(layer "F.SilkS")
\t\t(pts (xy 0 0) (xy 10 0))
\t\t(height 2)
\t\t(format
\t\t\t(prefix "")
\t\t\t(suffix "")
\t\t\t(units 2)
\t\t\t(units_format 1)
\t\t\t(precision 4)
\t\t\t(override_value "OK")
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
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t(justify left top)
\t\t\t)
\t\t\t(render_cache "STALE" 0
\t\t\t\t(polygon (pts (xy 500 500) (xy 501 500) (xy 501 501)))
\t\t\t)
\t\t)
\t)
)
""")

    svg = pcb.to_svg(layers=["F.SilkS"])
    path_data = " ".join(re.findall(r'd="([^"]+)"', svg))
    coords = [float(value) for value in re.findall(r"-?\d+(?:\.\d+)?", path_data)]

    assert svg.count("<path") > 1
    assert max(coords) < 50.0


def test_pcb_svg_dimension_text_controls_viewbox_without_other_graphics():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(dimension
\t\t(type aligned)
\t\t(layer "F.SilkS")
\t\t(pts (xy 0 0) (xy 10 0))
\t\t(height 2)
\t\t(format
\t\t\t(prefix "")
\t\t\t(suffix "")
\t\t\t(units 2)
\t\t\t(units_format 1)
\t\t\t(precision 4)
\t\t\t(override_value "OK")
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
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t(justify left top)
\t\t\t)
\t\t)
\t)
)
""")

    svg = pcb.to_svg(layers=["F.SilkS"])
    match = re.search(r'viewBox="[^ ]+ [^ ]+ ([^ ]+) ([^"]+)"', svg)

    assert match is not None
    assert float(match.group(1)) > 1.0
    assert float(match.group(2)) > 1.0
    assert 'viewBox="0 0 0 0"' not in svg
    assert "<path" in svg


def test_resolver_generates_board_text_box_cache_from_text_params():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(property "PART" "S")
\t(gr_text_box "${PART}"
\t\t(start 20 10)
\t\t(end 50 22)
\t\t(margins 0.6 0.6 0.6 0.6)
\t\t(layer "F.SilkS")
\t\t(effects
\t\t\t(font (face "Arial") (size 1.4 1.4) (thickness 0.14))
\t\t\t(justify left top)
\t\t)
\t\t(border yes)
\t\t(stroke (width 0.15) (type solid))
\t)
)
""")
    request = render_cache_request_for_board_text(
        pcb.gr_text_boxes[0],
        pcb,
        include_text_params=True,
    )

    result = RenderCacheResolver().ensure_cache(request)

    assert result.usable
    assert result.cache is not None
    assert request.text == "S"
    assert request.text_params is not None
    assert request.text_params.position_x == pytest.approx(20.525)
    assert request.text_params.position_y == pytest.approx(10.525)
    assert request.text_params.h_align == 0
    assert request.text_params.v_align == 0


def test_resolver_generates_table_cell_cache_from_text_params_and_variables():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(table
\t\t(column_count 1)
\t\t(layer "F.SilkS")
\t\t(border (external no) (header no))
\t\t(separators (rows no) (cols no))
\t\t(column_widths 30)
\t\t(row_heights 12)
\t\t(cells
\t\t\t(table_cell "${ADDR}:${ROW}:${COL}:${LAYER}"
\t\t\t\t(start 20 10)
\t\t\t\t(end 50 22)
\t\t\t\t(margins 0.6 0.6 0.6 0.6)
\t\t\t\t(span 1 1)
\t\t\t\t(layer "F.SilkS")
\t\t\t\t(effects
\t\t\t\t\t(font (face "Arial") (size 1.4 1.4) (thickness 0.14))
\t\t\t\t\t(justify left top)
\t\t\t\t)
\t\t\t)
\t\t)
\t)
)
""")
    table = pcb.tables[0]
    cell = table.cells[0]
    request = render_cache_request_for_table_cell(
        cell,
        table,
        pcb,
        include_text_params=True,
    )

    result = RenderCacheResolver().ensure_cache(request)

    assert result.usable
    assert result.cache is not None
    assert request.text == "A1:1:1:F.SilkS"
    assert request.text_params is not None
    assert request.text_params.position_x == pytest.approx(20.6)
    assert request.text_params.position_y == pytest.approx(10.6)
    assert result.source == RenderCacheSource.PYTHON_GENERATED_CACHE


def test_resolver_generates_dimension_text_cache_from_resolved_nested_gr_text():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(property "DIM_LABEL" "S")
\t(dimension
\t\t(type aligned)
\t\t(layer "F.SilkS")
\t\t(uuid "dimension-uuid")
\t\t(pts (xy 0 0) (xy 10 0))
\t\t(height 2)
\t\t(format
\t\t\t(prefix "")
\t\t\t(suffix "")
\t\t\t(units 2)
\t\t\t(units_format 1)
\t\t\t(precision 4)
\t\t\t(override_value "S")
\t\t)
\t\t(style
\t\t\t(thickness 0.15)
\t\t\t(arrow_length 1.27)
\t\t\t(text_position_mode 0)
\t\t\t(arrow_direction outward)
\t\t\t(extension_height 0.6)
\t\t\t(extension_offset 0)
\t\t)
\t\t(gr_text "${DIM_LABEL}"
\t\t\t(at 20 10 0)
\t\t\t(layer "F.SilkS")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 1.4 1.4) (thickness 0.14))
\t\t\t\t(justify left top)
\t\t\t)
\t\t)
\t)
)
""")
    request = render_cache_request_for_dimension_text(
        pcb.dimensions[0],
        pcb,
        include_text_params=True,
    )

    result = RenderCacheResolver().ensure_cache(request)

    assert result.usable
    assert result.cache is not None
    assert request.text == "S mm"
    assert request.text_params is not None
    assert request.text_params.position_x == pytest.approx(5.0)
    assert request.text_params.position_y == pytest.approx(0.46)
    assert result.source == RenderCacheSource.PYTHON_GENERATED_CACHE


def test_resolver_uses_kicad_auto_thickness_for_dimension_text_placement():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(dimension
\t\t(type aligned)
\t\t(layer "F.SilkS")
\t\t(uuid "dimension-uuid")
\t\t(pts (xy 0 0) (xy 10 0))
\t\t(height 2)
\t\t(format
\t\t\t(prefix "")
\t\t\t(suffix "")
\t\t\t(units 2)
\t\t\t(units_format 1)
\t\t\t(precision 4)
\t\t\t(override_value "S")
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
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 1.27 1.27))
\t\t\t\t(justify left top)
\t\t\t)
\t\t)
\t)
)
""")
    request = render_cache_request_for_dimension_text(
        pcb.dimensions[0],
        pcb,
        include_text_params=True,
    )

    result = RenderCacheResolver().ensure_cache(request)

    assert result.usable
    assert result.cache is not None
    assert request.text_params is not None
    assert request.text_params.stroke_width == pytest.approx(0.15875)
    assert request.text_params.position_x == pytest.approx(5.0)
    assert request.text_params.position_y == pytest.approx(0.57125)
    assert result.source == RenderCacheSource.PYTHON_GENERATED_CACHE


def test_resolver_generates_orthogonal_dimension_text_cache_from_source_geometry():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(dimension
\t\t(type orthogonal)
\t\t(layer "F.SilkS")
\t\t(uuid "dimension-uuid")
\t\t(pts (xy 0 0) (xy 10 10))
\t\t(height 2)
\t\t(orientation 1)
\t\t(format
\t\t\t(prefix "")
\t\t\t(suffix "")
\t\t\t(units 2)
\t\t\t(units_format 1)
\t\t\t(precision 4)
\t\t\t(override_value "S")
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
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 1.4 1.4) (thickness 0.14))
\t\t\t\t(justify left top)
\t\t\t)
\t\t)
\t)
)
""")
    request = render_cache_request_for_dimension_text(
        pcb.dimensions[0],
        pcb,
        include_text_params=True,
    )

    result = RenderCacheResolver().ensure_cache(request)

    assert result.usable
    assert result.cache is not None
    assert request.text == "S mm"
    assert request.text_params is not None
    assert request.text_params.position_x == pytest.approx(0.46)
    assert request.text_params.position_y == pytest.approx(5.0)
    assert request.text_params.angle == pytest.approx(90.0)
    assert result.source == RenderCacheSource.PYTHON_GENERATED_CACHE


def test_resolver_generates_radial_dimension_text_cache_from_source_geometry():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(dimension
\t\t(type radial)
\t\t(layer "F.SilkS")
\t\t(uuid "dimension-uuid")
\t\t(pts (xy 0 0) (xy 10 0))
\t\t(leader_length 4)
\t\t(format
\t\t\t(prefix "")
\t\t\t(suffix "")
\t\t\t(units 2)
\t\t\t(units_format 0)
\t\t\t(precision 4)
\t\t\t(override_value "S")
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
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 1.4 1.4) (thickness 0.14))
\t\t\t\t(justify left top)
\t\t\t)
\t\t)
\t)
)
""")
    request = render_cache_request_for_dimension_text(
        pcb.dimensions[0],
        pcb,
        include_text_params=True,
    )

    result = RenderCacheResolver().ensure_cache(request)

    assert result.usable
    assert result.cache is not None
    assert request.text == "S"
    assert request.text_params is not None
    assert request.text_params.position_x == pytest.approx(14.0)
    assert request.text_params.position_y == pytest.approx(6.0)
    assert request.text_params.angle == pytest.approx(90.0)
    assert result.source == RenderCacheSource.PYTHON_GENERATED_CACHE


def test_resolver_generates_leader_dimension_text_cache_from_source_geometry():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(dimension
\t\t(type leader)
\t\t(layer "F.SilkS")
\t\t(uuid "dimension-uuid")
\t\t(pts (xy 0 0) (xy 5 5))
\t\t(format
\t\t\t(prefix "")
\t\t\t(suffix "")
\t\t\t(units 2)
\t\t\t(units_format 0)
\t\t\t(precision 4)
\t\t\t(override_value "NOTE")
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
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 1.4 1.4) (thickness 0.14))
\t\t\t\t(justify left top)
\t\t\t)
\t\t)
\t)
)
""")
    request = render_cache_request_for_dimension_text(
        pcb.dimensions[0],
        pcb,
        include_text_params=True,
    )

    result = RenderCacheResolver().ensure_cache(request)

    assert result.usable
    assert result.cache is not None
    assert request.text == "NOTE"
    assert request.text_params is not None
    assert request.text_params.position_x == pytest.approx(10.0)
    assert request.text_params.position_y == pytest.approx(5.0)
    assert request.text_params.angle == pytest.approx(0.0)
    assert result.source == RenderCacheSource.PYTHON_GENERATED_CACHE


def test_resolver_generates_footprint_text_and_property_caches_from_text_params():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(footprint "Test:CacheText"
\t\t(layer "F.Cu")
\t\t(at 0 0 0)
\t\t(property "Reference" "D5"
\t\t\t(at 0 0 0)
\t\t\t(layer "F.SilkS")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t(justify left top)
\t\t\t)
\t\t)
\t\t(property "UserLabel" "${Reference}"
\t\t\t(at 0 4 0)
\t\t\t(layer "F.SilkS")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t(justify left top)
\t\t\t)
\t\t)
\t\t(fp_text reference "REF**"
\t\t\t(at 0 8 0)
\t\t\t(layer "F.SilkS")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 2 2) (thickness 0.2))
\t\t\t\t(justify left top)
\t\t\t)
\t\t)
\t)
)
""")
    footprint = pcb.footprints[0]
    property_request = render_cache_request_for_footprint_property(
        footprint.properties[1],
        footprint,
        include_text_params=True,
    )
    text_request = render_cache_request_for_footprint_text(
        footprint.fp_texts[0],
        footprint,
        include_text_params=True,
    )

    property_result = RenderCacheResolver().ensure_cache(property_request)
    text_result = RenderCacheResolver().ensure_cache(text_request)

    assert property_result.usable
    assert text_result.usable
    assert property_result.cache is not None
    assert text_result.cache is not None
    assert property_result.cache.text == "D5"
    assert text_result.cache.text == "D5"
    assert property_result.source == RenderCacheSource.PYTHON_GENERATED_CACHE
    assert text_result.source == RenderCacheSource.PYTHON_GENERATED_CACHE


def test_resolver_generates_footprint_text_box_cache_from_text_params():
    pcb = KiCadPcb.from_string("""(kicad_pcb
\t(version 20240108)
\t(generator "pcbnew")
\t(layers (0 "F.Cu" signal) (37 "F.SilkS" user))
\t(footprint "Test:CacheText"
\t\t(layer "F.Cu")
\t\t(at 0 0 0)
\t\t(property "Reference" "D5"
\t\t\t(at 0 0 0)
\t\t\t(layer "F.SilkS")
\t\t\t(effects (font (face "Arial") (size 1 1) (thickness 0.1)))
\t\t)
\t\t(fp_text_box "${Reference}"
\t\t\t(start 20 10)
\t\t\t(end 50 22)
\t\t\t(margins 0.6 0.6 0.6 0.6)
\t\t\t(layer "F.SilkS")
\t\t\t(effects
\t\t\t\t(font (face "Arial") (size 1.4 1.4) (thickness 0.14))
\t\t\t\t(justify left top)
\t\t\t)
\t\t\t(border yes)
\t\t\t(stroke (width 0.15) (type solid))
\t\t)
\t)
)
""")
    footprint = pcb.footprints[0]
    request = render_cache_request_for_footprint_text_box(
        footprint.fp_text_boxes[0],
        footprint,
        include_text_params=True,
    )

    result = RenderCacheResolver().ensure_cache(request)

    assert result.usable
    assert result.cache is not None
    assert result.cache.text == "D5"
    assert request.text_params is not None
    assert request.text_params.position_x == pytest.approx(20.525)
    assert request.text_params.position_y == pytest.approx(10.525)
