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


PlotterOperation = Union["ThickSegmentOperation", "ArcThreePointOperation", "CircleOperation", "RectOperation", "PlotPolyOperation", "BezierCurveOperation", "FlashPadCircleOperation", "FlashPadOvalOperation", "FlashPadRectOperation", "FlashPadRoundRectOperation", "FlashPadCustomOperation", "FlashPadTrapezOperation"]


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
    layer: str | UnsetType = field(default=UNSET)
    stroke_color: str | UnsetType = field(default=UNSET)
    fill_color: str | UnsetType = field(default=UNSET)
    line_style: PlotterLineStyle | UnsetType = field(default=UNSET)


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
    stroke_color: str | UnsetType = field(default=UNSET)
    fill_color: str | UnsetType = field(default=UNSET)
    line_style: PlotterLineStyle | UnsetType = field(default=UNSET)


class RectOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="Rect", tag_field="kind"):
    index: int
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
    index: int
    points: list[PlotterPoint]
    fill: PlotterFill
    width_nm: JavaScriptSafeInteger
    layer: str | UnsetType = field(default=UNSET)
    stroke_color: str | UnsetType = field(default=UNSET)
    fill_color: str | UnsetType = field(default=UNSET)
    line_style: PlotterLineStyle | UnsetType = field(default=UNSET)


class BezierCurveOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="BezierCurve", tag_field="kind"):
    index: int
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


PlotterFill = Literal["NO_FILL", "FILLED_SHAPE", "FILLED_WITH_BG_BODYCOLOR", "FILLED_WITH_COLOR", "HATCH", "REVERSE_HATCH", "CROSS_HATCH"]


PlotterLineStyle = Literal["DEFAULT", "SOLID", "DASH", "DOT", "DASH_DOT", "DASH_DOT_DOT"]


PlotterPoint = Annotated[list[JavaScriptSafeInteger], Meta(min_length=2, max_length=2)]


PlotterQuad = Annotated[list[PlotterPoint], Meta(min_length=4, max_length=4)]


SymbolPlotRecord = Union["SymbolHeaderPlotRecord", "LibSubsymbolPlotRecord"]


class SymbolHeaderPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="lib_symbol", tag_field="kind"):
    uuid: Literal[""]
    object_id: str
    operation_count: int
    operations: list[PlotterOperation]
    name: str
    style: int
    in_bom: bool
    on_board: bool
    power: bool
    extends_: str | UnsetType = field(default=UNSET, name="extends")
    unit: int | UnsetType = field(default=UNSET)


class LibSubsymbolPlotRecord(Struct, forbid_unknown_fields=True, frozen=True, tag="lib_subsymbol", tag_field="kind"):
    uuid: Literal[""]
    object_id: str
    operation_count: int
    operations: list[PlotterOperation]
    unit: int
    style: int


SymbolBooleanField = Literal["in_bom", "on_board"]


class SymbolSummary(Struct, forbid_unknown_fields=True, frozen=True):
    name: str
    in_bom: bool
    on_board: bool
    power: bool
    property_count: int
    subsymbol_count: int
    pin_count: int
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
    instance_order: int
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
    unit: int
    body_style: int
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
    slot: int
    source_bytes: str


SourceKind = Literal["project", "schematic", "symbol_library", "symbol_table", "worksheet", "other"]


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
    max_points: int
    source_path: str | UnsetType = field(default=UNSET)
    document_id: str | UnsetType = field(default=UNSET)


class FootprintPlotResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.footprint_plot.result"] = field(name="type")
    version: Literal["a0"]
    output_bytes: str
    total_operations: int
    diagnostics: list[Diagnostic]


class SymbolPlotDocumentA0(Struct, forbid_unknown_fields=True, frozen=True):
    schema: Literal["kicad.plotter_ir.a0"]
    source_kind: Literal["SYM"]
    total_operations: int
    records: list[SymbolPlotRecord]
    document_id: str
    coordinate_space: PlotterCoordinateSpace
    source_path: str | UnsetType = field(default=UNSET)


class SymbolPlotRequestA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.symbol_plot.request"] = field(name="type")
    version: Literal["a0"]
    symbol_name: str
    style: int
    max_source_bytes: str
    max_output_bytes: str
    max_depth: int
    max_symbols: int
    max_subsymbols: int
    max_operations: int
    max_points: int
    unit: int | UnsetType = field(default=UNSET)
    source_path: str | UnsetType = field(default=UNSET)
    document_id: str | UnsetType = field(default=UNSET)


class SymbolPlotResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.symbol_plot.result"] = field(name="type")
    version: Literal["a0"]
    output_bytes: str
    total_operations: int
    diagnostics: list[Diagnostic]


