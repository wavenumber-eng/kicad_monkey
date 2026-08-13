"""Generated strict msgspec transport bindings. Do not edit."""

from __future__ import annotations

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
    min_depth: int | UnsetType = field(default=UNSET)
    max_depth: int | UnsetType = field(default=UNSET)
    prune_heads: list[str] | UnsetType = field(default=UNSET)


class FormSpan(Struct, forbid_unknown_fields=True, frozen=True):
    path: list[str]
    depth: int
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
    operation_count: int
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


PlotterOperation = Union["ThickSegmentOperation", "ArcThreePointOperation", "CircleOperation", "RectOperation", "PlotPolyOperation", "FlashPadCircleOperation", "FlashPadOvalOperation", "FlashPadRectOperation", "FlashPadRoundRectOperation", "FlashPadCustomOperation", "FlashPadTrapezOperation"]


class ThickSegmentOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="ThickSegment", tag_field="kind"):
    index: int
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
    index: int
    start_x: JavaScriptSafeInteger
    start_y: JavaScriptSafeInteger
    mid_x: JavaScriptSafeInteger
    mid_y: JavaScriptSafeInteger
    end_x: JavaScriptSafeInteger
    end_y: JavaScriptSafeInteger
    fill: PlotterFill
    width_nm: JavaScriptSafeInteger
    layer: str


class CircleOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="Circle", tag_field="kind"):
    index: int
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


class RectOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="Rect", tag_field="kind"):
    index: int
    x1: JavaScriptSafeInteger
    y1: JavaScriptSafeInteger
    x2: JavaScriptSafeInteger
    y2: JavaScriptSafeInteger
    fill: PlotterFill
    width_nm: JavaScriptSafeInteger
    corner_radius_nm: JavaScriptSafeInteger
    layer: str


class PlotPolyOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="PlotPoly", tag_field="kind"):
    index: int
    points: list[PlotterPoint]
    fill: PlotterFill
    width_nm: JavaScriptSafeInteger
    layer: str


class FlashPadCircleOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadCircle", tag_field="kind"):
    index: int
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    diameter_nm: JavaScriptSafeInteger
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger


class FlashPadOvalOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadOval", tag_field="kind"):
    index: int
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    size_x_nm: JavaScriptSafeInteger
    size_y_nm: JavaScriptSafeInteger
    orient_deg: float
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger


class FlashPadRectOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadRect", tag_field="kind"):
    index: int
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    size_x_nm: JavaScriptSafeInteger
    size_y_nm: JavaScriptSafeInteger
    orient_deg: float
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger


class FlashPadRoundRectOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadRoundRect", tag_field="kind"):
    index: int
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    size_x_nm: JavaScriptSafeInteger
    size_y_nm: JavaScriptSafeInteger
    corner_radius_nm: JavaScriptSafeInteger
    orient_deg: float
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger


class FlashPadCustomOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadCustom", tag_field="kind"):
    index: int
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
    index: int
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    corners: PlotterQuad
    orient_deg: float
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger


PlotterDrillRole = Literal["pad_drill", "npth_hole"]


PlotterFill = Literal["NO_FILL", "FILLED_SHAPE"]


PlotterPoint = Annotated[list[JavaScriptSafeInteger], Meta(min_length=2, max_length=2)]


PlotterQuad = Annotated[list[PlotterPoint], Meta(min_length=4, max_length=4)]


class SExpressionBuildRequestA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.sexpr_build.request"] = field(name="type")
    version: Literal["a0"]
    root: Node
    max_output_bytes: str
    max_depth: int
    max_nodes: int


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
    max_depth: int
    max_selected_forms: int


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
    max_depth: int
    max_properties: int
    max_pads: int


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
    max_depth: int
    max_properties: int
    max_pads: int


class FootprintReadResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.footprint_read.result"] = field(name="type")
    version: Literal["a0"]
    name: str
    source_bytes: str
    properties: list[FootprintProperty]
    pad_count: int
    diagnostics: list[Diagnostic]


class FootprintPlotDocumentA0(Struct, forbid_unknown_fields=True, frozen=True):
    schema: Literal["kicad.plotter_ir.a0"]
    source_kind: Literal["MOD"]
    total_operations: int
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
    max_depth: int
    max_metadata_forms: int
    max_operations: int
    source_path: str | UnsetType = field(default=UNSET)
    document_id: str | UnsetType = field(default=UNSET)


class FootprintPlotResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.footprint_plot.result"] = field(name="type")
    version: Literal["a0"]
    output_bytes: str
    total_operations: int
    diagnostics: list[Diagnostic]


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
    total_operations = 0
    for record_index, record in enumerate(value.records):
        if record.operation_count != len(record.operations):
            raise msgspec.ValidationError(
                f"operation_count_mismatch at $.records[{record_index}].operation_count"
            )
        total_operations += len(record.operations)
        for operation_index, operation in enumerate(record.operations):
            path = f"$.records[{record_index}].operations[{operation_index}]"
            if isinstance(operation, (ThickSegmentOperation, CircleOperation)):
                _validate_shared_graphic_or_drill(operation, path)
            elif isinstance(operation, (
                FlashPadCircleOperation,
                FlashPadOvalOperation,
                FlashPadRectOperation,
                FlashPadRoundRectOperation,
                FlashPadCustomOperation,
                FlashPadTrapezOperation,
            )) and not operation.layers:
                raise msgspec.ValidationError(f"missing_layers at {path}")
            if isinstance(operation, FlashPadCustomOperation):
                widths = operation.polygon_widths_nm
                if widths is not UNSET and widths and len(widths) != len(operation.polygons):
                    raise msgspec.ValidationError(f"polygon_width_count_mismatch at {path}.polygon_widths_nm")
    if value.total_operations != total_operations:
        raise msgspec.ValidationError("operation_count_mismatch at $.total_operations")


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
    "FlashPadCircleOperation",
    "FlashPadOvalOperation",
    "FlashPadRectOperation",
    "FlashPadRoundRectOperation",
    "FlashPadCustomOperation",
    "FlashPadTrapezOperation",
    "PlotterDrillRole",
    "PlotterFill",
    "PlotterPoint",
    "PlotterQuad",
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
    "validate_footprint_plot_document_a0",
)
