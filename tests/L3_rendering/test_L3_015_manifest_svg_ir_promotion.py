"""L3 manifest-driven SVG/IR promotion checks."""

from __future__ import annotations

import html
import json
import re
from pathlib import Path
from typing import Any

import pytest

from kicad_monkey.testing.corpus import (
    get_kicad_corpus_case,
    get_kicad_corpus_root,
    iter_kicad_corpus_cases,
    load_kicad_corpus_manifest,
    resolve_kicad_manifest_path,
)


def _require_manifest():
    try:
        load_kicad_corpus_manifest(required=True)
    except Exception as exc:
        pytest.skip(f"KiCad corpus manifest unavailable: {exc}")


def _slug(value: str) -> str:
    return re.sub(r"[^A-Za-z0-9_.-]+", "_", value).strip("_")


def _schematic_source_counts(entries: list[Any]) -> dict[str, int]:
    counts: dict[str, int] = {}
    for entry in entries:
        schematic = entry.schematic
        source = getattr(schematic, "source_path", None)
        key = str(Path(source).resolve() if source else id(schematic))
        counts[key] = counts.get(key, 0) + 1
    return counts


def _schematic_output_name(entry: Any, source_path: Path, counts: dict[str, int]) -> str:
    key = str(source_path.resolve()) if source_path else str(id(entry.schematic))
    if counts.get(key, 0) > 1 and entry.sheet_name:
        return entry.sheet_name
    return source_path.stem


def _clear_output_files(path: Path, suffix: str) -> None:
    path.mkdir(parents=True, exist_ok=True)
    for output_file in path.glob(f"*{suffix}"):
        output_file.unlink()


_SCHEMATIC_ENRICHMENT_RE = re.compile(
    r'<metadata id="schematic-enrichment-a0"[^>]*>(.*?)</metadata>',
    re.DOTALL,
)

_SVG_GROUP_RE = re.compile(r"<g\b(?P<attrs>[^>]*)>", re.DOTALL)
_SVG_ATTR_RE = re.compile(r'([A-Za-z_:][-A-Za-z0-9_:.]*)="([^"]*)"')

def _schematic_enrichment_payload(svg: str) -> dict:
    match = _SCHEMATIC_ENRICHMENT_RE.search(svg)
    assert match is not None, f"Schematic enrichment metadata not found in:\n{svg[:800]}"
    return json.loads(html.unescape(match.group(1)))


def _svg_group_attrs(svg: str) -> list[dict[str, str]]:
    return [
        {name: html.unescape(value) for name, value in _SVG_ATTR_RE.findall(match.group("attrs"))}
        for match in _SVG_GROUP_RE.finditer(svg)
    ]


def _assert_schematic_svg_net_linkage(svg: str, svg_payload: dict) -> None:
    design_payload = svg_payload["design"]
    assert design_payload["schema"] == "kicad_monkey.design.a0"

    indexes = design_payload["indexes"]
    view_indexes = svg_payload["view_indexes"]
    svg_to_nets = indexes["svg_to_nets"]
    sheet_svg_to_nets = indexes["sheet_svg_to_nets"]
    if not svg_to_nets:
        assert not indexes["net_to_graphics"]
        assert not sheet_svg_to_nets
        assert not view_indexes["svg_to_net"]
        assert not view_indexes["svg_to_nets"]
        return
    assert svg_to_nets
    assert sheet_svg_to_nets
    assert indexes["net_to_graphics"]
    assert {
        "sheet_lookup_keys",
        "svg_to_net",
        "svg_to_nets",
        "net_to_svg",
        "net_uid_to_svg",
    }.issubset(view_indexes)

    pin_link_count = 0
    graphic_link_count = 0
    for net in design_payload["nets"]:
        net_name = net["name"]
        graphical = net["graphical"]
        for key, values in graphical.items():
            if key == "pins":
                continue
            for svg_id in values:
                assert net_name in svg_to_nets[svg_id]
                graphic_link_count += 1
        for pin in graphical["pins"]:
            svg_id = pin["svg_id"]
            assert net_name in svg_to_nets[svg_id]
            if pin.get("source_pin_id"):
                assert net_name in svg_to_nets[pin["source_pin_id"]]
            pin_link_count += 1

    assert graphic_link_count + pin_link_count

    sheet_keys = view_indexes["sheet_lookup_keys"]
    rendered_ids: set[str] = set()
    for attrs in _svg_group_attrs(svg):
        svg_id = attrs.get("id") or attrs.get("data-element-key") or attrs.get("data-uuid")
        if svg_id:
            rendered_ids.add(svg_id)
    declared_sheet_ids = {
        svg_id
        for sheet_key in sheet_keys
        for svg_id in sheet_svg_to_nets.get(sheet_key, {})
    }

    linked_group_count = 0
    for svg_id in sorted(rendered_ids & declared_sheet_ids):
        candidates = view_indexes["svg_to_nets"].get(svg_id, [])
        assert candidates
        assert all(candidate["name"] for candidate in candidates)
        if len(candidates) == 1:
            assert view_indexes["svg_to_net"][svg_id]["name"] == candidates[0]["name"]
            net_uid = view_indexes["svg_to_net"][svg_id]["uid"]
            if net_uid:
                assert svg_id in view_indexes["net_uid_to_svg"][net_uid]
        for candidate in candidates:
            assert svg_id in view_indexes["net_to_svg"][candidate["name"]]
        linked_group_count += 1

    if declared_sheet_ids:
        assert linked_group_count