class SymbolLibraryEditRequestA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.symbol_library_edit.request"] = field(name="type")
    version: Literal["a0"]
    symbol_name: str
    field: SymbolBooleanField
    value: bool
    max_source_bytes: str
    max_output_bytes: str
    max_depth: int
    max_symbols: int
    max_metadata_forms: int
    max_subsymbols: int
    max_pins: int


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
    max_depth: int
    max_symbols: int
    max_metadata_forms: int
    max_subsymbols: int
    max_pins: int


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
_symbol_plot_document_a0_decoder = msgspec.json.Decoder(SymbolPlotDocumentA0)


def decode_symbol_plot_document_a0(data: bytes) -> SymbolPlotDocumentA0:
    value = _symbol_plot_document_a0_decoder.decode(data)
    validate_symbol_plot_document_a0(value)
    return value


def validate_symbol_plot_document_a0(value: SymbolPlotDocumentA0) -> None:
    if not value.records or not isinstance(value.records[0], SymbolHeaderPlotRecord):
        raise msgspec.ValidationError("missing_symbol_header at $.records[0]")
    total_operations = 0
    for record_index, record in enumerate(value.records):
        if isinstance(record, SymbolHeaderPlotRecord):
            if record_index != 0 or record.operation_count != 0 or record.operations:
                raise msgspec.ValidationError(f"invalid_symbol_header at $.records[{record_index}]")
        elif record.operation_count != len(record.operations):
            raise msgspec.ValidationError(f"operation_count_mismatch at $.records[{record_index}].operation_count")
        total_operations += len(record.operations)
        for operation_index, operation in enumerate(record.operations):
            path = f"$.records[{record_index}].operations[{operation_index}]"
            allowed = isinstance(operation, (ArcThreePointOperation, CircleOperation, RectOperation, PlotPolyOperation, BezierCurveOperation))
            layer = None if not hasattr(operation, 'layer') or operation.layer is UNSET else operation.layer
            if not allowed or layer is not None:
                raise msgspec.ValidationError(f"invalid_symbol_operation at {path}")
            if isinstance(operation, CircleOperation):
                role = None if operation.role is UNSET else operation.role
                layers = [] if operation.layers is UNSET else operation.layers
                if role is not None or layers or operation.mask_margin_nm is not UNSET or operation.pad_size_x_nm is not UNSET or operation.pad_size_y_nm is not UNSET:
                    raise msgspec.ValidationError(f"invalid_symbol_operation at {path}")
    if value.total_operations != total_operations:
        raise msgspec.ValidationError("operation_count_mismatch at $.total_operations")
decode_symbol_plot_request_a0 = msgspec.json.Decoder(SymbolPlotRequestA0).decode
decode_symbol_plot_result_a0 = msgspec.json.Decoder(SymbolPlotResultA0).decode
decode_symbol_library_edit_request_a0 = msgspec.json.Decoder(SymbolLibraryEditRequestA0).decode
decode_symbol_library_edit_result_a0 = msgspec.json.Decoder(SymbolLibraryEditResultA0).decode
decode_symbol_library_read_request_a0 = msgspec.json.Decoder(SymbolLibraryReadRequestA0).decode
decode_symbol_library_read_result_a0 = msgspec.json.Decoder(SymbolLibraryReadResultA0).decode
decode_compiled_schematic_graph_a0 = msgspec.json.Decoder(CompiledSchematicGraphA0).decode
decode_source_bundle_manifest_a0 = msgspec.json.Decoder(SourceBundleManifestA0).decode


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
    "PlotterQuad",
    "SymbolPlotRecord",
    "SymbolHeaderPlotRecord",
    "LibSubsymbolPlotRecord",
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
    "SymbolPlotDocumentA0",
    "SymbolPlotRequestA0",
    "SymbolPlotResultA0",
    "SymbolLibraryEditRequestA0",
    "SymbolLibraryEditResultA0",
    "SymbolLibraryReadRequestA0",
    "SymbolLibraryReadResultA0",
    "CompiledSchematicGraphA0",
    "SourceBundleManifestA0",
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
    "decode_symbol_plot_document_a0",
    "decode_symbol_plot_request_a0",
    "decode_symbol_plot_result_a0",
    "decode_symbol_library_edit_request_a0",
    "decode_symbol_library_edit_result_a0",
    "decode_symbol_library_read_request_a0",
    "decode_symbol_library_read_result_a0",
    "decode_compiled_schematic_graph_a0",
    "decode_source_bundle_manifest_a0",
    "validate_footprint_plot_document_a0",
    "validate_symbol_plot_document_a0",
)
