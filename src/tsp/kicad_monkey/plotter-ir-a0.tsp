import "@typespec/json-schema";
import "./sexpr-a0.tsp";

using TypeSpec.JsonSchema;
using Wavenumber.KicadMonkey.Contracts.SexprA0;

namespace Wavenumber.KicadMonkey.Contracts.PlotterIrA0;

/** Integer that remains exact when decoded as a JavaScript number. */
@minValue(-9007199254740991)
@maxValue(9007199254740991)
scalar JavaScriptSafeInteger extends safeint;

/** Resource-bounded footprint plotter operation. Source bytes are out of band. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.footprint_plot.request:a0")
@extension("title", "Footprint plot request a0")
model FootprintPlotRequest {
  type: "kicad_monkey.footprint_plot.request";
  version: "a0";
  source_path?: string;
  document_id?: string;
  max_source_bytes: uint64;
  max_output_bytes: uint64;
  max_depth: uint32;
  max_metadata_forms: uint32;
  max_operations: uint32;
  max_points: uint32;
}

/** Metadata paired with a separate kicad.plotter_ir.a0 JSON byte buffer. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.footprint_plot.result:a0")
@extension("title", "Footprint plot result a0")
model FootprintPlotResult {
  type: "kicad_monkey.footprint_plot.result";
  version: "a0";
  output_bytes: uint64;
  total_operations: uint32;
  diagnostics: Diagnostic[];
}

/** Coordinate convention for the footprint plotter slice. */
model PlotterCoordinateSpace {
  unit: "nm";
  y_axis: "down";
}

/** Fill values shared by plotter operation producers. */
enum PlotterFill {
  noFill: "NO_FILL",
  filledShape: "FILLED_SHAPE",
  filledWithBackgroundBodyColor: "FILLED_WITH_BG_BODYCOLOR",
  filledWithColor: "FILLED_WITH_COLOR",
  hatch: "HATCH",
  reverseHatch: "REVERSE_HATCH",
  crossHatch: "CROSS_HATCH",
}

/** KiCad stroke styles carried without producer-specific decomposition. */
enum PlotterLineStyle {
  default: "DEFAULT",
  solid: "SOLID",
  dash: "DASH",
  dot: "DOT",
  dashDot: "DASH_DOT",
  dashDotDot: "DASH_DOT_DOT",
}

/** Semantic roles allowed on shared circle and segment drill operations. */
enum PlotterDrillRole {
  padDrill: "pad_drill",
  npthHole: "npth_hole",
  viaDrill: "via_drill",
  viaMaskDrill: "via_mask_drill",
}

/** Semantic roles allowed on board via flash operations. */
enum PlotterViaFlashRole {
  viaAperture: "via_aperture",
  viaMaskOpening: "via_mask_opening",
}

/**
 * Solid or decomposed segment shared by PCB, footprint, and drill producers.
 * Graphic state requires only layer. Drill state requires role plus layers;
 * NPTH drill state additionally requires all mask and pad-size hints. The
 * generated semantic validator enforces these mutually exclusive states.
 */
model ThickSegmentOperation {
  kind: "ThickSegment";
  index: uint32;
  start_x: JavaScriptSafeInteger;
  start_y: JavaScriptSafeInteger;
  end_x: JavaScriptSafeInteger;
  end_y: JavaScriptSafeInteger;
  width_nm: JavaScriptSafeInteger;
  layer?: string;
  role?: PlotterDrillRole;
  layers?: string[];
  mask_margin_nm?: JavaScriptSafeInteger;
  pad_size_x_nm?: JavaScriptSafeInteger;
  pad_size_y_nm?: JavaScriptSafeInteger;
}

/** Solid three-point arc. */
model ArcThreePointOperation {
  kind: "ArcThreePoint";
  index: uint32;
  start_x: JavaScriptSafeInteger;
  start_y: JavaScriptSafeInteger;
  mid_x: JavaScriptSafeInteger;
  mid_y: JavaScriptSafeInteger;
  end_x: JavaScriptSafeInteger;
  end_y: JavaScriptSafeInteger;
  fill: PlotterFill;
  width_nm: JavaScriptSafeInteger;
  layer?: string;
  stroke_color?: string;
  fill_color?: string;
  line_style?: PlotterLineStyle;
}

/**
 * Circle shared by graphical and drill producers. Graphic state requires only
 * layer. Drill state requires role plus layers; NPTH state additionally
 * requires all mask and pad-size hints. The generated semantic validator
 * enforces these mutually exclusive states.
 */