def _real_world_svg_ir_cases() -> list[dict]:
    try:
        return list(
            iter_kicad_corpus_cases(
                domain="schematic_ir",
                origin="real_world",
                status="active",
                required=True,
            )
        )
    except Exception:
        return []


def _real_world_board_cases() -> list[dict]:
    try:
        return list(
            iter_kicad_corpus_cases(
                domain="board_svg",
                origin="real_world",
                status="active",
                required=True,
            )
        )
    except Exception:
        return []


REAL_WORLD_SVG_IR_CASES = _real_world_svg_ir_cases()
REAL_WORLD_BOARD_CASES = _real_world_board_cases()


def _synthetic_schematic_svg_cases() -> list[dict]:
    try:
        return list(
            iter_kicad_corpus_cases(
                domain="schematic_svg",
                origin="synthetic",
                status="active",
                required=True,
            )
        )
    except Exception:
        return []


def _public_library_cases(domain: str) -> list[dict]:
    try:
        return list(
            iter_kicad_corpus_cases(
                domain=domain,
                origin="public_library",
                status="active",
                required=True,
            )
        )
    except Exception:
        return []


SYNTHETIC_SCHEMATIC_SVG_CASES = _synthetic_schematic_svg_cases()
PUBLIC_SYMBOL_SVG_CASES = _public_library_cases("symbol_svg")
PUBLIC_FOOTPRINT_SVG_CASES = _public_library_cases("footprint_svg")


def _recorder_drift_cases() -> list[dict]:
    try:
        return list(
            iter_kicad_corpus_cases(
                domain="schematic_recorder_drift",
                status="active",
                required=True,
            )
        )
    except Exception:
        return []


RECORDER_DRIFT_CASES = _recorder_drift_cases()


def test_taillight_repeated_child_schematic_instances_from_manifest():
    from kicad_monkey import KiCadDesign

    _require_manifest()
    case = get_kicad_corpus_case("real_world/taillight")
    assert case is not None
    project_path = resolve_kicad_manifest_path(case, "project_file")
    assert project_path is not None

    design = KiCadDesign.from_project_file(project_path)
    instances = design.schematic_instances()
    child_instances = design.child_schematic_instances(instances[0])

    assert len(instances) == 6
    assert len(child_instances) == 5
    assert {instance.sheet_name for instance in child_instances} == {
        "LED_Controller2",
        "LED_Controller3",
        "LED_Controller4",
        "LED_Controller5",
        "LED_Controller6",
    }
    assert {
        Path(instance.source_path).name
        for instance in child_instances
        if instance.source_path is not None
    } == {"LED_Controller.kicad_sch"}
    assert len({instance.sheet_instance_path for instance in child_instances}) == 5
    assert len({instance.sheet_path_uuids for instance in child_instances}) == 5
    assert all(design.parent_schematic_instance(instance) == instances[0] for instance in child_instances)

    led_source = child_instances[0].source_path
    assert led_source is not None
    assert [
        instance.sheet_path
        for instance in design.schematic_instances_for(led_source)
    ] == [
        instance.sheet_path
        for instance in child_instances
    ]


