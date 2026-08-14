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


PlotterOperation = Union["ThickSegmentOperation", "ArcThreePointOperation", "CircleOperation", "RectOperation", "PlotPolyOperation", "BezierCurveOperation", "FlashPadCircleOperation", "FlashPadOvalOperation", "FlashPadRectOperation", "FlashPadRoundRectOperation", "FlashPadCustomOperation", "FlashPadTrapezOperation"]


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


class FlashPadCircleOperation(Struct, forbid_unknown_fields=True, frozen=True, tag="FlashPadCircle", tag_field="kind"):
    index: Annotated[int, Meta(ge=0, le=4294967295)]
    x: JavaScriptSafeInteger
    y: JavaScriptSafeInteger
    diameter_nm: JavaScriptSafeInteger
    layers: list[str]
    mask_margin_nm: JavaScriptSafeInteger


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


PlotterDrillRole = Literal["pad_drill", "npth_hole"]


PlotterFill = Literal["NO_FILL", "FILLED_SHAPE", "FILLED_WITH_BG_BODYCOLOR", "FILLED_WITH_COLOR", "HATCH", "REVERSE_HATCH", "CROSS_HATCH"]


PlotterLineStyle = Literal["DEFAULT", "SOLID", "DASH", "DOT", "DASH_DOT", "DASH_DOT_DOT"]


PlotterPoint = Annotated[list[JavaScriptSafeInteger], Meta(min_length=2, max_length=2)]


PlotterQuad = Annotated[list[PlotterPoint], Meta(min_length=4, max_length=4)]


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
    id: str
    slot: Annotated[int, Meta(ge=0, le=4294967295)]
    sha256: Sha256Hex
    face_index: Annotated[int, Meta(ge=0, le=4294967295)]
    variations: list[FontVariationCoordinate]
    aliases: list[str]
    family: str | UnsetType = field(default=UNSET)
    style: str | UnsetType = field(default=UNSET)
    postscript_name: str | UnsetType = field(default=UNSET)


Sha256Hex = Annotated[str, Meta(pattern="^[0-9a-f]{64}$")]


class FontVariationCoordinate(Struct, forbid_unknown_fields=True, frozen=True):
    axis: OpenTypeTag
    value: float


OpenTypeTag = Annotated[str, Meta(pattern="^[ -~]{4}$")]


class FontSelection(Struct, forbid_unknown_fields=True, frozen=True):
    aliases: list[str]
    font_id: str | UnsetType = field(default=UNSET)


class ExactComparisonPolicy(Struct, forbid_unknown_fields=True, frozen=True, tag="exact", tag_field="mode"):
    pass


class ShapingInput(Struct, forbid_unknown_fields=True, frozen=True):
    font_id: str
    font_sha256: Sha256Hex
    face_index: Annotated[int, Meta(ge=0, le=4294967295)]
    variations: list[FontVariationCoordinate]
    text: str
    scale_x: TextSafeInteger
    scale_y: TextSafeInteger
    direction: TextDirection
    features: list[ShapingFeature]
    buffer_properties: ShapingBufferProperties
    script: OpenTypeTag | UnsetType = field(default=UNSET)
    language: str | UnsetType = field(default=UNSET)


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


TextSafeInteger = Annotated[int, Meta(ge=-9007199254740991, le=9007199254740991)]


TextDirection = Literal["left_to_right", "right_to_left", "top_to_bottom", "bottom_to_top"]


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


ShapingClusterLevel = Literal["monotone_graphemes", "monotone_characters", "characters"]


DefaultIgnorablePolicy = Literal["normal", "preserve", "remove"]


NumericComparisonPolicy = Union["ExactComparisonPolicy", "AbsoluteToleranceComparisonPolicy"]


OutlineCommand = Union["OutlineMoveTo", "OutlineLineTo", "OutlineQuadTo", "OutlineCurveTo", "OutlineClose"]


class AbsoluteToleranceComparisonPolicy(Struct, forbid_unknown_fields=True, frozen=True, tag="absolute_tolerance", tag_field="mode"):
    absolute_tolerance: NonNegativeFiniteFloat


class OutlineMoveTo(Struct, forbid_unknown_fields=True, frozen=True, tag="move_to", tag_field="kind"):
    x: float
    y: float


class OutlineLineTo(Struct, forbid_unknown_fields=True, frozen=True, tag="line_to", tag_field="kind"):
    x: float
    y: float


class OutlineQuadTo(Struct, forbid_unknown_fields=True, frozen=True, tag="quad_to", tag_field="kind"):
    control_x: float
    control_y: float
    x: float
    y: float


class OutlineCurveTo(Struct, forbid_unknown_fields=True, frozen=True, tag="curve_to", tag_field="kind"):
    control1_x: float
    control1_y: float
    control2_x: float
    control2_y: float
    x: float
    y: float


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


class FootprintPlotResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.footprint_plot.result"] = field(name="type")
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


class SymbolPlotResultA0(Struct, forbid_unknown_fields=True, frozen=True):
    type_: Literal["kicad_monkey.symbol_plot.result"] = field(name="type")
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
    case_id: str
    comparison: ExactComparisonPolicy
    input: ShapingInput
    glyphs: list[ShapedGlyph]


class OutlineVectorA0(Struct, forbid_unknown_fields=True, frozen=True):
    schema: Literal["kicad_monkey.outline_vector.a0"]
    type_: Literal["kicad_monkey.outline_vector"] = field(name="type")
    version: Literal["a0"]
    case_id: str
    coordinate_format: Literal["font_design_units_f64"]
    comparison: NumericComparisonPolicy
    font_id: str
    font_sha256: Sha256Hex
    face_index: Annotated[int, Meta(ge=0, le=4294967295)]
    variations: list[FontVariationCoordinate]
    glyph_id: Annotated[int, Meta(ge=0, le=4294967295)]
    units_per_em: Annotated[int, Meta(ge=0, le=4294967295)]
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
decode_font_resolution_request_a0 = msgspec.json.Decoder(FontResolutionRequestA0).decode
decode_shaping_record_a0 = msgspec.json.Decoder(ShapingRecordA0).decode
decode_outline_vector_a0 = msgspec.json.Decoder(OutlineVectorA0).decode


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
    "SourceSlot",
    "CanonicalUint64Decimal",
    "FontBundleEntry",
    "Sha256Hex",
    "FontVariationCoordinate",
    "OpenTypeTag",
    "FontSelection",
    "ExactComparisonPolicy",
    "ShapingInput",
    "ShapedGlyph",
    "TextSafeInteger",
    "TextDirection",
    "ShapingFeature",
    "ShapingBufferProperties",
    "ShapingClusterLevel",
    "DefaultIgnorablePolicy",
    "NumericComparisonPolicy",
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
    "SymbolPlotDocumentA0",
    "SymbolPlotRequestA0",
    "SymbolPlotResultA0",
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
    "decode_symbol_plot_document_a0",
    "decode_symbol_plot_request_a0",
    "decode_symbol_plot_result_a0",
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
    "resolve_font_selection_a0",
    "validate_font_bundle_manifest_a0",
    "validate_symbol_plot_document_a0",
)