model CircleOperation {
  kind: "Circle";
  index: uint32;
  cx: JavaScriptSafeInteger;
  cy: JavaScriptSafeInteger;
  diameter_nm: JavaScriptSafeInteger;
  fill: PlotterFill;
  width_nm: JavaScriptSafeInteger;
  layer?: string;
  role?: PlotterDrillRole;
  layers?: string[];
  mask_margin_nm?: JavaScriptSafeInteger;
  pad_size_x_nm?: JavaScriptSafeInteger;
  pad_size_y_nm?: JavaScriptSafeInteger;
  stroke_color?: string;
  fill_color?: string;
  line_style?: PlotterLineStyle;
}

/** Rectangle with square corners. */
model RectOperation {
  kind: "Rect";
  index: uint32;
  x1: JavaScriptSafeInteger;
  y1: JavaScriptSafeInteger;
  x2: JavaScriptSafeInteger;
  y2: JavaScriptSafeInteger;
  fill: PlotterFill;
  width_nm: JavaScriptSafeInteger;
  corner_radius_nm: JavaScriptSafeInteger;
  layer?: string;
  stroke_color?: string;
  fill_color?: string;
  line_style?: PlotterLineStyle;
}

/** Plotter point encoded as an exact coordinate pair. */
@minItems(2)
@maxItems(2)
model PlotterPoint is Array<JavaScriptSafeInteger>;

/** Filled or outlined polygon operation. */
model PlotPolyOperation {
  kind: "PlotPoly";
  index: uint32;
  points: PlotterPoint[];
  fill: PlotterFill;
  width_nm: JavaScriptSafeInteger;
  layer?: string;
  stroke_color?: string;
  fill_color?: string;
  line_style?: PlotterLineStyle;
}

/** Cubic Bézier shared by symbol and schematic producers. */
model BezierCurveOperation {
  kind: "BezierCurve";
  index: uint32;
  start_x: JavaScriptSafeInteger;
  start_y: JavaScriptSafeInteger;
  ctrl1_x: JavaScriptSafeInteger;
  ctrl1_y: JavaScriptSafeInteger;
  ctrl2_x: JavaScriptSafeInteger;
  ctrl2_y: JavaScriptSafeInteger;
  end_x: JavaScriptSafeInteger;
  end_y: JavaScriptSafeInteger;
  width_nm: JavaScriptSafeInteger;
  tolerance_nm: JavaScriptSafeInteger;
  layer?: string;
  stroke_color?: string;
  line_style?: PlotterLineStyle;
}

/**
 * Circular pad flash shared by footprint and PCB producers. Footprint pad
 * state requires mask_margin_nm and forbids role. Board via state requires
 * role and forbids mask_margin_nm. The generated semantic validator enforces
 * these mutually exclusive states.
 */
model FlashPadCircleOperation {
  kind: "FlashPadCircle";
  index: uint32;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  diameter_nm: JavaScriptSafeInteger;
  layers: string[];
  mask_margin_nm?: JavaScriptSafeInteger;
  role?: PlotterViaFlashRole;
}

/** Oval pad flash shared by footprint and PCB producers. */
model FlashPadOvalOperation {
  kind: "FlashPadOval";
  index: uint32;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  size_x_nm: JavaScriptSafeInteger;
  size_y_nm: JavaScriptSafeInteger;
  orient_deg: float64;
  layers: string[];
  mask_margin_nm: JavaScriptSafeInteger;
}

/** Rectangular pad flash shared by footprint and PCB producers. */
model FlashPadRectOperation {
  kind: "FlashPadRect";
  index: uint32;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  size_x_nm: JavaScriptSafeInteger;
  size_y_nm: JavaScriptSafeInteger;
  orient_deg: float64;
  layers: string[];
  mask_margin_nm: JavaScriptSafeInteger;
}

/** Rounded-rectangle pad flash shared by footprint and PCB producers. */
model FlashPadRoundRectOperation {
  kind: "FlashPadRoundRect";
  index: uint32;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  size_x_nm: JavaScriptSafeInteger;
  size_y_nm: JavaScriptSafeInteger;
  corner_radius_nm: JavaScriptSafeInteger;
  orient_deg: float64;
  layers: string[];
  mask_margin_nm: JavaScriptSafeInteger;
}

/**
 * Custom pad flash shared by footprint and PCB producers. Polygon coordinates
 * are pad-local. A non-empty polygon_widths_nm has one entry per polygon;
 * generated semantic validation enforces that relationship. An empty array is
 * equivalent to omission for generated Rust transport bindings.
 */
