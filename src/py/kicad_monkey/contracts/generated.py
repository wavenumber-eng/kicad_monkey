"""Generated strict msgspec transport bindings. Do not edit."""

from __future__ import annotations

import hashlib
import math
from dataclasses import dataclass

from typing import Annotated, Literal, Union

import msgspec
from msgspec import UNSET, Meta, Struct, UnsetType, field


class Node(Struct, forbid_unknown_fields=True, frozen=True):
    kind: NodeKind
    text: str | UnsetType = field(default=UNSET)
    integer: str | UnsetType = field(default=UNSET)
    float_: float | UnsetType = field(default=UNSET, name="float")
    children: list[Node] | UnsetType = field(default=UNSET)


NodeKind = Literal["list", "atom", "quoted", "integer", "float"]


class Diagnostic(Struct, forbid_unknown_fields=True, frozen=True):
    phase: DiagnosticPhase
    code: str
    message: str
    position: SourcePosition | UnsetType = field(default=UNSET)
    token: str | UnsetType = field(default=UNSET)


DiagnosticPhase = Literal["lex", "tree", "build"]


class SourcePosition(Struct, forbid_unknown_fields=True, frozen=True):
    offset: str
    line: str
    column: str


class Selector(Struct, forbid_unknown_fields=True, frozen=True):
    heads: list[str] | UnsetType = field(default=UNSET)
    paths: list[list[str]] | UnsetType = field(default=UNSET)
    min_depth: Annotated[int, Meta(ge=0, le=4294967295)] | UnsetType = field(default=UNSET)
    max_depth: Annotated[int, Meta(ge=0, le=4294967295)] | UnsetType = field(default=UNSET)
    prune_heads: list[str] | UnsetType = field(default=UNSET)


class FormSpan(Struct, forbid_unknown_fields=True, frozen=True):
    path: list[str]
    depth: Annotated[int, Meta(ge=0, le=4294967295)]
    start_offset: str
    end_offset: str
    line: str
    column: str
    end_line: str
    end_column: str
    head: str | UnsetType = field(default=UNSET)


class FootprintProperty(Struct, forbid_unknown_fields=True, frozen=True):
    name: str
    value: str


class FootprintPlotRecord(Struct, forbid_unknown_fields=True, frozen=True):
    uuid: str
    kind: Literal["footprint"]
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    name: str
    layer: str
    locked: bool
    placed: bool
    descr: str
    tags: str
    attr: list[str]


class PlotterCoordinateSpace(Struct, forbid_unknown_fields=True, frozen=True):
    unit: Literal["nm"]
    y_axis: Literal["down"]


JavaScriptSafeInteger = Annotated[int, Meta(ge=-9007199254740991, le=9007199254740991)]


PlotterOperation = Union["ThickSegmentOperation", "ArcThreePointOperation", "CircleOperation", "RectOperation", "PlotPolyOperation", "BezierCurveOperation", "TextOperation", "PlotImageOperation", "FlashPadCircleOperation", "FlashPadOvalOperation", "FlashPadRectOperation", "FlashPadRoundRectOperation", "FlashPadCustomOperation", "FlashPadTrapezOperation"]


class ThickSegmentOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="ThickSegment", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    start_x: JavaScriptSafeInteger
    start_y: JavaScriptSafeInteger
    end_x: JavaScriptSafeInteger
    end_y: JavaScriptSafeInteger
    width_nm: JavaScriptSafeInteger
    layer: str | UnsetType = field(default=UNSET)
    role: PlotterDrillRole | UnsetType = field(default=UNSET)
    layers: list[str] | UnsetType = field(default=UNSET)
    mask_margin_nm: JavaScriptSafeInteger | UnsetType = field(default=UNSET)
    pad_size_x_nm: JavaScriptSafeInteger | UnsetType = field(default=UNSET)
    pad_size_y_nm: JavaScriptSafeInteger | UnsetType = field(default=UNSET)


class ArcThreePointOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="ArcThreePoint", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    start_x: JavaScriptSafeInteger
    start_y: JavaScriptSafeInteger
    mid_x: JavaScriptSafeInteger
    mid_y: JavaScriptSafeInteger
    end_x: JavaScriptSafeInteger
    end_y: JavaScriptSafeInteger
    fill: PlotterFill
    width_nm: JavaScriptSafeInteger
    layer: str | UnsetType = field(default=UNSET)
    stroke_color: str | UnsetType = field(default=UNSET)
    fill_color: str | UnsetType = field(default=UNSET)
    line_style: PlotterLineStyle | UnsetType = field(default=UNSET)


class CircleOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="Circle", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    cx: JavaScriptSafeInteger
    cy: JavaScriptSafeInteger
    diameter_nm: JavaScriptSafeInteger
    fill: PlotterFill
    width_nm: JavaScriptSafeInteger
    layer: str | UnsetType = field(default=UNSET)
    role: PlotterDrillRole | UnsetType = field(default=UNSET)
    layers: list[str] | UnsetType = field(default=UNSET)
    mask_margin_nm: JavaScriptSafeInteger | UnsetType = field(default=UNSET)
    pad_size_x_nm: JavaScriptSafeInteger | UnsetType = field(default=UNSET)
    pad_size_y_nm: JavaScriptSafeInteger | UnsetType = field(default=UNSET)
    stroke_color: str | UnsetType = field(default=UNSET)
    fill_color: str | UnsetType = field(default=UNSET)
    line_style: PlotterLineStyle | UnsetType = field(default=UNSET)


class RectOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="Rect", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x1: JavaScriptSafeInteger
    y1: JavaScriptSafeInteger
    x2: JavaScriptSafeInteger
    y2: JavaScriptSafeInteger
    fill: PlotterFill
    width_nm: JavaScriptSafeInteger
    corner_radius_nm: JavaScriptSafeInteger
    layer: str | UnsetType = field(default=UNSET)
    stroke_color: str | UnsetType = field(default=UNSET)
    fill_color: str | UnsetType = field(default=UNSET)
    line_style: PlotterLineStyle | UnsetType = field(default=UNSET)


class PlotPolyOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="PlotPoly", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    points: list[PlotterPoint]
    fill: PlotterFill
    width_nm: JavaScriptSafeInteger
    layer: str | UnsetType = field(default=UNSET)
    stroke_color: str | UnsetType = field(default=UNSET)
    fill_color: str | UnsetType = field(default=UNSET)
    line_style: PlotterLineStyle | UnsetType = field(default=UNSET)


class BezierCurveOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="BezierCurve", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    start_x: JavaScriptSafeInteger
    start_y: JavaScriptSafeInteger
    ctrl1_x: JavaScriptSafeInteger
    ctrl1_y: JavaScriptSafeInteger
    ctrl2_x: JavaScriptSafeInteger
    ctrl2_y: JavaScriptSafeInteger
    end_x: JavaScriptSafeInteger
    end_y: JavaScriptSafeInteger
    width_nm: JavaScriptSafeInteger
    tolerance_nm: JavaScriptSafeInteger
    layer: str | UnsetType = field(default=UNSET)
    stroke_color: str | UnsetType = field(default=UNSET)
    line_style: PlotterLineStyle | UnsetType = field(default=UNSET)


class TextOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="Text", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    text: str
    color: str
    orient_deg: float
    size_x_nm: JavaScriptSafeInteger
    size_y_nm: JavaScriptSafeInteger
    h_align: PlotterTextHAlign
    v_align: PlotterTextVAlign
    pen_width_nm: JavaScriptSafeInteger
    italic: bool
    bold: bool
    multiline: bool
    font_face: str
    layer: str | UnsetType = field(default=UNSET)
    mirror: bool | UnsetType = field(default=UNSET)
    text_as_polygons: bool | UnsetType = field(default=UNSET)
    polyline_per_segment: bool | UnsetType = field(default=UNSET)
    knockout: bool | UnsetType = field(default=UNSET)
    render_cache_polygons: list[list[PlotterPoint]] | UnsetType = field(default=UNSET)
    render_cache: TextRenderCache | UnsetType = field(default=UNSET)
    render_cache_source: PlotterTextRenderCacheSource | UnsetType = field(default=UNSET)
    render_cache_exact: bool | UnsetType = field(default=UNSET)


class PlotImageOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="PlotImage", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    width_nm: JavaScriptSafeInteger
    height_nm: JavaScriptSafeInteger
    scale: float
    image_data_b64: str
    image_format: str
    stroke_color: str | UnsetType = field(default=UNSET)


class FlashPadCircleOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadCircle", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    diameter_nm: JavaScriptSafeInteger
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger | UnsetType = field(default=UNSET)
    role: PlotterViaFlashRole | UnsetType = field(default=UNSET)


class FlashPadOvalOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadOval", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    size_x_nm: JavaScriptSafeInteger
    size_y_nm: JavaScriptSafeInteger
    orient_deg: float
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger


class FlashPadRectOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadRect", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    size_x_nm: JavaScriptSafeInteger
    size_y_nm: JavaScriptSafeInteger
    orient_deg: float
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger


class FlashPadRoundRectOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadRoundRect", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    size_x_nm: JavaScriptSafeInteger
    size_y_nm: JavaScriptSafeInteger
    corner_radius_nm: JavaScriptSafeInteger
    orient_deg: float
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger


class FlashPadCustomOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadCustom", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    size_x_nm: JavaScriptSafeInteger
    size_y_nm: JavaScriptSafeInteger
    orient_deg: float
    polygons: list[list[PlotterPoint]]
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger
    polygon_widths_nm: list[JavaScriptSafeInteger] | UnsetType = field(default=UNSET)
    anchor_shape: str | UnsetType = field(default=UNSET)


class FlashPadTrapezOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadTrapez", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    corners: PlotterQuad
    orient_deg: float
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger


PlotterDrillRole = Literal["pad_drill", "npth_hole", "via_drill", "via_mask_drill"]


PlotterFill = Literal["NO_FILL", "FILLED_SHAPE", "FILLED_WITH_BG_BODYCOLOR", "FILLED_WITH_COLOR", "HATCH", "REVERSE_HATCH", "CROSS_HATCH"]


PlotterLineStyle = Literal["DEFAULT", "SOLID", "DASH", "DOT", "DASH_DOT", "DASH_DOT_DOT"]


PlotterPoint = Annotated[list[JavaScriptSafeInteger], Meta(min_length=2, max_length=2)]


PlotterTextHAlign = Literal["GR_TEXT_H_ALIGN_LEFT", "GR_TEXT_H_ALIGN_CENTER", "GR_TEXT_H_ALIGN_RIGHT"]


PlotterTextVAlign = Literal["GR_TEXT_V_ALIGN_TOP", "GR_TEXT_V_ALIGN_CENTER", "GR_TEXT_V_ALIGN_BOTTOM"]


class TextRenderCache(Struct, forbid_unknown_fields=True, frozen=True):
    schema: Literal["kicad.render_cache.v1"]
    unit: Literal["nm"]
    coordinate_space: PlotterTextRenderCacheCoordinateSpace
    text: str
    angle: float
    source: PlotterTextRenderCacheSource
    exact: bool
    polygons: list[TextRenderCachePolygon]
    knockout: bool | UnsetType = field(default=UNSET)


PlotterTextRenderCacheSource = Literal["existing_file_cache", "python_generated_cache", "native_generated_cache"]


PlotterViaFlashRole = Literal["via_aperture", "via_mask_opening"]


PlotterQuad = Annotated[list[PlotterPoint], Meta(min_length=4, max_length=4)]


PlotterTextRenderCacheCoordinateSpace = Literal["board", "footprint_local"]


class TextRenderCachePolygon(Struct, forbid_unknown_fields=True, frozen=True):
    contours: list[list[PlotterPoint]]


BoardPlotRecord = Union["BoardGraphicPlotRecord", "TrackSegmentPlotRecord", "TrackArcPlotRecord", "ViaPlotRecord", "TablePlotRecord", "DimensionPlotRecord", "ZoneFillPlotRecord", "BoardTextPlotRecord", "BoardTextBoxPlotRecord", "BoardFootprintPlotRecord"]


