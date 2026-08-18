"""Generated strict msgspec transport bindings. Do not edit."""

from __future__ import annotations

import base64
import binascii
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
    stroke_color: str | UnsetType = field(default=UNSET)


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
    context: PlotterOperationContext | UnsetType = field(default=UNSET)
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


class PlotterOperationContext(Struct, forbid_unknown_fields=True, frozen=True):
    hyperlink: PlotterHyperlink


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


class PlotterHyperlink(Struct, forbid_unknown_fields=True, frozen=True):
    href: Annotated[str, Meta(min_length=1)]


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
    stroke_color: str | UnsetType = field(default=UNSET)
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
    context: PlotterOperationContext | UnsetType = field(default=UNSET)
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


SchematicPlotRecord = Union["SchematicSheetHeaderPlotRecord", "SchematicWirePlotRecord", "SchematicBusPlotRecord", "SchematicBusEntryPlotRecord", "SchematicJunctionPlotRecord", "SchematicNoConnectPlotRecord", "SchematicLabelPlotRecord", "SchematicGlobalLabelPlotRecord", "SchematicHierarchicalLabelPlotRecord", "SchematicNetclassFlagPlotRecord", "SchematicTextPlotRecord", "SchematicTextBoxPlotRecord", "SchematicGraphicPolylinePlotRecord", "SchematicGraphicArcPlotRecord", "SchematicGraphicCirclePlotRecord", "SchematicGraphicRectanglePlotRecord", "SchematicGraphicBezierPlotRecord", "SchematicRuleAreaPlotRecord", "SchematicImagePlotRecord", "SchematicTablePlotRecord", "SchematicSymbolInstancePlotRecord", "SchematicSymbolOverplotPlotRecord", "SchematicSheetPlotRecord"]


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


class SchematicLabelPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="label", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    text: str


class SchematicGlobalLabelPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="global_label", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    text: str
    shape: SchematicLabelShape


class SchematicHierarchicalLabelPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="hierarchical_label", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    text: str
    shape: SchematicLabelShape


class SchematicNetclassFlagPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="netclass_flag", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    at_x_nm: JavaScriptSafeInteger
    at_y_nm: JavaScriptSafeInteger
    shape: SchematicNetclassFlagShape
    length_nm: JavaScriptSafeInteger


class SchematicTextPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="text", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    text: str


class SchematicTextBoxPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="text_box", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    text: str


class SchematicGraphicPolylinePlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="graphic_polyline", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]


class SchematicGraphicArcPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="graphic_arc", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]


class SchematicGraphicCirclePlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="graphic_circle", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]


class SchematicGraphicRectanglePlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="graphic_rectangle", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]


class SchematicGraphicBezierPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="graphic_bezier", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]


class SchematicRuleAreaPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="rule_area", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    shape: SchematicRuleAreaShape
    locked: bool
    exclude_from_sim: bool
    in_bom: bool
    on_board: bool
    dnp: bool


class SchematicImagePlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="image", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    scale: float
    image_format: SchematicImageFormat
    width_nm: JavaScriptSafeInteger
    height_nm: JavaScriptSafeInteger


class SchematicTablePlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="table", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[PlotterOperation]
    cell_count: Annotated[int, Meta(ge=0, le=4294967295)]


class SchematicSymbolInstancePlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="symbol_instance", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[SchematicSymbolOperation]
    lib_id: str
    lib_name: str
    reference: str
    at_x_nm: JavaScriptSafeInteger
    at_y_nm: JavaScriptSafeInteger
    at_angle_deg: float
    mirror: str | None
    unit: Annotated[int, Meta(ge=0, le=4294967295)]
    convert: Annotated[int, Meta(ge=0, le=4294967295)]
    in_bom: bool
    on_board: bool
    dnp: bool
    exclude_from_sim: bool
    in_pos_files: bool


class SchematicSymbolOverplotPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="symbol_overplot", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[SchematicSymbolOperation]
    source_symbol_uuid: str
    lib_id: str


class SchematicSheetPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="sheet", tag_field="kind"):
    uuid: str
    object_id: str
    operation_count: Annotated[int, Meta(ge=0, le=4294967295)]
    operations: list[SchematicSheetOperation]
    sheet_name: str
    sheet_file: str
    at_x_nm: JavaScriptSafeInteger
    at_y_nm: JavaScriptSafeInteger
    size_x_nm: JavaScriptSafeInteger
    size_y_nm: JavaScriptSafeInteger
    dnp: bool


class SchematicPlotTitleBlock(Struct, forbid_unknown_fields=True, frozen=True):
    title: str
    date: str
    rev: str
    company: str
    comments: RecordString


SchematicLabelShape = Literal["input", "output", "bidirectional", "tri_state", "passive", "dot", "round", "diamond", "rectangle"]


SchematicNetclassFlagShape = Literal["round", "dot", "diamond", "rectangle"]


SchematicRuleAreaShape = Literal["polyline", "rectangle", "arc", "circle", "bezier"]


SchematicImageFormat = Literal["png", "jpeg", "bmp"]


SchematicSymbolOperation = Union["ThickSegmentOperation", "ArcThreePointOperation", "CircleOperation", "RectOperation", "PlotPolyOperation", "BezierCurveOperation", "TextOperation", "PlotImageOperation", "FlashPadCircleOperation", "FlashPadOvalOperation", "FlashPadRectOperation", "FlashPadRoundRectOperation", "FlashPadCustomOperation", "FlashPadTrapezOperation", "SchematicSymbolStartBlockOperation", "SchematicSymbolEndBlockOperation"]


SchematicSheetOperation = Union["ThickSegmentOperation", "RectOperation", "PlotPolyOperation", "TextOperation", "SchematicSheetStartBlockOperation", "SchematicSheetEndBlockOperation"]


RecordString = dict[str, str]


class SchematicSymbolStartBlockOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="StartBlock", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    label: str
    data_uuid: str
    data_ref: Literal["symbol_pin"]
    object_id: str
    extra_attrs: SchematicSymbolPinBlockAttrs


class SchematicSymbolEndBlockOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="EndBlock", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]


class SchematicSheetStartBlockOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="StartBlock", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    label: str
    data_uuid: str
    data_ref: Literal["sheet_pin"]
    object_id: str
    extra_attrs: SchematicSheetPinBlockAttrs


class SchematicSheetEndBlockOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="EndBlock", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]


SchematicSymbolPinBlockAttrs = dict[str, str]


SchematicSheetPinBlockAttrs = dict[str, str]


SchematicWorksheetMode = Literal["default", "provided"]


class SchematicTextVariable(Struct, forbid_unknown_fields=True, frozen=True):
    name: str
    value: str


SchematicTextOffsetRatio = Annotated[float, Meta(ge=0, le=1.7976931348623157e+308)]


SchematicDefaultLineWidthNm = Annotated[int, Meta(ge=84700, le=9007199254740991)]


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


class NativeFileSlot(Struct, forbid_unknown_fields=True, frozen=True):
    slot: Annotated[int, Meta(ge=0, le=4294967295)]
    path: str


class NativeDesignFactsLimits(Struct, forbid_unknown_fields=True, frozen=True):
    max_sources: Annotated[int, Meta(ge=0, le=4294967295)]
    max_source_bytes: CanonicalUint64Decimal
    max_total_source_bytes: CanonicalUint64Decimal
    max_path_bytes: Annotated[int, Meta(ge=0, le=4294967295)]
    max_output_bytes: CanonicalUint64Decimal


class NativeNetlistMetadata(Struct, forbid_unknown_fields=True, frozen=True):
    source_path: str
    date: str
    tool: str


NativeSvgPlotDocument = Union["NativeFootprintSvgDocument", "NativeSymbolSvgDocument", "NativeBoardSvgDocument", "NativeSchematicSvgDocument"]


class NativeSvgViewport(Struct, forbid_unknown_fields=True, frozen=True):
    min_x_nm: JavaScriptSafeInteger
    min_y_nm: JavaScriptSafeInteger
    width_nm: NativeSvgPositiveSafeInteger
    height_nm: NativeSvgPositiveSafeInteger


