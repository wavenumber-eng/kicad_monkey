//! Rust core for KiCad Monkey.
//!
//! The first implementation slice is the byte-oriented KiCad S-expression
//! reader/writer foundation. Higher-level typed source models and interchange
//! operations are layered on this crate after parser parity is established.

#![forbid(unsafe_code)]

pub mod board_plotter_ir;
pub mod compiled_schematic_graph;
pub mod design_json;
pub mod document_metadata;
mod fake_style;
pub mod font_outline;
pub mod footprint;
mod footprint_plotter_text;
mod footprint_text;
pub mod kicad_netlist;
pub mod pcb;
pub mod plotter_ir;
pub mod plotter_text_cache;
pub mod plotter_types;
pub mod project;
pub mod schematic_bundle;
mod schematic_bundle_indexes;
mod schematic_bundle_limits;
pub mod schematic_bus;
pub mod schematic_bus_connectivity;
pub mod schematic_connectivity;
pub mod schematic_design;
mod schematic_effective;
pub mod schematic_embedded;
pub mod schematic_netlist;
pub mod schematic_plot_contract;
pub mod schematic_plotter_ir;
mod schematic_project;
mod schematic_segment_index;
mod schematic_source;
mod schematic_terminals;
pub mod sexpr;
pub mod sexpr_mutation;
pub mod sexpr_projection;
pub mod source_bundle;
pub mod symbol_library;
mod symbol_pin;
pub mod symbol_plotter_ir;
mod symbol_text;
pub mod text_bezier;
pub mod text_block_layout;
pub mod text_contours;
pub mod text_layout;
pub mod text_linebreak;
mod text_markup;
pub mod text_metadata;
pub mod text_render_cache;
pub mod text_shaping;
pub mod text_topology;
pub mod worksheet;
mod worksheet_preflight;