model FlashPadCustomOperation {
  kind: "FlashPadCustom";
  index: uint32;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  size_x_nm: JavaScriptSafeInteger;
  size_y_nm: JavaScriptSafeInteger;
  orient_deg: float64;
  polygons: PlotterPoint[][];
  polygon_widths_nm?: JavaScriptSafeInteger[];
  anchor_shape?: string;
  layers: string[];
  mask_margin_nm: JavaScriptSafeInteger;
}

/** Horizontal text alignments emitted by the board producers. */
enum PlotterTextHAlign {
  left: "GR_TEXT_H_ALIGN_LEFT",
  center: "GR_TEXT_H_ALIGN_CENTER",
  right: "GR_TEXT_H_ALIGN_RIGHT",
}

/** Vertical text alignments emitted by the board producers. */
enum PlotterTextVAlign {
  top: "GR_TEXT_V_ALIGN_TOP",
  center: "GR_TEXT_V_ALIGN_CENTER",
  bottom: "GR_TEXT_V_ALIGN_BOTTOM",
}

/** One render-cache polygon as ordered contours, exterior ring first. */
model TextRenderCachePolygon {
  contours: PlotterPoint[][];
}

/**
 * Typed authored render cache mirrored from `(render_cache ...)` forms. The
 * promoted producers only forward file caches, so `source` is pinned to
 * `existing_file_cache`; `knockout` appears when the knockout background
 * restructure replaced the polygons.
 */
model TextRenderCache {
  schema: "kicad.render_cache.v1";
  unit: "nm";
  coordinate_space: "board";
  text: string;
  angle: float64;
  source: "existing_file_cache";
  exact: boolean;
  polygons: TextRenderCachePolygon[];
  knockout?: boolean;
}

/**
 * Stroke or cached text operation. Boolean marker keys (`mirror`,
 * `text_as_polygons`, `polyline_per_segment`, `knockout`) are present-only
 * -when-true, matching the established Python emitter. Render-cache keys
 * appear together when an authored cache resolves; `render_cache_polygons`
 * carries the exterior rings in nanometres.
 */
model TextOperation {
  kind: "Text";
  index: uint32;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  text: string;
  color: string;
  orient_deg: float64;
  size_x_nm: JavaScriptSafeInteger;
  size_y_nm: JavaScriptSafeInteger;
  h_align: PlotterTextHAlign;
  v_align: PlotterTextVAlign;
  pen_width_nm: JavaScriptSafeInteger;
  italic: boolean;
  bold: boolean;
  multiline: boolean;
  font_face: string;
  layer?: string;
  mirror?: boolean;
  text_as_polygons?: boolean;
  polyline_per_segment?: boolean;
  knockout?: boolean;
  render_cache_polygons?: PlotterPoint[][];
  render_cache?: TextRenderCache;
  render_cache_source?: "existing_file_cache";
  render_cache_exact?: boolean;
}

/** Four pad-local trapezoid corners. */
@minItems(4)
@maxItems(4)
model PlotterQuad is Array<PlotterPoint>;

/** Trapezoid pad flash shared by footprint and PCB producers. */
model FlashPadTrapezOperation {
  kind: "FlashPadTrapez";
  index: uint32;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  corners: PlotterQuad;
  orient_deg: float64;
  layers: string[];
  mask_margin_nm: JavaScriptSafeInteger;
}

/** Shared plotter operation vocabulary promoted across source producers. */
union PlotterOperation {
  thickSegment: ThickSegmentOperation,
  arcThreePoint: ArcThreePointOperation,
  circle: CircleOperation,
  rect: RectOperation,
  plotPoly: PlotPolyOperation,
  bezierCurve: BezierCurveOperation,
  text: TextOperation,
  flashPadCircle: FlashPadCircleOperation,
  flashPadOval: FlashPadOvalOperation,
  flashPadRect: FlashPadRectOperation,
  flashPadRoundRect: FlashPadRoundRectOperation,
  flashPadCustom: FlashPadCustomOperation,
  flashPadTrapez: FlashPadTrapezOperation,
}

/** One footprint record in the promoted non-text graphics slice. */
model FootprintPlotRecord {
  uuid: string;
  kind: "footprint";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  name: string;
  layer: string;
  locked: boolean;
  placed: boolean;
  descr: string;
  tags: string;
  attr: string[];
}

/**
 * Strict non-text footprint subset of kicad.plotter_ir.a0. Producers and
 * consumers must run generated semantic validation after structural decoding.
 */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.footprint_plot.document:a0")
