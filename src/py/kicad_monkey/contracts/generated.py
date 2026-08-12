"""Generated strict msgspec transport bindings. Do not edit."""

from __future__ import annotations

from typing import Literal

import msgspec
from msgspec import UNSET, Struct, UnsetType, field


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
    operations: list[ThickSegmentOperation]
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


class ThickSegmentOperation(Struct, forbid_unknown_fields=True, frozen=True):
    kind: Literal["ThickSegment"]
    index: int
    start_x: int
    start_y: int
    end_x: int
    end_y: int
    width_nm: int
    layer: str


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
    version: int
    generator: str
    generator_version: str
    source_path: str | UnsetType = field(default=UNSET)


class FootprintPlotRequestA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.footprint_plot.request"] = field(name="type")
    version: Literal["a0"]
    max_source_bytes: str
    max_output_bytes: str
    max_depth: int
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
decode_footprint_plot_document_a0 = msgspec.json.Decoder(FootprintPlotDocumentA0).decode
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
    "ThickSegmentOperation",
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
)