RECORDER_STATE_KINDS = {
    "SetColor",
    "SetCurrentLineWidth",
    "SetDash",
    "SetViewport",
    "SetPageSettings",
    "StartPlot",
    "EndPlot",
    "StartBlock",
    "EndBlock",
    "PenTo",
}
GEOMETRY_OP_KINDS = {
    "ArcThreePoint",
    "BezierCurve",
    "Circle",
    "FlashPadCircle",
    "FlashPadCustom",
    "FlashPadOval",
    "FlashPadRect",
    "FlashPadRoundRect",
    "FlashPadTrapez",
    "FlashRegularPolygon",
    "PlotPoly",
    "Rect",
    "Text",
    "ThickArc",
    "ThickSegment",
}
PAD_FLASH_OP_KINDS = {
    "FlashPadCircle",
    "FlashPadCustom",
    "FlashPadOval",
    "FlashPadRect",
    "FlashPadRoundRect",
    "FlashPadTrapez",
    "FlashRegularPolygon",
}


def _enum_value(value: object) -> str:
    enum_value = getattr(value, "value", None)
    return str(enum_value) if enum_value is not None else str(value)


def _pad_expects_flash_op(pad: object) -> bool:
    pad_type = _enum_value(getattr(pad, "pad_type", ""))
    if pad_type != "np_thru_hole":
        return True

    shape = _enum_value(getattr(pad, "shape", ""))
    if shape != "circle":
        return True

    drill = getattr(pad, "drill", None)
    if drill is None:
        return True

    size_x = float(getattr(pad, "size_x", 0.0) or 0.0)
    size_y = float(getattr(pad, "size_y", 0.0) or 0.0)
    return max(size_x, size_y) > float(drill)


def _op_kind(op) -> str:
    return op.kind.value if hasattr(op.kind, "value") else str(op.kind)


def _op_hist(doc) -> dict[str, int]:
    out: dict[str, int] = {}
    for record in doc.records:
        for op in record.operations:
            kind = _op_kind(op)
            out[kind] = out.get(kind, 0) + 1
    return out


def _op_total(doc) -> int:
    return sum(len(record.operations) for record in doc.records)


def _svg_rendered_shape_count(svg: str) -> int:
    background_rects = len(
        re.findall(r'<rect\s+x="0"\s+y="0"[^>]*fill="#[Ff][Ff][Ff][Ff][Ff][Ff]"', svg)
    )
    return (
        len(re.findall(r"<(?:path|polyline|line|polygon|circle)\b", svg))
        + max(len(re.findall(r"<rect\b", svg)) - background_rects, 0)
    )


def _pcb_has_renderable_content(pcb) -> bool:
    """Return whether a board carries PCB objects that should emit IR records."""
    collections = (
        "gr_lines",
        "gr_arcs",
        "gr_circles",
        "gr_rects",
        "gr_polys",
        "gr_curves",
        "gr_texts",
        "gr_text_boxes",
        "images",
        "tables",
        "dimensions",
        "segments",
        "arcs",
        "vias",
        "zones",
        "footprints",
    )
    return any(len(getattr(pcb, name, []) or []) for name in collections)