@extension("title", "Footprint plot document a0")
model FootprintPlotDocument {
  schema: "kicad.plotter_ir.a0";
  source_kind: "MOD";
  total_operations: uint32;
  records: FootprintPlotRecord[];
  source_path?: string;
  document_id: string;
  coordinate_space: PlotterCoordinateSpace;
  version: JavaScriptSafeInteger;
  generator: string;
  generator_version: string;
}

/**
 * One exact net-name to ordered net-class assignment mirrored from the
 * project sidecar's `net_settings.netclass_assignments` entries.
 */
model BoardNetClassAssignment {
  net_name: string;
  classes: string[];
}

/**
 * One project-sidecar text variable. The producer case-expands names to
 * original/lower/upper aliases and overlays board `(property ...)` values,
 * matching the established `board_text_variables` merge.
 */
model BoardTextVariable {
  name: string;
  value: string;
}

/** Resource-bounded board plotter operation. Source bytes are out of band. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.board_plot.request:a0")
@extension("title", "Board plot request a0")
model BoardPlotRequest {
  type: "kicad_monkey.board_plot.request";
  version: "a0";
  source_path?: string;
  document_id?: string;
  max_source_bytes: uint64;
  max_output_bytes: uint64;
  max_depth: uint32;
  max_graphics: uint32;
  max_operations: uint32;
  max_points: uint32;
  max_text_bytes: uint64;
  max_parse_nodes: uint32;
  max_input_points: uint32;
  max_input_polygons: uint32;
  max_cache_polygons: uint32;
  max_cache_contours: uint32;
  net_class_assignments?: BoardNetClassAssignment[];
  text_variables?: BoardTextVariable[];
}

/** Metadata paired with a separate board plotter-IR JSON byte buffer. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.board_plot.result:a0")
@extension("title", "Board plot result a0")
model BoardPlotResult {
  type: "kicad_monkey.board_plot.result";
  version: "a0";
  output_bytes: uint64;
  total_operations: uint32;
  diagnostics: Diagnostic[];
}

/** Board graphic record kinds promoted in the first board slice. */
enum BoardGraphicRecordKind {
  grLine: "gr_line",
  grArc: "gr_arc",
  grCircle: "gr_circle",
  grRect: "gr_rect",
  grPoly: "gr_poly",
  grCurve: "gr_curve",
}

/**
 * One board-level graphic record. The carrier layer travels on the record;
 * the contained operations are layerless graphic-state operations.
 */
model BoardGraphicPlotRecord {
  uuid: string;
  kind: BoardGraphicRecordKind;
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  layer: string | null;
}

/** One board track segment record with its net attribution. */
model TrackSegmentPlotRecord {
  uuid: string;
  kind: "segment";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  layer: string;
  locked: boolean;
  net_id?: JavaScriptSafeInteger;
  net_name?: string;
  net_class?: string;
  net_classes?: string[];
}

/** One board track arc record with its net attribution. */
model TrackArcPlotRecord {
  uuid: string;
  kind: "track_arc";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  layer: string;
  net_id?: JavaScriptSafeInteger;
  net_name?: string;
  net_class?: string;
  net_classes?: string[];
}

/** Via construction kinds mirrored from the established producer. */
enum BoardViaType {
  through: "through",
  blind: "blind",
  buried: "buried",
  micro: "micro",
}

/** Stringified boolean metadata mirrored from the established producer. */
enum PlotterStringBool {
  yes: "true",
  no: "false",
}

/**
 * One board via record: copper aperture, synthetic drill, and per-side mask
 * opening/drill pairs when tenting explicitly exposes that side. IPC-4761
 * fabrication metadata mirrors the established stringified booleans.
 */
model ViaPlotRecord {
  uuid: string;
  kind: "via";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  layers: string[];
  drill: float64;
  size: float64;
  via_type: BoardViaType;
  hole_kind: "round";
  hole_plating: "plated";
  hole_render: "drill";
  ipc4761_tenting_front?: PlotterStringBool;
  ipc4761_tenting_back?: PlotterStringBool;
  ipc4761_covering_front?: PlotterStringBool;
  ipc4761_covering_back?: PlotterStringBool;
  ipc4761_plugging_front?: PlotterStringBool;
  ipc4761_plugging_back?: PlotterStringBool;
  ipc4761_capping?: PlotterStringBool;
  ipc4761_filling?: PlotterStringBool;
  ipc4761_metadata?: "true";
  net_id?: JavaScriptSafeInteger;
  net_name?: string;
  net_class?: string;
  net_classes?: string[];
}

