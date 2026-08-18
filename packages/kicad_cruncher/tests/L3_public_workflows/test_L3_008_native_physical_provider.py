"""Cross-package evidence for the Windows no-fallback PCB physical provider."""

from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import xml.etree.ElementTree as ET
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path

import pytest
from kicad_cruncher import kicad_cruncher_cmd_design as design_cmd
from kicad_cruncher import kicad_cruncher_cmd_pcb_svg as pcb_svg_cmd
from kicad_cruncher.kicad_cruncher_native_physical import (
    NativePhysicalProvider,
    _normalize_native_board_svg,
    _split_native_drill_group,
)
from kicad_cruncher.kicad_cruncher_pcb_svg_compositor import (
    PcbSvgCompositionRenderCache,
    _bind_unlayered_footprint_block,
    _pad_view_payloads,
)
from kicad_monkey import KiCadPcb, mm_to_nm, parse_sexp
from kicad_monkey.kicad_pcb_bounds import compute_pcb_svg_bounding_box

_WORKSPACE = Path(__file__).resolve().parents[4]
_BOARD_VECTORS = _WORKSPACE / "tests" / "parity" / "board_plotter_a0_vectors.json"
_NATIVE_EXE = _WORKSPACE / "target" / "debug" / (
    "kicad-monkey-native.exe" if os.name == "nt" else "kicad-monkey-native"
)
_HLR_PROJECT = (
    _WORKSPACE
    / "packages"
    / "kicad_cruncher"
    / "tests"
    / "corpus"
    / "kicad"
    / "projects"
    / "hlr_test"
    / "hlr_test.kicad_pro"
)


def _board(
    case_id: str = "embedded-footprints-follow-zones-and-keep-local-ownership",
) -> KiCadPcb:
    vectors = json.loads(_BOARD_VECTORS.read_text(encoding="utf-8"))["vectors"]
    case = next(
        item
        for item in vectors
        if item["id"] == case_id
    )
    return KiCadPcb.from_sexp(parse_sexp(case["source"]))


def _mask_board() -> KiCadPcb:
    return KiCadPcb.from_string(
        """(kicad_pcb
  (version 20240108)
  (generator pcbnew)
  (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (44 "Edge.Cuts" user))
  (gr_rect (start 0 0) (end 20 20)
    (stroke (width 0.1) (type solid)) (fill none) (layer "Edge.Cuts"))
  (footprint "Fixture:Mask" (layer "F.Cu") (at 10 10)
    (pad "1" smd rect (at 0 0) (size 1 0.5)
      (layers "F.Cu" "F.Mask") (solder_mask_margin 0.1))
    (pad "2" smd custom (at 3 0) (size 1 1)
      (layers "F.Cu" "F.Mask") (solder_mask_margin 0.137)
      (options (clearance outline) (anchor rect))
      (primitives
        (gr_poly (pts (xy -0.5 -0.5) (xy 0.5 -0.5) (xy 0 0.5))
          (width 0) (fill yes)))))
)"""
    )


def _forbid_legacy(monkeypatch: pytest.MonkeyPatch) -> None:
    def forbidden(*_args: object, **_kwargs: object) -> object:
        raise AssertionError("legacy physical SVG renderer was called")

    monkeypatch.setattr(KiCadPcb, "to_svg", forbidden)
    monkeypatch.setattr("kicad_monkey.kicad_ir_to_svg.render_ir_to_svg", forbidden)


@dataclass
class _CountingProvider:
    inner: NativePhysicalProvider
    calls: int = 0

    def render_pcb_root(self, document: object, bbox: object) -> object:
        self.calls += 1
        return self.inner.render_pcb_root(document, bbox)  # type: ignore[arg-type]