@pytest.mark.parametrize("case", REAL_WORLD_SVG_IR_CASES, ids=lambda case: case["name"])
def test_promoted_real_world_all_sheets_render_to_ir_and_svg_from_manifest(case):
    from kicad_monkey import (
        KiCadDesign,
        render_ir_to_svg,
        validate_schematic_svg_compiled_graph_view,
    )
    from kicad_monkey.kicad_sch_svg_renderer import KiCadSvgRenderOptions
    from kicad_monkey.kicad_schematic_svg_enrichment import (
        KICAD_SCHEMATIC_SVG_ENRICHMENT_SCHEMA,
        schematic_root_svg_attrs,
        schematic_svg_enrichment_metadata_element,
        schematic_svg_enrichment_payload,
    )

    project_path = resolve_kicad_manifest_path(case, "project_file")
    output_root = resolve_kicad_manifest_path(case, "output_root")
    assert project_path is not None and output_root is not None

    ir_out = output_root / "schematic_ir"
    svg_out = output_root / "schematic_svg"
    _clear_output_files(ir_out, ".json")
    _clear_output_files(svg_out, ".svg")

    design = KiCadDesign.from_project_file(project_path)
    entries = design.schematic_instances()
    assert entries
    source_counts = _schematic_source_counts(entries)
    design_payload = design.to_json(include_indexes=True)
    render_options = KiCadSvgRenderOptions.enriched_default()

    for index, entry in enumerate(entries, start=1):
        schematic = entry.schematic
        source_path = Path(schematic.source_path) if schematic.source_path else Path(f"sheet_{index}")
        document_id = schematic.uuid or f"{source_path.stem}_{index}"
        doc = design.to_schematic_ir(
            schematic=schematic,
            sheet_index=entry.sheet_number,
            sheet_count=len(entries),
            sheet_path=entry.sheet_path,
            sheet_instance_path=entry.sheet_instance_path,
            sheet_name=entry.sheet_name or source_path.stem,
            document_id=document_id,
        )
        assert doc.records, source_path.name
        assert any(record.kind in {"symbol_instance", "sheet_header"} for record in doc.records)

        profile_obj: Any = render_options.profile
        profile_value = str(getattr(profile_obj, "value", profile_obj))
        payload = schematic_svg_enrichment_payload(
            design_payload,
            source_path=source_path,
            sheet_name=entry.sheet_name,
            sheet_path=entry.sheet_path,
            sheet_instance_path=entry.sheet_instance_path,
            profile=profile_value,
            compiled_schematic_graph=design_payload["compiled_schematic_graph"],
            schematic_instance=entry,
            compiled_graph_artifact="../compiled_schematic_graph.json",
        )
        graph_view = payload["compiled_schematic_graph_view"]
        svg = render_ir_to_svg(
            doc,
            options=render_options,
            root_extra_attrs=schematic_root_svg_attrs(
                source_path=source_path,
                sheet_name=entry.sheet_name,
                sheet_path=entry.sheet_path,
                profile=profile_value,
                compiled_graph_view=graph_view,
            ),
            metadata_elements=[schematic_svg_enrichment_metadata_element(payload)],
        )
        assert svg.startswith("<?xml")
        assert "<svg" in svg and "</svg>" in svg
        assert "data-ref=\"sheet_header\"" in svg
        assert f'data-enrichment-schema="{KICAD_SCHEMATIC_SVG_ENRICHMENT_SCHEMA}"' in svg
        validate_schematic_svg_compiled_graph_view(
            svg,
            design_payload["compiled_schematic_graph"],
            graph_view,
        )
        svg_payload = _schematic_enrichment_payload(svg)
        assert svg_payload["schema"] == KICAD_SCHEMATIC_SVG_ENRICHMENT_SCHEMA
        assert "components" in svg_payload["design"]
        assert "nets" in svg_payload["design"]
        _assert_schematic_svg_net_linkage(svg, svg_payload)

        output_name = _schematic_output_name(entry, source_path, source_counts)
        stem = f"{index:02d}_{_slug(output_name)}"
        (ir_out / f"{stem}.json").write_text(
            json.dumps(doc.to_normalized_dict(source_path=source_path.name), indent=2) + "\n",
            encoding="utf-8",
        )
        (svg_out / f"{stem}.svg").write_text(svg, encoding="utf-8")

    if case.get("name") == "speedy_processing_module":
        svg_names = {path.name for path in svg_out.glob("*.svg")}
        assert any("TPS62A02_BUCK_1V0" in name for name in svg_names)
        assert not any(re.match(r"\d+_TPS62A02_BUCK\.svg$", name) for name in svg_names)
        sample_payload = _schematic_enrichment_payload(
            next(svg_out.glob("*TPS62A02_BUCK_1V0.svg")).read_text(encoding="utf-8")
        )
        assert sample_payload["design"]["components"]
        assert sample_payload["design"]["nets"]
        assert sample_payload["view_indexes"]["svg_to_net"]


@pytest.mark.parametrize(
    "case",
    SYNTHETIC_SCHEMATIC_SVG_CASES,
    ids=lambda case: case["name"],
)
def test_synthetic_schematic_cases_render_to_ir_and_svg_from_manifest(case):
    from kicad_monkey import (
        KiCadSchematic,
        render_ir_to_svg,
        render_schematic_svg,
        schematic_to_ir,
    )

    input_file = resolve_kicad_manifest_path(case, "input_file")
    output_root = resolve_kicad_manifest_path(case, "output_root")
    assert input_file is not None and output_root is not None

    sch = KiCadSchematic.from_file(input_file)
    doc = schematic_to_ir(
        sch,
        source_path=input_file.name,
        document_id=sch.uuid or input_file.stem,
        sheet_name=input_file.stem,
    )
    assert doc.records
    assert any(record.kind in {"symbol_instance", "wire", "label"} for record in doc.records)

    ir_svg = render_ir_to_svg(doc)
    public_svg = render_schematic_svg(sch)
    assert "<svg" in ir_svg and "</svg>" in ir_svg
    assert "<svg" in public_svg and "</svg>" in public_svg

    ir_out = output_root / "schematic_ir"
    svg_out = output_root / "schematic_svg"
    ir_out.mkdir(parents=True, exist_ok=True)
    svg_out.mkdir(parents=True, exist_ok=True)
    (ir_out / f"{case['name']}.json").write_text(
        json.dumps(doc.to_normalized_dict(source_path=input_file.name), indent=2) + "\n",
        encoding="utf-8",
    )
    (svg_out / f"{case['name']}__ir.svg").write_text(ir_svg, encoding="utf-8")
    (svg_out / f"{case['name']}__public.svg").write_text(public_svg, encoding="utf-8")