class NativeSvgRenderLimits(Struct, forbid_unknown_fields=True, frozen=True):
    max_records: Annotated[int, Meta(ge=0, le=4294967295)]
    max_operations: Annotated[int, Meta(ge=0, le=4294967295)]
    max_points: CanonicalUint64Decimal
    max_text_bytes: CanonicalUint64Decimal
    max_image_encoded_bytes: CanonicalUint64Decimal
    max_block_depth: Annotated[int, Meta(ge=0, le=4294967295)]
    max_svg_elements: CanonicalUint64Decimal
    max_render_work: CanonicalUint64Decimal
    max_svg_bytes: CanonicalUint64Decimal
    max_result_bytes: CanonicalUint64Decimal


class NativeFootprintSvgDocument(Struct, forbid_unknown_fields=True, frozen=True, tag="footprint", tag_field="kind"):
    value: FootprintPlotDocumentA0


class NativeSymbolSvgDocument(Struct, forbid_unknown_fields=True, frozen=True, tag="symbol", tag_field="kind"):
    value: SymbolPlotDocumentA0


class NativeBoardSvgDocument(Struct, forbid_unknown_fields=True, frozen=True, tag="board", tag_field="kind"):
    value: BoardPlotDocumentA0


class NativeSchematicSvgDocument(Struct, forbid_unknown_fields=True, frozen=True, tag="schematic", tag_field="kind"):
    value: SchematicPlotDocumentA0


NativeSvgPositiveSafeInteger = Annotated[int, Meta(ge=1, le=9007199254740991)]


NativeErrorKind = Literal["request", "path", "io", "resource_limit", "core"]


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
    text_offset_ratio: SchematicTextOffsetRatio
    default_line_width_nm: SchematicDefaultLineWidthNm
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
    max_labels: Annotated[int, Meta(ge=0, le=4294967295)]
    max_global_labels: Annotated[int, Meta(ge=0, le=4294967295)]
    max_hierarchical_labels: Annotated[int, Meta(ge=0, le=4294967295)]
    max_netclass_flags: Annotated[int, Meta(ge=0, le=4294967295)]
    max_netclass_flag_properties: Annotated[int, Meta(ge=0, le=4294967295)]
    max_texts: Annotated[int, Meta(ge=0, le=4294967295)]
    max_text_boxes: Annotated[int, Meta(ge=0, le=4294967295)]
    max_text_box_lines: Annotated[int, Meta(ge=0, le=4294967295)]
    max_polylines: Annotated[int, Meta(ge=0, le=4294967295)]
    max_arcs: Annotated[int, Meta(ge=0, le=4294967295)]
    max_circles: Annotated[int, Meta(ge=0, le=4294967295)]
    max_rectangles: Annotated[int, Meta(ge=0, le=4294967295)]
    max_beziers: Annotated[int, Meta(ge=0, le=4294967295)]
    max_rule_areas: Annotated[int, Meta(ge=0, le=4294967295)]
    max_images: Annotated[int, Meta(ge=0, le=4294967295)]
    max_tables: Annotated[int, Meta(ge=0, le=4294967295)]
    max_table_cells: Annotated[int, Meta(ge=0, le=4294967295)]
    max_table_cell_lines: Annotated[int, Meta(ge=0, le=4294967295)]
    max_image_data_parts: Annotated[int, Meta(ge=0, le=4294967295)]
    max_image_encoded_bytes: str
    max_image_decoded_bytes: str
    max_image_width_px: Annotated[int, Meta(ge=0, le=4294967295)]
    max_image_height_px: Annotated[int, Meta(ge=0, le=4294967295)]
    max_image_pixels: str
    max_image_decode_work: str
    max_symbols: Annotated[int, Meta(ge=0, le=4294967295)]
    max_symbol_overplots: Annotated[int, Meta(ge=0, le=4294967295)]
    max_symbol_properties: Annotated[int, Meta(ge=0, le=4294967295)]
    max_symbol_pins: Annotated[int, Meta(ge=0, le=4294967295)]
    max_library_symbols: Annotated[int, Meta(ge=0, le=4294967295)]
    max_library_subsymbols: Annotated[int, Meta(ge=0, le=4294967295)]
    max_library_pins: Annotated[int, Meta(ge=0, le=4294967295)]
    max_symbol_overlap_checks: str
    max_sheets: Annotated[int, Meta(ge=0, le=4294967295)]
    max_sheet_properties: Annotated[int, Meta(ge=0, le=4294967295)]
    max_sheet_pins: Annotated[int, Meta(ge=0, le=4294967295)]
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


class NativeHandshakeA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.native.handshake"] = field(name="type")
    version: Literal["a0"]
    engine_version: str
    operations: list[Literal["design-facts"]]