pub use board_plotter_ir::{
    BoardDimensionOperation, BoardDimensionRecord, BoardFootprintBlock,
    BoardFootprintBlockAttributes, BoardFootprintChildAttributes, BoardFootprintChildMetadata,
    BoardFootprintOperation, BoardFootprintPlacement, BoardFootprintRecord, BoardGraphicRecord,
    BoardGraphicRecordKind, BoardNetClassAssignments, BoardNetClassExtras, BoardPlotDocument,
    BoardPlotLimits, BoardPlotRecord, BoardSegmentRecord, BoardTableOperation, BoardTableRecord,
    BoardTextBoxOperation, BoardTextBoxRecord, BoardTextHAlign, BoardTextOperation,
    BoardTextRecord, BoardTextRenderCache, BoardTextRenderCacheCoordinateSpace,
    BoardTextRenderCacheSource, BoardTextVAlign, BoardTextVariables, BoardTrackArcRecord,
    BoardViaFabrication, BoardViaOperation, BoardViaOperationKind, BoardViaRecord, BoardViaType,
    BoardZoneRecord, board_plot_document, board_plot_document_with_net_classes,
    board_plot_document_with_sidecars, board_plot_document_with_text_cache_sidecar,
};
pub use compiled_schematic_graph::{
    CompiledGraphIdentityAllocator, CompiledGraphIdentityError, CompiledSchematicGraphLimits,
    IdentityMapping, build_compiled_schematic_graph, compiled_schematic_graph_design_scope,
    validate_compiled_schematic_graph,
};
pub use design_json::{
    KiCadDesignFacts, KiCadDesignJsonError, KiCadDesignJsonLimits, KiCadDesignJsonPaths,
    KiCadDesignPcb, KiCadDesignSourcePath, KiCadSchematicInstance, build_kicad_design_facts,
    build_kicad_design_json, build_kicad_design_json_with_limits,
};
pub use document_metadata::{KiCadPaper, KiCadTitleBlock};
pub use font_outline::{
    FONT_OUTLINE_ENGINE, FontOutlineError, FontOutlineErrorKind, FontOutlineFace,
    FontOutlineFaceRequest, FontOutlineLimits, FontOutlineOutput, FontOutlineRequest,
    HINTED_FONT_OUTLINE_ENGINE, HintedFontOutlineFace, extract_font_outline_a0,
};
pub use footprint::{FootprintEdit, FootprintLimits, FootprintProperty, FootprintView};
pub use footprint_text::{FootprintGraphicalProperty, FootprintText, FootprintTextBox};
pub use kicad_netlist::{
    KiCadDesignSheet, KiCadLibPart, KiCadLibPartPin, KiCadNet, KiCadNetClass, KiCadNetlist,
    KiCadNetlistComponent, KiCadNetlistComponentUnit, KiCadNetlistEndpoint,
    KiCadNetlistGraphicalIds, KiCadNetlistJsonMetadata, KiCadNetlistLimits, KiCadNetlistTerminal,
    build_kicad_netlist, build_kicad_netlist_json, emit_kicad_netlist,
};
pub use pcb::{
    PcbBarcode, PcbBoardMetadata, PcbBoardVariant, PcbCounts, PcbDimension, PcbDocument,
    PcbDrillLayerSpan, PcbDrillProperties, PcbEdit, PcbEmbeddedFile, PcbFamily, PcbFootprint,
    PcbFootprintGraphic, PcbFootprintProperty, PcbFootprintText, PcbFootprintTextBox,
    PcbFootprintTransform, PcbFrontBackOptionalBool, PcbGeneratedItem, PcbGraphic, PcbGraphicKind,
    PcbGroup, PcbHole, PcbHoleOwner, PcbHoleShape, PcbImage, PcbLayer, PcbLimits,
    PcbModelReference, PcbNet, PcbNetRef, PcbPad, PcbPadCustomOptions, PcbPadCustomPrimitive,
    PcbPadDrill, PcbPoint, PcbPostMachiningProperties, PcbProfileOwner, PcbProfilePrimitive,
    PcbProperty, PcbRoutingArc, PcbSegment, PcbSelection, PcbSetup, PcbStackup, PcbStackupLayer,
    PcbTable, PcbTableCell, PcbTeardropParameters, PcbVia, PcbView, PcbZone, PcbZoneFilledPolygon,
    PcbZoneKeepout, PcbZoneLayerConnections, PcbZoneLayerProperty, PcbZonePlacement,
    PcbZonePlacementSource, PcbZonePolygon,
};
pub use plotter_ir::{FootprintPlotDocument, FootprintPlotLimits, footprint_plot_document};
pub use plotter_text_cache::{PlotterTextCacheLimits, PlotterTextCacheResources, PlotterTextFont};
pub use plotter_types::{
    ArcThreePoint, BezierCurve, FlashPadCircle, FlashPadCustom, FlashPadOval, FlashPadRect,
    FlashPadRoundRect, FlashPadTrapez, PlotterCircle, PlotterFill, PlotterImage, PlotterLineStyle,
    PlotterOperation, PlotterPoly, PlotterRect, PlotterText, PlotterTextHAlign, PlotterTextVAlign,
    ThickSegment,
};
pub use project::{
    ProjectBoardDesignSettings, ProjectDiffPairDimensions, ProjectDocument, ProjectError,
    ProjectErrorKind, ProjectLimits, ProjectNetClass, ProjectNetClassPattern, ProjectNetSettings,
    ProjectTuningDefaults, ProjectTuningSettings, ProjectVariant, ProjectView,
};
pub use schematic_bundle::{
    SchematicBundleIndex, SchematicBundleLimits, SchematicDefinition, SchematicDocument,
    SchematicDocumentLimits, SchematicOccurrence, SchematicPageInstance, SchematicSheet,
};
pub use schematic_bus::{
    SchematicBusExpansionError, SchematicBusExpansionErrorKind, SchematicBusExpansionLimits,
    SchematicBusPattern, canonical_bus_member_name, expand_schematic_bus_label,
    is_schematic_bus_label, parse_schematic_bus_group, parse_schematic_bus_vector,
};
pub use schematic_bus_connectivity::{
    SchematicBusConnectivityLimits, SchematicBusDriver, SchematicBusDriverKind,
    SchematicBusSubgraph, SchematicDriverPriority, build_schematic_bus_subgraphs,
};
pub use schematic_connectivity::{
    SchematicGraphicalIds, SchematicLabelDriver, SchematicOccurrenceConnectivityLimits,
    SchematicPinDriver, SchematicSubpartSettings, SchematicWireDriverKind, SchematicWireSubgraph,
    build_schematic_occurrence_subgraphs, build_schematic_occurrence_subgraphs_with_settings,
};
pub use schematic_design::{
    SchematicDesignNet, SchematicDesignNetLimits, SchematicDesignNetMember,
    SchematicDesignNetTerminal, SchematicHierarchyNetBinding, SchematicScalarDesignNetlist,
    build_schematic_scalar_design_nets, build_schematic_scalar_design_nets_with_settings,
};
pub use schematic_effective::SchematicEffectiveSymbol;
pub use schematic_netlist::{
    SchematicLocalNet, SchematicLocalNetLimits, SchematicLocalNetTerminal,
    build_schematic_occurrence_nets, build_schematic_occurrence_nets_with_settings,
};
pub use schematic_plot_contract::{
    SchematicPlotContractBudget, SchematicPlotContractError, SchematicPlotContractLimits,
    schematic_plot_document_budget, schematic_plot_document_json,
};
pub use schematic_plotter_ir::{
    SchematicAnnotationRecord, SchematicAnnotationRecordKind, SchematicCanvas,
    SchematicConnectivityRecord, SchematicConnectivityRecordKind, SchematicDrawingSettings,
    SchematicGraphicRecord, SchematicGraphicRecordKind, SchematicImageRecord, SchematicPlotContext,
    SchematicPlotDocument, SchematicPlotLimits, SchematicPlotOperation, SchematicPlotRecord,
    SchematicPlotVariables, SchematicRuleAreaRecord, SchematicRuleAreaShape,
    SchematicSheetHeaderRecord, SchematicSheetPinAttrs, SchematicSheetPinBlock,
    SchematicSheetRecord, SchematicStyledThickSegment, SchematicSymbolInstanceRecord,
    SchematicSymbolOverplotRecord, SchematicSymbolPinAttrs, SchematicSymbolPinBlock,
    SchematicTableRecord, SchematicTextOperation, SchematicTitleBlock, schematic_plot_document,
    schematic_plot_document_with_annotations, schematic_plot_document_with_graphics,
    schematic_plot_document_with_sheets, schematic_plot_document_with_symbols,
};
pub use schematic_source::{
    SCHEMATIC_IU_PER_MM, SchematicBusAlias, SchematicBusEntry, SchematicConnectivity,
    SchematicJunction, SchematicLabel, SchematicLabelScope, SchematicLegacySymbolInstance,
    SchematicLibraryPin, SchematicLibrarySubsymbol, SchematicLibrarySymbol, SchematicNoConnect,
    SchematicPinShape, SchematicPlacedSymbol, SchematicPoint, SchematicPolyline, SchematicSheetPin,
    SchematicSymbolInstance, SchematicSymbolInstanceLookupError, SchematicSymbolInstanceVariant,
    SchematicSymbolPin, SchematicSymbolProperty, SchematicSymbolVariantField,
};
pub use schematic_terminals::SchematicSymbolTerminal;
pub use source_bundle::{
    SourceBundle, SourceBundleError, SourceBundleErrorKind, SourceBundleLimits, SourceFile,
};
pub use symbol_library::{
    SymbolBooleanField, SymbolLibraryEdit, SymbolLibraryLimits, SymbolLibraryView, SymbolSummary,
};
pub use symbol_plotter_ir::{
    SymbolPlotDocument, SymbolPlotLimits, SymbolPlotRecord, symbol_plot_document,
    symbol_plot_document_with_text_variables,
};
pub use symbol_text::SymbolTextVariables;
pub use text_bezier::{
    TextBezierError, TextBezierErrorKind, TextBezierLimits, TextBezierOutput, TextPoint,
    flatten_cubic_bezier, flatten_quadratic_bezier,
};
pub use text_block_layout::{
    KICAD_TEXT_INTERLINE_FACTOR, KICAD_TEXT_OVERBAR_HEIGHT_RATIO, KICAD_TEXT_TAB_WIDTH_FACTOR,
    TextBlockLayoutLimits, TextBlockLayoutOutput, TextBlockLayoutRequest,
    layout_text_block_hinted_a0,
};
pub use text_contours::{
    KICAD_OUTLINE_FACE_SCALER, KICAD_OUTLINE_SIZE_COMPENSATION, KICAD_SUBSCRIPT_FACE_SCALER,
    KICAD_SUBSCRIPT_SUPERSCRIPT_SIZE_RATIO, KICAD_SUBSCRIPT_VERTICAL_OFFSET_RATIO,
    KICAD_SUPERSCRIPT_VERTICAL_OFFSET_RATIO, KICAD_TEXT_BEZIER_ERROR, TextContour,
    TextContourError, TextContourErrorKind, TextContourLimits, TextContourOutput,
    TextContourRequest, shape_text_contours_a0, shape_text_contours_hinted_a0,
};
pub use text_layout::{
    KICAD_TEXT_HEIGHT_FUDGE_FACTOR, TextHorizontalAlignment, TextLayoutRequest,
    TextVerticalAlignment, layout_single_line_text_a0, layout_single_line_text_hinted_a0,
};
pub use text_linebreak::{TextLinebreakLimits, linebreak_text_block_hinted_a0};
pub use text_metadata::{KiCadColor, KiCadFont, KiCadTextEffects};
pub use text_render_cache::{
    TextRenderCache, TextRenderCacheError, TextRenderCacheErrorKind, TextRenderCacheLimits,
    TextRenderCachePolygon, generate_text_render_cache_a0,
    generate_text_render_cache_block_hinted_a0, generate_text_render_cache_hinted_a0,
    read_text_render_cache_a0, write_text_render_cache_a0,
};
pub use text_shaping::{
    TEXT_SHAPING_ENGINE, TextShapingError, TextShapingErrorKind, TextShapingLimits,
    TextShapingOutput, shape_text_a0,
};
pub use text_topology::{TextTopologyLimits, TextTopologyOutput, fracture_text_contours_a0};
pub use worksheet::{
    WorksheetBitmap, WorksheetColor, WorksheetCorner, WorksheetDocument, WorksheetFont,
    WorksheetFormat, WorksheetItem, WorksheetLimits, WorksheetLine, WorksheetMetadata,
    WorksheetPoint, WorksheetPolygon, WorksheetRect, WorksheetRepeat, WorksheetSetup,
    WorksheetText, WorksheetView,
};

pub use sexpr::{
    Error, ErrorKind, ErrorPhase, FormatOptions, Lexer, Limits, Patch, Position, Sexp, Token,
    TokenKind, apply_patches, apply_patches_with_limit, build, build_with_limit, format, lex,
    parse, parse_bytes, parse_with_limits, utf8_text,
};
pub use sexpr_mutation::{
    Walk, find_path, remove_all_elements, remove_element, replace_element, set_value,
    transform_descendants, walk,
};
pub use sexpr_projection::{
    FormSpan, ProjectionLimits, Selector, StructuralIndex, parse_form, read_form_bytes,
    scan_form_spans, scan_form_spans_with_limits, scan_reader_form_spans,
};
#[cfg(feature = "measurement")]
#[doc(hidden)]
pub use sexpr_projection::{measure_form_span_sort, measure_reader_form_span_sort};