/** Board table grid/border segments followed by optional faced cell text. */
model TablePlotRecord {
  uuid: string;
  kind: "table";
  object_id: "table";
  operation_count: uint32;
  operations: PlotterOperation[];
  layers: string[];
  cell_count: uint32;
}

/**
 * One zone fill record bundling every `filled_polygon` ring. The parallel
 * `fill_layers`/`fill_island` arrays annotate the rings so consumers can
 * split or colour-key without re-walking the source zone.
 */
model ZoneFillPlotRecord {
  uuid: string;
  kind: "zone_fill";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  layers: string[];
  fill_layers: string[];
  fill_island: boolean[];
  net_id?: JavaScriptSafeInteger;
  net_name?: string;
  net_class?: string;
  net_classes?: string[];
}

/**
 * One board free-text record. `hide` mirrors the established serializer's
 * getattr default and is always false for board gr_text carriers.
 */
model BoardTextPlotRecord {
  uuid: string;
  kind: "gr_text";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  layer: string;
  text: string;
  hide: boolean;
}

/**
 * One board text-box record. A visible border contributes a leading Rect
 * operation; empty resolved text drops the Text operation.
 */
model BoardTextBoxPlotRecord {
  uuid: string;
  kind: "gr_text_box";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  layer: string;
  text: string;
  border: boolean;
}

union BoardPlotRecord {
  graphic: BoardGraphicPlotRecord,
  segment: TrackSegmentPlotRecord,
  trackArc: TrackArcPlotRecord,
  via: ViaPlotRecord,
  table: TablePlotRecord,
  zoneFill: ZoneFillPlotRecord,
  text: BoardTextPlotRecord,
  textBox: BoardTextBoxPlotRecord,
}

/**
 * Strict board graphics, text, tracks, vias, and authored-zone-fill subset of
 * kicad.plotter_ir.a0. Producers and consumers must run generated semantic
 * validation after structural decoding.
 */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.board_plot.document:a0")
@extension("title", "Board plot document a0")
model BoardPlotDocument {
  schema: "kicad.plotter_ir.a0";
  source_kind: "PCB";
  total_operations: uint32;
  records: BoardPlotRecord[];
  source_path?: string;
  document_id: string;
  coordinate_space: PlotterCoordinateSpace;
  version: JavaScriptSafeInteger;
  generator: string;
  generator_version: string;
  thickness_mm: float64;
  paper: string;
}

/** Resource-bounded library-symbol plot operation. Source bytes are out of band. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.symbol_plot.request:a0")
@extension("title", "Symbol plot request a0")
model SymbolPlotRequest {
  type: "kicad_monkey.symbol_plot.request";
  version: "a0";
  symbol_name: string;
  unit?: uint32;
  style: uint32;
  source_path?: string;
  document_id?: string;
  max_source_bytes: uint64;
  max_output_bytes: uint64;
  max_depth: uint32;
  max_symbols: uint32;
  max_subsymbols: uint32;
  max_operations: uint32;
  max_points: uint32;
}

/** Metadata paired with a separate symbol plotter-IR JSON byte buffer. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.symbol_plot.result:a0")
@extension("title", "Symbol plot result a0")
model SymbolPlotResult {
  type: "kicad_monkey.symbol_plot.result";
  version: "a0";
  output_bytes: uint64;
  total_operations: uint32;
  diagnostics: Diagnostic[];
}

/** Leading metadata record for the selected library symbol. */
model SymbolHeaderPlotRecord {
  uuid: "";
  kind: "lib_symbol";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  name: string;
  `extends`?: string;
  unit?: uint32;
  style: uint32;
  in_bom: boolean;
  on_board: boolean;
  power: boolean;
}

/** One selected graphical subsymbol record. */
model LibSubsymbolPlotRecord {
  uuid: "";
  kind: "lib_subsymbol";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  unit: uint32;
  style: uint32;
}

union SymbolPlotRecord {
  header: SymbolHeaderPlotRecord,
  subsymbol: LibSubsymbolPlotRecord,
}

/** Strict non-text library-symbol subset of kicad.plotter_ir.a0. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.symbol_plot.document:a0")
@extension("title", "Symbol plot document a0")
model SymbolPlotDocument {
  schema: "kicad.plotter_ir.a0";
  source_kind: "SYM";
  total_operations: uint32;
  records: SymbolPlotRecord[];
  source_path?: string;
  document_id: string;
  coordinate_space: PlotterCoordinateSpace;
}