class NativeHandshakeA1(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.native.handshake"] = field(name="type")
    version: Literal["a1"]
    engine_version: str
    operations: tuple[Literal["design-facts"], Literal["render-svg"]]


class NativeHandshakeA2(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.native.handshake"] = field(name="type")
    version: Literal["a2"]
    engine_version: str
    operations: tuple[Literal["design-facts"], Literal["render-svg"], Literal["design-facts-a1"]]


class NativeDesignFactsRequestA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.native.design_facts.request"] = field(name="type")
    version: Literal["a0"]
    bundle_root: str
    manifest: SourceBundleManifestA0
    file_slots: list[NativeFileSlot]
    limits: NativeDesignFactsLimits
    netlist: NativeNetlistMetadata


class NativeDesignFactsResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.native.design_facts.result"] = field(name="type")
    version: Literal["a0"]
    engine_version: str
    compiled_schematic_graph: CompiledSchematicGraphA0
    kicad_netlist_version: Literal["E"]
    kicad_netlist: str


class NativeDesignFactsRequestA1(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.native.design_facts.request"] = field(name="type")
    version: Literal["a1"]
    resource_profile: Literal["design-facts-bounded-a1"]
    bundle_root: str
    manifest: SourceBundleManifestA0
    file_slots: list[NativeFileSlot]
    limits: NativeDesignFactsLimits
    netlist: NativeNetlistMetadata


class NativeDesignFactsResultA1(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.native.design_facts.result"] = field(name="type")
    version: Literal["a1"]
    engine_version: str
    resource_profile: Literal["design-facts-bounded-a1"]
    source_snapshot_sha256: Annotated[str, Meta(pattern="^[0-9a-f]{64}$")]
    compiled_schematic_graph: CompiledSchematicGraphA0
    kicad_netlist_version: Literal["E"]
    kicad_netlist: str
    kicad_netlist_bytes: CanonicalUint64Decimal
    kicad_netlist_sha256: Annotated[str, Meta(pattern="^[0-9a-f]{64}$")]


class NativeSVGRenderRequestA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.native.svg.request"] = field(name="type")
    version: Literal["a0"]
    profile: Literal["plotter-base-a0"]
    document: NativeSvgPlotDocument
    viewport: NativeSvgViewport
    limits: NativeSvgRenderLimits


class NativeSVGRenderResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.native.svg.result"] = field(name="type")
    version: Literal["a0"]
    engine_version: str
    profile: Literal["plotter-base-a0"]
    source_kind: Union[Literal["MOD"], Literal["SYM"], Literal["PCB"], Literal["SCH"]]
    document_id: str
    svg_utf8: str
    svg_bytes: CanonicalUint64Decimal
    svg_sha256: Annotated[str, Meta(pattern="^[0-9a-f]{64}$")]


class NativeErrorA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.native.error"] = field(name="type")
    version: Literal["a0"]
    kind: NativeErrorKind
    message: str


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
        operation.context is not UNSET,
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
    if isinstance(operation, ThickSegmentOperation) and operation.stroke_color is not UNSET:
        raise msgspec.ValidationError(f"invalid_segment_color at {path}.stroke_color")
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
        for operation in record.operations:
            if isinstance(operation, TextOperation) and operation.context is not UNSET:
                raise msgspec.ValidationError(f"invalid_board_text_context at {path}.operations")
            if isinstance(operation, ThickSegmentOperation) and operation.stroke_color is not UNSET:
                raise msgspec.ValidationError(f"invalid_board_segment_color at {path}.operations")
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
            forbidden = (operation.role is not UNSET, bool(layers), operation.mask_margin_nm is not UNSET, operation.pad_size_x_nm is not UNSET, operation.pad_size_y_nm is not UNSET, operation.stroke_color is not UNSET)
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
    if operation.context is not UNSET:
        raise msgspec.ValidationError(f"invalid_board_text_context at {path}.context")
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
        if isinstance(operation, BoardFootprintThickSegmentOperation):
            if operation.stroke_color is not UNSET:
                raise msgspec.ValidationError(f"invalid_board_footprint_segment_color at {path}")
            expected = 'text-box-border' if data_ref == 'fp_text_box' else 'line'
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
    if not math.isfinite(operation.orient_deg) or operation.context is not UNSET or operation.mirror is not UNSET or operation.text_as_polygons is not UNSET or operation.polyline_per_segment is not UNSET or operation.knockout is False:
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
                    operation.context is not UNSET,
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
    phases = {SchematicSheetHeaderPlotRecord: 0, SchematicWirePlotRecord: 1, SchematicBusPlotRecord: 2, SchematicBusEntryPlotRecord: 3, SchematicJunctionPlotRecord: 4, SchematicNoConnectPlotRecord: 5, SchematicLabelPlotRecord: 6, SchematicGlobalLabelPlotRecord: 7, SchematicHierarchicalLabelPlotRecord: 8, SchematicNetclassFlagPlotRecord: 9, SchematicTextPlotRecord: 10, SchematicTextBoxPlotRecord: 11, SchematicGraphicPolylinePlotRecord: 12, SchematicGraphicArcPlotRecord: 13, SchematicGraphicCirclePlotRecord: 14, SchematicGraphicRectanglePlotRecord: 15, SchematicGraphicBezierPlotRecord: 16, SchematicRuleAreaPlotRecord: 17, SchematicImagePlotRecord: 18, SchematicTablePlotRecord: 19, SchematicSymbolInstancePlotRecord: 20, SchematicSymbolOverplotPlotRecord: 21, SchematicSheetPlotRecord: 22}
    previous_phase = -1
    total_operations = 0
    for record_index, record in enumerate(value.records):
        path = f'$.records[{record_index}]'
        phase = phases[type(record)]
        if phase < previous_phase or (phase == 0 and record_index != 0):
            raise msgspec.ValidationError(f"invalid_schematic_record_order at {path}")
        previous_phase = phase
        label_record = isinstance(record, (SchematicLabelPlotRecord, SchematicGlobalLabelPlotRecord, SchematicHierarchicalLabelPlotRecord))
        symbol_record = isinstance(record, (SchematicSymbolInstancePlotRecord, SchematicSymbolOverplotPlotRecord))
        sheet_record = isinstance(record, SchematicSheetPlotRecord)
        if (label_record and record.object_id != record.text) or (sheet_record and record.object_id != record.sheet_name) or (not label_record and not isinstance(record, SchematicNetclassFlagPlotRecord) and not symbol_record and not sheet_record and record.object_id != record.uuid):
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
        elif isinstance(record, SchematicNoConnectPlotRecord):
            _validate_schematic_no_connect_record(record, path)
        elif isinstance(record, (SchematicLabelPlotRecord, SchematicGlobalLabelPlotRecord, SchematicHierarchicalLabelPlotRecord)):
            _validate_schematic_label_record(record, path)
        elif isinstance(record, SchematicNetclassFlagPlotRecord):
            _validate_schematic_netclass_flag_record(record, path)
        elif isinstance(record, SchematicTextPlotRecord):
            _validate_schematic_text_record(record, path)
        elif isinstance(record, SchematicTextBoxPlotRecord):
            _validate_schematic_text_box_record(record, path)
        elif isinstance(record, (SchematicGraphicPolylinePlotRecord, SchematicGraphicArcPlotRecord, SchematicGraphicCirclePlotRecord, SchematicGraphicRectanglePlotRecord)):
            _validate_schematic_graphic_record(record, path)
        elif isinstance(record, SchematicGraphicBezierPlotRecord):
            _validate_schematic_bezier_record(record, path)
        elif isinstance(record, SchematicRuleAreaPlotRecord):
            _validate_schematic_rule_area_record(record, path)
        elif isinstance(record, SchematicImagePlotRecord):
            _validate_schematic_image_record(record, path)
        elif isinstance(record, SchematicTablePlotRecord):
            _validate_schematic_table_record(record, path)
        elif symbol_record:
            _validate_schematic_symbol_record(record, path)
        else:
            _validate_schematic_sheet_record(record, path)
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
            forbidden = (operation.context is not UNSET, operation.mirror is not UNSET, operation.text_as_polygons is not UNSET, operation.polyline_per_segment is not UNSET, operation.knockout is not UNSET, operation.render_cache_polygons is not UNSET, operation.render_cache is not UNSET, operation.render_cache_source is not UNSET, operation.render_cache_exact is not UNSET)
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


def _validate_schematic_annotation_text(operation: TextOperation, path: str) -> None:
    forbidden = (operation.layer is not UNSET, operation.mirror is not UNSET, operation.text_as_polygons is not UNSET, operation.polyline_per_segment is not UNSET, operation.knockout is not UNSET, operation.render_cache_polygons is not UNSET, operation.render_cache is not UNSET, operation.render_cache_source is not UNSET, operation.render_cache_exact is not UNSET)
    if any(forbidden) or not math.isfinite(operation.orient_deg):
        raise msgspec.ValidationError(f"invalid_annotation_text at {path}")
    if operation.context is not UNSET:
        href = operation.context.hyperlink.href
        if not href or href != href.strip():
            raise msgspec.ValidationError(f"invalid_hyperlink_context at {path}.context.hyperlink.href")


def _validate_schematic_label_record(record: SchematicLabelPlotRecord | SchematicGlobalLabelPlotRecord | SchematicHierarchicalLabelPlotRecord, path: str) -> None:
    decorated = isinstance(record, (SchematicGlobalLabelPlotRecord, SchematicHierarchicalLabelPlotRecord)) and record.shape in ('input', 'output', 'bidirectional', 'tri_state', 'passive')
    expected_count = 2 if decorated else 1
    if len(record.operations) != expected_count or not isinstance(record.operations[0], TextOperation):
        raise msgspec.ValidationError(f"invalid_label_record at {path}.operations")
    text = record.operations[0]
    _validate_schematic_annotation_text(text, f'{path}.operations[0]')
    if text.text != record.text.replace('{slash}', '/'):
        raise msgspec.ValidationError(f"invalid_label_text at {path}.text")
    if not decorated:
        return
    decoration = record.operations[1]
    if not isinstance(decoration, PlotPolyOperation):
        raise msgspec.ValidationError(f"invalid_label_decoration at {path}.operations[1]")
    expected_color = '#840000FF' if isinstance(record, SchematicGlobalLabelPlotRecord) else '#725600FF'
    expected_points = 7 if isinstance(record, SchematicGlobalLabelPlotRecord) else (6 if record.shape in ('input', 'output') else 5)
    layer = None if decoration.layer is UNSET else decoration.layer
    if layer is not None or decoration.fill != 'NO_FILL' or decoration.width_nm != 152_400 or decoration.stroke_color != expected_color or decoration.fill_color is not UNSET or decoration.line_style is not UNSET or len(decoration.points) != expected_points or decoration.points[0] != decoration.points[-1]:
        raise msgspec.ValidationError(f"invalid_label_decoration at {path}.operations[1]")


def _validate_schematic_netclass_flag_record(record: SchematicNetclassFlagPlotRecord, path: str) -> None:
    if record.shape in ('round', 'dot'):
        if len(record.operations) < 2 or not isinstance(record.operations[0], ThickSegmentOperation) or not isinstance(record.operations[1], CircleOperation):
            raise msgspec.ValidationError(f"invalid_netclass_marker at {path}.operations")
        segment, marker = record.operations[:2]
        segment_layer = None if segment.layer is UNSET else segment.layer
        segment_layers = [] if segment.layers is UNSET else segment.layers
        segment_forbidden = (segment.role is not UNSET, bool(segment_layers), segment.mask_margin_nm is not UNSET, segment.pad_size_x_nm is not UNSET, segment.pad_size_y_nm is not UNSET)
        if segment_layer is not None or any(segment_forbidden) or segment.width_nm <= 0 or segment.stroke_color != '#484848FF' or (segment.start_x, segment.start_y) != (record.at_x_nm, record.at_y_nm):
            raise msgspec.ValidationError(f"invalid_netclass_segment at {path}.operations[0]")
        marker_layer = None if marker.layer is UNSET else marker.layer
        marker_layers = [] if marker.layers is UNSET else marker.layers
        marker_forbidden = (marker.role is not UNSET, bool(marker_layers), marker.mask_margin_nm is not UNSET, marker.pad_size_x_nm is not UNSET, marker.pad_size_y_nm is not UNSET, marker.line_style is not UNSET)
        symbol_size = 355_600 if record.shape == 'dot' else 508_000
        expected_fill = 'FILLED_SHAPE' if record.shape == 'dot' else 'NO_FILL'
        expected_width = 0 if record.shape == 'dot' else segment.width_nm
        expected_fill_color = '#484848FF' if record.shape == 'dot' else UNSET
        if marker_layer is not None or any(marker_forbidden) or marker.diameter_nm != 2 * symbol_size or marker.fill != expected_fill or marker.width_nm != expected_width or marker.stroke_color != '#484848FF' or marker.fill_color != expected_fill_color:
            raise msgspec.ValidationError(f"invalid_netclass_circle at {path}.operations[1]")
        text_start = 2
    else:
        if not record.operations or not isinstance(record.operations[0], PlotPolyOperation):
            raise msgspec.ValidationError(f"invalid_netclass_marker at {path}.operations")
        marker = record.operations[0]
        layer = None if marker.layer is UNSET else marker.layer
        expected_points = 7 if record.shape == 'diamond' else 8
        if layer is not None or marker.fill != 'NO_FILL' or marker.width_nm <= 0 or marker.stroke_color != '#484848FF' or marker.fill_color is not UNSET or marker.line_style is not UNSET or len(marker.points) != expected_points or marker.points[0] != [record.at_x_nm, record.at_y_nm] or marker.points[-1] != marker.points[0]:
            raise msgspec.ValidationError(f"invalid_netclass_polygon at {path}.operations[0]")
        text_start = 1
    for index, operation in enumerate(record.operations[text_start:], start=text_start):
        if not isinstance(operation, TextOperation):
            raise msgspec.ValidationError(f"invalid_netclass_property at {path}.operations[{index}]")
        _validate_schematic_annotation_text(operation, f'{path}.operations[{index}]')


def _validate_schematic_text_record(record: SchematicTextPlotRecord, path: str) -> None:
    if len(record.operations) != 1 or not isinstance(record.operations[0], TextOperation):
        raise msgspec.ValidationError(f"invalid_schematic_text at {path}.operations")
    operation = record.operations[0]
    _validate_schematic_annotation_text(operation, f'{path}.operations[0]')
    expected = record.text[:-1] if record.text.endswith('\n') else record.text
    if operation.text != expected or operation.multiline != ('\n' in operation.text):
        raise msgspec.ValidationError(f"invalid_schematic_text at {path}.text")


def _validate_schematic_text_box_record(record: SchematicTextBoxPlotRecord, path: str) -> None:
    text_start = _validate_schematic_text_box_prefix(record.operations, path)
    _validate_schematic_text_box_lines(record.operations, text_start, path)


def _validate_schematic_text_box_prefix(operations: list[PlotterOperation], path: str) -> int:
    if not operations or not isinstance(operations[0], RectOperation):
        raise msgspec.ValidationError(f"invalid_text_box at {path}.operations")
    first = operations[0]
    first_layer = None if first.layer is UNSET else first.layer
    fill_color = None if first.fill_color is UNSET else first.fill_color
    single_fill_valid = (first.fill == 'NO_FILL' and first.fill_color is UNSET) or first.fill != 'NO_FILL'
    if first_layer is not None or first.corner_radius_nm != 0 or first.width_nm < 0 or not _valid_schematic_color(first.stroke_color) or (fill_color is not None and not _valid_schematic_color(fill_color)) or first.line_style is UNSET or not single_fill_valid:
        raise msgspec.ValidationError(f"invalid_text_box_outline at {path}.operations[0]")
    if first.fill in ('NO_FILL', 'FILLED_SHAPE'):
        return 1
    else:
        if len(operations) < 2 or not isinstance(operations[1], RectOperation):
            raise msgspec.ValidationError(f"invalid_text_box_fill_pass at {path}.operations")
        outline = operations[1]
        outline_layer = None if outline.layer is UNSET else outline.layer
        same_geometry = (first.x1, first.y1, first.x2, first.y2, first.corner_radius_nm) == (outline.x1, outline.y1, outline.x2, outline.y2, outline.corner_radius_nm)
        if first.width_nm != 0 or first.fill_color is UNSET or first.stroke_color != first.fill_color or outline_layer is not None or not same_geometry or outline.fill != 'NO_FILL' or outline.width_nm < 0 or not _valid_schematic_color(outline.stroke_color) or outline.fill_color is not UNSET or outline.line_style != first.line_style:
            raise msgspec.ValidationError(f"invalid_text_box_fill_pass at {path}.operations[:2]")
        return 2


def _validate_schematic_text_box_lines(operations: list[PlotterOperation], text_start: int, path: str) -> None:
    for index, operation in enumerate(operations[text_start:], start=text_start):
        if not isinstance(operation, TextOperation) or not operation.text or operation.multiline:
            raise msgspec.ValidationError(f"invalid_text_box_line at {path}.operations[{index}]")
        _validate_schematic_annotation_text(operation, f'{path}.operations[{index}]')


def _valid_schematic_color(value: object) -> bool:
    return isinstance(value, str) and len(value) == 9 and value[0] == '#' and all(char in '0123456789ABCDEF' for char in value[1:])


def _schematic_graphic_geometry(operation: PlotterOperation) -> tuple:
    if isinstance(operation, PlotPolyOperation):
        return tuple(tuple(point) for point in operation.points)
    if isinstance(operation, ArcThreePointOperation):
        return (operation.start_x, operation.start_y, operation.mid_x, operation.mid_y, operation.end_x, operation.end_y)
    if isinstance(operation, CircleOperation):
        return (operation.cx, operation.cy, operation.diameter_nm)
    if isinstance(operation, RectOperation):
        return (operation.x1, operation.y1, operation.x2, operation.y2, operation.corner_radius_nm)
    raise msgspec.ValidationError('invalid_graphic_operation')


def _validate_schematic_graphic_operation(operation: PlotterOperation, path: str) -> None:
    layer = None if operation.layer is UNSET else operation.layer
    if layer is not None or operation.width_nm < 0 or operation.stroke_color is UNSET or not _valid_schematic_color(operation.stroke_color) or (operation.fill_color is not UNSET and not _valid_schematic_color(operation.fill_color)) or operation.line_style is UNSET:
        raise msgspec.ValidationError(f"invalid_graphic_style at {path}")
    if isinstance(operation, PlotPolyOperation) and len(operation.points) < 2:
        raise msgspec.ValidationError(f"invalid_graphic_points at {path}.points")
    if isinstance(operation, CircleOperation):
        forbidden = (operation.role is not UNSET, bool([] if operation.layers is UNSET else operation.layers), operation.mask_margin_nm is not UNSET, operation.pad_size_x_nm is not UNSET, operation.pad_size_y_nm is not UNSET)
        if any(forbidden) or operation.diameter_nm < 0:
            raise msgspec.ValidationError(f"invalid_graphic_circle at {path}")
    if isinstance(operation, RectOperation) and operation.corner_radius_nm < 0:
        raise msgspec.ValidationError(f"invalid_graphic_rectangle at {path}")


def _validate_schematic_graphic_operations(operations: list[PlotterOperation], expected_type: type, path: str, *, closed: bool = False) -> None:
    if len(operations) not in (1, 2) or not all(isinstance(operation, expected_type) for operation in operations):
        raise msgspec.ValidationError(f"invalid_graphic_record at {path}.operations")
    for index, operation in enumerate(operations):
        _validate_schematic_graphic_operation(operation, f'{path}.operations[{index}]')
    first = operations[0]
    if closed and (not isinstance(first, PlotPolyOperation) or first.points[0] != first.points[-1]):
        raise msgspec.ValidationError(f"open_rule_area at {path}.operations[0].points")
    if len(operations) == 1:
        valid_fill = (first.fill == 'NO_FILL' and first.fill_color is UNSET) or first.fill == 'FILLED_SHAPE'
        if not valid_fill:
            raise msgspec.ValidationError(f"invalid_graphic_fill at {path}.operations[0]")
        return
    outline = operations[1]
    if first.fill in ('NO_FILL', 'FILLED_SHAPE') or first.width_nm != 0 or first.fill_color is UNSET or first.stroke_color != first.fill_color or outline.fill != 'NO_FILL' or outline.fill_color is not UNSET or outline.line_style != first.line_style or _schematic_graphic_geometry(first) != _schematic_graphic_geometry(outline):
        raise msgspec.ValidationError(f"invalid_graphic_fill_pair at {path}.operations")


def _validate_schematic_graphic_record(record: SchematicGraphicPolylinePlotRecord | SchematicGraphicArcPlotRecord | SchematicGraphicCirclePlotRecord | SchematicGraphicRectanglePlotRecord, path: str) -> None:
    expected = PlotPolyOperation if isinstance(record, SchematicGraphicPolylinePlotRecord) else ArcThreePointOperation if isinstance(record, SchematicGraphicArcPlotRecord) else CircleOperation if isinstance(record, SchematicGraphicCirclePlotRecord) else RectOperation
    _validate_schematic_graphic_operations(record.operations, expected, path)


def _validate_schematic_bezier_operation(operation: BezierCurveOperation, path: str) -> None:
    layer = None if operation.layer is UNSET else operation.layer
    if layer is not None or operation.width_nm < 0 or operation.tolerance_nm != 0 or operation.stroke_color is UNSET or not _valid_schematic_color(operation.stroke_color) or operation.line_style is UNSET:
        raise msgspec.ValidationError(f"invalid_graphic_bezier at {path}")


def _validate_schematic_bezier_record(record: SchematicGraphicBezierPlotRecord, path: str) -> None:
    if len(record.operations) != 1 or not isinstance(record.operations[0], BezierCurveOperation):
        raise msgspec.ValidationError(f"invalid_graphic_bezier at {path}.operations")
    _validate_schematic_bezier_operation(record.operations[0], f'{path}.operations[0]')


def _validate_schematic_rule_area_record(record: SchematicRuleAreaPlotRecord, path: str) -> None:
    expected = {'polyline': PlotPolyOperation, 'rectangle': RectOperation, 'arc': ArcThreePointOperation, 'circle': CircleOperation, 'bezier': BezierCurveOperation}[record.shape]
    if expected is BezierCurveOperation:
        if len(record.operations) != 1 or not isinstance(record.operations[0], BezierCurveOperation):
            raise msgspec.ValidationError(f"invalid_rule_area at {path}.operations")
        _validate_schematic_bezier_operation(record.operations[0], f'{path}.operations[0]')
    else:
        _validate_schematic_graphic_operations(record.operations, expected, path, closed=record.shape == 'polyline')


def _schematic_image_metadata(value: str) -> tuple[str, int, int, int | None, int | None] | None:
    if any(character in ' \t\r\n\v\f' for character in value):
        return None
    try:
        data = base64.b64decode(value, validate=True)
    except (binascii.Error, ValueError):
        return None
    if base64.b64encode(data).decode('ascii') != value:
        return None
    if len(data) >= 33 and data[:8] == b'\x89PNG\r\n\x1a\n' and data[8:16] == b'\x00\x00\x00\rIHDR':
        width, height = int.from_bytes(data[16:20], 'big'), int.from_bytes(data[20:24], 'big')
        ppm_x = ppm_y = None
        position = 8
        while position + 12 <= len(data):
            length = int.from_bytes(data[position:position + 4], 'big')
            end = position + 12 + length
            if end > len(data): return None
            kind = data[position + 4:position + 8]
            payload = data[position + 8:position + 8 + length]
            if kind == b'pHYs' and length >= 9 and payload[8] == 1:
                ppm_x = int.from_bytes(payload[:4], 'big') or None
                ppm_y = int.from_bytes(payload[4:8], 'big') or None
            position = end
            if kind == b'IEND': break
        return ('png', width, height, _schematic_ppi_from_ppm(ppm_x), _schematic_ppi_from_ppm(ppm_y)) if width > 0 and height > 0 else None
    if len(data) >= 4 and data[:2] == b'\xff\xd8':
        position, ppi_x, ppi_y = 2, None, None
        while position + 9 <= len(data):
            if data[position] != 0xFF:
                position += 1
                continue
            marker = data[position + 1]
            position += 2
            if marker in (0xD8, 0xD9): continue
            if position + 2 > len(data): return None
            length = int.from_bytes(data[position:position + 2], 'big')
            if length < 2 or position + length > len(data): return None
            payload = data[position + 2:position + length]
            if marker == 0xE0 and payload.startswith(b'JFIF\x00') and len(payload) >= 12:
                units, density_x, density_y = payload[7], int.from_bytes(payload[8:10], 'big'), int.from_bytes(payload[10:12], 'big')
                if density_x > 0 and density_y > 0:
                    if units == 1: ppi_x, ppi_y = density_x, density_y
                    elif units == 2: ppi_x, ppi_y = round(density_x * 2.54), round(density_y * 2.54)
            if marker in (0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7, 0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF):
                if length < 7: return None
                height, width = int.from_bytes(data[position + 3:position + 5], 'big'), int.from_bytes(data[position + 5:position + 7], 'big')
                return ('jpeg', width, height, ppi_x, ppi_y) if width > 0 and height > 0 else None
            position += length
        return None
    if len(data) >= 26 and data[:2] == b'BM':
        dib = int.from_bytes(data[14:18], 'little')
        if dib == 12:
            width, height, ppi_x, ppi_y = int.from_bytes(data[18:20], 'little'), int.from_bytes(data[20:22], 'little'), None, None
        elif dib >= 40 and len(data) >= 54:
            width = abs(int.from_bytes(data[18:22], 'little', signed=True))
            height = abs(int.from_bytes(data[22:26], 'little', signed=True))
            ppi_x = _schematic_bmp_ppi(int.from_bytes(data[38:42], 'little', signed=True))
            ppi_y = _schematic_bmp_ppi(int.from_bytes(data[42:46], 'little', signed=True))
        else: return None
        return ('bmp', width, height, ppi_x, ppi_y) if width > 0 and height > 0 else None
    return None


def _schematic_ppi_from_ppm(value: int | None) -> int | None:
    if value is None or value <= 0: return None
    return round(value * 0.0254) or None


def _schematic_bmp_ppi(value: int) -> int | None:
    if value <= 0: return None
    return round((value // 100) * 2.54) or None


def _schematic_image_extent(size_px: int, scale: float, ppi: int | None) -> int:
    return round(size_px * scale * 25.4 / (ppi if ppi and ppi > 0 else 300.0) * 1_000_000.0)


def _validate_schematic_image_record(record: SchematicImagePlotRecord, path: str) -> None:
    if len(record.operations) != 1 or not isinstance(record.operations[0], PlotImageOperation):
        raise msgspec.ValidationError(f"invalid_schematic_image at {path}.operations")
    operation = record.operations[0]
    metadata = _schematic_image_metadata(operation.image_data_b64)
    if metadata is None or not math.isfinite(operation.scale) or operation.scale <= 0 or operation.stroke_color != '#0000C2FF':
        raise msgspec.ValidationError(f"invalid_schematic_image at {path}.operations[0]")
    image_format, width_px, height_px, ppi_x, ppi_y = metadata
    try:
        width_nm, height_nm = _schematic_image_extent(width_px, operation.scale, ppi_x), _schematic_image_extent(height_px, operation.scale, ppi_y)
    except (OverflowError, ValueError):
        raise msgspec.ValidationError(f"invalid_schematic_image_extent at {path}.operations[0]") from None
    if image_format != operation.image_format or (record.scale, record.image_format, record.width_nm, record.height_nm) != (operation.scale, operation.image_format, operation.width_nm, operation.height_nm) or (operation.width_nm, operation.height_nm) != (width_nm, height_nm) or width_nm <= 0 or height_nm <= 0:
        raise msgspec.ValidationError(f"invalid_schematic_image_metadata at {path}")


def _validate_schematic_table_record(record: SchematicTablePlotRecord, path: str) -> None:
    operation_index = 0
    cells = 0
    while operation_index < len(record.operations):
        cell_path = f'{path}.operations[{operation_index}]'
        prefix = _validate_schematic_text_box_prefix(record.operations[operation_index:], cell_path)
        operation_index += prefix
        while operation_index < len(record.operations) and isinstance(record.operations[operation_index], TextOperation):
            operation = record.operations[operation_index]
            if not operation.text or operation.multiline:
                raise msgspec.ValidationError(f"invalid_table_cell_line at {path}.operations[{operation_index}]")
            _validate_schematic_annotation_text(operation, f'{path}.operations[{operation_index}]')
            operation_index += 1
        cells += 1
    if cells != record.cell_count:
        raise msgspec.ValidationError(f"table_cell_count_mismatch at {path}.cell_count")


def _validate_schematic_symbol_text(operation: TextOperation, path: str, in_pin: bool) -> None:
    _validate_schematic_annotation_text(operation, path)
    if in_pin and operation.context is not UNSET:
        raise msgspec.ValidationError(f"invalid_symbol_pin_text at {path}")


def _validate_schematic_symbol_record(record: SchematicSymbolInstancePlotRecord | SchematicSymbolOverplotPlotRecord, path: str) -> None:
    if isinstance(record, SchematicSymbolInstancePlotRecord):
        if record.object_id != (record.lib_id or record.uuid) or not math.isfinite(record.at_angle_deg) or record.mirror not in (None, 'x', 'y'):
            raise msgspec.ValidationError(f"invalid_symbol_instance at {path}")
        parent_uuid = record.uuid
    else:
        if record.uuid != f'{record.source_symbol_uuid}:overplot' or record.object_id != (record.lib_id or record.source_symbol_uuid):
            raise msgspec.ValidationError(f"invalid_symbol_overplot at {path}")
        parent_uuid = record.source_symbol_uuid
    block_start = None
    allowed_attrs = {'primitive', 'object-type', 'pin', 'symbol-uuid', 'designator', 'lib-pin-uuid'}
    for operation_index, operation in enumerate(record.operations):
        operation_path = f'{path}.operations[{operation_index}]'
        if isinstance(operation, SchematicSymbolStartBlockOperation):
            if block_start is not None or operation.label != operation.data_uuid or not operation.label or operation.data_ref != 'symbol_pin' or not operation.object_id:
                raise msgspec.ValidationError(f"invalid_symbol_pin_block at {operation_path}")
            attrs = operation.extra_attrs
            if set(attrs) - allowed_attrs or attrs.get('primitive') != 'pin' or attrs.get('object-type') != 'pin' or attrs.get('symbol-uuid') != parent_uuid or any(not isinstance(value, str) or not value for value in attrs.values()):
                raise msgspec.ValidationError(f"invalid_symbol_pin_attrs at {operation_path}.extra_attrs")
            block_start = operation_index
            continue
        if isinstance(operation, SchematicSymbolEndBlockOperation):
            if block_start is None or operation_index == block_start + 1:
                raise msgspec.ValidationError(f"invalid_symbol_pin_block at {operation_path}")
            block_start = None
            continue
        if isinstance(operation, (PlotImageOperation, FlashPadCircleOperation, FlashPadOvalOperation, FlashPadRectOperation, FlashPadRoundRectOperation, FlashPadCustomOperation, FlashPadTrapezOperation)):
            raise msgspec.ValidationError(f"invalid_symbol_operation at {operation_path}")
        if isinstance(operation, TextOperation):
            _validate_schematic_symbol_text(operation, operation_path, block_start is not None)
        elif hasattr(operation, 'layer') and operation.layer is not UNSET:
            raise msgspec.ValidationError(f"invalid_symbol_operation at {operation_path}")
    if block_start is not None:
        raise msgspec.ValidationError(f"invalid_symbol_pin_block at {path}.operations")


def _schematic_sheet_rect_state(operation: RectOperation) -> tuple:
    return (operation.x1, operation.y1, operation.x2, operation.y2, operation.fill, operation.width_nm, operation.corner_radius_nm, operation.layer, operation.stroke_color, operation.fill_color, operation.line_style)


def _validate_schematic_sheet_outline(operation: RectOperation, record: SchematicSheetPlotRecord, path: str) -> None:
    expected = (record.at_x_nm, record.at_y_nm, record.at_x_nm + record.size_x_nm, record.at_y_nm + record.size_y_nm)
    layer = None if operation.layer is UNSET else operation.layer
    if (operation.x1, operation.y1, operation.x2, operation.y2) != expected or operation.fill != 'NO_FILL' or operation.width_nm < 0 or operation.corner_radius_nm != 0 or layer is not None or not _valid_schematic_color(operation.stroke_color) or operation.fill_color is not UNSET or operation.line_style is UNSET:
        raise msgspec.ValidationError(f"invalid_sheet_outline at {path}")


def _validate_schematic_sheet_pin(text: TextOperation, decoration: PlotPolyOperation | None, record: SchematicSheetPlotRecord, path: str, attrs: dict[str, str] | None = None) -> None:
    _validate_schematic_annotation_text(text, f'{path}.text')
    if text.multiline:
        raise msgspec.ValidationError(f"invalid_sheet_pin_text at {path}.text")
    shape = None
    if attrs is not None:
        required = {'primitive', 'object-type', 'sheet-uuid', 'sheet-name', 'sheet-file', 'pin', 'pin-name', 'shape'}
        if set(attrs) != required or attrs.get('primitive') != 'sheet-entry' or attrs.get('object-type') != 'sheet-pin' or attrs.get('sheet-uuid') != record.uuid or attrs.get('sheet-name') != record.sheet_name or attrs.get('sheet-file') != record.sheet_file or attrs.get('pin') != attrs.get('pin-name'):
            raise msgspec.ValidationError(f"invalid_sheet_pin_attrs at {path}.extra_attrs")
        shape = attrs.get('shape')
        if shape not in ('input', 'output', 'bidirectional', 'tri_state', 'passive', 'dot', 'round', 'diamond', 'rectangle') or text.text != attrs['pin-name'].replace('{slash}', '/'):
            raise msgspec.ValidationError(f"invalid_sheet_pin_attrs at {path}.extra_attrs")
    decoration_required = shape is None or shape in ('input', 'output', 'bidirectional', 'tri_state', 'passive')
    if decoration_required != (decoration is not None):
        raise msgspec.ValidationError(f"invalid_sheet_pin_decoration at {path}.decoration")
    if decoration is None:
        return
    expected_points = (6,) if shape in ('input', 'output') else (5,) if shape is not None else (5, 6)
    layer = None if decoration.layer is UNSET else decoration.layer
    expected_color = '#949391FF' if record.dnp else '#006464FF'
    if layer is not None or decoration.fill != 'NO_FILL' or decoration.width_nm != text.pen_width_nm or decoration.stroke_color != expected_color or decoration.fill_color is not UNSET or decoration.line_style is not UNSET or len(decoration.points) not in expected_points or decoration.points[0] != decoration.points[-1]:
        raise msgspec.ValidationError(f"invalid_sheet_pin_decoration at {path}.decoration")


def _validate_schematic_sheet_marker(operation: ThickSegmentOperation, path: str) -> None:
    forbidden = (operation.layer is not UNSET, operation.role is not UNSET, operation.layers is not UNSET, operation.mask_margin_nm is not UNSET, operation.pad_size_x_nm is not UNSET, operation.pad_size_y_nm is not UNSET)
    if any(forbidden) or operation.width_nm != 457_200 or operation.stroke_color != '#DC090DD9':
        raise msgspec.ValidationError(f"invalid_sheet_dnp_marker at {path}")


def _validate_schematic_sheet_record(record: SchematicSheetPlotRecord, path: str) -> None:
    if record.object_id != record.sheet_name or record.size_x_nm <= 0 or record.size_y_nm <= 0 or len(record.operations) < 2 or not isinstance(record.operations[0], RectOperation) or not isinstance(record.operations[1], RectOperation):
        raise msgspec.ValidationError(f"invalid_sheet_record at {path}")
    first, outline = record.operations[:2]
    _validate_schematic_sheet_outline(outline, record, f'{path}.operations[1]')
    if first.fill == 'FILLED_SHAPE':
        expected = (record.at_x_nm, record.at_y_nm, record.at_x_nm + record.size_x_nm, record.at_y_nm + record.size_y_nm)
        layer = None if first.layer is UNSET else first.layer
        if (first.x1, first.y1, first.x2, first.y2) != expected or first.width_nm != 0 or first.corner_radius_nm != 0 or layer is not None or not _valid_schematic_color(first.stroke_color) or first.fill_color != first.stroke_color or first.line_style is not UNSET:
            raise msgspec.ValidationError(f"invalid_sheet_background at {path}.operations[0]")
    else:
        _validate_schematic_sheet_outline(first, record, f'{path}.operations[0]')
        if _schematic_sheet_rect_state(first) != _schematic_sheet_rect_state(outline):
            raise msgspec.ValidationError(f"invalid_sheet_outline_pair at {path}.operations[:2]")
    content_end = len(record.operations) - (2 if record.dnp else 0)
    if content_end < 2:
        raise msgspec.ValidationError(f"invalid_sheet_dnp_marker at {path}.operations")
    if record.dnp:
        first_marker, second_marker = record.operations[-2:]
        if not isinstance(first_marker, ThickSegmentOperation) or not isinstance(second_marker, ThickSegmentOperation):
            raise msgspec.ValidationError(f"invalid_sheet_dnp_marker at {path}.operations")
        _validate_schematic_sheet_marker(first_marker, f'{path}.operations[{content_end}]')
        _validate_schematic_sheet_marker(second_marker, f'{path}.operations[{content_end + 1}]')
        if first_marker.start_x != second_marker.end_x or first_marker.end_x != second_marker.start_x or first_marker.start_y != second_marker.start_y or first_marker.end_y != second_marker.end_y:
            raise msgspec.ValidationError(f"invalid_sheet_dnp_geometry at {path}.operations[-2:]")
    operation_index = 2
    saw_property = False
    while operation_index < content_end:
        operation = record.operations[operation_index]
        operation_path = f'{path}.operations[{operation_index}]'
        if isinstance(operation, SchematicSheetStartBlockOperation):
            has_decoration = operation_index + 3 < content_end and isinstance(record.operations[operation_index + 2], PlotPolyOperation) and isinstance(record.operations[operation_index + 3], SchematicSheetEndBlockOperation)
            no_decoration = operation_index + 2 < content_end and isinstance(record.operations[operation_index + 2], SchematicSheetEndBlockOperation)
            if saw_property or operation.label != operation.data_uuid or operation.label != operation.object_id or not operation.label or operation.data_ref != 'sheet_pin' or operation_index + 1 >= content_end or not isinstance(record.operations[operation_index + 1], TextOperation) or not (has_decoration or no_decoration):
                raise msgspec.ValidationError(f"invalid_sheet_pin_block at {operation_path}")
            decoration = record.operations[operation_index + 2] if has_decoration else None
            _validate_schematic_sheet_pin(record.operations[operation_index + 1], decoration, record, operation_path, operation.extra_attrs)
            operation_index += 4 if has_decoration else 3
            continue
        if isinstance(operation, TextOperation):
            if operation_index + 1 < content_end and isinstance(record.operations[operation_index + 1], PlotPolyOperation):
                if saw_property:
                    raise msgspec.ValidationError(f"invalid_sheet_pin_order at {operation_path}")
                _validate_schematic_sheet_pin(operation, record.operations[operation_index + 1], record, operation_path)
                operation_index += 2
                continue
            saw_property = True
            _validate_schematic_annotation_text(operation, operation_path)
            if operation.multiline:
                raise msgspec.ValidationError(f"invalid_sheet_property_text at {operation_path}")
            operation_index += 1
            continue
        raise msgspec.ValidationError(f"invalid_sheet_operation at {operation_path}")
_schematic_plot_request_a0_decoder = msgspec.json.Decoder(SchematicPlotRequestA0)


def decode_schematic_plot_request_a0(data: bytes) -> SchematicPlotRequestA0:
    value = _schematic_plot_request_a0_decoder.decode(data)
    validate_schematic_plot_request_a0(value)
    return value


def validate_schematic_plot_request_a0(value: SchematicPlotRequestA0) -> None:
    fields = ('max_source_bytes', 'max_worksheet_bytes', 'max_output_bytes', 'max_text_bytes', 'max_metadata_bytes', 'max_image_encoded_bytes', 'max_image_decoded_bytes', 'max_image_pixels', 'max_image_decode_work', 'max_symbol_overlap_checks', 'max_text_variable_bytes', 'max_worksheet_bitmap_encoded_bytes', 'max_worksheet_bitmap_decoded_bytes', 'max_worksheet_bitmap_pixels', 'max_worksheet_bitmap_decode_work')
    for field_name in fields:
        encoded = getattr(value, field_name)
        if not encoded or not encoded.isascii() or not encoded.isdigit() or int(encoded) > 18_446_744_073_709_551_615:
            raise msgspec.ValidationError(f"invalid_uint64 at $.{field_name}")
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
_native_handshake_a0_decoder = msgspec.json.Decoder(NativeHandshakeA0)


def decode_native_handshake_a0(data: bytes) -> NativeHandshakeA0:
    value = _native_handshake_a0_decoder.decode(data)
    validate_native_handshake_a0(value)
    return value


def validate_native_handshake_a0(value: NativeHandshakeA0) -> None:
    if not value.engine_version:
        raise msgspec.ValidationError("invalid_value at $.engine_version")
    if len(value.operations) != 1 or value.operations[0] != 'design-facts':
        raise msgspec.ValidationError("unsupported_contract at $.operations")
_native_handshake_a1_decoder = msgspec.json.Decoder(NativeHandshakeA1)


def decode_native_handshake_a1(data: bytes) -> NativeHandshakeA1:
    value = _native_handshake_a1_decoder.decode(data)
    validate_native_handshake_a1(value)
    return value


def validate_native_handshake_a1(value: NativeHandshakeA1) -> None:
    if not value.engine_version:
        raise msgspec.ValidationError("invalid_value at $.engine_version")
    if value.operations != ('design-facts', 'render-svg'):
        raise msgspec.ValidationError("unsupported_contract at $.operations")
_native_handshake_a2_decoder = msgspec.json.Decoder(NativeHandshakeA2)


def decode_native_handshake_a2(data: bytes) -> NativeHandshakeA2:
    value = _native_handshake_a2_decoder.decode(data)
    validate_native_handshake_a2(value)
    return value


def validate_native_handshake_a2(value: NativeHandshakeA2) -> None:
    if not value.engine_version:
        raise msgspec.ValidationError("invalid_value at $.engine_version")
    if value.operations != ('design-facts', 'render-svg', 'design-facts-a1'):
        raise msgspec.ValidationError("unsupported_contract at $.operations")
_native_design_facts_request_a0_decoder = msgspec.json.Decoder(NativeDesignFactsRequestA0)


def decode_native_design_facts_request_a0(data: bytes) -> NativeDesignFactsRequestA0:
    value = _native_design_facts_request_a0_decoder.decode(data)
    validate_native_design_facts_request_a0(value)
    return value


def validate_native_design_facts_request_a0(value: NativeDesignFactsRequestA0) -> None:
    fields = ('max_source_bytes', 'max_total_source_bytes', 'max_output_bytes')
    for field_name in fields:
        encoded = getattr(value.limits, field_name)
        if int(encoded) > 18_446_744_073_709_551_615:
            raise msgspec.ValidationError(f"invalid_uint64 at $.limits.{field_name}")
    for index, source in enumerate(value.manifest.sources):
        if int(source.source_bytes) > 18_446_744_073_709_551_615:
            raise msgspec.ValidationError(f"invalid_uint64 at $.manifest.sources[{index}].source_bytes")
_native_design_facts_result_a0_decoder = msgspec.json.Decoder(NativeDesignFactsResultA0)


def decode_native_design_facts_result_a0(data: bytes) -> NativeDesignFactsResultA0:
    value = _native_design_facts_result_a0_decoder.decode(data)
    validate_native_design_facts_result_a0(value)
    return value


def validate_native_design_facts_result_a0(value: NativeDesignFactsResultA0) -> None:
    if not value.engine_version:
        raise msgspec.ValidationError("invalid_value at $.engine_version")
_native_design_facts_request_a1_decoder = msgspec.json.Decoder(NativeDesignFactsRequestA1)


def decode_native_design_facts_request_a1(data: bytes) -> NativeDesignFactsRequestA1:
    value = _native_design_facts_request_a1_decoder.decode(data)
    validate_native_design_facts_request_a1(value)
    return value


def validate_native_design_facts_request_a1(value: NativeDesignFactsRequestA1) -> None:
    fields = ('max_source_bytes', 'max_total_source_bytes', 'max_output_bytes')
    for field_name in fields:
        encoded = getattr(value.limits, field_name)
        if int(encoded) > 18_446_744_073_709_551_615:
            raise msgspec.ValidationError(f"invalid_uint64 at $.limits.{field_name}")
    for index, source in enumerate(value.manifest.sources):
        if int(source.source_bytes) > 18_446_744_073_709_551_615:
            raise msgspec.ValidationError(f"invalid_uint64 at $.manifest.sources[{index}].source_bytes")
_native_design_facts_result_a1_decoder = msgspec.json.Decoder(NativeDesignFactsResultA1)


def decode_native_design_facts_result_a1(data: bytes) -> NativeDesignFactsResultA1:
    value = _native_design_facts_result_a1_decoder.decode(data)
    validate_native_design_facts_result_a1(value)
    return value


def validate_native_design_facts_result_a1(value: NativeDesignFactsResultA1) -> None:
    if not value.engine_version:
        raise msgspec.ValidationError("invalid_value at $.engine_version")
    if not value.kicad_netlist:
        raise msgspec.ValidationError("invalid_value at $.kicad_netlist")
    declared_bytes = int(value.kicad_netlist_bytes)
    if declared_bytes > 18_446_744_073_709_551_615:
        raise msgspec.ValidationError("invalid_uint64 at $.kicad_netlist_bytes")
    encoded = value.kicad_netlist.encode('utf-8')
    if declared_bytes != len(encoded):
        raise msgspec.ValidationError("length_mismatch at $.kicad_netlist_bytes")
    if value.kicad_netlist_sha256 != hashlib.sha256(encoded).hexdigest():
        raise msgspec.ValidationError("hash_mismatch at $.kicad_netlist_sha256")
_native_svg_render_request_a0_decoder = msgspec.json.Decoder(NativeSVGRenderRequestA0)


def decode_native_svg_render_request_a0(data: bytes) -> NativeSVGRenderRequestA0:
    value = _native_svg_render_request_a0_decoder.decode(data)
    validate_native_svg_render_request_a0(value)
    return value


def validate_native_svg_render_request_a0(value: NativeSVGRenderRequestA0) -> None:
    for field_name in (
        'max_points',
        'max_text_bytes',
        'max_image_encoded_bytes',
        'max_svg_elements',
        'max_render_work',
        'max_svg_bytes',
        'max_result_bytes',
    ):
        _validate_native_uint64(getattr(value.limits, field_name), f'$.limits.{field_name}')
    document = value.document
    if isinstance(document, NativeFootprintSvgDocument):
        validate_footprint_plot_document_a0(document.value)
        expected_source_kind = 'MOD'
    elif isinstance(document, NativeSymbolSvgDocument):
        validate_symbol_plot_document_a0(document.value)
        expected_source_kind = 'SYM'
    elif isinstance(document, NativeBoardSvgDocument):
        validate_board_plot_document_a0(document.value)
        expected_source_kind = 'PCB'
    elif isinstance(document, NativeSchematicSvgDocument):
        validate_schematic_plot_document_a0(document.value)
        expected_source_kind = 'SCH'
        canvas = document.value.canvas
        viewport = value.viewport
        if (
            viewport.min_x_nm != 0
            or viewport.min_y_nm != 0
            or viewport.width_nm != canvas.width_nm
            or viewport.height_nm != canvas.height_nm
        ):
            raise msgspec.ValidationError("viewport_mismatch at $.viewport")
    else:
        raise msgspec.ValidationError("unsupported_contract at $.document.kind")
    if not document.value.document_id:
        raise msgspec.ValidationError("invalid_value at $.document.value.document_id")
    if document.value.source_kind != expected_source_kind:
        raise msgspec.ValidationError("source_kind_mismatch at $.document.value.source_kind")


def _validate_native_uint64(value: str, path: str) -> None:
    canonical = value == '0' or (
        bool(value)
        and value[0] in '123456789'
        and value.isascii()
        and value.isdecimal()
    )
    if (
        not canonical
        or len(value) > 20
        or (len(value) == 20 and value > '18446744073709551615')
    ):
        raise msgspec.ValidationError(f"invalid_uint64 at {path}")
_native_svg_render_result_a0_decoder = msgspec.json.Decoder(NativeSVGRenderResultA0)


def decode_native_svg_render_result_a0(data: bytes) -> NativeSVGRenderResultA0:
    value = _native_svg_render_result_a0_decoder.decode(data)
    validate_native_svg_render_result_a0(value)
    return value


def validate_native_svg_render_result_a0(value: NativeSVGRenderResultA0) -> None:
    if not value.engine_version:
        raise msgspec.ValidationError("invalid_value at $.engine_version")
    if not value.document_id:
        raise msgspec.ValidationError("invalid_value at $.document_id")
    if not value.svg_utf8:
        raise msgspec.ValidationError("invalid_value at $.svg_utf8")
    _validate_native_uint64(value.svg_bytes, '$.svg_bytes')
    if int(value.svg_bytes) != len(value.svg_utf8.encode('utf-8')):
        raise msgspec.ValidationError("length_mismatch at $.svg_bytes")
    if value.svg_sha256 != hashlib.sha256(value.svg_utf8.encode('utf-8')).hexdigest():
        raise msgspec.ValidationError("hash_mismatch at $.svg_sha256")
decode_native_error_a0 = msgspec.json.Decoder(NativeErrorA0).decode


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
    "PlotterOperationContext",
    "TextRenderCache",
    "PlotterTextRenderCacheSource",
    "PlotterViaFlashRole",
    "PlotterQuad",
    "PlotterHyperlink",
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
    "SchematicLabelPlotRecord",
    "SchematicGlobalLabelPlotRecord",
    "SchematicHierarchicalLabelPlotRecord",
    "SchematicNetclassFlagPlotRecord",
    "SchematicTextPlotRecord",
    "SchematicTextBoxPlotRecord",
    "SchematicGraphicPolylinePlotRecord",
    "SchematicGraphicArcPlotRecord",
    "SchematicGraphicCirclePlotRecord",
    "SchematicGraphicRectanglePlotRecord",
    "SchematicGraphicBezierPlotRecord",
    "SchematicRuleAreaPlotRecord",
    "SchematicImagePlotRecord",
    "SchematicTablePlotRecord",
    "SchematicSymbolInstancePlotRecord",
    "SchematicSymbolOverplotPlotRecord",
    "SchematicSheetPlotRecord",
    "SchematicPlotTitleBlock",
    "SchematicLabelShape",
    "SchematicNetclassFlagShape",
    "SchematicRuleAreaShape",
    "SchematicImageFormat",
    "SchematicSymbolOperation",
    "SchematicSheetOperation",
    "RecordString",
    "SchematicSymbolStartBlockOperation",
    "SchematicSymbolEndBlockOperation",
    "SchematicSheetStartBlockOperation",
    "SchematicSheetEndBlockOperation",
    "SchematicSymbolPinBlockAttrs",
    "SchematicSheetPinBlockAttrs",
    "SchematicWorksheetMode",
    "SchematicTextVariable",
    "SchematicTextOffsetRatio",
    "SchematicDefaultLineWidthNm",
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
    "NativeFileSlot",
    "NativeDesignFactsLimits",
    "NativeNetlistMetadata",
    "NativeSvgPlotDocument",
    "NativeSvgViewport",
    "NativeSvgRenderLimits",
    "NativeFootprintSvgDocument",
    "NativeSymbolSvgDocument",
    "NativeBoardSvgDocument",
    "NativeSchematicSvgDocument",
    "NativeSvgPositiveSafeInteger",
    "NativeErrorKind",
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
    "NativeHandshakeA0",
    "NativeHandshakeA1",
    "NativeHandshakeA2",
    "NativeDesignFactsRequestA0",
    "NativeDesignFactsResultA0",
    "NativeDesignFactsRequestA1",
    "NativeDesignFactsResultA1",
    "NativeSVGRenderRequestA0",
    "NativeSVGRenderResultA0",
    "NativeErrorA0",
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
    "decode_native_handshake_a0",
    "decode_native_handshake_a1",
    "decode_native_handshake_a2",
    "decode_native_design_facts_request_a0",
    "decode_native_design_facts_result_a0",
    "decode_native_design_facts_request_a1",
    "decode_native_design_facts_result_a1",
    "decode_native_svg_render_request_a0",
    "decode_native_svg_render_result_a0",
    "decode_native_error_a0",
    "validate_footprint_plot_document_a0",
    "validate_board_plot_document_a0",
    "resolve_font_selection_a0",
    "validate_font_bundle_manifest_a0",
    "validate_outline_vector_a0",
    "validate_shaping_record_a0",
    "validate_symbol_plot_document_a0",
    "validate_schematic_plot_request_a0",
    "validate_schematic_plot_document_a0",
    "validate_native_handshake_a0",
    "validate_native_handshake_a1",
    "validate_native_handshake_a2",
    "validate_native_design_facts_request_a0",
    "validate_native_design_facts_result_a0",
    "validate_native_design_facts_request_a1",
    "validate_native_design_facts_result_a1",
    "validate_native_svg_render_request_a0",
    "validate_native_svg_render_result_a0",
)