class BoardGraphicPlotRecordGrLine(Struct, forbid_unknown_fields=True, frozen=True, tag="gr_line", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    layer: str | None


class BoardGraphicPlotRecordGrArc(Struct, forbid_unknown_fields=True, frozen=True, tag="gr_arc", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    layer: str | None


class BoardGraphicPlotRecordGrCircle(Struct, forbid_unknown_fields=True, frozen=True, tag="gr_circle", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    layer: str | None


class BoardGraphicPlotRecordGrRect(Struct, forbid_unknown_fields=True, frozen=True, tag="gr_rect", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    layer: str | None


class BoardGraphicPlotRecordGrPoly(Struct, forbid_unknown_fields=True, frozen=True, tag="gr_poly", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    layer: str | None


class BoardGraphicPlotRecordGrCurve(Struct, forbid_unknown_fields=True, frozen=True, tag="gr_curve", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    layer: str | None


BoardGraphicPlotRecord = Union[BoardGraphicPlotRecordGrLine, BoardGraphicPlotRecordGrArc, BoardGraphicPlotRecordGrCircle, BoardGraphicPlotRecordGrRect, BoardGraphicPlotRecordGrPoly, BoardGraphicPlotRecordGrCurve]


class TrackSegmentPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="segment", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    layer: str
    locked: bool
    net_id: JavaScriptSafeInteger | UnsetType = field(default=UNSET)
    net_name: str | UnsetType = field(default=UNSET)
    net_class: str | UnsetType = field(default=UNSET)
    net_classes: list[str] | UnsetType = field(default=UNSET)


class TrackArcPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="track_arc", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    layer: str
    net_id: JavaScriptSafeInteger | UnsetType = field(default=UNSET)
    net_name: str | UnsetType = field(default=UNSET)
    net_class: str | UnsetType = field(default=UNSET)
    net_classes: list[str] | UnsetType = field(default=UNSET)


class ViaPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="via", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    layers: list[str]
    drill: float
    size: float
    via_type: BoardViaType
    hole_kind: Literal["round"]
    hole_plating: Literal["plated"]
    hole_render: Literal["drill"]
    ipc4761_tenting_front: PlotterStringBool | UnsetType = field(default=UNSET)
    ipc4761_tenting_back: PlotterStringBool | UnsetType = field(default=UNSET)
    ipc4761_covering_front: PlotterStringBool | UnsetType = field(default=UNSET)
    ipc4761_covering_back: PlotterStringBool | UnsetType = field(default=UNSET)
    ipc4761_plugging_front: PlotterStringBool | UnsetType = field(default=UNSET)
    ipc4761_plugging_back: PlotterStringBool | UnsetType = field(default=UNSET)
    ipc4761_capping: PlotterStringBool | UnsetType = field(default=UNSET)
    ipc4761_filling: PlotterStringBool | UnsetType = field(default=UNSET)
    ipc4761_metadata: Literal["true"] | UnsetType = field(default=UNSET)
    net_id: JavaScriptSafeInteger | UnsetType = field(default=UNSET)
    net_name: str | UnsetType = field(default=UNSET)
    net_class: str | UnsetType = field(default=UNSET)
    net_classes: list[str] | UnsetType = field(default=UNSET)


class TablePlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="table", tag_field="kind"):
    uuid: str
    object_id: Literal["table"]
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    layers: list[str]
    cell_count: Annotated[int, Meta(ge=0, le=4294967295)]


class DimensionPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="dimension", tag_field="kind"):
    uuid: str
    object_id: Literal["dimension"]
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    layers: list[str]
    dimension_type: BoardDimensionType
    text: str | UnsetType = field(default=UNSET)


class ZoneFillPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="zone_fill", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    layers: list[str]
    fill_layers: list[str]
    fill_island: list[bool]
    net_id: JavaScriptSafeInteger | UnsetType = field(default=UNSET)
    net_name: str | UnsetType = field(default=UNSET)
    net_class: str | UnsetType = field(default=UNSET)
    net_classes: list[str] | UnsetType = field(default=UNSET)


class BoardTextPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="gr_text", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    layer: str
    text: str
    hide: bool


class BoardTextBoxPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="gr_text_box", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    layer: str
    text: str
    border: bool


class BoardFootprintPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="footprint", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[BoardFootprintOperation]
    library_link: str
    reference: str
    value: str
    layer: str
    locked: bool
    descr: str
    tags: str
    attr: list[str]
    placement: BoardFootprintPlacement


BoardGraphicRecordKind = Literal["gr_line", "gr_arc", "gr_circle", "gr_rect", "gr_poly", "gr_curve"]


BoardViaType = Literal["through", "blind", "buried", "micro"]


PlotterStringBool = Literal["true", "false"]


BoardDimensionType = Literal["aligned", "orthogonal", "radial", "leader", "center"]


BoardFootprintOperation = Union["BoardFootprintThickSegmentOperation", "BoardFootprintArcThreePointOperation", "BoardFootprintCircleOperation", "BoardFootprintRectOperation", "BoardFootprintPlotPolyOperation", "BoardFootprintBezierCurveOperation", "BoardFootprintTextOperation", "BoardFootprintFlashPadCircleOperation", "BoardFootprintFlashPadOvalOperation", "BoardFootprintFlashPadRectOperation", "BoardFootprintFlashPadRoundRectOperation", "BoardFootprintFlashPadCustomOperation", "BoardFootprintFlashPadTrapezOperation", "BoardFootprintStartBlockOperation", "BoardFootprintEndBlockOperation"]


class BoardFootprintPlacement(Struct, forbid_unknown_fields=True, frozen=True):
    x_nm: JavaScriptSafeInteger
    y_nm: JavaScriptSafeInteger
    angle_deg: float


class BoardFootprintThickSegmentOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="ThickSegment", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    start_x: JavaScriptSafeInteger
    start_y: JavaScriptSafeInteger
    end_x: JavaScriptSafeInteger
    end_y: JavaScriptSafeInteger
    width_nm: JavaScriptSafeInteger
    layer: str | UnsetType = field(default=UNSET)
    role: PlotterDrillRole | UnsetType = field(default=UNSET)
    layers: list[str] | UnsetType = field(default=UNSET)
    mask_margin_nm: JavaScriptSafeInteger | UnsetType = field(default=UNSET)
    pad_size_x_nm: JavaScriptSafeInteger | UnsetType = field(default=UNSET)
    pad_size_y_nm: JavaScriptSafeInteger | UnsetType = field(default=UNSET)
    label: str | UnsetType = field(default=UNSET)
    data_uuid: str | UnsetType = field(default=UNSET)
    data_ref: BoardFootprintChildRef | UnsetType = field(default=UNSET)
    object_id: str | UnsetType = field(default=UNSET)
    extra_attrs: BoardFootprintChildAttrs | UnsetType = field(default=UNSET)


class BoardFootprintArcThreePointOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="ArcThreePoint", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    start_x: JavaScriptSafeInteger
    start_y: JavaScriptSafeInteger
    mid_x: JavaScriptSafeInteger
    mid_y: JavaScriptSafeInteger
    end_x: JavaScriptSafeInteger
    end_y: JavaScriptSafeInteger
    fill: PlotterFill
    width_nm: JavaScriptSafeInteger
    layer: str | UnsetType = field(default=UNSET)
    stroke_color: str | UnsetType = field(default=UNSET)
    fill_color: str | UnsetType = field(default=UNSET)
    line_style: PlotterLineStyle | UnsetType = field(default=UNSET)
    label: str | UnsetType = field(default=UNSET)
    data_uuid: str | UnsetType = field(default=UNSET)
    data_ref: BoardFootprintChildRef | UnsetType = field(default=UNSET)
    object_id: str | UnsetType = field(default=UNSET)
    extra_attrs: BoardFootprintChildAttrs | UnsetType = field(default=UNSET)


class BoardFootprintCircleOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="Circle", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    cx: JavaScriptSafeInteger
    cy: JavaScriptSafeInteger
    diameter_nm: JavaScriptSafeInteger
    fill: PlotterFill
    width_nm: JavaScriptSafeInteger
    layer: str | UnsetType = field(default=UNSET)
    role: PlotterDrillRole | UnsetType = field(default=UNSET)
    layers: list[str] | UnsetType = field(default=UNSET)
    mask_margin_nm: JavaScriptSafeInteger | UnsetType = field(default=UNSET)
    pad_size_x_nm: JavaScriptSafeInteger | UnsetType = field(default=UNSET)
    pad_size_y_nm: JavaScriptSafeInteger | UnsetType = field(default=UNSET)
    stroke_color: str | UnsetType = field(default=UNSET)
    fill_color: str | UnsetType = field(default=UNSET)
    line_style: PlotterLineStyle | UnsetType = field(default=UNSET)
    label: str | UnsetType = field(default=UNSET)
    data_uuid: str | UnsetType = field(default=UNSET)
    data_ref: BoardFootprintChildRef | UnsetType = field(default=UNSET)
    object_id: str | UnsetType = field(default=UNSET)
    extra_attrs: BoardFootprintChildAttrs | UnsetType = field(default=UNSET)


class BoardFootprintRectOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="Rect", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x1: JavaScriptSafeInteger
    y1: JavaScriptSafeInteger
    x2: JavaScriptSafeInteger
    y2: JavaScriptSafeInteger
    fill: PlotterFill
    width_nm: JavaScriptSafeInteger
    corner_radius_nm: JavaScriptSafeInteger
    layer: str | UnsetType = field(default=UNSET)
    stroke_color: str | UnsetType = field(default=UNSET)
    fill_color: str | UnsetType = field(default=UNSET)
    line_style: PlotterLineStyle | UnsetType = field(default=UNSET)
    label: str | UnsetType = field(default=UNSET)
    data_uuid: str | UnsetType = field(default=UNSET)
    data_ref: BoardFootprintChildRef | UnsetType = field(default=UNSET)
    object_id: str | UnsetType = field(default=UNSET)
    extra_attrs: BoardFootprintChildAttrs | UnsetType = field(default=UNSET)


class BoardFootprintPlotPolyOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="PlotPoly", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    points: list[PlotterPoint]
    fill: PlotterFill
    width_nm: JavaScriptSafeInteger
    layer: str | UnsetType = field(default=UNSET)
    stroke_color: str | UnsetType = field(default=UNSET)
    fill_color: str | UnsetType = field(default=UNSET)
    line_style: PlotterLineStyle | UnsetType = field(default=UNSET)
    label: str | UnsetType = field(default=UNSET)
    data_uuid: str | UnsetType = field(default=UNSET)
    data_ref: BoardFootprintChildRef | UnsetType = field(default=UNSET)
    object_id: str | UnsetType = field(default=UNSET)
    extra_attrs: BoardFootprintChildAttrs | UnsetType = field(default=UNSET)


class BoardFootprintBezierCurveOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="BezierCurve", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    start_x: JavaScriptSafeInteger
    start_y: JavaScriptSafeInteger
    ctrl1_x: JavaScriptSafeInteger
    ctrl1_y: JavaScriptSafeInteger
    ctrl2_x: JavaScriptSafeInteger
    ctrl2_y: JavaScriptSafeInteger
    end_x: JavaScriptSafeInteger
    end_y: JavaScriptSafeInteger
    width_nm: JavaScriptSafeInteger
    tolerance_nm: JavaScriptSafeInteger
    layer: str | UnsetType = field(default=UNSET)
    stroke_color: str | UnsetType = field(default=UNSET)
    line_style: PlotterLineStyle | UnsetType = field(default=UNSET)
    label: str | UnsetType = field(default=UNSET)
    data_uuid: str | UnsetType = field(default=UNSET)
    data_ref: BoardFootprintChildRef | UnsetType = field(default=UNSET)
    object_id: str | UnsetType = field(default=UNSET)
    extra_attrs: BoardFootprintChildAttrs | UnsetType = field(default=UNSET)


class BoardFootprintTextOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="Text", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    text: str
    color: str
    orient_deg: float
    size_x_nm: JavaScriptSafeInteger
    size_y_nm: JavaScriptSafeInteger
    h_align: PlotterTextHAlign
    v_align: PlotterTextVAlign
    pen_width_nm: JavaScriptSafeInteger
    italic: bool
    bold: bool
    multiline: bool
    font_face: str
    layer: str | UnsetType = field(default=UNSET)
    mirror: bool | UnsetType = field(default=UNSET)
    text_as_polygons: bool | UnsetType = field(default=UNSET)
    polyline_per_segment: bool | UnsetType = field(default=UNSET)
    knockout: bool | UnsetType = field(default=UNSET)
    render_cache_polygons: list[list[PlotterPoint]] | UnsetType = field(default=UNSET)
    render_cache: TextRenderCache | UnsetType = field(default=UNSET)
    render_cache_source: PlotterTextRenderCacheSource | UnsetType = field(default=UNSET)
    render_cache_exact: bool | UnsetType = field(default=UNSET)
    label: str | UnsetType = field(default=UNSET)
    data_uuid: str | UnsetType = field(default=UNSET)
    data_ref: BoardFootprintChildRef | UnsetType = field(default=UNSET)
    object_id: str | UnsetType = field(default=UNSET)
    extra_attrs: BoardFootprintChildAttrs | UnsetType = field(default=UNSET)


class BoardFootprintFlashPadCircleOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadCircle", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    diameter_nm: JavaScriptSafeInteger
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger | UnsetType = field(default=UNSET)
    role: PlotterViaFlashRole | UnsetType = field(default=UNSET)
    label: str | UnsetType = field(default=UNSET)
    data_uuid: str | UnsetType = field(default=UNSET)
    data_ref: BoardFootprintChildRef | UnsetType = field(default=UNSET)
    object_id: str | UnsetType = field(default=UNSET)
    extra_attrs: BoardFootprintChildAttrs | UnsetType = field(default=UNSET)


class BoardFootprintFlashPadOvalOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadOval", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    size_x_nm: JavaScriptSafeInteger
    size_y_nm: JavaScriptSafeInteger
    orient_deg: float
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger
    label: str | UnsetType = field(default=UNSET)
    data_uuid: str | UnsetType = field(default=UNSET)
    data_ref: BoardFootprintChildRef | UnsetType = field(default=UNSET)
    object_id: str | UnsetType = field(default=UNSET)
    extra_attrs: BoardFootprintChildAttrs | UnsetType = field(default=UNSET)


class BoardFootprintFlashPadRectOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadRect", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    size_x_nm: JavaScriptSafeInteger
    size_y_nm: JavaScriptSafeInteger
    orient_deg: float
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger
    label: str | UnsetType = field(default=UNSET)
    data_uuid: str | UnsetType = field(default=UNSET)
    data_ref: BoardFootprintChildRef | UnsetType = field(default=UNSET)
    object_id: str | UnsetType = field(default=UNSET)
    extra_attrs: BoardFootprintChildAttrs | UnsetType = field(default=UNSET)


class BoardFootprintFlashPadRoundRectOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadRoundRect", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    size_x_nm: JavaScriptSafeInteger
    size_y_nm: JavaScriptSafeInteger
    corner_radius_nm: JavaScriptSafeInteger
    orient_deg: float
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger
    label: str | UnsetType = field(default=UNSET)
    data_uuid: str | UnsetType = field(default=UNSET)
    data_ref: BoardFootprintChildRef | UnsetType = field(default=UNSET)
    object_id: str | UnsetType = field(default=UNSET)
    extra_attrs: BoardFootprintChildAttrs | UnsetType = field(default=UNSET)


class BoardFootprintFlashPadCustomOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadCustom", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    size_x_nm: JavaScriptSafeInteger
    size_y_nm: JavaScriptSafeInteger
    orient_deg: float
    polygons: list[list[PlotterPoint]]
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger
    polygon_widths_nm: list[JavaScriptSafeInteger] | UnsetType = field(default=UNSET)
    anchor_shape: str | UnsetType = field(default=UNSET)
    label: str | UnsetType = field(default=UNSET)
    data_uuid: str | UnsetType = field(default=UNSET)
    data_ref: BoardFootprintChildRef | UnsetType = field(default=UNSET)
    object_id: str | UnsetType = field(default=UNSET)
    extra_attrs: BoardFootprintChildAttrs | UnsetType = field(default=UNSET)


class BoardFootprintFlashPadTrapezOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadTrapez", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    corners: PlotterQuad
    orient_deg: float
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger
    label: str | UnsetType = field(default=UNSET)
    data_uuid: str | UnsetType = field(default=UNSET)
    data_ref: BoardFootprintChildRef | UnsetType = field(default=UNSET)
    object_id: str | UnsetType = field(default=UNSET)
    extra_attrs: BoardFootprintChildAttrs | UnsetType = field(default=UNSET)


class BoardFootprintStartBlockOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="StartBlock", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    label: str
    data_uuid: str
    data_ref: Union[Literal["pad"], Literal["pad_hole"]]
    object_id: str
    extra_attrs: BoardFootprintPadBlockAttrs
    layers: list[str] | UnsetType = field(default=UNSET)


class BoardFootprintEndBlockOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="EndBlock", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]


BoardFootprintChildRef = Literal["property", "fp_text", "fp_text_box", "fp_line", "fp_arc", "fp_circle", "fp_rect", "fp_poly"]


class BoardFootprintChildAttrs(Struct, forbid_unknown_fields=True, frozen=True):
    component: str
    component_uid: str
    component_uuid: str
    footprint: str
    primitive: Union[Literal["footprint-text"], Literal["footprint-graphic"]]
    footprint_primitive: BoardFootprintChildRef
    footprint_object_index: Annotated[int, Meta(ge=0, le=4294967295)]
    layer_name: str | UnsetType = field(default=UNSET)
    layer_role: BoardFootprintLayerRole | UnsetType = field(default=UNSET)
    footprint_subop_index: Annotated[int, Meta(ge=0, le=4294967295)] | UnsetType = field(default=UNSET)
    footprint_text_role: Union[Literal["designator"], Literal["value"], Literal["property"], Literal["user"]] | UnsetType = field(default=UNSET)
    property_name: str | UnsetType = field(default=UNSET)
    fp_text_type: str | UnsetType = field(default=UNSET)
    footprint_graphic_kind: Union[Literal["text-box-border"], Literal["line"], Literal["arc"], Literal["circle"], Literal["rect"], Literal["poly"]] | UnsetType = field(default=UNSET)


class BoardFootprintPadBlockAttrs(Struct, forbid_unknown_fields=True, frozen=True):
    primitive: Union[Literal["pad"], Literal["pad-hole"]]
    component: str | UnsetType = field(default=UNSET)
    component_uid: str | UnsetType = field(default=UNSET)
    component_uuid: str | UnsetType = field(default=UNSET)
    footprint: str | UnsetType = field(default=UNSET)
    pad_number: str | UnsetType = field(default=UNSET)
    pad_designator: str | UnsetType = field(default=UNSET)
    pad_type: str | UnsetType = field(default=UNSET)
    pad_shape: str | UnsetType = field(default=UNSET)
    layer_names: str | UnsetType = field(default=UNSET)
    net_index: str | UnsetType = field(default=UNSET)
    net_id: str | UnsetType = field(default=UNSET)
    net: str | UnsetType = field(default=UNSET)
    net_class: str | UnsetType = field(default=UNSET)
    net_classes: str | UnsetType = field(default=UNSET)
    hole_owner: str | UnsetType = field(default=UNSET)
    hole_kind: Union[Literal["round"], Literal["slot"]] | UnsetType = field(default=UNSET)
    hole_plating: Union[Literal["plated"], Literal["non_plated"]] | UnsetType = field(default=UNSET)
    hole_render: Literal["drill"] | UnsetType = field(default=UNSET)
    hole_width_mm: str | UnsetType = field(default=UNSET)
    hole_height_mm: str | UnsetType = field(default=UNSET)
    hole_diameter_mm: str | UnsetType = field(default=UNSET)


BoardFootprintLayerRole = Literal["copper", "silkscreen", "soldermask", "paste", "fab", "courtyard", "board-outline", "drill", "user", "other"]


class BoardNetClassAssignment(Struct, forbid_unknown_fields=True, frozen=True):
    net_name: str
    classes: list[str]


class BoardTextVariable(Struct, forbid_unknown_fields=True, frozen=True):
    name: str
    value: str


SymbolPlotRecord = Union["SymbolHeaderPlotRecord", "LibSubsymbolPlotRecord"]


class SymbolHeaderPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="lib_symbol", tag_field="kind"):
    uuid: Literal[""]
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    name: str
    style: Annotated[int, Meta(ge=0, le=4294967295)]
    in_bom: bool
    on_board: bool
    power: bool
    extends_: str | UnsetType = field(default=UNSET, name="extends")
    unit: Annotated[int, Meta(ge=0, le=4294967295)] | UnsetType = field(default=UNSET)


class LibSubsymbolPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="lib_subsymbol", tag_field="kind"):
    uuid: Literal[""]
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    unit: Annotated[int, Meta(ge=0, le=4294967295)]
    style: Annotated[int, Meta(ge=0, le=4294967295)]


class SymbolTextVariable(Struct, forbid_unknown_fields=True, frozen=True):
    name: str
    value: str


SchematicPlotRecord = Union["SchematicSheetHeaderPlotRecord", "SchematicWirePlotRecord", "SchematicBusPlotRecord", "SchematicBusEntryPlotRecord", "SchematicJunctionPlotRecord", "SchematicNoConnectPlotRecord"]


class SchematicPlotCanvas(Struct, forbid_unknown_fields=True, frozen=True):
    width_nm: JavaScriptSafeInteger
    height_nm: JavaScriptSafeInteger


class SchematicSheetHeaderPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="sheet_header", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    paper_size: str
    paper_width_mm: float | None
    paper_height_mm: float | None
    paper_portrait: bool
    sheet_width_nm: JavaScriptSafeInteger
    sheet_height_nm: JavaScriptSafeInteger
    version: JavaScriptSafeInteger
    generator: str
    generator_version: str
    title_block: SchematicPlotTitleBlock | UnsetType = field(default=UNSET)


class SchematicWirePlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="wire", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]


class SchematicBusPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="bus", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]


class SchematicBusEntryPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="bus_entry", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]


class SchematicJunctionPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="junction", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    color: str | None | UnsetType = field(default=UNSET)


class SchematicNoConnectPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="no_connect", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]


class SchematicPlotTitleBlock(Struct, forbid_unknown_fields=True, frozen=True):
    title: str
    date: str
    rev: str
    company: str
    comments: RecordString


RecordString = dict[str, str]


SchematicWorksheetMode = Literal["default", "provided"]


class SchematicTextVariable(Struct, forbid_unknown_fields=True, frozen=True):
    name: str
    value: str


SymbolBooleanField = Literal["in_bom", "on_board"]


class SymbolSummary(Struct, forbid_unknown_fields=True, frozen=True):
    name: str
    in_bom: bool
    on_board: bool
    power: bool
    property_count: Annotated[int, Meta(ge=0, le=4294967295)]
    subsymbol_count: Annotated[int, Meta(ge=0, le=4294967295)]
    pin_count: Annotated[int, Meta(ge=0, le=4294967295)]
    extends_: str | UnsetType = field(default=UNSET, name="extends")
    power_kind: str | UnsetType = field(default=UNSET)


class UnitDefinition(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["sch.unit_definition"] = field(name="type")
    id: str
    display_name: str
    page_definition_refs: list[str]
    source_identity: SourceIdentity


class PageDefinition(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["sch.page_definition"] = field(name="type")
    id: str
    display_name: str
    unit_definition_ref: str
    source_identity: SourceIdentity


class UnitOccurrence(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["sch.unit_occurrence"] = field(name="type")
    id: str
    display_name: str
    unit_definition_ref: str
    page_occurrence_refs: list[str]
    source_identity: SourceIdentity
    parent_hierarchy_occurrence_ref: str | UnsetType = field(default=UNSET)


class PageOccurrence(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["sch.page_occurrence"] = field(name="type")
    id: str
    display_name: str
    page_definition_ref: str
    unit_occurrence_ref: str
    instance_order: Annotated[int, Meta(ge=0, le=4294967295)]
    source_identity: SourceIdentity
    address_key: str | UnsetType = field(default=UNSET)
    sheet_number: str | UnsetType = field(default=UNSET)


class HierarchyOccurrence(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["sch.hierarchy_occurrence"] = field(name="type")
    id: str
    parent_unit_occurrence_ref: str
    parent_page_occurrence_ref: str
    child_unit_occurrence_ref: str
    source_identity: SourceIdentity


class ComponentOccurrence(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["sch.component_occurrence"] = field(name="type")
    id: str
    page_occurrence_ref: str
    source_designator: str
    physical_designator: str
    display_designator: str
    unit: Annotated[int, Meta(ge=1, le=4294967295)]
    body_style: Annotated[int, Meta(ge=0, le=4294967295)]
    source_identity: SourceIdentity
    design_component_ref: str | UnsetType = field(default=UNSET)


class LocalNetOccurrence(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["sch.local_net_occurrence"] = field(name="type")
    id: str
    page_occurrence_ref: str
    display_name: str
    aliases: list[str]
    source_identity: SourceIdentity
    design_net_ref: str | UnsetType = field(default=UNSET)
    qualified_name: str | UnsetType = field(default=UNSET)


class TerminalOccurrence(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["sch.terminal_occurrence"] = field(name="type")
    id: str
    page_occurrence_ref: str
    role: TerminalRole
    name: str
    pin_designator: str
    source_identity: SourceIdentity
    local_net_occurrence_ref: str | UnsetType = field(default=UNSET)
    design_net_ref: str | UnsetType = field(default=UNSET)
    component_occurrence_ref: str | UnsetType = field(default=UNSET)
    design_component_pin_ref: str | UnsetType = field(default=UNSET)
    resolution_diagnostics: list[ResolutionDiagnostic] | UnsetType = field(default=UNSET)


class HierarchyTerminalBinding(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["sch.hierarchy_terminal_binding"] = field(name="type")
    id: str
    hierarchy_occurrence_ref: str
    parent_terminal_occurrence_ref: str
    child_terminal_occurrence_ref: str
    source_identity: SourceIdentity
    design_net_ref: str | UnsetType = field(default=UNSET)


class GraphicalArtifactLink(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["sch.graphical_artifact_link"] = field(name="type")
    id: str
    page_occurrence_ref: str
    target_type: GraphicalTargetType
    target_ref: str
    artifact_key: Literal["sch.dwg_scene"]
    element_id: str
    source_identity: SourceIdentity


class SourceIdentity(Struct, forbid_unknown_fields=True, frozen=True):
    sch_source_key_source_uuid: str | UnsetType = field(default=UNSET, name="sch.source_key.source_uuid")
    sch_source_key_source_path: str | UnsetType = field(default=UNSET, name="sch.source_key.source_path")
    sch_source_key_source_record: str | UnsetType = field(default=UNSET, name="sch.source_key.source_record")
    sch_source_key_source_subobject: str | UnsetType = field(default=UNSET, name="sch.source_key.source_subobject")
    sch_source_key_compiled_net: str | UnsetType = field(default=UNSET, name="sch.source_key.compiled_net")
    sch_source_key_artifact_element: str | UnsetType = field(default=UNSET, name="sch.source_key.artifact_element")


TerminalRole = Literal["component_pin", "sheet_entry", "port", "power_port"]


ResolutionDiagnostic = Literal["logical_pin_unresolved", "component_occurrence_unresolved", "hierarchy_terminal_binding_unresolved", "design_net_unresolved"]


GraphicalTargetType = Literal["sch.component_occurrence", "sch.hierarchy_occurrence", "sch.terminal_occurrence", "sch.local_net_occurrence", "sch.page_occurrence"]


class SourceBundleSource(Struct, forbid_unknown_fields=True, frozen=True):
    path: str
    kind: SourceKind
    slot: SourceSlot
    source_bytes: CanonicalUint64Decimal


SourceKind = Literal["project", "schematic", "symbol_library", "symbol_table", "worksheet", "other"]


SourceSlot = Annotated[int, Meta(ge=0, le=4294967295)]


CanonicalUint64Decimal = Annotated[str, Meta(pattern="^(0|[1-9][0-9]{0,19})$")]


class FontBundleEntry(Struct, forbid_unknown_fields=True, frozen=True):
    id: StableTextId
    slot: Annotated[int, Meta(ge=0, le=4294967295)]
    sha256: Sha256Hex
    face_index: Annotated[int, Meta(ge=0, le=4294967295)]
    variations: list[FontVariationCoordinate]
    aliases: list[str]
    family: str | UnsetType = field(default=UNSET)
    style: str | UnsetType = field(default=UNSET)
    postscript_name: str | UnsetType = field(default=UNSET)


StableTextId = Annotated[str, Meta(pattern="^[A-Za-z0-9][A-Za-z0-9._:-]*$")]


Sha256Hex = Annotated[str, Meta(pattern="^[0-9a-f]{64}$")]


class FontVariationCoordinate(Struct, forbid_unknown_fields=True, frozen=True):
    axis: OpenTypeTag
    value: FiniteFloat


OpenTypeTag = Annotated[str, Meta(pattern="^[ -~]{4}$")]


FiniteFloat = Annotated[float, Meta(ge=-1.7976931348623157e+308, le=1.7976931348623157e+308)]


class FontSelection(Struct, forbid_unknown_fields=True, frozen=True):
    aliases: list[str]
    font_id: StableTextId | UnsetType = field(default=UNSET)


class ExactComparisonPolicy(Struct, forbid_unknown_fields=True, frozen=True, tag="exact", tag_field="mode"):
    pass


class ShapingInput(Struct, forbid_unknown_fields=True, frozen=True):
    font_id: StableTextId
    font_sha256: Sha256Hex
    face_index: Annotated[int, Meta(ge=0, le=4294967295)]
    variations: list[FontVariationCoordinate]
    text: str
    text_index_unit: Literal["utf8_byte_offset"]
    scale_x: Annotated[int, Meta(ge=-2147483648, le=2147483647)]
    scale_y: Annotated[int, Meta(ge=-2147483648, le=2147483647)]
    direction: TextDirection
    features: list[ShapingFeature]
    buffer_properties: ShapingBufferProperties
    script: OpenTypeTag | UnsetType = field(default=UNSET)
    language: NonEmptyText | UnsetType = field(default=UNSET)


class ShapedGlyph(Struct, forbid_unknown_fields=True, frozen=True):
    glyph_id: Annotated[int, Meta(ge=0, le=4294967295)]
    cluster: Annotated[int, Meta(ge=0, le=4294967295)]
    x_advance: TextSafeInteger
    y_advance: TextSafeInteger
    x_offset: TextSafeInteger
    y_offset: TextSafeInteger
    unsafe_to_break: bool
    safe_to_insert_tatweel: bool
    unsafe_to_concat: bool


TextDirection = Literal["left_to_right", "right_to_left", "top_to_bottom", "bottom_to_top"]


NonEmptyText = Annotated[str, Meta(min_length=1)]


class ShapingFeature(Struct, forbid_unknown_fields=True, frozen=True):
    tag: OpenTypeTag
    value: Annotated[int, Meta(ge=0, le=4294967295)]
    start: Annotated[int, Meta(ge=0, le=4294967295)]
    end: Annotated[int, Meta(ge=0, le=4294967295)]


class ShapingBufferProperties(Struct, forbid_unknown_fields=True, frozen=True):
    cluster_level: ShapingClusterLevel
    beginning_of_text: bool
    end_of_text: bool
    default_ignorables: DefaultIgnorablePolicy
    do_not_insert_dotted_circle: bool
    produce_unsafe_to_concat: bool
    produce_safe_to_insert_tatweel: bool


TextSafeInteger = Annotated[int, Meta(ge=-9007199254740991, le=9007199254740991)]


ShapingClusterLevel = Literal["monotone_graphemes", "monotone_characters", "characters"]


DefaultIgnorablePolicy = Literal["normal", "preserve", "remove"]


CoordinateComparisonPolicy = Union["ExactComparisonPolicy", "AbsoluteToleranceComparisonPolicy"]


PositiveUint32 = Annotated[int, Meta(ge=1, le=4294967295)]


OutlineCommand = Union["OutlineMoveTo", "OutlineLineTo", "OutlineQuadTo", "OutlineCurveTo", "OutlineClose"]


class AbsoluteToleranceComparisonPolicy(Struct, forbid_unknown_fields=True, frozen=True, tag="absolute_tolerance", tag_field="mode"):
    absolute_tolerance: NonNegativeFiniteFloat


class OutlineMoveTo(Struct, forbid_unknown_fields=True, frozen=True, tag="move_to", tag_field="kind"):
    x: FiniteFloat
    y: FiniteFloat


class OutlineLineTo(Struct, forbid_unknown_fields=True, frozen=True, tag="line_to", tag_field="kind"):
    x: FiniteFloat
    y: FiniteFloat


class OutlineQuadTo(Struct, forbid_unknown_fields=True, frozen=True, tag="quad_to", tag_field="kind"):
    control_x: FiniteFloat
    control_y: FiniteFloat
    x: FiniteFloat
    y: FiniteFloat


class OutlineCurveTo(Struct, forbid_unknown_fields=True, frozen=True, tag="curve_to", tag_field="kind"):
    control1_x: FiniteFloat
    control1_y: FiniteFloat
    control2_x: FiniteFloat
    control2_y: FiniteFloat
    x: FiniteFloat
    y: FiniteFloat


class OutlineClose(Struct, forbid_unknown_fields=True, frozen=True, tag="close", tag_field="kind"):
    pass


NonNegativeFiniteFloat = Annotated[float, Meta(ge=0)]


class SExpressionBuildRequestA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.sexpr_build.request"] = field(name="type")
    version: Literal["a0"]
    root: Node
    max_output_bytes: str
    max_depth: Annotated[int, Meta(ge=0, le=4294967295)]
    max_nodes: Annotated[int, Meta(ge=0, le=4294967295)]


class SExpressionBuildResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.sexpr_build.result"] = field(name="type")
    version: Literal["a0"]
    output_bytes: str
    diagnostics: list[Diagnostic]


class SExpressionScanRequestA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.sexpr_scan.request"] = field(name="type")
    version: Literal["a0"]
    selector: Selector
    max_source_bytes: str
    max_depth: Annotated[int, Meta(ge=0, le=4294967295)]
    max_selected_forms: Annotated[int, Meta(ge=0, le=4294967295)]


class SExpressionScanResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.sexpr_scan.result"] = field(name="type")
    version: Literal["a0"]
    source_bytes: str
    forms: list[FormSpan]
    diagnostics: list[Diagnostic]


class FootprintEditRequestA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.footprint_edit.request"] = field(name="type")
    version: Literal["a0"]
    property_name: str
    value: str
    max_source_bytes: str
    max_output_bytes: str
    max_depth: Annotated[int, Meta(ge=0, le=4294967295)]
    max_properties: Annotated[int, Meta(ge=0, le=4294967295)]
    max_pads: Annotated[int, Meta(ge=0, le=4294967295)]


class FootprintEditResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.footprint_edit.result"] = field(name="type")
    version: Literal["a0"]
    changed: bool
    output_bytes: str
    diagnostics: list[Diagnostic]


class FootprintReadRequestA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.footprint_read.request"] = field(name="type")
    version: Literal["a0"]
    max_source_bytes: str
    max_depth: Annotated[int, Meta(ge=0, le=4294967295)]
    max_properties: Annotated[int, Meta(ge=0, le=4294967295)]
    max_pads: Annotated[int, Meta(ge=0, le=4294967295)]


class FootprintReadResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.footprint_read.result"] = field(name="type")
    version: Literal["a0"]
    name: str
    source_bytes: str
    properties: list[FootprintProperty]
    pad_count: Annotated[int, Meta(ge=0, le=4294967295)]
    diagnostics: list[Diagnostic]


class FootprintPlotDocumentA0(Struct, forbid_unknown_fields=True, frozen=True):
    schema: Literal["kicad.plotter_ir.a0"]
    source_kind: Literal["MOD"]
    total_operations: Annotated[int, Meta(ge=0, le=4294967295)]
    records: list[FootprintPlotRecord]
    document_id: str
    coordinate_space: PlotterCoordinateSpace
    version: JavaScriptSafeInteger
    generator: str
    generator_version: str
    source_path: str | UnsetType = field(default=UNSET)


class FootprintPlotRequestA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.footprint_plot.request"] = field(name="type")
    version: Literal["a0"]
    max_source_bytes: str
    max_output_bytes: str
    max_depth: Annotated[int, Meta(ge=0, le=4294967295)]
    max_metadata_forms: Annotated[int, Meta(ge=0, le=4294967295)]
    max_operations: Annotated[int, Meta(ge=0, le=4294967295)]
    max_points: Annotated[int, Meta(ge=0, le=4294967295)]
    source_path: str | UnsetType = field(default=UNSET)
    document_id: str | UnsetType = field(default=UNSET)
    max_text_carriers: Annotated[int, Meta(ge=0, le=4294967295)] | UnsetType = field(default=UNSET)
    max_text_bytes: str | UnsetType = field(default=UNSET)


class FootprintPlotResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.footprint_plot.result"] = field(name="type")
    version: Literal["a0"]
    output_bytes: str
    total_operations: Annotated[int, Meta(ge=0, le=4294967295)]
    diagnostics: list[Diagnostic]


class BoardPlotDocumentA0(Struct, forbid_unknown_fields=True, frozen=True):
    schema: Literal["kicad.plotter_ir.a0"]
    source_kind: Literal["PCB"]
    total_operations: Annotated[int, Meta(ge=0, le=4294967295)]
    records: list[BoardPlotRecord]
    document_id: str
    coordinate_space: PlotterCoordinateSpace
    version: JavaScriptSafeInteger
    generator: str
    generator_version: str
    thickness_mm: float
    paper: str
    source_path: str | UnsetType = field(default=UNSET)


class BoardPlotRequestA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.board_plot.request"] = field(name="type")
    version: Literal["a0"]
    max_source_bytes: str
    max_output_bytes: str
    max_depth: Annotated[int, Meta(ge=0, le=4294967295)]
    max_graphics: Annotated[int, Meta(ge=0, le=4294967295)]
    max_operations: Annotated[int, Meta(ge=0, le=4294967295)]
    max_points: Annotated[int, Meta(ge=0, le=4294967295)]
    max_text_bytes: str
    max_parse_nodes: Annotated[int, Meta(ge=0, le=4294967295)]
    max_input_points: Annotated[int, Meta(ge=0, le=4294967295)]
    max_input_polygons: Annotated[int, Meta(ge=0, le=4294967295)]
    max_cache_polygons: Annotated[int, Meta(ge=0, le=4294967295)]
    max_cache_contours: Annotated[int, Meta(ge=0, le=4294967295)]
    source_path: str | UnsetType = field(default=UNSET)
    document_id: str | UnsetType = field(default=UNSET)
    net_class_assignments: list[BoardNetClassAssignment] | UnsetType = field(default=UNSET)
    text_variables: list[BoardTextVariable] | UnsetType = field(default=UNSET)


class BoardPlotResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.board_plot.result"] = field(name="type")
    version: Literal["a0"]
    output_bytes: str
    total_operations: Annotated[int, Meta(ge=0, le=4294967295)]
    diagnostics: list[Diagnostic]


class SymbolPlotDocumentA0(Struct, forbid_unknown_fields=True, frozen=True):
    schema: Literal["kicad.plotter_ir.a0"]
    source_kind: Literal["SYM"]
    total_operations: Annotated[int, Meta(ge=0, le=4294967295)]
    records: list[SymbolPlotRecord]
    document_id: str
    coordinate_space: PlotterCoordinateSpace
    source_path: str | UnsetType = field(default=UNSET)


class SymbolPlotRequestA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.symbol_plot.request"] = field(name="type")
    version: Literal["a0"]
    symbol_name: str
    style: Annotated[int, Meta(ge=0, le=4294967295)]
    max_source_bytes: str
    max_output_bytes: str
    max_depth: Annotated[int, Meta(ge=0, le=4294967295)]
    max_symbols: Annotated[int, Meta(ge=0, le=4294967295)]
    max_subsymbols: Annotated[int, Meta(ge=0, le=4294967295)]
    max_operations: Annotated[int, Meta(ge=0, le=4294967295)]
    max_points: Annotated[int, Meta(ge=0, le=4294967295)]
    unit: Annotated[int, Meta(ge=0, le=4294967295)] | UnsetType = field(default=UNSET)
    source_path: str | UnsetType = field(default=UNSET)
    document_id: str | UnsetType = field(default=UNSET)
    max_text_carriers: Annotated[int, Meta(ge=0, le=4294967295)] | UnsetType = field(default=UNSET)
    max_text_bytes: str | UnsetType = field(default=UNSET)
    text_variables: list[SymbolTextVariable] | UnsetType = field(default=UNSET)


class SymbolPlotResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.symbol_plot.result"] = field(name="type")
    version: Literal["a0"]
    output_bytes: str
    total_operations: Annotated[int, Meta(ge=0, le=4294967295)]
    diagnostics: list[Diagnostic]


class SchematicPlotDocumentA0(Struct, forbid_unknown_fields=True, frozen=True):
    schema: Literal["kicad.plotter_ir.a0"]
    source_kind: Literal["SCH"]
    total_operations: Annotated[int, Meta(ge=0, le=4294967295)]
    records: list[SchematicPlotRecord]
    document_id: str
    canvas: SchematicPlotCanvas
    coordinate_space: PlotterCoordinateSpace
    source_path: str | UnsetType = field(default=UNSET)


class SchematicPlotRequestA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.schematic_plot.request"] = field(name="type")
    version: Literal["a0"]
    sheet_index: Annotated[int, Meta(ge=1, le=4294967295)]
    sheet_count: Annotated[int, Meta(ge=1, le=4294967295)]
    sheet_path: str
    sheet_name: str
    worksheet_mode: SchematicWorksheetMode
    max_source_bytes: str
    max_worksheet_bytes: str
    max_output_bytes: str
    max_depth: Annotated[int, Meta(ge=0, le=4294967295)]
    max_parse_nodes: Annotated[int, Meta(ge=0, le=4294967295)]
    max_selected_forms: Annotated[int, Meta(ge=0, le=4294967295)]
    max_records: Annotated[int, Meta(ge=0, le=4294967295)]
    max_operations: Annotated[int, Meta(ge=0, le=4294967295)]
    max_points: Annotated[int, Meta(ge=0, le=4294967295)]
    max_input_points: Annotated[int, Meta(ge=0, le=4294967295)]
    max_text_bytes: str
    max_metadata_bytes: str
    max_wires: Annotated[int, Meta(ge=0, le=4294967295)]
    max_buses: Annotated[int, Meta(ge=0, le=4294967295)]
    max_bus_entries: Annotated[int, Meta(ge=0, le=4294967295)]
    max_junctions: Annotated[int, Meta(ge=0, le=4294967295)]
    max_no_connects: Annotated[int, Meta(ge=0, le=4294967295)]
    max_text_variables: Annotated[int, Meta(ge=0, le=4294967295)]
    max_text_variable_bytes: str
    max_worksheet_items: Annotated[int, Meta(ge=0, le=4294967295)]
    max_worksheet_repeats: Annotated[int, Meta(ge=0, le=4294967295)]
    max_worksheet_point_sets: Annotated[int, Meta(ge=0, le=4294967295)]
    max_worksheet_points: Annotated[int, Meta(ge=0, le=4294967295)]
    max_worksheet_bitmap_data_parts: Annotated[int, Meta(ge=0, le=4294967295)]
    max_worksheet_bitmap_encoded_bytes: str
    max_worksheet_bitmap_decoded_bytes: str
    max_worksheet_bitmap_width_px: Annotated[int, Meta(ge=0, le=4294967295)]
    max_worksheet_bitmap_height_px: Annotated[int, Meta(ge=0, le=4294967295)]
    max_worksheet_bitmap_pixels: str
    max_worksheet_bitmap_decode_work: str
    source_path: str | UnsetType = field(default=UNSET)
    document_id: str | UnsetType = field(default=UNSET)
    text_variables: list[SchematicTextVariable] | UnsetType = field(default=UNSET)


class SchematicPlotResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.schematic_plot.result"] = field(name="type")
    version: Literal["a0"]
    output_bytes: str
    total_operations: Annotated[int, Meta(ge=0, le=4294967295)]
    diagnostics: list[Diagnostic]


class SymbolLibraryEditRequestA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.symbol_library_edit.request"] = field(name="type")
    version: Literal["a0"]
    symbol_name: str
    field: SymbolBooleanField
    value: bool
    max_source_bytes: str
    max_output_bytes: str
    max_depth: Annotated[int, Meta(ge=0, le=4294967295)]
    max_symbols: Annotated[int, Meta(ge=0, le=4294967295)]
    max_metadata_forms: Annotated[int, Meta(ge=0, le=4294967295)]
    max_subsymbols: Annotated[int, Meta(ge=0, le=4294967295)]
    max_pins: Annotated[int, Meta(ge=0, le=4294967295)]


class SymbolLibraryEditResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.symbol_library_edit.result"] = field(name="type")
    version: Literal["a0"]
    changed: bool
    output_bytes: str
    diagnostics: list[Diagnostic]


class SymbolLibraryReadRequestA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.symbol_library_read.request"] = field(name="type")
    version: Literal["a0"]
    max_source_bytes: str
    max_depth: Annotated[int, Meta(ge=0, le=4294967295)]
    max_symbols: Annotated[int, Meta(ge=0, le=4294967295)]
    max_metadata_forms: Annotated[int, Meta(ge=0, le=4294967295)]
    max_subsymbols: Annotated[int, Meta(ge=0, le=4294967295)]
    max_pins: Annotated[int, Meta(ge=0, le=4294967295)]


class SymbolLibraryReadResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.symbol_library_read.result"] = field(name="type")
    version: Literal["a0"]
    source_bytes: str
    symbols: list[SymbolSummary]
    diagnostics: list[Diagnostic]


class CompiledSchematicGraphA0(Struct, forbid_unknown_fields=True, frozen=True):
    schema: Literal["kicad_monkey.compiled_schematic_graph.a0"]
    type_: Literal["sch.compiled_schematic_graph"] = field(name="type")
    identity_namespace: Literal["sch.compiled_schematic_graph.a0"]
    unit_definitions: list[UnitDefinition]
    page_definitions: list[PageDefinition]
    unit_occurrences: list[UnitOccurrence]
    page_occurrences: list[PageOccurrence]
    hierarchy_occurrences: list[HierarchyOccurrence]
    component_occurrences: list[ComponentOccurrence]
    local_net_occurrences: list[LocalNetOccurrence]
    terminal_occurrences: list[TerminalOccurrence]
    hierarchy_terminal_bindings: list[HierarchyTerminalBinding]
    graphical_artifact_links: list[GraphicalArtifactLink]


class SourceBundleManifestA0(Struct, forbid_unknown_fields=True, frozen=True):
    schema: Literal["kicad_monkey.source_bundle_manifest.a0"]
    type_: Literal["kicad_monkey.source_bundle_manifest"] = field(name="type")
    version: Literal["a0"]
    root_schematic_path: str
    sources: list[SourceBundleSource]
    project_path: str | UnsetType = field(default=UNSET)


class FontBundleManifestA0(Struct, forbid_unknown_fields=True, frozen=True):
    schema: Literal["kicad_monkey.font_bundle.a0"]
    type_: Literal["kicad_monkey.font_bundle"] = field(name="type")
    version: Literal["a0"]
    fonts: list[FontBundleEntry]


class FontResolutionRequestA0(Struct, forbid_unknown_fields=True, frozen=True):
    schema: Literal["kicad_monkey.font_resolution_request.a0"]
    type_: Literal["kicad_monkey.font_resolution_request"] = field(name="type")
    version: Literal["a0"]
    selection: FontSelection


class ShapingRecordA0(Struct, forbid_unknown_fields=True, frozen=True):
    schema: Literal["kicad_monkey.shaping_record.a0"]
    type_: Literal["kicad_monkey.shaping_record"] = field(name="type")
    version: Literal["a0"]
    case_id: StableTextId
    comparison: ExactComparisonPolicy
    input: ShapingInput
    glyphs: list[ShapedGlyph]


class OutlineVectorA0(Struct, forbid_unknown_fields=True, frozen=True):
    schema: Literal["kicad_monkey.outline_vector.a0"]
    type_: Literal["kicad_monkey.outline_vector"] = field(name="type")
    version: Literal["a0"]
    case_id: StableTextId
    coordinate_format: Literal["font_design_units_f64"]
    coordinate_comparison: CoordinateComparisonPolicy
    font_id: StableTextId
    font_sha256: Sha256Hex
    face_index: Annotated[int, Meta(ge=0, le=4294967295)]
    variations: list[FontVariationCoordinate]
    glyph_id: Annotated[int, Meta(ge=0, le=4294967295)]
    units_per_em: PositiveUint32
    commands: list[OutlineCommand]


decode_sexpr_build_request_a0 = msgspec.json.Decoder(SExpressionBuildRequestA0).decode
decode_sexpr_build_result_a0 = msgspec.json.Decoder(SExpressionBuildResultA0).decode
decode_sexpr_scan_request_a0 = msgspec.json.Decoder(SExpressionScanRequestA0).decode
decode_sexpr_scan_result_a0 = msgspec.json.Decoder(SExpressionScanResultA0).decode
decode_footprint_edit_request_a0 = msgspec.json.Decoder(FootprintEditRequestA0).decode
decode_footprint_edit_result_a0 = msgspec.json.Decoder(FootprintEditResultA0).decode
decode_footprint_read_request_a0 = msgspec.json.Decoder(FootprintReadRequestA0).decode
decode_footprint_read_result_a0 = msgspec.json.Decoder(FootprintReadResultA0).decode
_footprint_plot_document_a0_decoder = msgspec.json.Decoder(FootprintPlotDocumentA0)


def decode_footprint_plot_document_a0(data: bytes) -> FootprintPlotDocumentA0:
    value = _footprint_plot_document_a0_decoder.decode(data)
    validate_footprint_plot_document_a0(value)
    return value


def validate_footprint_plot_document_a0(value: FootprintPlotDocumentA0) -> None:
    if len(value.records) != 1:
        raise msgspec.ValidationError("invalid_footprint_document at $.records")
    total_operations = 0
    for record_index, record in enumerate(value.records):
        if record.object_id != record.name:
            raise msgspec.ValidationError(f"invalid_footprint_record at $.records[{record_index}]")
        if record.operation_count != len(record.operations):
            raise msgspec.ValidationError(
                f"operation_count_mismatch at $.records[{record_index}].operation_count"
            )
        total_operations += len(record.operations)
        for operation_index, operation in enumerate(record.operations):
            path = f"$.records[{record_index}].operations[{operation_index}]"
            if operation.index != operation_index:
                raise msgspec.ValidationError(f"operation_index_mismatch at {path}.index")
            if isinstance(operation, (ThickSegmentOperation, CircleOperation)):
                _validate_shared_graphic_or_drill(operation, path)
            elif isinstance(operation, TextOperation):
                _validate_footprint_text(operation, path)
            elif isinstance(operation, (ArcThreePointOperation, RectOperation, PlotPolyOperation, BezierCurveOperation)):
                if operation.layer is UNSET or not operation.layer:
                    raise msgspec.ValidationError(f"missing_layer at {path}")
            elif isinstance(operation, (
                FlashPadCircleOperation,
                FlashPadOvalOperation,
                FlashPadRectOperation,
                FlashPadRoundRectOperation,
                FlashPadCustomOperation,
                FlashPadTrapezOperation,
            )):
                if not operation.layers:
                    raise msgspec.ValidationError(f"missing_layers at {path}")
                if isinstance(operation, FlashPadCircleOperation) and (
                    operation.mask_margin_nm is UNSET or operation.role is not UNSET
                ):
                    raise msgspec.ValidationError(f"invalid_pad_operation at {path}")
            else:
                raise msgspec.ValidationError(f"invalid_footprint_operation at {path}")
            if isinstance(operation, FlashPadCustomOperation):
                widths = operation.polygon_widths_nm
                if widths is not UNSET and widths and len(widths) != len(operation.polygons):
                    raise msgspec.ValidationError(f"polygon_width_count_mismatch at {path}.polygon_widths_nm")
    if value.total_operations != total_operations:
        raise msgspec.ValidationError("operation_count_mismatch at $.total_operations")


def _validate_footprint_text(operation: TextOperation, path: str) -> None:
    forbidden = (
        operation.mirror is not UNSET,
        operation.text_as_polygons is not UNSET,
        operation.polyline_per_segment is not UNSET,
        operation.knockout is not UNSET,
        operation.render_cache_polygons is not UNSET,
        operation.render_cache is not UNSET,
        operation.render_cache_source is not UNSET,
        operation.render_cache_exact is not UNSET,
    )
    if operation.layer is UNSET or not operation.layer or any(forbidden):
        raise msgspec.ValidationError(f"invalid_footprint_text at {path}")


def _validate_shared_graphic_or_drill(operation: ThickSegmentOperation | CircleOperation, path: str) -> None:
    layer = None if operation.layer is UNSET else operation.layer
    role = None if operation.role is UNSET else operation.role
    layers = [] if operation.layers is UNSET else operation.layers
    has_mask = operation.mask_margin_nm is not UNSET
    has_size_x = operation.pad_size_x_nm is not UNSET
    has_size_y = operation.pad_size_y_nm is not UNSET
    graphic = (
        role is None and layer is not None and not layers
        and not has_mask and not has_size_x and not has_size_y
    )
    pad_drill = (
        role == "pad_drill" and layer is None and bool(layers)
        and not has_mask and not has_size_x and not has_size_y
    )
    npth_hole = (
        role == "npth_hole" and layer is None and bool(layers)
        and has_mask and has_size_x and has_size_y
    )
    if not (graphic or pad_drill or npth_hole):
        raise msgspec.ValidationError(f"conflicting_plotter_fields at {path}")
decode_footprint_plot_request_a0 = msgspec.json.Decoder(FootprintPlotRequestA0).decode
decode_footprint_plot_result_a0 = msgspec.json.Decoder(FootprintPlotResultA0).decode
_board_plot_document_a0_decoder = msgspec.json.Decoder(BoardPlotDocumentA0)


def decode_board_plot_document_a0(data: bytes) -> BoardPlotDocumentA0:
    value = _board_plot_document_a0_decoder.decode(data)
    validate_board_plot_document_a0(value)
    return value


def validate_board_plot_document_a0(value: BoardPlotDocumentA0) -> None:
    if value.schema != "kicad.plotter_ir.a0" or value.source_kind != "PCB" or value.coordinate_space.unit != "nm" or value.coordinate_space.y_axis != "down":
        raise msgspec.ValidationError("invalid_board_document at $")
    total_operations = 0
    saw_footprint = False
    for record_index, record in enumerate(value.records):
        path = f'$.records[{record_index}]'
        if any(isinstance(operation, PlotImageOperation) for operation in record.operations):
            raise msgspec.ValidationError(f"invalid_board_operation at {path}.operations")
        if isinstance(record, BoardFootprintPlotRecord):
            saw_footprint = True
            _validate_board_footprint_plot_record(record, path)
        elif saw_footprint:
            raise msgspec.ValidationError(f"invalid_board_record_order at {path}")
        if record.operation_count != len(record.operations):
            raise msgspec.ValidationError(f"operation_count_mismatch at {path}.operation_count")
        total_operations += len(record.operations)
        if isinstance(record, DimensionPlotRecord):
            _validate_dimension_plot_record(record, path)
    if value.total_operations != total_operations:
        raise msgspec.ValidationError("operation_count_mismatch at $.total_operations")


def _validate_dimension_plot_record(record: DimensionPlotRecord, path: str) -> None:
    if not record.layers or record.layers != sorted(set(record.layers)):
        raise msgspec.ValidationError(f"invalid_dimension at {path}.layers")
    saw_text = False
    marker_count = 0
    for operation_index, operation in enumerate(record.operations):
        operation_path = f'{path}.operations[{operation_index}]'
        if operation.index != operation_index:
            raise msgspec.ValidationError(f"operation_index_mismatch at {operation_path}.index")
        if isinstance(operation, TextOperation):
            if operation_index != 0 or saw_text:
                raise msgspec.ValidationError(f"invalid_dimension at {operation_path}")
            saw_text = True
            layer = None if operation.layer is UNSET else operation.layer
            if not operation.font_face or layer not in record.layers:
                raise msgspec.ValidationError(f"invalid_dimension at {operation_path}")
            _validate_board_text_payload(operation, operation_path)
        elif isinstance(operation, ThickSegmentOperation):
            layer = None if operation.layer is UNSET else operation.layer
            layers = [] if operation.layers is UNSET else operation.layers
            forbidden = (operation.role is not UNSET, bool(layers), operation.mask_margin_nm is not UNSET, operation.pad_size_x_nm is not UNSET, operation.pad_size_y_nm is not UNSET)
            if layer not in record.layers or any(forbidden):
                raise msgspec.ValidationError(f"invalid_dimension at {operation_path}")
        elif isinstance(operation, CircleOperation):
            marker_count += 1
            layer = None if operation.layer is UNSET else operation.layer
            layers = [] if operation.layers is UNSET else operation.layers
            forbidden = (operation.role is not UNSET, bool(layers), operation.mask_margin_nm is not UNSET, operation.pad_size_x_nm is not UNSET, operation.pad_size_y_nm is not UNSET, operation.stroke_color is not UNSET, operation.fill_color is not UNSET, operation.line_style is not UNSET)
            if record.dimension_type != "orthogonal" or marker_count > 1 or layer not in record.layers or operation.fill != "FILLED_SHAPE" or operation.diameter_nm != 200_000 or operation.width_nm != 0 or any(forbidden):
                raise msgspec.ValidationError(f"invalid_dimension at {operation_path}")
        else:
            raise msgspec.ValidationError(f"invalid_dimension at {operation_path}")


def _validate_board_text_payload(operation: TextOperation, path: str) -> None:
    markers = (operation.mirror, operation.text_as_polygons, operation.polyline_per_segment, operation.knockout)
    if any(marker is not UNSET and marker is not True for marker in markers):
        raise msgspec.ValidationError(f"invalid_board_text at {path}")
    if (operation.text_as_polygons is not UNSET) != (not operation.font_face):
        raise msgspec.ValidationError(f"invalid_board_text at {path}")
    has_cache = operation.render_cache is not UNSET
    polygons = [] if operation.render_cache_polygons is UNSET else operation.render_cache_polygons
    if has_cache != (operation.render_cache_source is not UNSET) or has_cache != (operation.render_cache_exact is not UNSET) or has_cache == (not polygons):
        raise msgspec.ValidationError(f"invalid_board_text at {path}")
    if not has_cache:
        if operation.knockout is not UNSET:
            raise msgspec.ValidationError(f"invalid_board_text at {path}")
        return
    cache = operation.render_cache
    if cache.schema != "kicad.render_cache.v1" or cache.unit != "nm" or cache.coordinate_space != "board" or cache.source != operation.render_cache_source or cache.text != operation.text or cache.angle != operation.orient_deg or cache.exact != operation.render_cache_exact or cache.knockout != operation.knockout:
        raise msgspec.ValidationError(f"invalid_board_text at {path}")
    if len(cache.polygons) != len(polygons):
        raise msgspec.ValidationError(f"invalid_board_text at {path}")
    for polygon, exterior in zip(cache.polygons, polygons):
        if not polygon.contours or any(len(contour) < 3 for contour in polygon.contours) or polygon.contours[0] != exterior:
            raise msgspec.ValidationError(f"invalid_board_text at {path}")


def _validate_board_footprint_plot_record(record: BoardFootprintPlotRecord, path: str) -> None:
    if record.object_id != record.library_link or not math.isfinite(record.placement.angle_deg):
        raise msgspec.ValidationError(f"invalid_board_footprint at {path}")
    operation_index = 0
    pad_phase = False
    last_key = None
    while operation_index < len(record.operations):
        operation = record.operations[operation_index]
        operation_path = f'{path}.operations[{operation_index}]'
        if isinstance(operation, BoardFootprintStartBlockOperation):
            pad_phase = True
            if operation_index + 2 >= len(record.operations) or not isinstance(record.operations[operation_index + 2], BoardFootprintEndBlockOperation):
                raise msgspec.ValidationError(f"invalid_board_footprint at {operation_path}")
            inner = record.operations[operation_index + 1]
            end = record.operations[operation_index + 2]
            _validate_board_footprint_header(operation, operation_index, 'StartBlock', operation_path)
            _validate_board_footprint_header(inner, operation_index + 1, _board_footprint_expected_kind(inner), f'{path}.operations[{operation_index + 1}]')
            _validate_board_footprint_header(end, operation_index + 2, 'EndBlock', f'{path}.operations[{operation_index + 2}]')
            _validate_board_footprint_pad_block(record, operation, inner, operation_path)
            operation_index += 3
            continue
        if pad_phase or isinstance(operation, BoardFootprintEndBlockOperation):
            raise msgspec.ValidationError(f"invalid_board_footprint at {operation_path}")
        key = _validate_board_footprint_child(record, operation, operation_index, operation_path)
        if last_key is not None and last_key >= key:
            raise msgspec.ValidationError(f"invalid_board_footprint_order at {operation_path}")
        last_key = key
        operation_index += 1


def _validate_board_footprint_header(operation: object, index: int, kind: str, path: str) -> None:
    if operation.index != index:
        raise msgspec.ValidationError(f"invalid_board_footprint_header at {path}")


def _board_footprint_expected_kind(operation: object) -> str:
    kinds = ((BoardFootprintThickSegmentOperation, 'ThickSegment'), (BoardFootprintArcThreePointOperation, 'ArcThreePoint'), (BoardFootprintCircleOperation, 'Circle'), (BoardFootprintRectOperation, 'Rect'), (BoardFootprintPlotPolyOperation, 'PlotPoly'), (BoardFootprintBezierCurveOperation, 'BezierCurve'), (BoardFootprintTextOperation, 'Text'), (BoardFootprintFlashPadCircleOperation, 'FlashPadCircle'), (BoardFootprintFlashPadOvalOperation, 'FlashPadOval'), (BoardFootprintFlashPadRectOperation, 'FlashPadRect'), (BoardFootprintFlashPadRoundRectOperation, 'FlashPadRoundRect'), (BoardFootprintFlashPadCustomOperation, 'FlashPadCustom'), (BoardFootprintFlashPadTrapezOperation, 'FlashPadTrapez'), (BoardFootprintStartBlockOperation, 'StartBlock'), (BoardFootprintEndBlockOperation, 'EndBlock'))
    for operation_type, kind in kinds:
        if isinstance(operation, operation_type):
            return kind
    raise msgspec.ValidationError("invalid_board_footprint_operation")


def _validate_board_footprint_child(record: BoardFootprintPlotRecord, operation: object, index: int, path: str) -> tuple[int, int, int]:
    allowed = (BoardFootprintThickSegmentOperation, BoardFootprintArcThreePointOperation, BoardFootprintCircleOperation, BoardFootprintRectOperation, BoardFootprintPlotPolyOperation, BoardFootprintTextOperation)
    if not isinstance(operation, allowed):
        raise msgspec.ValidationError(f"invalid_board_footprint_child at {path}")
    _validate_board_footprint_header(operation, index, _board_footprint_expected_kind(operation), path)
    metadata = (operation.label, operation.data_uuid, operation.data_ref, operation.object_id, operation.extra_attrs)
    if any(value is UNSET for value in metadata):
        raise msgspec.ValidationError(f"invalid_board_footprint_metadata at {path}")
    attrs = operation.extra_attrs
    layer = None if operation.layer is UNSET else operation.layer
    layer_name = None if attrs.layer_name is UNSET else attrs.layer_name
    if not operation.label or not operation.data_uuid or not operation.object_id or operation.data_ref != attrs.footprint_primitive or attrs.component != record.reference or attrs.component_uid != record.uuid or attrs.component_uuid != record.uuid or attrs.footprint != record.library_link or layer_name != layer or (attrs.layer_name is UNSET) != (attrs.layer_role is UNSET) or (layer is not None and attrs.layer_role != _board_footprint_layer_role(layer)):
        raise msgspec.ValidationError(f"invalid_board_footprint_metadata at {path}")
    _validate_board_footprint_child_shape(operation, attrs, path)
    phases = {'property': 0, 'fp_text': 1, 'fp_text_box': 2, 'fp_line': 3, 'fp_arc': 4, 'fp_circle': 5, 'fp_rect': 6, 'fp_poly': 7}
    sub_index = 0 if attrs.footprint_subop_index is UNSET else attrs.footprint_subop_index
    return (phases[operation.data_ref], attrs.footprint_object_index, sub_index)


def _validate_board_footprint_child_shape(operation: object, attrs: BoardFootprintChildAttrs, path: str) -> None:
    data_ref = operation.data_ref
    if isinstance(operation, BoardFootprintTextOperation):
        valid_ref = data_ref in ('property', 'fp_text', 'fp_text_box')
        valid_attrs = attrs.primitive == 'footprint-text' and attrs.footprint_text_role is not UNSET and attrs.footprint_graphic_kind is UNSET and ((data_ref == 'property') == (attrs.property_name is not UNSET)) and ((data_ref == 'fp_text') == (attrs.fp_text_type is not UNSET))
        _validate_board_footprint_text(operation, path)
    else:
        expected = None
        if isinstance(operation, BoardFootprintThickSegmentOperation): expected = 'text-box-border' if data_ref == 'fp_text_box' else 'line'
        elif isinstance(operation, BoardFootprintArcThreePointOperation): expected = 'arc'
        elif isinstance(operation, BoardFootprintCircleOperation): expected = 'circle'
        elif isinstance(operation, BoardFootprintRectOperation): expected = 'text-box-border' if data_ref == 'fp_text_box' else 'rect'
        elif isinstance(operation, BoardFootprintPlotPolyOperation): expected = 'poly'
        valid_refs = {BoardFootprintThickSegmentOperation: ('fp_text_box', 'fp_line'), BoardFootprintArcThreePointOperation: ('fp_arc',), BoardFootprintCircleOperation: ('fp_circle',), BoardFootprintRectOperation: ('fp_text_box', 'fp_rect'), BoardFootprintPlotPolyOperation: ('fp_poly',)}
        valid_ref = data_ref in valid_refs[type(operation)]
        valid_attrs = attrs.primitive == 'footprint-graphic' and attrs.footprint_text_role is UNSET and attrs.property_name is UNSET and attrs.fp_text_type is UNSET and attrs.footprint_graphic_kind == expected
    subop_required = data_ref in ('fp_text_box', 'fp_line', 'fp_arc')
    if not valid_ref or not valid_attrs or ((attrs.footprint_subop_index is not UNSET) != subop_required):
        raise msgspec.ValidationError(f"invalid_board_footprint_shape at {path}")


def _board_footprint_layer_role(layer: str) -> str:
    if layer.endswith('.Cu') or layer in ('*.Cu', 'F&B.Cu'): return 'copper'
    if layer.endswith('.SilkS'): return 'silkscreen'
    if layer.endswith('.Mask') or layer == '*.Mask': return 'soldermask'
    if layer.endswith('.Paste'): return 'paste'
    if layer.endswith('.Fab'): return 'fab'
    if layer.endswith('.Courtyard'): return 'courtyard'
    if layer == 'Edge.Cuts': return 'board-outline'
    if layer == 'DRILLS': return 'drill'
    if layer.endswith('.User') or layer.startswith('User.'): return 'user'
    return 'other'


def _validate_board_footprint_text(operation: BoardFootprintTextOperation, path: str) -> None:
    if not math.isfinite(operation.orient_deg) or operation.mirror is not UNSET or operation.text_as_polygons is not UNSET or operation.polyline_per_segment is not UNSET or operation.knockout is False:
        raise msgspec.ValidationError(f"invalid_board_footprint_cache at {path}")
    has_cache = operation.render_cache is not UNSET
    polygons = [] if operation.render_cache_polygons is UNSET else operation.render_cache_polygons
    if has_cache != (operation.render_cache_source is not UNSET) or has_cache != (operation.render_cache_exact is not UNSET) or has_cache == (not polygons):
        raise msgspec.ValidationError(f"invalid_board_footprint_cache at {path}")
    if not has_cache:
        if operation.knockout is not UNSET: raise msgspec.ValidationError(f'invalid_board_footprint_cache at {path}')
        return
    cache = operation.render_cache
    if cache.schema != 'kicad.render_cache.v1' or cache.unit != 'nm' or cache.coordinate_space != 'footprint_local' or cache.source != operation.render_cache_source or cache.text != operation.text or not math.isfinite(cache.angle) or cache.exact != operation.render_cache_exact or cache.knockout != operation.knockout or len(cache.polygons) != len(polygons):
        raise msgspec.ValidationError(f"invalid_board_footprint_cache at {path}")
    for polygon, exterior in zip(cache.polygons, polygons):
        if not polygon.contours or any(len(contour) < 3 for contour in polygon.contours) or polygon.contours[0] != exterior:
            raise msgspec.ValidationError(f"invalid_board_footprint_cache at {path}")


def _validate_board_footprint_pad_block(record: BoardFootprintPlotRecord, start: BoardFootprintStartBlockOperation, inner: object, path: str) -> None:
    attrs = start.extra_attrs
    expected_component = record.reference if record.reference else UNSET
    expected_uuid = record.uuid if record.uuid else UNSET
    expected_footprint = record.library_link if record.library_link else UNSET
    pad_number_valid = (attrs.pad_number == start.object_id) if attrs.pad_number is not UNSET else start.object_id == 'pad'
    expected_designator = UNSET if attrs.pad_number is UNSET else (f'{record.reference}-{attrs.pad_number}' if record.reference else attrs.pad_number)
    inner_layers_value = getattr(inner, 'layers', UNSET)
    inner_layers = [] if inner_layers_value is UNSET else inner_layers_value
    expected_layer_names = ','.join(inner_layers) if inner_layers else UNSET
    common = attrs.component == expected_component and attrs.component_uid == expected_uuid and attrs.component_uuid == expected_uuid and attrs.footprint == expected_footprint and pad_number_valid and attrs.pad_designator == expected_designator and (attrs.pad_type is UNSET or bool(attrs.pad_type)) and (attrs.pad_shape is UNSET or bool(attrs.pad_shape)) and attrs.layer_names == expected_layer_names and start.label == start.data_uuid
    metadata = tuple(getattr(inner, name, UNSET) for name in ('label', 'data_uuid', 'data_ref', 'object_id', 'extra_attrs'))
    if not common or any(value is not UNSET for value in metadata):
        raise msgspec.ValidationError(f"invalid_board_footprint_pad at {path}")
    if start.data_ref == 'pad':
        hole_names = ('hole_owner', 'hole_kind', 'hole_plating', 'hole_render', 'hole_width_mm', 'hole_height_mm', 'hole_diameter_mm')
        layers = [] if start.layers is UNSET else start.layers
        valid = attrs.primitive == 'pad' and all(getattr(attrs, name) is UNSET for name in hole_names) and bool(layers) and isinstance(inner, (BoardFootprintFlashPadCircleOperation, BoardFootprintFlashPadOvalOperation, BoardFootprintFlashPadRectOperation, BoardFootprintFlashPadRoundRectOperation, BoardFootprintFlashPadCustomOperation, BoardFootprintFlashPadTrapezOperation)) and inner.layers == layers
        if isinstance(inner, BoardFootprintFlashPadCircleOperation): valid = valid and inner.mask_margin_nm is not UNSET and inner.role is UNSET
        if isinstance(inner, BoardFootprintFlashPadCustomOperation): valid = valid and (inner.polygon_widths_nm is UNSET or not inner.polygon_widths_nm or len(inner.polygon_widths_nm) == len(inner.polygons))
    else:
        round_hole = attrs.hole_kind == 'round' and attrs.hole_diameter_mm is not UNSET and attrs.hole_width_mm is UNSET and attrs.hole_height_mm is UNSET
        slot_hole = attrs.hole_kind == 'slot' and attrs.hole_diameter_mm is UNSET and attrs.hole_width_mm is not UNSET and attrs.hole_height_mm is not UNSET
        valid = attrs.primitive == 'pad-hole' and start.label.endswith(':hole') and attrs.hole_owner == start.label[:-5] and attrs.hole_plating in ('plated', 'non_plated') and attrs.hole_render == 'drill' and (round_hole or slot_hole) and isinstance(inner, (BoardFootprintCircleOperation, BoardFootprintThickSegmentOperation)) and inner.layer is UNSET and bool(inner.layers)
        if valid and attrs.hole_plating == 'plated': valid = inner.role == 'pad_drill' and inner.mask_margin_nm is UNSET and inner.pad_size_x_nm is UNSET and inner.pad_size_y_nm is UNSET
        elif valid: valid = inner.role == 'npth_hole' and inner.mask_margin_nm is not UNSET and inner.pad_size_x_nm is not UNSET and inner.pad_size_y_nm is not UNSET
    if not valid:
        raise msgspec.ValidationError(f"invalid_board_footprint_pad at {path}")
decode_board_plot_request_a0 = msgspec.json.Decoder(BoardPlotRequestA0).decode
decode_board_plot_result_a0 = msgspec.json.Decoder(BoardPlotResultA0).decode
_symbol_plot_document_a0_decoder = msgspec.json.Decoder(SymbolPlotDocumentA0)


def decode_symbol_plot_document_a0(data: bytes) -> SymbolPlotDocumentA0:
    value = _symbol_plot_document_a0_decoder.decode(data)
    validate_symbol_plot_document_a0(value)
    return value


def validate_symbol_plot_document_a0(value: SymbolPlotDocumentA0) -> None:
    if value.schema != "kicad.plotter_ir.a0" or value.source_kind != "SYM" or value.coordinate_space.unit != "nm" or value.coordinate_space.y_axis != "down":
        raise msgspec.ValidationError("invalid_symbol_document at $")
    if not value.records or not isinstance(value.records[0], SymbolHeaderPlotRecord):
        raise msgspec.ValidationError("missing_symbol_header at $.records[0]")
    total_operations = 0
    for record_index, record in enumerate(value.records):
        if isinstance(record, SymbolHeaderPlotRecord):
            if record_index != 0 or record.object_id != record.name or record.operation_count != 0 or record.operations:
                raise msgspec.ValidationError(f"invalid_symbol_header at $.records[{record_index}]")
        elif not record.object_id:
            raise msgspec.ValidationError(f"invalid_symbol_record at $.records[{record_index}]")
        if record.operation_count != len(record.operations):
            raise msgspec.ValidationError(f"operation_count_mismatch at $.records[{record_index}].operation_count")
        total_operations += len(record.operations)
        for operation_index, operation in enumerate(record.operations):
            path = f"$.records[{record_index}].operations[{operation_index}]"
            if operation.index != total_operations - len(record.operations) + operation_index:
                raise msgspec.ValidationError(f"operation_index_mismatch at {path}.index")
            allowed = isinstance(operation, (ArcThreePointOperation, CircleOperation, RectOperation, PlotPolyOperation, BezierCurveOperation, TextOperation))
            layer = None if not hasattr(operation, 'layer') or operation.layer is UNSET else operation.layer
            if not allowed or (not isinstance(operation, TextOperation) and layer is not None):
                raise msgspec.ValidationError(f"invalid_symbol_operation at {path}")
            if isinstance(operation, CircleOperation):
                role = None if operation.role is UNSET else operation.role
                layers = [] if operation.layers is UNSET else operation.layers
                if role is not None or layers or operation.mask_margin_nm is not UNSET or operation.pad_size_x_nm is not UNSET or operation.pad_size_y_nm is not UNSET:
                    raise msgspec.ValidationError(f"invalid_symbol_operation at {path}")
            if isinstance(operation, TextOperation):
                forbidden = (
                    layer is not None,
                    operation.mirror is not UNSET,
                    operation.text_as_polygons is not UNSET,
                    operation.polyline_per_segment is not UNSET,
                    operation.knockout is not UNSET,
                    operation.render_cache_polygons is not UNSET,
                    operation.render_cache is not UNSET,
                    operation.render_cache_source is not UNSET,
                    operation.render_cache_exact is not UNSET,
                )
                if any(forbidden):
                    raise msgspec.ValidationError(f"invalid_symbol_text at {path}")
    if value.total_operations != total_operations:
        raise msgspec.ValidationError("operation_count_mismatch at $.total_operations")
decode_symbol_plot_request_a0 = msgspec.json.Decoder(SymbolPlotRequestA0).decode
decode_symbol_plot_result_a0 = msgspec.json.Decoder(SymbolPlotResultA0).decode
_schematic_plot_document_a0_decoder = msgspec.json.Decoder(SchematicPlotDocumentA0)


def decode_schematic_plot_document_a0(data: bytes) -> SchematicPlotDocumentA0:
    value = _schematic_plot_document_a0_decoder.decode(data)
    validate_schematic_plot_document_a0(value)
    return value


def validate_schematic_plot_document_a0(value: SchematicPlotDocumentA0) -> None:
    if value.schema != "kicad.plotter_ir.a0" or value.source_kind != "SCH" or value.coordinate_space.unit != "nm" or value.coordinate_space.y_axis != "down":
        raise msgspec.ValidationError("invalid_schematic_document at $")
    if not value.records or not isinstance(value.records[0], SchematicSheetHeaderPlotRecord):
        raise msgspec.ValidationError("missing_sheet_header at $.records[0]")
    phases = {SchematicSheetHeaderPlotRecord: 0, SchematicWirePlotRecord: 1, SchematicBusPlotRecord: 2, SchematicBusEntryPlotRecord: 3, SchematicJunctionPlotRecord: 4, SchematicNoConnectPlotRecord: 5}
    previous_phase = -1
    total_operations = 0
    for record_index, record in enumerate(value.records):
        path = f'$.records[{record_index}]'
        phase = phases[type(record)]
        if phase < previous_phase or (phase == 0 and record_index != 0):
            raise msgspec.ValidationError(f"invalid_schematic_record_order at {path}")
        previous_phase = phase
        if record.object_id != record.uuid:
            raise msgspec.ValidationError(f"invalid_schematic_record_identity at {path}")
        if record.operation_count != len(record.operations):
            raise msgspec.ValidationError(f"operation_count_mismatch at {path}.operation_count")
        for operation_index, operation in enumerate(record.operations):
            if operation.index != operation_index:
                raise msgspec.ValidationError(f"operation_index_mismatch at {path}.operations[{operation_index}].index")
        if isinstance(record, SchematicSheetHeaderPlotRecord):
            _validate_schematic_sheet_header(value, record, path)
        elif isinstance(record, (SchematicWirePlotRecord, SchematicBusPlotRecord, SchematicBusEntryPlotRecord)):
            _validate_schematic_polyline_record(record, path)
        elif isinstance(record, SchematicJunctionPlotRecord):
            _validate_schematic_junction_record(record, path)
        else:
            _validate_schematic_no_connect_record(record, path)
        total_operations += len(record.operations)
    if value.total_operations != total_operations:
        raise msgspec.ValidationError("operation_count_mismatch at $.total_operations")


def _validate_schematic_sheet_header(value: SchematicPlotDocumentA0, record: SchematicSheetHeaderPlotRecord, path: str) -> None:
    if value.canvas.width_nm != record.sheet_width_nm or value.canvas.height_nm != record.sheet_height_nm or record.sheet_width_nm <= 0 or record.sheet_height_nm <= 0:
        raise msgspec.ValidationError(f"invalid_sheet_header at {path}")
    if not record.operations or not isinstance(record.operations[0], RectOperation):
        raise msgspec.ValidationError(f"invalid_sheet_background at {path}.operations[0]")
    background = record.operations[0]
    background_layer = None if background.layer is UNSET else background.layer
    if (background.x1, background.y1, background.x2, background.y2) != (0, 0, record.sheet_width_nm, record.sheet_height_nm) or background.fill != 'FILLED_SHAPE' or background.width_nm != 100 or background.corner_radius_nm != 0 or background_layer is not None or background.stroke_color != '#F5F4EFFF' or background.fill_color != '#F5F4EFFF':
        raise msgspec.ValidationError(f"invalid_sheet_background at {path}.operations[0]")
    for operation_index, operation in enumerate(record.operations[1:], start=1):
        operation_path = f'{path}.operations[{operation_index}]'
        if not isinstance(operation, (RectOperation, PlotPolyOperation, TextOperation, PlotImageOperation)):
            raise msgspec.ValidationError(f"invalid_worksheet_operation at {operation_path}")
        layer = None if not hasattr(operation, 'layer') or operation.layer is UNSET else operation.layer
        if layer is not None:
            raise msgspec.ValidationError(f"invalid_worksheet_operation at {operation_path}")
        if isinstance(operation, RectOperation) and (operation.fill != 'NO_FILL' or operation.width_nm < 152_400 or operation.corner_radius_nm != 0 or operation.stroke_color != '#840000FF' or operation.fill_color is not UNSET or operation.line_style is not UNSET):
            raise msgspec.ValidationError(f"invalid_worksheet_rect at {operation_path}")
        if isinstance(operation, PlotPolyOperation) and (len(operation.points) != 2 or operation.fill != 'NO_FILL' or operation.width_nm < 152_400 or operation.stroke_color != '#840000FF' or operation.fill_color is not UNSET or operation.line_style is not UNSET):
            raise msgspec.ValidationError(f"invalid_worksheet_polyline at {operation_path}")
        if isinstance(operation, TextOperation):
            forbidden = (operation.mirror is not UNSET, operation.text_as_polygons is not UNSET, operation.polyline_per_segment is not UNSET, operation.knockout is not UNSET, operation.render_cache_polygons is not UNSET, operation.render_cache is not UNSET, operation.render_cache_source is not UNSET, operation.render_cache_exact is not UNSET)
            if any(forbidden) or not math.isfinite(operation.orient_deg):
                raise msgspec.ValidationError(f"invalid_worksheet_text at {operation_path}")
        if isinstance(operation, PlotImageOperation) and (operation.image_format != 'png' or not math.isfinite(operation.scale) or operation.scale <= 0 or operation.width_nm < 0 or operation.height_nm < 0 or operation.stroke_color != '#840000FF' or not _valid_schematic_png_base64(operation.image_data_b64)):
            raise msgspec.ValidationError(f"invalid_worksheet_image at {operation_path}")


def _valid_schematic_png_base64(value: str) -> bool:
    prefix = bytearray()
    quartet: list[int] = []
    ended = False
    for character in value:
        if character in ' \t\r\n\v\f':
            return False
        if ended:
            return False
        code = ord(character)
        if 65 <= code <= 90: sextet = code - 65
        elif 97 <= code <= 122: sextet = code - 97 + 26
        elif 48 <= code <= 57: sextet = code - 48 + 52
        elif character == '+': sextet = 62
        elif character == '/': sextet = 63
        elif character == '=': sextet = 64
        else: return False
        quartet.append(sextet)
        if len(quartet) != 4:
            continue
        if quartet[0] >= 64 or quartet[1] >= 64:
            return False
        if quartet[2] == 64:
            if quartet[3] != 64 or quartet[1] & 0x0F:
                return False
            decoded_len = 1
            ended = True
        elif quartet[3] == 64:
            if quartet[2] & 0x03:
                return False
            decoded_len = 2
            ended = True
        else:
            decoded_len = 3
        decoded = ((quartet[0] << 2) | (quartet[1] >> 4), ((quartet[1] << 4) | (quartet[2] >> 2)) & 0xFF, ((quartet[2] << 6) | quartet[3]) & 0xFF)
        prefix.extend(decoded[:min(decoded_len, 33 - len(prefix))])
        quartet.clear()
    if quartet or len(prefix) < 33:
        return False
    width = int.from_bytes(prefix[16:20], 'big')
    height = int.from_bytes(prefix[20:24], 'big')
    return prefix[:8] == b'\x89PNG\r\n\x1a\n' and prefix[8:12] == b'\x00\x00\x00\r' and prefix[12:16] == b'IHDR' and width > 0 and height > 0


def _validate_schematic_polyline_record(record: SchematicWirePlotRecord | SchematicBusPlotRecord | SchematicBusEntryPlotRecord, path: str) -> None:
    if len(record.operations) != 1 or not isinstance(record.operations[0], PlotPolyOperation):
        raise msgspec.ValidationError(f"invalid_connectivity_record at {path}")
    operation = record.operations[0]
    layer = None if operation.layer is UNSET else operation.layer
    if layer is not None or operation.fill != 'NO_FILL' or operation.width_nm < 0 or operation.stroke_color is UNSET or not operation.stroke_color or operation.line_style is UNSET or not operation.points:
        raise msgspec.ValidationError(f"invalid_connectivity_polyline at {path}.operations[0]")
    if isinstance(record, SchematicBusEntryPlotRecord) and len(operation.points) != 2:
        raise msgspec.ValidationError(f"invalid_bus_entry at {path}.operations[0].points")


def _validate_schematic_junction_record(record: SchematicJunctionPlotRecord, path: str) -> None:
    if len(record.operations) != 1 or not isinstance(record.operations[0], CircleOperation):
        raise msgspec.ValidationError(f"invalid_junction at {path}")
    operation = record.operations[0]
    layer = None if operation.layer is UNSET else operation.layer
    role = None if operation.role is UNSET else operation.role
    layers = [] if operation.layers is UNSET else operation.layers
    forbidden = (role is not None, bool(layers), operation.mask_margin_nm is not UNSET, operation.pad_size_x_nm is not UNSET, operation.pad_size_y_nm is not UNSET)
    if layer is not None or any(forbidden) or operation.fill != 'FILLED_SHAPE' or operation.width_nm != 0 or operation.diameter_nm <= 0 or operation.stroke_color is UNSET or operation.fill_color is UNSET or operation.stroke_color != operation.fill_color:
        raise msgspec.ValidationError(f"invalid_junction at {path}.operations[0]")
    expected_color = '#009600FF' if record.color is UNSET or record.color is None else record.color
    if expected_color != operation.stroke_color:
        raise msgspec.ValidationError(f"invalid_junction_color at {path}.color")


def _validate_schematic_no_connect_record(record: SchematicNoConnectPlotRecord, path: str) -> None:
    if len(record.operations) != 2 or not all(isinstance(operation, PlotPolyOperation) for operation in record.operations):
        raise msgspec.ValidationError(f"invalid_no_connect at {path}")
    first, second = record.operations
    for operation_index, operation in enumerate((first, second)):
        layer = None if operation.layer is UNSET else operation.layer
        if layer is not None or operation.fill != 'NO_FILL' or operation.width_nm <= 0 or operation.stroke_color != '#000084FF' or operation.line_style is not UNSET or len(operation.points) != 2:
            raise msgspec.ValidationError(f"invalid_no_connect at {path}.operations[{operation_index}]")
    if first.width_nm != second.width_nm or first.points[0][0] != second.points[0][0] or first.points[1][0] != second.points[1][0] or first.points[0][1] != second.points[1][1] or first.points[1][1] != second.points[0][1]:
        raise msgspec.ValidationError(f"invalid_no_connect_geometry at {path}.operations")
decode_schematic_plot_request_a0 = msgspec.json.Decoder(SchematicPlotRequestA0).decode
decode_schematic_plot_result_a0 = msgspec.json.Decoder(SchematicPlotResultA0).decode
decode_symbol_library_edit_request_a0 = msgspec.json.Decoder(SymbolLibraryEditRequestA0).decode
decode_symbol_library_edit_result_a0 = msgspec.json.Decoder(SymbolLibraryEditResultA0).decode
decode_symbol_library_read_request_a0 = msgspec.json.Decoder(SymbolLibraryReadRequestA0).decode
decode_symbol_library_read_result_a0 = msgspec.json.Decoder(SymbolLibraryReadResultA0).decode
decode_compiled_schematic_graph_a0 = msgspec.json.Decoder(CompiledSchematicGraphA0).decode
_source_bundle_manifest_a0_decoder = msgspec.json.Decoder(SourceBundleManifestA0)


def decode_source_bundle_manifest_a0(data: bytes) -> SourceBundleManifestA0:
    value = _source_bundle_manifest_a0_decoder.decode(data)
    for source in value.sources:
        if int(source.source_bytes) > 18_446_744_073_709_551_615:
            raise msgspec.ValidationError("source_bytes exceeds uint64")
    return value
@dataclass(frozen=True, slots=True)
class _ValidatedFontBundleA0:
    manifest: FontBundleManifestA0
    id_index: dict[str, int]
    alias_index: dict[str, int | None]


_font_bundle_manifest_a0_decoder = msgspec.json.Decoder(FontBundleManifestA0)


def decode_font_bundle_manifest_a0(data: bytes) -> FontBundleManifestA0:
    return _font_bundle_manifest_a0_decoder.decode(data)


def validate_font_bundle_manifest_a0(
    value: FontBundleManifestA0,
    buffers: list[bytes] | tuple[bytes, ...],
    *,
    max_fonts: int = 4_096,
    max_font_bytes: int = 256 * 1024 * 1024,
    max_total_font_bytes: int = 1024 * 1024 * 1024,
    max_aliases_per_font: int = 4_096,
    max_variations_per_font: int = 4_096,
    max_metadata_string_bytes: int = 64 * 1024 * 1024,
) -> _ValidatedFontBundleA0:
    if value.schema != "kicad_monkey.font_bundle.a0" or value.type_ != "kicad_monkey.font_bundle" or value.version != "a0":
        raise msgspec.ValidationError("unsupported_contract at $")
    limits = (max_fonts, max_font_bytes, max_total_font_bytes, max_aliases_per_font, max_variations_per_font, max_metadata_string_bytes)
    if any(limit < 0 for limit in limits):
        raise msgspec.ValidationError("invalid_limit at $")
    if len(value.fonts) > max_fonts:
        raise msgspec.ValidationError("resource_limit at $.fonts")
    if len(value.fonts) != len(buffers):
        raise msgspec.ValidationError("buffer_count_mismatch at $.fonts")
    ids: set[str] = set()
    slots: set[int] = set()
    id_index: dict[str, int] = {}
    alias_index: dict[str, int | None] = {}
    total_bytes = 0
    metadata_string_bytes = 0
    for index, font in enumerate(value.fonts):
        path = f"$.fonts[{index}]"
        if not font.id or font.id in ids:
            raise msgspec.ValidationError(f"duplicate_font_id at {path}.id")
        _validate_font_text_identity(font.id, f'{path}.id')
        ids.add(font.id)
        id_index[font.id] = index
        if font.slot in slots:
            raise msgspec.ValidationError(f"duplicate_font_slot at {path}.slot")
        slots.add(font.slot)
        if font.slot >= len(buffers):
            raise msgspec.ValidationError(f"invalid_slot at {path}.slot")
        if len(font.sha256) != 64 or any(char not in '0123456789abcdef' for char in font.sha256):
            raise msgspec.ValidationError(f"invalid_hash at {path}.sha256")
        if len(font.aliases) > max_aliases_per_font or len(font.variations) > max_variations_per_font:
            raise msgspec.ValidationError(f"resource_limit at {path}")
        if any(not alias for alias in font.aliases) or len(set(font.aliases)) != len(font.aliases):
            raise msgspec.ValidationError(f"invalid_alias at {path}.aliases")
        axes: set[str] = set()
        for variation_index, variation in enumerate(font.variations):
            axis = variation.axis
            if len(axis) != 4 or any(ord(char) < 32 or ord(char) > 126 for char in axis) or not math.isfinite(variation.value) or axis in axes:
                raise msgspec.ValidationError(f"invalid_variation at {path}.variations[{variation_index}]")
            axes.add(axis)
        strings = [font.id, font.sha256, *font.aliases, *(variation.axis for variation in font.variations)]
        strings.extend(value for value in (font.family, font.style, font.postscript_name) if value is not UNSET)
        metadata_string_bytes += sum(_font_utf8_len(value) for value in strings)
        if metadata_string_bytes > max_metadata_string_bytes:
            raise msgspec.ValidationError("resource_limit at $.fonts")
        for alias in font.aliases:
            if alias in alias_index and alias_index[alias] != index:
                alias_index[alias] = None
            else:
                alias_index[alias] = index
        buffer = buffers[font.slot]
        if len(buffer) > max_font_bytes:
            raise msgspec.ValidationError(f"resource_limit at {path}.slot")
        total_bytes += len(buffer)
        if total_bytes > max_total_font_bytes:
            raise msgspec.ValidationError("resource_limit at $.fonts")
    for index, font in enumerate(value.fonts):
        if hashlib.sha256(buffers[font.slot]).hexdigest() != font.sha256:
            path = f"$.fonts[{index}]"
            raise msgspec.ValidationError(f"hash_mismatch at {path}.sha256")
    return _ValidatedFontBundleA0(value, id_index, alias_index)


def resolve_font_selection_a0(
    bundle: _ValidatedFontBundleA0,
    request: FontResolutionRequestA0,
    *,
    max_request_aliases: int = 4_096,
    max_request_string_bytes: int = 16 * 1024 * 1024,
) -> FontBundleEntry:
    if request.schema != "kicad_monkey.font_resolution_request.a0" or request.type_ != "kicad_monkey.font_resolution_request" or request.version != "a0":
        raise msgspec.ValidationError("unsupported_contract at $")
    if max_request_aliases < 0 or max_request_string_bytes < 0:
        raise msgspec.ValidationError("invalid_limit at $.selection")
    if len(request.selection.aliases) > max_request_aliases:
        raise msgspec.ValidationError("resource_limit at $.selection.aliases")
    font_id = None if request.selection.font_id is UNSET else request.selection.font_id
    request_strings = [*request.selection.aliases]
    if font_id is not None:
        _validate_font_text_identity(font_id, '$.selection.font_id')
        request_strings.append(font_id)
    if sum(_font_utf8_len(value) for value in request_strings) > max_request_string_bytes:
        raise msgspec.ValidationError("resource_limit at $.selection")
    if font_id == '':
        raise msgspec.ValidationError("invalid_selection at $.selection.font_id")
    if any(not alias for alias in request.selection.aliases) or len(set(request.selection.aliases)) != len(request.selection.aliases):
        raise msgspec.ValidationError("invalid_selection at $.selection.aliases")
    if font_id is not None:
        if font_id in bundle.id_index:
            return bundle.manifest.fonts[bundle.id_index[font_id]]
        raise msgspec.ValidationError("missing_font at $.selection.font_id")
    matched: int | None = None
    for alias in request.selection.aliases:
        if alias not in bundle.alias_index:
            continue
        target = bundle.alias_index[alias]
        if target is None or (matched is not None and matched != target):
            raise msgspec.ValidationError("ambiguous_font at $.selection.aliases")
        matched = target
    if matched is None:
        raise msgspec.ValidationError("missing_font at $.selection")
    return bundle.manifest.fonts[matched]


def _font_utf8_len(value: str) -> int:
    total = 0
    for char in value:
        codepoint = ord(char)
        total += 1 if codepoint < 0x80 else 2 if codepoint < 0x800 else 3 if codepoint < 0x10000 else 4
    return total


def _validate_font_text_identity(value: str, path: str) -> None:
    if not value or not value[0].isascii() or not value[0].isalnum() or any(
        not char.isascii() or (not char.isalnum() and char not in '._:-') for char in value[1:]
    ):
        raise msgspec.ValidationError(f"invalid_text_id at {path}")


def _font_tag_valid(value: str) -> bool:
    return len(value) == 4 and all(char.isascii() and ' ' <= char <= '~' for char in value)


def _validate_font_hash(value: str, path: str) -> None:
    if len(value) != 64 or any(char not in '0123456789abcdef' for char in value):
        raise msgspec.ValidationError(f"invalid_hash at {path}")


def _validate_font_variations(value: list[FontVariationCoordinate], path: str) -> None:
    axes: set[str] = set()
    for index, variation in enumerate(value):
        if not _font_tag_valid(variation.axis) or not math.isfinite(variation.value) or variation.axis in axes:
            raise msgspec.ValidationError(f"invalid_variation at {path}[{index}]")
        axes.add(variation.axis)
decode_font_resolution_request_a0 = msgspec.json.Decoder(FontResolutionRequestA0).decode
_shaping_record_a0_decoder = msgspec.json.Decoder(ShapingRecordA0)


def decode_shaping_record_a0(data: bytes) -> ShapingRecordA0:
    value = _shaping_record_a0_decoder.decode(data)
    validate_shaping_record_a0(value)
    return value


def validate_shaping_record_a0(value: ShapingRecordA0) -> None:
    if value.schema != "kicad_monkey.shaping_record.a0" or value.type_ != "kicad_monkey.shaping_record" or value.version != "a0":
        raise msgspec.ValidationError("unsupported_contract at $")
    if not isinstance(value.comparison, ExactComparisonPolicy):
        raise msgspec.ValidationError("invalid_comparison at $.comparison")
    if value.input.text_index_unit != "utf8_byte_offset":
        raise msgspec.ValidationError("invalid_text_index at $.input.text_index_unit")
    _validate_font_text_identity(value.case_id, '$.case_id')
    _validate_font_text_identity(value.input.font_id, '$.input.font_id')
    _validate_font_hash(value.input.font_sha256, '$.input.font_sha256')
    _validate_font_variations(value.input.variations, '$.input.variations')
    if value.input.script is not UNSET and not _font_tag_valid(value.input.script):
        raise msgspec.ValidationError("invalid_tag at $.input.script")
    if value.input.language is not UNSET and not value.input.language:
        raise msgspec.ValidationError("invalid_language at $.input.language")
    char_starts: set[int] = set()
    offset = 0
    for char in value.input.text:
        char_starts.add(offset)
        offset += _font_utf8_len(char)
    feature_endpoints = {*char_starts, offset}
    feature_tags: set[str] = set()
    for index, feature in enumerate(value.input.features):
        if not _font_tag_valid(feature.tag):
            raise msgspec.ValidationError(f"invalid_tag at $.input.features[{index}].tag")
        if feature.tag in feature_tags:
            raise msgspec.ValidationError(f"duplicate_feature_tag at $.input.features[{index}].tag")
        feature_tags.add(feature.tag)
        global_range = feature.start == 0 and feature.end == 4_294_967_295
        bounded = feature.start <= feature.end and feature.start in feature_endpoints and feature.end in feature_endpoints
        if not global_range and not bounded:
            raise msgspec.ValidationError(f"invalid_text_index at $.input.features[{index}]")
    for index, glyph in enumerate(value.glyphs):
        if glyph.cluster not in char_starts:
            raise msgspec.ValidationError(f"invalid_text_index at $.glyphs[{index}].cluster")
_outline_vector_a0_decoder = msgspec.json.Decoder(OutlineVectorA0)


def decode_outline_vector_a0(data: bytes) -> OutlineVectorA0:
    value = _outline_vector_a0_decoder.decode(data)
    validate_outline_vector_a0(value)
    return value


def validate_outline_vector_a0(value: OutlineVectorA0) -> None:
    if value.schema != "kicad_monkey.outline_vector.a0" or value.type_ != "kicad_monkey.outline_vector" or value.version != "a0":
        raise msgspec.ValidationError("unsupported_contract at $")
    if value.coordinate_format != "font_design_units_f64":
        raise msgspec.ValidationError("unsupported_contract at $.coordinate_format")
    _validate_font_text_identity(value.case_id, '$.case_id')
    _validate_font_text_identity(value.font_id, '$.font_id')
    _validate_font_hash(value.font_sha256, '$.font_sha256')
    _validate_font_variations(value.variations, '$.variations')
    if value.units_per_em <= 0:
        raise msgspec.ValidationError("invalid_units_per_em at $.units_per_em")
    comparison = value.coordinate_comparison
    if isinstance(comparison, AbsoluteToleranceComparisonPolicy):
        if not math.isfinite(comparison.absolute_tolerance) or comparison.absolute_tolerance < 0:
            raise msgspec.ValidationError("invalid_comparison at $.coordinate_comparison")
    elif not isinstance(comparison, ExactComparisonPolicy):
        raise msgspec.ValidationError("invalid_comparison at $.coordinate_comparison")
    for index, command in enumerate(value.commands):
        if isinstance(command, (OutlineMoveTo, OutlineLineTo)):
            coordinates = (command.x, command.y)
        elif isinstance(command, OutlineQuadTo):
            coordinates = (command.control_x, command.control_y, command.x, command.y)
        elif isinstance(command, OutlineCurveTo):
            coordinates = (command.control1_x, command.control1_y, command.control2_x, command.control2_y, command.x, command.y)
        else:
            coordinates = ()
        if any(not math.isfinite(coordinate) for coordinate in coordinates):
            raise msgspec.ValidationError(f"invalid_coordinate at $.commands[{index}]")


__all__ = (
    "Node",
    "NodeKind",
    "Diagnostic",
    "DiagnosticPhase",
    "SourcePosition",
    "Selector",
    "FormSpan",
    "FootprintProperty",
    "FootprintPlotRecord",
    "PlotterCoordinateSpace",
    "JavaScriptSafeInteger",
    "PlotterOperation",
    "ThickSegmentOperation",
    "ArcThreePointOperation",
    "CircleOperation",
    "RectOperation",
    "PlotPolyOperation",
    "BezierCurveOperation",
    "TextOperation",
    "PlotImageOperation",
    "FlashPadCircleOperation",
    "FlashPadOvalOperation",
    "FlashPadRectOperation",
    "FlashPadRoundRectOperation",
    "FlashPadCustomOperation",
    "FlashPadTrapezOperation",
    "PlotterDrillRole",
    "PlotterFill",
    "PlotterLineStyle",
    "PlotterPoint",
    "PlotterTextHAlign",
    "PlotterTextVAlign",
    "TextRenderCache",
    "PlotterTextRenderCacheSource",
    "PlotterViaFlashRole",
    "PlotterQuad",
    "PlotterTextRenderCacheCoordinateSpace",
    "TextRenderCachePolygon",
    "BoardPlotRecord",
    "BoardGraphicPlotRecord",
    "TrackSegmentPlotRecord",
    "TrackArcPlotRecord",
    "ViaPlotRecord",
    "TablePlotRecord",
    "DimensionPlotRecord",
    "ZoneFillPlotRecord",
    "BoardTextPlotRecord",
    "BoardTextBoxPlotRecord",
    "BoardFootprintPlotRecord",
    "BoardGraphicRecordKind",
    "BoardViaType",
    "PlotterStringBool",
    "BoardDimensionType",
    "BoardFootprintOperation",
    "BoardFootprintPlacement",
    "BoardFootprintThickSegmentOperation",
    "BoardFootprintArcThreePointOperation",
    "BoardFootprintCircleOperation",
    "BoardFootprintRectOperation",
    "BoardFootprintPlotPolyOperation",
    "BoardFootprintBezierCurveOperation",
    "BoardFootprintTextOperation",
    "BoardFootprintFlashPadCircleOperation",
    "BoardFootprintFlashPadOvalOperation",
    "BoardFootprintFlashPadRectOperation",
    "BoardFootprintFlashPadRoundRectOperation",
    "BoardFootprintFlashPadCustomOperation",
    "BoardFootprintFlashPadTrapezOperation",
    "BoardFootprintStartBlockOperation",
    "BoardFootprintEndBlockOperation",
    "BoardFootprintChildRef",
    "BoardFootprintChildAttrs",
    "BoardFootprintPadBlockAttrs",
    "BoardFootprintLayerRole",
    "BoardNetClassAssignment",
    "BoardTextVariable",
    "SymbolPlotRecord",
    "SymbolHeaderPlotRecord",
    "LibSubsymbolPlotRecord",
    "SymbolTextVariable",
    "SchematicPlotRecord",
    "SchematicPlotCanvas",
    "SchematicSheetHeaderPlotRecord",
    "SchematicWirePlotRecord",
    "SchematicBusPlotRecord",
    "SchematicBusEntryPlotRecord",
    "SchematicJunctionPlotRecord",
    "SchematicNoConnectPlotRecord",
    "SchematicPlotTitleBlock",
    "RecordString",
    "SchematicWorksheetMode",
    "SchematicTextVariable",
    "SymbolBooleanField",
    "SymbolSummary",
    "UnitDefinition",
    "PageDefinition",
    "UnitOccurrence",
    "PageOccurrence",
    "HierarchyOccurrence",
    "ComponentOccurrence",
    "LocalNetOccurrence",
    "TerminalOccurrence",
    "HierarchyTerminalBinding",
    "GraphicalArtifactLink",
    "SourceIdentity",
    "TerminalRole",
    "ResolutionDiagnostic",
    "GraphicalTargetType",
    "SourceBundleSource",
    "SourceKind",
    "SourceSlot",
    "CanonicalUint64Decimal",
    "FontBundleEntry",
    "StableTextId",
    "Sha256Hex",
    "FontVariationCoordinate",
    "OpenTypeTag",
    "FiniteFloat",
    "FontSelection",
    "ExactComparisonPolicy",
    "ShapingInput",
    "ShapedGlyph",
    "TextDirection",
    "NonEmptyText",
    "ShapingFeature",
    "ShapingBufferProperties",
    "TextSafeInteger",
    "ShapingClusterLevel",
    "DefaultIgnorablePolicy",
    "CoordinateComparisonPolicy",
    "PositiveUint32",
    "OutlineCommand",
    "AbsoluteToleranceComparisonPolicy",
    "OutlineMoveTo",
    "OutlineLineTo",
    "OutlineQuadTo",
    "OutlineCurveTo",
    "OutlineClose",
    "NonNegativeFiniteFloat",
    "SExpressionBuildRequestA0",
    "SExpressionBuildResultA0",
    "SExpressionScanRequestA0",
    "SExpressionScanResultA0",
    "FootprintEditRequestA0",
    "FootprintEditResultA0",
    "FootprintReadRequestA0",
    "FootprintReadResultA0",
    "FootprintPlotDocumentA0",
    "FootprintPlotRequestA0",
    "FootprintPlotResultA0",
    "BoardPlotDocumentA0",
    "BoardPlotRequestA0",
    "BoardPlotResultA0",
    "SymbolPlotDocumentA0",
    "SymbolPlotRequestA0",
    "SymbolPlotResultA0",
    "SchematicPlotDocumentA0",
    "SchematicPlotRequestA0",
    "SchematicPlotResultA0",
    "SymbolLibraryEditRequestA0",
    "SymbolLibraryEditResultA0",
    "SymbolLibraryReadRequestA0",
    "SymbolLibraryReadResultA0",
    "CompiledSchematicGraphA0",
    "SourceBundleManifestA0",
    "FontBundleManifestA0",
    "FontResolutionRequestA0",
    "ShapingRecordA0",
    "OutlineVectorA0",
    "decode_sexpr_build_request_a0",
    "decode_sexpr_build_result_a0",
    "decode_sexpr_scan_request_a0",
    "decode_sexpr_scan_result_a0",
    "decode_footprint_edit_request_a0",
    "decode_footprint_edit_result_a0",
    "decode_footprint_read_request_a0",
    "decode_footprint_read_result_a0",
    "decode_footprint_plot_document_a0",
    "decode_footprint_plot_request_a0",
    "decode_footprint_plot_result_a0",
    "decode_board_plot_document_a0",
    "decode_board_plot_request_a0",
    "decode_board_plot_result_a0",
    "decode_symbol_plot_document_a0",
    "decode_symbol_plot_request_a0",
    "decode_symbol_plot_result_a0",
    "decode_schematic_plot_document_a0",
    "decode_schematic_plot_request_a0",
    "decode_schematic_plot_result_a0",
    "decode_symbol_library_edit_request_a0",
    "decode_symbol_library_edit_result_a0",
    "decode_symbol_library_read_request_a0",
    "decode_symbol_library_read_result_a0",
    "decode_compiled_schematic_graph_a0",
    "decode_source_bundle_manifest_a0",
    "decode_font_bundle_manifest_a0",
    "decode_font_resolution_request_a0",
    "decode_shaping_record_a0",
    "decode_outline_vector_a0",
    "validate_footprint_plot_document_a0",
    "validate_board_plot_document_a0",
    "resolve_font_selection_a0",
    "validate_font_bundle_manifest_a0",
    "validate_outline_vector_a0",
    "validate_shaping_record_a0",
    "validate_symbol_plot_document_a0",
    "validate_schematic_plot_document_a0",
)