@pytest.mark.parametrize("case", REAL_WORLD_BOARD_CASES, ids=lambda case: case["name"])
def test_promoted_real_world_board_renders_review_layers_from_manifest(case):
    from kicad_monkey import KiCadPcb, pcb_to_ir

    board_path = resolve_kicad_manifest_path(case, "board_file")
    output_root = resolve_kicad_manifest_path(case, "output_root")
    assert board_path is not None and output_root is not None

    pcb = KiCadPcb.from_file(board_path)
    doc = pcb_to_ir(pcb, source_path=board_path.name, document_id=case["name"])
    hist = _op_hist(doc)

    svg = pcb.to_svg(layers=["F.Cu", "B.Cu", "Edge.Cuts"])
    assert svg.startswith("<?xml")
    assert "<svg" in svg and "</svg>" in svg

    ir_dir = output_root / "board_ir"
    out_dir = output_root / "board_svg"
    ir_dir.mkdir(parents=True, exist_ok=True)
    out_dir.mkdir(parents=True, exist_ok=True)

    if not doc.records:
        # Some promoted schematic-documentation projects intentionally carry an
        # empty PCB only so KiCad treats the folder as a complete project.
        assert not _pcb_has_renderable_content(pcb), board_path
        assert len(svg) > 100
        (ir_dir / f"{case['name']}.json").write_text(
            json.dumps(doc.to_normalized_dict(source_path=board_path.name), indent=2) + "\n",
            encoding="utf-8",
        )
        (out_dir / f"{case['name']}__F_Cu__B_Cu__Edge_Cuts.svg").write_text(
            svg,
            encoding="utf-8",
        )
        return

    assert sum(hist.get(kind, 0) for kind in GEOMETRY_OP_KINDS) > 0
    assert _svg_rendered_shape_count(svg) > 0

    (ir_dir / f"{case['name']}.json").write_text(
        json.dumps(doc.to_normalized_dict(source_path=board_path.name), indent=2) + "\n",
        encoding="utf-8",
    )
    (out_dir / f"{case['name']}__F_Cu__B_Cu__Edge_Cuts.svg").write_text(svg, encoding="utf-8")


def test_custom_pads_project_board_ir_covers_custom_and_trapezoid_pads_from_manifest():
    from kicad_monkey import KiCadPcb, pcb_to_ir

    _require_manifest()
    case = get_kicad_corpus_case("project_corpus/common/custom_pads_test/input/custom_pads_test")
    assert case is not None
    board_path = resolve_kicad_manifest_path(case, "board_file")
    output_root = resolve_kicad_manifest_path(case, "output_root")
    assert board_path is not None and output_root is not None

    pcb = KiCadPcb.from_file(board_path)
    doc = pcb_to_ir(pcb, source_path=board_path.name, document_id=case["name"])
    hist = _op_hist(doc)
    assert hist.get("FlashPadCustom", 0) > 0, hist
    assert hist.get("FlashPadTrapez", 0) > 0, hist

    ir_dir = output_root / "board_ir"
    ir_dir.mkdir(parents=True, exist_ok=True)
    (ir_dir / f"{case['name']}.json").write_text(
        json.dumps(doc.to_normalized_dict(source_path=board_path.name), indent=2) + "\n",
        encoding="utf-8",
    )


