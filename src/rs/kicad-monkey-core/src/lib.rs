//! Rust core for KiCad Monkey.
//!
//! The first implementation slice is the byte-oriented KiCad S-expression
//! reader/writer foundation. Higher-level typed source models and interchange
//! operations are layered on this crate after parser parity is established.

#![forbid(unsafe_code)]

pub mod footprint;
pub mod pcb;
pub mod plotter_ir;
pub mod plotter_types;
pub mod sexpr;
pub mod sexpr_mutation;
pub mod sexpr_projection;
pub mod symbol_library;
mod symbol_pin;
pub mod symbol_plotter_ir;

pub use footprint::{FootprintEdit, FootprintLimits, FootprintProperty, FootprintView};
pub use pcb::{
    PcbBarcode, PcbBoardMetadata, PcbBoardVariant, PcbCounts, PcbDimension, PcbDocument, PcbEdit,
    PcbEmbeddedFile, PcbFamily, PcbFootprint, PcbFootprintGraphic, PcbFootprintProperty,
    PcbFootprintTransform, PcbGeneratedItem, PcbGraphic, PcbGraphicKind, PcbGroup, PcbHole,
    PcbHoleOwner, PcbHoleShape, PcbImage, PcbLayer, PcbLimits, PcbModelReference, PcbNet,
    PcbNetRef, PcbPad, PcbPadDrill, PcbPoint, PcbProfileOwner, PcbProfilePrimitive, PcbProperty,
    PcbRoutingArc, PcbSegment, PcbSelection, PcbSetup, PcbStackup, PcbStackupLayer, PcbTable,
    PcbTableCell, PcbVia, PcbView, PcbZone, PcbZoneFilledPolygon, PcbZoneKeepout,
    PcbZoneLayerProperty, PcbZonePlacement, PcbZonePlacementSource, PcbZonePolygon,
};
pub use plotter_ir::{FootprintPlotDocument, FootprintPlotLimits, footprint_plot_document};
pub use plotter_types::{
    ArcThreePoint, BezierCurve, FlashPadCircle, FlashPadCustom, FlashPadOval, FlashPadRect,
    FlashPadRoundRect, FlashPadTrapez, PlotterCircle, PlotterFill, PlotterLineStyle,
    PlotterOperation, PlotterPoly, PlotterRect, ThickSegment,
};
pub use symbol_library::{
    SymbolBooleanField, SymbolLibraryEdit, SymbolLibraryLimits, SymbolLibraryView, SymbolSummary,
};
pub use symbol_plotter_ir::{
    SymbolPlotDocument, SymbolPlotLimits, SymbolPlotRecord, symbol_plot_document,
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