def test_native_provider_is_no_fallback_enriched_and_cached(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    assert _NATIVE_EXE.is_file(), f"native executable must be built first: {_NATIVE_EXE}"
    _forbid_legacy(monkeypatch)
    pcb = _board()
    provider = _CountingProvider(NativePhysicalProvider(executable=_NATIVE_EXE))
    cache = PcbSvgCompositionRenderCache(pcb, physical_provider=provider)

    first = cache.root_svg(pcb, ["F.Cu", "Edge.Cuts"])
    second = cache.root_svg(pcb, ["F.Cu", "Edge.Cuts"])

    assert provider.calls == 1
    assert ET.tostring(first) == ET.tostring(second)
    assert first.attrib["data-enrichment-schema"] == "kicad_monkey.pcb.svg.enrichment.a0"
    assert first.attrib["data-view-kind"] == "layer_set"
    assert first.attrib["viewBox"].startswith("0 0 ")
    assert first.attrib["width"].endswith("mm")
    top_level_transforms = [child.attrib.get("transform", "") for child in first]
    assert any("scale(0.000001) translate(" in value for value in top_level_transforms)
    bbox = compute_pcb_svg_bounding_box(pcb, None)
    expected_transform = (
        "scale(0.000001) "
        f"translate({-mm_to_nm(float(bbox.min_x))} {-mm_to_nm(float(bbox.min_y))})"
    )
    assert all(
        child.attrib.get("transform", "").startswith(expected_transform)
        for child in first
        if child.attrib.get("data-ref") not in {None, "drill_overlay"}
    )
    assert any(element.attrib.get("data-layer-name") == "F.Cu" for element in first.iter())
    assert any(element.attrib.get("data-primitive") == "pad-hole" for element in first.iter())
    _assert_terminal_drill_overlays(first, primitive="pad-hole")
    provenance = cache.native_provenance(["F.Cu", "Edge.Cuts"])
    assert provenance is not None
    assert provenance.backend == "kicad-monkey-native"
    assert provenance.document_id.startswith("pcb-sha256:")
    assert len(provenance.document_sha256) == 64
    normalized = provider.inner.render_pcb_root(cache._computed_base_doc(), bbox)
    normalized_bytes = normalized.svg_text.encode("utf-8")
    import hashlib

    assert normalized.provenance.svg_bytes == len(normalized_bytes)
    assert normalized.provenance.svg_sha256 == hashlib.sha256(normalized_bytes).hexdigest()


def test_native_normalizer_rejects_wrong_viewport_or_record_identity() -> None:
    pcb = _board()
    cache = PcbSvgCompositionRenderCache(pcb, physical_provider=object())  # type: ignore[arg-type]
    document = cache._computed_base_doc()
    bbox = compute_pcb_svg_bounding_box(pcb, None)
    viewport = {
        "min_x_nm": mm_to_nm(float(bbox.min_x)),
        "min_y_nm": mm_to_nm(float(bbox.min_y)),
        "width_nm": mm_to_nm(float(bbox.width)),
        "height_nm": mm_to_nm(float(bbox.height)),
    }
    group_fragments = [
        f'<g id="{record.uuid}" data-ref="{record.kind}" '
        f'data-object-id="{record.object_id}" />'
        for record in document.records
    ]
    groups = "".join(group_fragments)
    wrong_viewport = (
        '<svg xmlns="http://www.w3.org/2000/svg"><rect />'
        f'<g transform="translate(0 0)">{groups}</g></svg>'
    )
    with pytest.raises(ValueError, match="viewport transform"):
        _normalize_native_board_svg(wrong_viewport, document, viewport)

    correct_transform = f'translate({-viewport["min_x_nm"]} {-viewport["min_y_nm"]})'
    wrong_identity = (
        '<svg xmlns="http://www.w3.org/2000/svg"><rect />'
        f'<g transform="{correct_transform}"><g id="wrong" data-ref="wrong" '
        'data-object-id="wrong" />'
        f'{"".join(group_fragments[1:])}</g></svg>'
    )
    with pytest.raises(ValueError, match="record id"):
        _normalize_native_board_svg(wrong_identity, document, viewport)

    wrong_tag = (
        '<svg xmlns="http://www.w3.org/2000/svg"><rect />'
        f'<g transform="{correct_transform}"><path id="{document.records[0].uuid}" '
        f'data-ref="{document.records[0].kind}" '
        f'data-object-id="{document.records[0].object_id}" />'
        f'{"".join(group_fragments[1:])}</g></svg>'
    )
    with pytest.raises(ValueError, match="not a group"):
        _normalize_native_board_svg(wrong_tag, document, viewport)


def test_no_drill_multiline_record_does_not_require_one_child_per_operation() -> None:
    from kicad_monkey.kicad_plotter_ir import (
        KiCadPlotterOp,
        KiCadPlotterOpKind,
        KiCadPlotterRecord,
    )

    operation = KiCadPlotterOp(
        kind=KiCadPlotterOpKind.TEXT,
        payload={"text": "first\nsecond", "x": 0, "y": 0},
    )
    record = KiCadPlotterRecord(
        uuid="multiline",
        kind="gr_text",
        object_id="multiline",
        operations=[operation],
    )
    group = ET.fromstring("<g><text>first</text><text>second</text></g>")
    overlay, retained = _split_native_drill_group(group, record)
    assert overlay is None
    assert retained == [operation]


def test_mask_pad_materialization_matches_enriched_authority() -> None:
    from kicad_monkey.kicad_ir_to_svg import _custom_pad_rings
    from kicad_monkey.kicad_plotter_ir import KiCadPlotterOpKind

    rect = {
        "x": 0,
        "y": 0,
        "size_x_nm": 1000,
        "size_y_nm": 500,
        "layers": ["F.Cu", "F.Mask"],
        "mask_margin_nm": 100,
    }
    rect_views = _pad_view_payloads("FlashPadRect", rect, ["F.Cu", "F.Mask"])
    assert rect_views[0] == ("", KiCadPlotterOpKind.FLASH_PAD_RECT, rect)
    assert rect_views[1][0:2] == (":mask", KiCadPlotterOpKind.FLASH_PAD_ROUNDRECT)
    assert rect_views[1][2]["size_x_nm"] == 1200
    assert rect_views[1][2]["size_y_nm"] == 700
    assert rect_views[1][2]["corner_radius_nm"] == 100

    negative = dict(rect, mask_margin_nm=-100)
    negative_view = _pad_view_payloads("FlashPadRect", negative, ["F.Mask"])
    assert negative_view[0][1] == KiCadPlotterOpKind.FLASH_PAD_RECT
    assert negative_view[0][2]["size_x_nm"] == 800
    assert negative_view[0][2]["size_y_nm"] == 300

    negative_roundrect = dict(
        rect,
        size_x_nm=2000,
        size_y_nm=1000,
        corner_radius_nm=200,
        mask_margin_nm=-300,
    )
    negative_roundrect_view = _pad_view_payloads(
        "FlashPadRoundRect", negative_roundrect, ["F.Mask"]
    )
    assert negative_roundrect_view[0][1] == KiCadPlotterOpKind.FLASH_PAD_ROUNDRECT
    assert negative_roundrect_view[0][2]["size_x_nm"] == 1400
    assert negative_roundrect_view[0][2]["size_y_nm"] == 400
    assert negative_roundrect_view[0][2]["corner_radius_nm"] == 0

    trapez = dict(rect, corners=[[-500, -250], [500, -250], [400, 250], [-400, 250]])
    trapez_views = _pad_view_payloads(
        "FlashPadTrapez", trapez, ["F.Cu", "F.Mask"]
    )
    assert len(trapez_views) == 1

    custom = {
        "x": 0,
        "y": 0,
        "polygons": [[[0, 0], [1000, 0], [333, 777]]],
        "polygon_widths_nm": [0],
        "layers": ["F.Cu", "F.Mask"],
        "mask_margin_nm": 137,
    }
    custom_views = _pad_view_payloads("FlashPadCustom", custom, ["F.Cu", "F.Mask"])
    assert custom_views[0][2]["polygons"] == custom["polygons"]
    expected_rings = _custom_pad_rings(custom, expand_for_mask=True)
    actual_rings = custom_views[1][2]["polygons"]
    assert len(actual_rings) == len(expected_rings)
    assert max(
        abs(float(actual) - expected)
        for actual_ring, expected_ring in zip(actual_rings, expected_rings, strict=True)
        for actual_point, expected_point in zip(actual_ring, expected_ring, strict=True)
        for actual, expected in zip(actual_point, expected_point, strict=True)
    ) <= 0.5


def test_unlayered_footprint_blocks_are_bound_to_the_selected_view() -> None:
    from kicad_monkey.kicad_plotter_ir import (
        KiCadPlotterOp,
        KiCadPlotterOpKind,
        KiCadPlotterRecord,
    )

    start = KiCadPlotterOp(
        kind=KiCadPlotterOpKind.START_BLOCK,
        payload={"extra_attrs": {"primitive": "pad-hole"}},
    )
    inner = KiCadPlotterOp(
        kind=KiCadPlotterOpKind.CIRCLE,
        payload={"layers": [], "role": "npth_hole"},
    )
    record = KiCadPlotterRecord(
        uuid="unlayered",
        kind="footprint",
        object_id="unlayered",
        extras={"layer": "B.Cu"},
    )
    bound_start, bound_inner = _bind_unlayered_footprint_block(
        start,
        inner,
        ["F.Cu", "Edge.Cuts"],
        record,
    )
    assert bound_start.payload["layers"] == ["F.Cu", "Edge.Cuts"]
    assert bound_start.payload["extra_attrs"]["layer_names"] == "F.Cu,Edge.Cuts"
    assert bound_inner.payload["layers"] == ["F.Cu", "Edge.Cuts"]


def test_native_provider_places_via_drills_in_terminal_overlays(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    assert _NATIVE_EXE.is_file(), f"native executable must be built first: {_NATIVE_EXE}"
    _forbid_legacy(monkeypatch)
    pcb = _board("via-fabrication-and-mask-edge-cases")
    cache = PcbSvgCompositionRenderCache(
        pcb,
        physical_provider=NativePhysicalProvider(executable=_NATIVE_EXE),
    )
    root = cache.root_svg(pcb, ["F.Cu", "B.Cu", "Edge.Cuts"])
    _assert_terminal_drill_overlays(root, primitive="via-hole")


def test_native_provider_materializes_mixed_standard_and_custom_mask_views(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    _forbid_legacy(monkeypatch)
    pcb = _mask_board()
    cache = PcbSvgCompositionRenderCache(
        pcb,
        physical_provider=NativePhysicalProvider(executable=_NATIVE_EXE),
    )
    root = cache.root_svg(pcb, ["F.Cu", "F.Mask", "Edge.Cuts"])
    pad_groups = [
        element for element in root.iter() if element.attrib.get("data-ref") == "pad"
    ]
    assert [element.attrib["id"].endswith(":mask") for element in pad_groups] == [
        False,
        True,
        False,
        True,
    ]
    assert ET.tostring(pad_groups[0]) != ET.tostring(pad_groups[1])
    assert ET.tostring(pad_groups[2]) != ET.tostring(pad_groups[3])


def test_public_design_command_renders_pcb_artifacts_through_native_provider(
    tmp_path: Path,
) -> None:
    env = dict(os.environ)
    env["KICAD_CRUNCHER_NATIVE_PHYSICAL"] = "1"
    env["KICAD_MONKEY_NATIVE"] = str(_NATIVE_EXE)
    output = tmp_path / "review"
    completed = subprocess.run(
        [
            sys.executable,
            "-m",
            "kicad_cruncher",
            "design",
            str(_HLR_PROJECT),
            "-o",
            str(output),
        ],
        cwd=_WORKSPACE,
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=180,
        check=False,
    )
    assert completed.returncode == 0, completed.stdout + completed.stderr
    manifest = json.loads((output / "design_review_manifest.json").read_text("utf-8"))
    assert manifest["pcb_svgs"]
    rendered = (output / manifest["pcb_svgs"][0]["file"]).read_text("utf-8")
    assert "kicad_monkey.pcb.svg.enrichment.a0" in rendered


def _assert_terminal_drill_overlays(root: ET.Element, *, primitive: str) -> None:
    children = list(root)
    overlay_indexes = [
        index
        for index, child in enumerate(children)
        if child.attrib.get("data-ref") == "drill_overlay"
    ]
    assert overlay_indexes
    assert overlay_indexes == list(range(overlay_indexes[0], len(children)))
    assert any(
        element.attrib.get("data-primitive") == primitive
        for child in children[overlay_indexes[0] :]
        for element in child.iter()
    )
    assert not any(
        element.attrib.get("data-primitive") == primitive
        for child in children[: overlay_indexes[0]]
        for element in child.iter()
    )


def test_native_provider_failure_never_retries_or_publishes_cache(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    _forbid_legacy(monkeypatch)

    class FailingProvider:
        def render_pcb_root(self, _document: object, _bbox: object) -> object:
            raise RuntimeError("native physical sentinel failure")

    pcb = _board()
    cache = PcbSvgCompositionRenderCache(pcb, physical_provider=FailingProvider())
    with pytest.raises(RuntimeError, match="native physical sentinel failure"):
        cache.root_svg(pcb, ["F.Cu", "Edge.Cuts"])
    assert cache._root_svg_text_by_layers == {}
    assert cache.native_provenance(["F.Cu", "Edge.Cuts"]) is None


def test_native_provider_aggregate_root_limit_is_preflighted(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    assert _NATIVE_EXE.is_file(), f"native executable must be built first: {_NATIVE_EXE}"
    _forbid_legacy(monkeypatch)
    pcb = _board()
    provider = _CountingProvider(NativePhysicalProvider(executable=_NATIVE_EXE))
    cache = PcbSvgCompositionRenderCache(
        pcb,
        physical_provider=provider,
        max_native_roots=1,
    )
    cache.root_svg(pcb, ["F.Cu", "Edge.Cuts"])
    with pytest.raises(ValueError, match="root ceiling"):
        cache.root_svg(pcb, ["B.Cu", "Edge.Cuts"])
    assert provider.calls == 1


def test_native_provider_retained_byte_limit_leaves_no_cached_provenance(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    assert _NATIVE_EXE.is_file(), f"native executable must be built first: {_NATIVE_EXE}"
    _forbid_legacy(monkeypatch)
    pcb = _board()
    provider = _CountingProvider(NativePhysicalProvider(executable=_NATIVE_EXE))
    cache = PcbSvgCompositionRenderCache(
        pcb,
        physical_provider=provider,
        max_retained_native_svg_bytes=1,
    )
    with pytest.raises(ValueError, match="retained-byte ceiling"):
        cache.root_svg(pcb, ["F.Cu", "Edge.Cuts"])
    assert provider.calls == 1
    assert cache._root_svg_text_by_layers == {}
    assert cache.native_provenance(["F.Cu", "Edge.Cuts"]) is None


def test_pcb_svg_command_stages_all_inputs_before_publication(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    output = tmp_path / "published"
    output.mkdir()
    (output / "existing.svg").write_text("keep", encoding="utf-8")
    inputs = [tmp_path / "first.kicad_pcb", tmp_path / "second.kicad_pcb"]
    for item in inputs:
        item.write_text("fixture", encoding="utf-8")
    calls = 0

    def render(_config: object, input_file: Path, *, output_dir: Path) -> int:
        nonlocal calls
        calls += 1
        output_dir.mkdir(parents=True, exist_ok=True)
        (output_dir / f"{input_file.stem}.svg").write_text("staged", encoding="utf-8")
        if calls == 2:
            raise RuntimeError("later native failure")
        return 1

    monkeypatch.setattr(pcb_svg_cmd, "_render_a0_board_outputs", render)
    result = pcb_svg_cmd._render_pcb_svg_to_output(
        inputs,
        output,
        {item.resolve(): object() for item in inputs},  # type: ignore[dict-item]
    )
    assert result == 1
    assert (output / "existing.svg").read_text(encoding="utf-8") == "keep"
    assert sorted(path.name for path in output.iterdir()) == ["existing.svg"]


@pytest.mark.parametrize("existing_destination", [False, True])
def test_pcb_svg_command_failure_preserves_destination_and_authored_config_boundary(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
    *,
    existing_destination: bool,
) -> None:
    input_file = tmp_path / "failure.kicad_pcb"
    input_file.write_text("(kicad_pcb (version 20240108))", encoding="utf-8")
    output = tmp_path / "published"
    if existing_destination:
        output.mkdir()
        (output / "existing.svg").write_bytes(b"keep\r\nexact\n")

    before = {
        path.relative_to(output).as_posix(): path.read_bytes()
        for path in output.rglob("*")
        if path.is_file()
    } if output.exists() else None

    def fail(_config: object, _input_file: Path, *, output_dir: Path) -> int:
        output_dir.mkdir(parents=True, exist_ok=True)
        (output_dir / "partial.svg").write_text("partial", encoding="utf-8")
        raise RuntimeError("native PCB render failed")

    monkeypatch.setattr(pcb_svg_cmd, "_render_a0_board_outputs", fail)
    result = pcb_svg_cmd.cmd_pcb_svg(
        argparse.Namespace(file=input_file, output=output, config=None)
    )

    assert result == 1
    config_path = input_file.parent / pcb_svg_cmd.PCB_SVG_CONFIG_FILENAME
    assert config_path.is_file()
    assert "kicad_cruncher.pcb_svg.config.a0" in config_path.read_text(encoding="utf-8")
    if before is None:
        assert not output.exists()
    else:
        after = {
            path.relative_to(output).as_posix(): path.read_bytes()
            for path in output.rglob("*")
            if path.is_file()
        }
        assert after == before


def test_design_review_staging_preserves_destination_on_native_failure(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    output = tmp_path / "published"
    output.mkdir()
    (output / "existing.json").write_text("keep", encoding="utf-8")

    def fail(_input: Path, staging: Path, **_kwargs: object) -> object:
        staging.mkdir(parents=True, exist_ok=True)
        (staging / "partial.svg").write_text("partial", encoding="utf-8")
        raise RuntimeError("native PCB render failed")

    monkeypatch.setattr(design_cmd, "_write_design_review_bundle_staged", fail)
    with pytest.raises(RuntimeError, match="native PCB render failed"):
        design_cmd.write_design_review_bundle(tmp_path / "input.kicad_pro", output)
    assert (output / "existing.json").read_text(encoding="utf-8") == "keep"
    assert sorted(path.name for path in output.iterdir()) == ["existing.json"]


@pytest.mark.parametrize(
    "publish",
    [pcb_svg_cmd._publish_staged_pcb_svg_tree, design_cmd._publish_design_review_tree],
)
def test_successful_tree_publication_replaces_stale_destination(
    tmp_path: Path,
    publish: Callable[[Path, Path], None],
) -> None:
    staging_parent = tmp_path / "transaction"
    staging = staging_parent / "staging"
    destination = tmp_path / "published"
    staging.mkdir(parents=True)
    destination.mkdir()
    (staging / "new.svg").write_text("new", encoding="utf-8")
    (destination / "stale.svg").write_text("stale", encoding="utf-8")

    publish(staging, destination)

    assert not staging.exists()
    assert sorted(path.name for path in destination.iterdir()) == ["new.svg"]
    assert (destination / "new.svg").read_text(encoding="utf-8") == "new"