def test_synthetic_ir_coverage_gap_fixture_writes_remaining_op_kinds():
    from kicad_monkey import (
        KiCadFillType,
        KiCadPlotterDocument,
        KiCadPlotterOp,
        KiCadPlotterRecord,
        render_ir_to_svg,
    )

    _require_manifest()
    ops = [
        KiCadPlotterOp.arc_center_angle(
            cx=20_000_000,
            cy=20_000_000,
            start_angle_deg=0.0,
            sweep_deg=90.0,
            radius_nm=5_000_000,
            fill=KiCadFillType.NO_FILL,
            width_nm=150_000,
        ),
        KiCadPlotterOp.thick_arc(
            cx=40_000_000,
            cy=20_000_000,
            start_angle_deg=180.0,
            sweep_deg=-90.0,
            radius_nm=5_000_000,
            width_nm=300_000,
        ),
        KiCadPlotterOp.flash_pad_custom(
            x=60_000_000,
            y=20_000_000,
            size_x_nm=4_000_000,
            size_y_nm=2_000_000,
            orient_deg=0.0,
            polygons=[
                [
                    [-2_000_000, -1_000_000],
                    [2_000_000, -1_000_000],
                    [0, 1_500_000],
                ]
            ],
        ),
        KiCadPlotterOp.flash_pad_trapez(
            x=80_000_000,
            y=20_000_000,
            corners=[
                [-2_000_000, 1_000_000],
                [2_000_000, 500_000],
                [1_500_000, -1_000_000],
                [-1_500_000, -500_000],
            ],
            orient_deg=0.0,
        ),
        KiCadPlotterOp.flash_reg_polygon(
            x=100_000_000,
            y=20_000_000,
            diameter_nm=5_000_000,
            corner_count=6,
            orient_deg=30.0,
        ),
    ]
    doc = KiCadPlotterDocument(
        source_kind="SYNTHETIC",
        source_path="ir_coverage_gap_fixture",
        document_id="ir_coverage_gap_fixture",
        canvas={"width_nm": 120_000_000, "height_nm": 40_000_000},
        records=[
            KiCadPlotterRecord(
                uuid="ir_coverage_gap_fixture",
                kind="synthetic_ir_coverage",
                object_id="ir_coverage_gap_fixture",
                operations=ops,
            )
        ],
    )
    hist = _op_hist(doc)
    assert {
        "ArcCenterAngle",
        "ThickArc",
        "FlashPadCustom",
        "FlashPadTrapez",
        "FlashRegularPolygon",
    } <= set(hist)

    out_root = get_kicad_corpus_root() / "ir_coverage" / "output"
    ir_dir = out_root / "synthetic_ir"
    svg_dir = out_root / "synthetic_svg"
    ir_dir.mkdir(parents=True, exist_ok=True)
    svg_dir.mkdir(parents=True, exist_ok=True)
    (ir_dir / "ir_coverage_gap_fixture.json").write_text(
        json.dumps(doc.to_normalized_dict(source_path="ir_coverage_gap_fixture"), indent=2) + "\n",
        encoding="utf-8",
    )
    (svg_dir / "ir_coverage_gap_fixture.svg").write_text(render_ir_to_svg(doc), encoding="utf-8")


@pytest.mark.parametrize(
    "case",
    PUBLIC_SYMBOL_SVG_CASES,
    ids=lambda case: case["name"],
)
def test_public_official_symbol_libraries_render_units_from_manifest(case):
    from kicad_monkey import KiCadSymbolLib

    input_file = resolve_kicad_manifest_path(case, "input_file")
    output_root = resolve_kicad_manifest_path(case, "output_root")
    assert input_file is not None and output_root is not None

    lib = KiCadSymbolLib.from_file(input_file)
    symbol_name = case.get("symbol_name") or case["name"]
    symbol = lib.get_symbol(symbol_name)
    assert symbol is not None
    assert symbol.unit_count >= 1

    out_dir = output_root / _slug(symbol_name)
    svgs = lib.to_svg(output_dir=out_dir)
    assert symbol_name in svgs
    assert set(svgs[symbol_name]) == set(range(1, symbol.unit_count + 1))

    for unit, svg in svgs[symbol_name].items():
        assert "<svg" in svg and "</svg>" in svg, (symbol_name, unit)
        doc = lib.symbol_to_ir(symbol_name, part_id=unit)
        assert doc.records, (symbol_name, unit)
        assert doc.extras["selection"]["unit"] == unit
        assert any(
            record.kind == "lib_subsymbol" and record.operations
            for record in doc.records
        ), (symbol_name, unit)
        hist = _op_hist(doc)
        assert sum(hist.values()) > 0, (symbol_name, unit)
        assert GEOMETRY_OP_KINDS.intersection(hist), (symbol_name, unit, hist)


@pytest.mark.parametrize(
    "case",
    PUBLIC_FOOTPRINT_SVG_CASES,
    ids=lambda case: case["name"],
)
def test_public_official_footprints_render_to_ir_and_svg_from_manifest(case):
    from kicad_monkey import KiCadFootprint, footprint_to_ir

    input_file = resolve_kicad_manifest_path(case, "input_file")
    output_root = resolve_kicad_manifest_path(case, "output_root")
    assert input_file is not None and output_root is not None

    fp = KiCadFootprint.from_file(input_file)
    doc = footprint_to_ir(fp, source_path=input_file.name, document_id=fp.name)
    assert doc.records
    assert doc.records[0].kind == "footprint"
    hist = _op_hist(doc)
    assert sum(hist.values()) > 0
    if fp.pads:
        expected_pad_flashes = sum(
            1 for pad in fp.pads if _pad_expects_flash_op(pad)
        )
        assert sum(hist.get(kind, 0) for kind in PAD_FLASH_OP_KINDS) >= expected_pad_flashes
    if fp.fp_lines:
        assert hist.get("ThickSegment", 0) >= len(fp.fp_lines)
    if fp.fp_arcs:
        assert hist.get("ArcThreePoint", 0) >= len(fp.fp_arcs)
    if fp.fp_circles:
        assert hist.get("Circle", 0) >= len(fp.fp_circles)
    if fp.fp_rects:
        assert hist.get("Rect", 0) >= len(fp.fp_rects)
    if fp.fp_polys:
        assert hist.get("PlotPoly", 0) >= len(fp.fp_polys)

    svg = fp.to_svg()
    assert "<?xml" in svg and "<svg" in svg and "</svg>" in svg
    assert len(svg) > 100

    ir_out = output_root / "footprint_ir"
    svg_out = output_root / "footprint_svg"
    ir_out.mkdir(parents=True, exist_ok=True)
    svg_out.mkdir(parents=True, exist_ok=True)
    safe_name = _slug(fp.name)
    (ir_out / f"{safe_name}.json").write_text(
        json.dumps(doc.to_normalized_dict(source_path=input_file.name), indent=2) + "\n",
        encoding="utf-8",
    )
    (svg_out / f"{safe_name}.svg").write_text(svg, encoding="utf-8")


def test_mimxrt685_symbol_library_renders_each_unit_from_manifest():
    from kicad_monkey import KiCadSymbolLib

    _require_manifest()
    case = get_kicad_corpus_case("internal_library/symbol_svg/MIMXRT685SFVKB")
    assert case is not None
    input_file = resolve_kicad_manifest_path(case, "input_file")
    output_root = resolve_kicad_manifest_path(case, "output_root")
    assert input_file is not None and output_root is not None

    lib = KiCadSymbolLib.from_file(input_file)
    assert hasattr(KiCadSymbolLib, "symbol_to_svg")
    assert hasattr(KiCadSymbolLib, "symbol_to_ir")
    assert hasattr(KiCadSymbolLib, "to_svg")

    symbol = lib.get_symbol("MIMXRT685SFVKB")
    assert symbol is not None
    assert symbol.unit_count >= 8

    out_dir = output_root / "MIMXRT685SFVKB"
    svgs = lib.to_svg(output_dir=out_dir)
    assert set(svgs["MIMXRT685SFVKB"]) == set(range(1, symbol.unit_count + 1))

    for unit, svg in svgs["MIMXRT685SFVKB"].items():
        assert "<svg" in svg and "</svg>" in svg, unit
        assert f"_unit{unit}.svg" in svg
        doc = lib.symbol_to_ir("MIMXRT685SFVKB", part_id=unit)
        assert _op_total(doc) > 0, unit
        assert any(
            record.kind == "lib_subsymbol" and record.operations
            for record in doc.records
        ), unit

    ir = lib.symbol_to_ir("MIMXRT685SFVKB", part_id=1)
    assert ir.source_kind == "SYM"
    assert ir.extras["selection"]["unit"] == 1


@pytest.mark.parametrize(
    "case",
    RECORDER_DRIFT_CASES,
    ids=lambda case: case["name"],
)
def test_schematic_recorder_drift_cases_match_manifest_oracles(case):
    from kicad_monkey import (
        KiCadSchematic,
        compute_op_equivalence,
        compute_recorder_drift,
        load_recorder_file,
        schematic_to_ir,
    )

    input_file = resolve_kicad_manifest_path(case, "input_file")
    recorder_file = resolve_kicad_manifest_path(case, "recorder_file")
    output_root = resolve_kicad_manifest_path(case, "output_root")
    assert input_file is not None and input_file.exists()
    assert recorder_file is not None and recorder_file.exists()
    assert output_root is not None

    recorder_doc = load_recorder_file(recorder_file)
    schematic = KiCadSchematic.from_file(input_file)
    monkey_doc = schematic_to_ir(
        schematic,
        source_path=input_file.name,
        document_id=input_file.stem,
        sheet_name=input_file.stem,
    )
    report = compute_recorder_drift(recorder_doc, monkey_doc)

    assert report.recorder_geometric_ops >= int(case["min_recorder_geometric_ops"])
    assert report.monkey_total_ops > 0
    assert report.coverage_ratio >= float(case["min_coverage_ratio"])

    expected_drift = case.get("expected_canvas_drift_nm")
    if expected_drift is not None:
        expected_canvas_drift = tuple(expected_drift)
        assert report.canvas_drift_nm == expected_canvas_drift or (
            expected_canvas_drift != (0, 0)
            and report.canvas_drift_nm == (0, 0)
        )

    allowed_recorder_only = RECORDER_STATE_KINDS | set(
        case.get("expected_recorder_only_kinds") or []
    )
    unexpected_recorder_only = set(report.recorder_only_kinds) - allowed_recorder_only
    assert not unexpected_recorder_only, report.to_dict()

    allowed_monkey_only = set(case.get("expected_monkey_only_kinds") or [])
    unexpected_monkey_only = set(report.monkey_only_kinds) - allowed_monkey_only
    assert not unexpected_monkey_only, report.to_dict()

    for kind in case.get("required_recorder_kinds") or []:
        assert report.recorder_hist.get(kind, 0) > 0, report.to_dict()

    op_report = compute_op_equivalence(
        recorder_doc,
        monkey_doc,
        tolerance_nm=float(case.get("op_equivalence_tolerance_nm", 0.0)),
        fold_pen_to_runs=bool(case.get("op_equivalence_fold_pen_to_runs", False)),
        ignore_stroked_text_runs=bool(
            case.get("op_equivalence_ignore_stroked_text_runs", False)
        ),
        match_strategy=case.get("op_equivalence_strategy", "windowed_by_kind"),
        match_window=int(case.get("op_equivalence_match_window", 0)),
        compare_styles=bool(case.get("op_equivalence_compare_styles", True)),
    )
    match_ratio = (
        op_report.matched_pairs / op_report.recorder_total
        if op_report.recorder_total
        else 0.0
    )
    if not op_report.equivalent:
        assert op_report.matched_pairs >= int(
            case.get("min_op_equivalence_matched_pairs", 0)
        ), op_report.to_dict()
        assert match_ratio >= float(
            case.get("min_op_equivalence_match_ratio", 0.0)
        ), op_report.to_dict()
        max_short = case.get("max_op_equivalence_monkey_short")
        if max_short is not None:
            assert op_report.monkey_short <= int(max_short), op_report.to_dict()
        max_long = case.get("max_op_equivalence_monkey_long")
        if max_long is not None:
            assert op_report.monkey_long <= int(max_long), op_report.to_dict()
    max_style = case.get("max_op_equivalence_style_mismatches")
    if max_style is not None:
        assert op_report.style_mismatches <= int(max_style), op_report.to_dict()
    elif bool(case.get("op_equivalence_compare_styles", True)):
        assert op_report.style_mismatches == 0, op_report.to_dict()
    expected_first_kind = case.get("expected_op_equivalence_first_divergence_kind")
    if expected_first_kind:
        if not op_report.equivalent:
            assert op_report.first_divergence is not None, op_report.to_dict()
            assert op_report.first_divergence.kind == expected_first_kind

    out_dir = output_root / "recorder_drift"
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / f"{_slug(case['name'])}.json").write_text(
        json.dumps(report.to_dict(), indent=2) + "\n",
        encoding="utf-8",
    )

    op_out_dir = output_root / "op_equivalence"
    op_out_dir.mkdir(parents=True, exist_ok=True)
    op_payload = op_report.to_dict()
    op_payload["match_ratio"] = match_ratio
    (op_out_dir / f"{_slug(case['name'])}.json").write_text(
        json.dumps(op_payload, indent=2) + "\n",
        encoding="utf-8",
    )
