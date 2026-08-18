/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * One explicitly discriminated frozen Phase-5 plot-document root.
 */
export type NativeSvgPlotDocument =
  NativeFootprintSvgDocument | NativeSymbolSvgDocument | NativeBoardSvgDocument | NativeSchematicSvgDocument;
/**
 * Shared plotter operation vocabulary promoted across source producers.
 */
export type PlotterOperation =
  | ThickSegmentOperation
  | ArcThreePointOperation
  | CircleOperation
  | RectOperation
  | PlotPolyOperation
  | BezierCurveOperation
  | TextOperation
  | PlotImageOperation
  | FlashPadCircleOperation
  | FlashPadOvalOperation
  | FlashPadRectOperation
  | FlashPadRoundRectOperation
  | FlashPadCustomOperation
  | FlashPadTrapezOperation;
/**
 * Integer that remains exact when decoded as a JavaScript number.
 */
export type JavaScriptSafeInteger = number;
/**
 * Semantic roles allowed on shared circle and segment drill operations.
 */
export type PlotterDrillRole = "pad_drill" | "npth_hole" | "via_drill" | "via_mask_drill";
/**
 * Fill values shared by plotter operation producers.
 */
export type PlotterFill =
  | "NO_FILL"
  | "FILLED_SHAPE"
  | "FILLED_WITH_BG_BODYCOLOR"
  | "FILLED_WITH_COLOR"
  | "HATCH"
  | "REVERSE_HATCH"
  | "CROSS_HATCH";
/**
 * KiCad stroke styles carried without producer-specific decomposition.
 */
export type PlotterLineStyle = "DEFAULT" | "SOLID" | "DASH" | "DOT" | "DASH_DOT" | "DASH_DOT_DOT";
/**
 * Plotter point encoded as an exact coordinate pair.
 *
 * @minItems 2
 * @maxItems 2
 */
export type PlotterPoint = [JavaScriptSafeInteger, JavaScriptSafeInteger];
/**
 * Horizontal text alignments emitted by the board producers.
 */
export type PlotterTextHAlign = "GR_TEXT_H_ALIGN_LEFT" | "GR_TEXT_H_ALIGN_CENTER" | "GR_TEXT_H_ALIGN_RIGHT";
/**
 * Vertical text alignments emitted by the board producers.
 */
export type PlotterTextVAlign = "GR_TEXT_V_ALIGN_TOP" | "GR_TEXT_V_ALIGN_CENTER" | "GR_TEXT_V_ALIGN_BOTTOM";
/**
 * Coordinate space carried by one typed text render cache.
 */
export type PlotterTextRenderCacheCoordinateSpace = "board" | "footprint_local";
/**
 * Provenance of one attached text render cache.
 */
export type PlotterTextRenderCacheSource = "existing_file_cache" | "python_generated_cache" | "native_generated_cache";
/**
 * Semantic roles allowed on board via flash operations.
 */
export type PlotterViaFlashRole = "via_aperture" | "via_mask_opening";
/**
 * Four pad-local trapezoid corners.
 *
 * @minItems 4
 * @maxItems 4
 */
export type PlotterQuad = [PlotterPoint, PlotterPoint, PlotterPoint, PlotterPoint];
export type SymbolPlotRecord = SymbolHeaderPlotRecord | LibSubsymbolPlotRecord;
export type BoardPlotRecord =
  | BoardGraphicPlotRecord
  | TrackSegmentPlotRecord
  | TrackArcPlotRecord
  | ViaPlotRecord
  | TablePlotRecord
  | DimensionPlotRecord
  | ZoneFillPlotRecord
  | BoardTextPlotRecord
  | BoardTextBoxPlotRecord
  | BoardFootprintPlotRecord;
/**
 * Board graphic record kinds promoted in the first board slice.
 */
export type BoardGraphicRecordKind = "gr_line" | "gr_arc" | "gr_circle" | "gr_rect" | "gr_poly" | "gr_curve";
/**
 * Via construction kinds mirrored from the established producer.
 */
export type BoardViaType = "through" | "blind" | "buried" | "micro";
/**
 * Stringified boolean metadata mirrored from the established producer.
 */
export type PlotterStringBool = "true" | "false";
/**
 * Board dimension construction styles supported by KiCad's PCB plotter.
 */
export type BoardDimensionType = "aligned" | "orthogonal" | "radial" | "leader" | "center";
/**
 * Strict operation vocabulary for one board-embedded footprint record.
 */
export type BoardFootprintOperation =
  | BoardFootprintThickSegmentOperation
  | BoardFootprintArcThreePointOperation
  | BoardFootprintCircleOperation
  | BoardFootprintRectOperation
  | BoardFootprintPlotPolyOperation
  | BoardFootprintBezierCurveOperation
  | BoardFootprintTextOperation
  | BoardFootprintFlashPadCircleOperation
  | BoardFootprintFlashPadOvalOperation
  | BoardFootprintFlashPadRectOperation
  | BoardFootprintFlashPadRoundRectOperation
  | BoardFootprintFlashPadCustomOperation
  | BoardFootprintFlashPadTrapezOperation
  | BoardFootprintStartBlockOperation
  | BoardFootprintEndBlockOperation;
/**
 * Source child kinds emitted directly on embedded-footprint drawing operations.
 */
export type BoardFootprintChildRef =
  "property" | "fp_text" | "fp_text_box" | "fp_line" | "fp_arc" | "fp_circle" | "fp_rect" | "fp_poly";
/**
 * Normalized PCB layer roles mirrored by enriched footprint-child metadata.
 */
export type BoardFootprintLayerRole =
  "copper" | "silkscreen" | "soldermask" | "paste" | "fab" | "courtyard" | "board-outline" | "drill" | "user" | "other";
/**
 * Strict source-record vocabulary through the P5_071 hierarchical sheets.
 */
export type SchematicPlotRecord =
  | SchematicSheetHeaderPlotRecord
  | SchematicWirePlotRecord
  | SchematicBusPlotRecord
  | SchematicBusEntryPlotRecord
  | SchematicJunctionPlotRecord
  | SchematicNoConnectPlotRecord
  | SchematicLabelPlotRecord
  | SchematicGlobalLabelPlotRecord
  | SchematicHierarchicalLabelPlotRecord
  | SchematicNetclassFlagPlotRecord
  | SchematicTextPlotRecord
  | SchematicTextBoxPlotRecord
  | SchematicGraphicPolylinePlotRecord
  | SchematicGraphicArcPlotRecord
  | SchematicGraphicCirclePlotRecord
  | SchematicGraphicRectanglePlotRecord
  | SchematicGraphicBezierPlotRecord
  | SchematicRuleAreaPlotRecord
  | SchematicImagePlotRecord
  | SchematicTablePlotRecord
  | SchematicSymbolInstancePlotRecord
  | SchematicSymbolOverplotPlotRecord
  | SchematicSheetPlotRecord;
/**
 * Signal-direction shapes preserved on global and hierarchical labels.
 */
export type SchematicLabelShape =
  "input" | "output" | "bidirectional" | "tri_state" | "passive" | "dot" | "round" | "diamond" | "rectangle";
/**
 * Marker shapes admitted by schematic netclass/directive flags.
 */
export type SchematicNetclassFlagShape = "round" | "dot" | "diamond" | "rectangle";
/**
 * Source-shape discriminator retained by a schematic rule area.
 */
export type SchematicRuleAreaShape = "polyline" | "rectangle" | "arc" | "circle" | "bezier";
/**
 * Decoded image formats admitted by the schematic image family.
 */
export type SchematicImageFormat = "png" | "jpeg" | "bmp";
/**
 * Strict operation vocabulary for placed symbols and their overplots.
 */
export type SchematicSymbolOperation =
  | ThickSegmentOperation
  | ArcThreePointOperation
  | CircleOperation
  | RectOperation
  | PlotPolyOperation
  | BezierCurveOperation
  | TextOperation
  | PlotImageOperation
  | FlashPadCircleOperation
  | FlashPadOvalOperation
  | FlashPadRectOperation
  | FlashPadRoundRectOperation
  | FlashPadCustomOperation
  | FlashPadTrapezOperation
  | SchematicSymbolStartBlockOperation
  | SchematicSymbolEndBlockOperation;
/**
 * Strict operation vocabulary for hierarchical sheets and their pins.
 */
export type SchematicSheetOperation =
  | ThickSegmentOperation
  | RectOperation
  | PlotPolyOperation
  | TextOperation
  | SchematicSheetStartBlockOperation
  | SchematicSheetEndBlockOperation;
/**
 * Positive viewport dimension that remains exact in JavaScript.
 */
export type NativeSvgPositiveSafeInteger = number;
/**
 * Canonical decimal wire encoding for an unsigned 64-bit byte count.
 */
export type CanonicalUint64Decimal = string;

/**
 * Strict request for deterministic, presentation-neutral base SVG.
 */
export interface NativeSVGRenderRequestA0 {
  type: "kicad_monkey.native.svg.request";
  version: "a0";
  profile: "plotter-base-a0";
  document: NativeSvgPlotDocument;
  viewport: NativeSvgViewport;
  limits: NativeSvgRenderLimits;
}
export interface NativeFootprintSvgDocument {
  kind: "footprint";
  value: FootprintPlotDocumentA0;
}
/**
 * Strict standalone-footprint subset of kicad.plotter_ir.a0. Producers and
 * consumers must run generated semantic validation after structural decoding.
 */
export interface FootprintPlotDocumentA0 {
  schema: "kicad.plotter_ir.a0";
  source_kind: "MOD";
  total_operations: number;
  records: FootprintPlotRecord[];
  source_path?: string;
  document_id: string;
  coordinate_space: PlotterCoordinateSpace;
  version: JavaScriptSafeInteger;
  generator: string;
  generator_version: string;
}
/**
 * One standalone footprint record in canonical text/geometry/pad order.
 */
export interface FootprintPlotRecord {
  uuid: string;
  kind: "footprint";
  object_id: string;
  operation_count: number;
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
 * Solid or decomposed segment shared by PCB, footprint, and drill producers.
 * Graphic state requires only layer. Drill state requires role plus layers;
 * NPTH drill state additionally requires all mask and pad-size hints. The
 * generated semantic validator enforces these mutually exclusive states.
 */
export interface ThickSegmentOperation {
  kind: "ThickSegment";
  index: number;
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
  stroke_color?: string;
}
/**
 * Solid three-point arc.
 */
export interface ArcThreePointOperation {
  kind: "ArcThreePoint";
  index: number;
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
export interface CircleOperation {
  kind: "Circle";
  index: number;
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
/**
 * Rectangle with square corners.
 */
export interface RectOperation {
  kind: "Rect";
  index: number;
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
/**
 * Filled or outlined polygon operation.
 */
export interface PlotPolyOperation {
  kind: "PlotPoly";
  index: number;
  points: PlotterPoint[];
  fill: PlotterFill;
  width_nm: JavaScriptSafeInteger;
  layer?: string;
  stroke_color?: string;
  fill_color?: string;
  line_style?: PlotterLineStyle;
}
/**
 * Cubic Bézier shared by symbol and schematic producers.
 */
export interface BezierCurveOperation {
  kind: "BezierCurve";
  index: number;
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
 * Stroke or cached text operation. Boolean marker keys (`mirror`,
 * `text_as_polygons`, `polyline_per_segment`, `knockout`) are present-only
 * -when-true, matching the established Python emitter. Render-cache keys
 * appear together when an authored cache resolves; `render_cache_polygons`
 * carries the exterior rings in nanometres.
 */
export interface TextOperation {
  kind: "Text";
  index: number;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  text: string;
  color: string;
  orient_deg: number;
  size_x_nm: JavaScriptSafeInteger;
  size_y_nm: JavaScriptSafeInteger;
  h_align: PlotterTextHAlign;
  v_align: PlotterTextVAlign;
  pen_width_nm: JavaScriptSafeInteger;
  italic: boolean;
  bold: boolean;
  multiline: boolean;
  font_face: string;
  context?: PlotterOperationContext;
  layer?: string;
  mirror?: boolean;
  text_as_polygons?: boolean;
  polyline_per_segment?: boolean;
  knockout?: boolean;
  render_cache_polygons?: PlotterPoint[][];
  render_cache?: TextRenderCache;
  render_cache_source?: PlotterTextRenderCacheSource;
  render_cache_exact?: boolean;
}
/**
 * Strict operation-local context emitted by current plotter producers.
 */
export interface PlotterOperationContext {
  hyperlink: PlotterHyperlink;
}
/**
 * One exact hyperlink attached to an authored plotter text carrier.
 */
export interface PlotterHyperlink {
  href: string;
}
/**
 * Typed render cache from an authored `(render_cache ...)` form, the Python
 * resolver, or the deterministic native hinted outline engine. `knockout` appears when the
 * knockout background restructure replaced the polygons.
 */
export interface TextRenderCache {
  schema: "kicad.render_cache.v1";
  unit: "nm";
  coordinate_space: PlotterTextRenderCacheCoordinateSpace;
  text: string;
  angle: number;
  source: PlotterTextRenderCacheSource;
  exact: boolean;
  polygons: TextRenderCachePolygon[];
  knockout?: boolean;
}
/**
 * One render-cache polygon as ordered contours, exterior ring first.
 */
export interface TextRenderCachePolygon {
  contours: PlotterPoint[][];
}
/**
 * Decoded image placement shared by worksheet and schematic producers.
 */
export interface PlotImageOperation {
  kind: "PlotImage";
  index: number;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  width_nm: JavaScriptSafeInteger;
  height_nm: JavaScriptSafeInteger;
  scale: number;
  image_data_b64: string;
  image_format: string;
  stroke_color?: string;
}
/**
 * Circular pad flash shared by footprint and PCB producers. Footprint pad
 * state requires mask_margin_nm and forbids role. Board via state requires
 * role and forbids mask_margin_nm. The generated semantic validator enforces
 * these mutually exclusive states.
 */
export interface FlashPadCircleOperation {
  kind: "FlashPadCircle";
  index: number;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  diameter_nm: JavaScriptSafeInteger;
  layers: string[];
  mask_margin_nm?: JavaScriptSafeInteger;
  role?: PlotterViaFlashRole;
}
/**
 * Oval pad flash shared by footprint and PCB producers.
 */
export interface FlashPadOvalOperation {
  kind: "FlashPadOval";
  index: number;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  size_x_nm: JavaScriptSafeInteger;
  size_y_nm: JavaScriptSafeInteger;
  orient_deg: number;
  layers: string[];
  mask_margin_nm: JavaScriptSafeInteger;
}
/**
 * Rectangular pad flash shared by footprint and PCB producers.
 */
export interface FlashPadRectOperation {
  kind: "FlashPadRect";
  index: number;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  size_x_nm: JavaScriptSafeInteger;
  size_y_nm: JavaScriptSafeInteger;
  orient_deg: number;
  layers: string[];
  mask_margin_nm: JavaScriptSafeInteger;
}
/**
 * Rounded-rectangle pad flash shared by footprint and PCB producers.
 */
export interface FlashPadRoundRectOperation {
  kind: "FlashPadRoundRect";
  index: number;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  size_x_nm: JavaScriptSafeInteger;
  size_y_nm: JavaScriptSafeInteger;
  corner_radius_nm: JavaScriptSafeInteger;
  orient_deg: number;
  layers: string[];
  mask_margin_nm: JavaScriptSafeInteger;
}
/**
 * Custom pad flash shared by footprint and PCB producers. Polygon coordinates
 * are pad-local. A non-empty polygon_widths_nm has one entry per polygon;
 * generated semantic validation enforces that relationship. An empty array is
 * equivalent to omission for generated Rust transport bindings.
 */
export interface FlashPadCustomOperation {
  kind: "FlashPadCustom";
  index: number;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  size_x_nm: JavaScriptSafeInteger;
  size_y_nm: JavaScriptSafeInteger;
  orient_deg: number;
  polygons: PlotterPoint[][];
  polygon_widths_nm?: JavaScriptSafeInteger[];
  anchor_shape?: string;
  layers: string[];
  mask_margin_nm: JavaScriptSafeInteger;
}
/**
 * Trapezoid pad flash shared by footprint and PCB producers.
 */
export interface FlashPadTrapezOperation {
  kind: "FlashPadTrapez";
  index: number;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  corners: PlotterQuad;
  orient_deg: number;
  layers: string[];
  mask_margin_nm: JavaScriptSafeInteger;
}
/**
 * Coordinate convention for the footprint plotter slice.
 */
export interface PlotterCoordinateSpace {
  unit: "nm";
  y_axis: "down";
}
export interface NativeSymbolSvgDocument {
  kind: "symbol";
  value: SymbolPlotDocumentA0;
}
/**
 * Strict cache-free library-symbol geometry and text subset of kicad.plotter_ir.a0.
 */
export interface SymbolPlotDocumentA0 {
  schema: "kicad.plotter_ir.a0";
  source_kind: "SYM";
  total_operations: number;
  records: SymbolPlotRecord[];
  source_path?: string;
  document_id: string;
  coordinate_space: PlotterCoordinateSpace;
}
/**
 * Leading metadata record for the selected library symbol.
 */
export interface SymbolHeaderPlotRecord {
  uuid: "";
  kind: "lib_symbol";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  name: string;
  extends?: string;
  unit?: number;
  style: number;
  in_bom: boolean;
  on_board: boolean;
  power: boolean;
}
/**
 * One selected graphical subsymbol record.
 */
export interface LibSubsymbolPlotRecord {
  uuid: "";
  kind: "lib_subsymbol";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  unit: number;
  style: number;
}
export interface NativeBoardSvgDocument {
  kind: "board";
  value: BoardPlotDocumentA0;
}
/**
 * Strict board graphics, text, tracks, vias, tables, dimensions, authored
 * zone fills, and embedded footprints subset of kicad.plotter_ir.a0. Producers
 * and consumers must run generated semantic validation after structural decoding.
 */
export interface BoardPlotDocumentA0 {
  schema: "kicad.plotter_ir.a0";
  source_kind: "PCB";
  total_operations: number;
  records: BoardPlotRecord[];
  source_path?: string;
  document_id: string;
  coordinate_space: PlotterCoordinateSpace;
  version: JavaScriptSafeInteger;
  generator: string;
  generator_version: string;
  thickness_mm: number;
  paper: string;
}
/**
 * One board-level graphic record. The carrier layer travels on the record;
 * the contained operations are layerless graphic-state operations.
 */
export interface BoardGraphicPlotRecord {
  uuid: string;
  kind: BoardGraphicRecordKind;
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  layer: string | null;
}
/**
 * One board track segment record with its net attribution.
 */
export interface TrackSegmentPlotRecord {
  uuid: string;
  kind: "segment";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  layer: string;
  locked: boolean;
  net_id?: JavaScriptSafeInteger;
  net_name?: string;
  net_class?: string;
  net_classes?: string[];
}
/**
 * One board track arc record with its net attribution.
 */
export interface TrackArcPlotRecord {
  uuid: string;
  kind: "track_arc";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  layer: string;
  net_id?: JavaScriptSafeInteger;
  net_name?: string;
  net_class?: string;
  net_classes?: string[];
}
/**
 * One board via record: copper aperture, synthetic drill, and per-side mask
 * opening/drill pairs when tenting explicitly exposes that side. IPC-4761
 * fabrication metadata mirrors the established stringified booleans.
 */
export interface ViaPlotRecord {
  uuid: string;
  kind: "via";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  layers: string[];
  drill: number;
  size: number;
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
/**
 * Board table grid/border segments followed by optional faced cell text.
 */
export interface TablePlotRecord {
  uuid: string;
  kind: "table";
  object_id: "table";
  operation_count: number;
  operations: PlotterOperation[];
  layers: string[];
  cell_count: number;
}
/**
 * Dimension text (when present) followed by layered construction geometry.
 */
export interface DimensionPlotRecord {
  uuid: string;
  kind: "dimension";
  object_id: "dimension";
  operation_count: number;
  operations: PlotterOperation[];
  layers: string[];
  text?: string;
  dimension_type: BoardDimensionType;
}
/**
 * One zone fill record bundling every `filled_polygon` ring. The parallel
 * `fill_layers`/`fill_island` arrays annotate the rings so consumers can
 * split or colour-key without re-walking the source zone.
 */
export interface ZoneFillPlotRecord {
  uuid: string;
  kind: "zone_fill";
  object_id: string;
  operation_count: number;
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
export interface BoardTextPlotRecord {
  uuid: string;
  kind: "gr_text";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  layer: string;
  text: string;
  hide: boolean;
}
/**
 * One board text-box record. A visible border contributes a leading Rect
 * operation; empty resolved text drops the Text operation.
 */
export interface BoardTextBoxPlotRecord {
  uuid: string;
  kind: "gr_text_box";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  layer: string;
  text: string;
  border: boolean;
}
/**
 * One board-embedded footprint in canonical child and pad-block order.
 */
export interface BoardFootprintPlotRecord {
  uuid: string;
  kind: "footprint";
  object_id: string;
  operation_count: number;
  operations: BoardFootprintOperation[];
  library_link: string;
  reference: string;
  value: string;
  layer: string;
  locked: boolean;
  descr: string;
  tags: string;
  attr: string[];
  placement: BoardFootprintPlacement;
}
export interface BoardFootprintThickSegmentOperation {
  kind: "ThickSegment";
  index: number;
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
  stroke_color?: string;
  label?: string;
  data_uuid?: string;
  data_ref?: BoardFootprintChildRef;
  object_id?: string;
  extra_attrs?: BoardFootprintChildAttrs;
}
/**
 * SVG-enrichment metadata retained on one embedded-footprint child operation.
 */
export interface BoardFootprintChildAttrs {
  component: string;
  component_uid: string;
  component_uuid: string;
  footprint: string;
  layer_name?: string;
  layer_role?: BoardFootprintLayerRole;
  primitive: "footprint-text" | "footprint-graphic";
  footprint_primitive: BoardFootprintChildRef;
  footprint_object_index: number;
  footprint_subop_index?: number;
  footprint_text_role?: "designator" | "value" | "property" | "user";
  property_name?: string;
  fp_text_type?: string;
  footprint_graphic_kind?: "text-box-border" | "line" | "arc" | "circle" | "rect" | "poly";
}
export interface BoardFootprintArcThreePointOperation {
  kind: "ArcThreePoint";
  index: number;
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
  label?: string;
  data_uuid?: string;
  data_ref?: BoardFootprintChildRef;
  object_id?: string;
  extra_attrs?: BoardFootprintChildAttrs;
}
export interface BoardFootprintCircleOperation {
  kind: "Circle";
  index: number;
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
  label?: string;
  data_uuid?: string;
  data_ref?: BoardFootprintChildRef;
  object_id?: string;
  extra_attrs?: BoardFootprintChildAttrs;
}
export interface BoardFootprintRectOperation {
  kind: "Rect";
  index: number;
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
  label?: string;
  data_uuid?: string;
  data_ref?: BoardFootprintChildRef;
  object_id?: string;
  extra_attrs?: BoardFootprintChildAttrs;
}
export interface BoardFootprintPlotPolyOperation {
  kind: "PlotPoly";
  index: number;
  points: PlotterPoint[];
  fill: PlotterFill;
  width_nm: JavaScriptSafeInteger;
  layer?: string;
  stroke_color?: string;
  fill_color?: string;
  line_style?: PlotterLineStyle;
  label?: string;
  data_uuid?: string;
  data_ref?: BoardFootprintChildRef;
  object_id?: string;
  extra_attrs?: BoardFootprintChildAttrs;
}
export interface BoardFootprintBezierCurveOperation {
  kind: "BezierCurve";
  index: number;
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
  label?: string;
  data_uuid?: string;
  data_ref?: BoardFootprintChildRef;
  object_id?: string;
  extra_attrs?: BoardFootprintChildAttrs;
}
export interface BoardFootprintTextOperation {
  kind: "Text";
  index: number;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  text: string;
  color: string;
  orient_deg: number;
  size_x_nm: JavaScriptSafeInteger;
  size_y_nm: JavaScriptSafeInteger;
  h_align: PlotterTextHAlign;
  v_align: PlotterTextVAlign;
  pen_width_nm: JavaScriptSafeInteger;
  italic: boolean;
  bold: boolean;
  multiline: boolean;
  font_face: string;
  context?: PlotterOperationContext;
  layer?: string;
  mirror?: boolean;
  text_as_polygons?: boolean;
  polyline_per_segment?: boolean;
  knockout?: boolean;
  render_cache_polygons?: PlotterPoint[][];
  render_cache?: TextRenderCache;
  render_cache_source?: PlotterTextRenderCacheSource;
  render_cache_exact?: boolean;
  label?: string;
  data_uuid?: string;
  data_ref?: BoardFootprintChildRef;
  object_id?: string;
  extra_attrs?: BoardFootprintChildAttrs;
}
export interface BoardFootprintFlashPadCircleOperation {
  kind: "FlashPadCircle";
  index: number;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  diameter_nm: JavaScriptSafeInteger;
  layers: string[];
  mask_margin_nm?: JavaScriptSafeInteger;
  role?: PlotterViaFlashRole;
  label?: string;
  data_uuid?: string;
  data_ref?: BoardFootprintChildRef;
  object_id?: string;
  extra_attrs?: BoardFootprintChildAttrs;
}
export interface BoardFootprintFlashPadOvalOperation {
  kind: "FlashPadOval";
  index: number;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  size_x_nm: JavaScriptSafeInteger;
  size_y_nm: JavaScriptSafeInteger;
  orient_deg: number;
  layers: string[];
  mask_margin_nm: JavaScriptSafeInteger;
  label?: string;
  data_uuid?: string;
  data_ref?: BoardFootprintChildRef;
  object_id?: string;
  extra_attrs?: BoardFootprintChildAttrs;
}
export interface BoardFootprintFlashPadRectOperation {
  kind: "FlashPadRect";
  index: number;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  size_x_nm: JavaScriptSafeInteger;
  size_y_nm: JavaScriptSafeInteger;
  orient_deg: number;
  layers: string[];
  mask_margin_nm: JavaScriptSafeInteger;
  label?: string;
  data_uuid?: string;
  data_ref?: BoardFootprintChildRef;
  object_id?: string;
  extra_attrs?: BoardFootprintChildAttrs;
}
export interface BoardFootprintFlashPadRoundRectOperation {
  kind: "FlashPadRoundRect";
  index: number;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  size_x_nm: JavaScriptSafeInteger;
  size_y_nm: JavaScriptSafeInteger;
  corner_radius_nm: JavaScriptSafeInteger;
  orient_deg: number;
  layers: string[];
  mask_margin_nm: JavaScriptSafeInteger;
  label?: string;
  data_uuid?: string;
  data_ref?: BoardFootprintChildRef;
  object_id?: string;
  extra_attrs?: BoardFootprintChildAttrs;
}
export interface BoardFootprintFlashPadCustomOperation {
  kind: "FlashPadCustom";
  index: number;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  size_x_nm: JavaScriptSafeInteger;
  size_y_nm: JavaScriptSafeInteger;
  orient_deg: number;
  polygons: PlotterPoint[][];
  polygon_widths_nm?: JavaScriptSafeInteger[];
  anchor_shape?: string;
  layers: string[];
  mask_margin_nm: JavaScriptSafeInteger;
  label?: string;
  data_uuid?: string;
  data_ref?: BoardFootprintChildRef;
  object_id?: string;
  extra_attrs?: BoardFootprintChildAttrs;
}
export interface BoardFootprintFlashPadTrapezOperation {
  kind: "FlashPadTrapez";
  index: number;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  corners: PlotterQuad;
  orient_deg: number;
  layers: string[];
  mask_margin_nm: JavaScriptSafeInteger;
  label?: string;
  data_uuid?: string;
  data_ref?: BoardFootprintChildRef;
  object_id?: string;
  extra_attrs?: BoardFootprintChildAttrs;
}
/**
 * Opening operation for one embedded pad or drill SVG group.
 */
export interface BoardFootprintStartBlockOperation {
  kind: "StartBlock";
  index: number;
  label: string;
  data_uuid: string;
  data_ref: "pad" | "pad_hole";
  object_id: string;
  extra_attrs: BoardFootprintPadBlockAttrs;
  layers?: string[];
}
/**
 * Stringified SVG-enrichment attributes on an embedded pad block.
 */
export interface BoardFootprintPadBlockAttrs {
  primitive: "pad" | "pad-hole";
  component?: string;
  component_uid?: string;
  component_uuid?: string;
  footprint?: string;
  pad_number?: string;
  pad_designator?: string;
  pad_type?: string;
  pad_shape?: string;
  layer_names?: string;
  net_index?: string;
  net_id?: string;
  net?: string;
  net_class?: string;
  net_classes?: string;
  hole_owner?: string;
  hole_kind?: "round" | "slot";
  hole_plating?: "plated" | "non_plated";
  hole_render?: "drill";
  hole_width_mm?: string;
  hole_height_mm?: string;
  hole_diameter_mm?: string;
}
/**
 * Closing operation for one embedded pad or drill SVG group.
 */
export interface BoardFootprintEndBlockOperation {
  kind: "EndBlock";
  index: number;
}
/**
 * Footprint-local placement applied by board renderers.
 */
export interface BoardFootprintPlacement {
  x_nm: JavaScriptSafeInteger;
  y_nm: JavaScriptSafeInteger;
  angle_deg: number;
}
export interface NativeSchematicSvgDocument {
  kind: "schematic";
  value: SchematicPlotDocumentA0;
}
/**
 * Strict schematic subset through the P5_071 hierarchical-sheet family.
 */
export interface SchematicPlotDocumentA0 {
  schema: "kicad.plotter_ir.a0";
  source_kind: "SCH";
  total_operations: number;
  records: SchematicPlotRecord[];
  source_path?: string;
  document_id: string;
  canvas: SchematicPlotCanvas;
  coordinate_space: PlotterCoordinateSpace;
}
/**
 * Leading paper, title-block, background, and worksheet record.
 */
export interface SchematicSheetHeaderPlotRecord {
  uuid: string;
  kind: "sheet_header";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  paper_size: string;
  paper_width_mm: number | null;
  paper_height_mm: number | null;
  paper_portrait: boolean;
  sheet_width_nm: JavaScriptSafeInteger;
  sheet_height_nm: JavaScriptSafeInteger;
  version: JavaScriptSafeInteger;
  generator: string;
  generator_version: string;
  title_block?: SchematicPlotTitleBlock;
}
/**
 * Typed title-block metadata carried by the leading sheet-header record.
 */
export interface SchematicPlotTitleBlock {
  title: string;
  date: string;
  rev: string;
  company: string;
  comments: RecordString;
}
export interface RecordString {
  [k: string]: string;
}
/**
 * One schematic wire polyline.
 */
export interface SchematicWirePlotRecord {
  uuid: string;
  kind: "wire";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
}
/**
 * One schematic bus polyline.
 */
export interface SchematicBusPlotRecord {
  uuid: string;
  kind: "bus";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
}
/**
 * One schematic bus-entry segment.
 */
export interface SchematicBusEntryPlotRecord {
  uuid: string;
  kind: "bus_entry";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
}
/**
 * One schematic junction marker.
 */
export interface SchematicJunctionPlotRecord {
  uuid: string;
  kind: "junction";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  /**
   * Authored junction color; null preserves an authored transparent color.
   */
  color?: string | null;
}
/**
 * One schematic no-connect cross.
 */
export interface SchematicNoConnectPlotRecord {
  uuid: string;
  kind: "no_connect";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
}
/**
 * One local schematic label.
 */
export interface SchematicLabelPlotRecord {
  uuid: string;
  kind: "label";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  text: string;
}
/**
 * One global schematic label and its optional decoration.
 */
export interface SchematicGlobalLabelPlotRecord {
  uuid: string;
  kind: "global_label";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  text: string;
  shape: SchematicLabelShape;
}
/**
 * One hierarchical schematic label and its optional decoration.
 */
export interface SchematicHierarchicalLabelPlotRecord {
  uuid: string;
  kind: "hierarchical_label";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  text: string;
  shape: SchematicLabelShape;
}
/**
 * One netclass/directive flag with its visible property text.
 */
export interface SchematicNetclassFlagPlotRecord {
  uuid: string;
  kind: "netclass_flag";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  at_x_nm: JavaScriptSafeInteger;
  at_y_nm: JavaScriptSafeInteger;
  shape: SchematicNetclassFlagShape;
  length_nm: JavaScriptSafeInteger;
}
/**
 * One ordinary top-level schematic text annotation.
 */
export interface SchematicTextPlotRecord {
  uuid: string;
  kind: "text";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  text: string;
}
/**
 * One schematic text box with its canonical outline and plotted lines.
 */
export interface SchematicTextBoxPlotRecord {
  uuid: string;
  kind: "text_box";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  text: string;
}
/**
 * One top-level schematic graphic polyline.
 */
export interface SchematicGraphicPolylinePlotRecord {
  uuid: string;
  kind: "graphic_polyline";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
}
/**
 * One top-level schematic graphic arc.
 */
export interface SchematicGraphicArcPlotRecord {
  uuid: string;
  kind: "graphic_arc";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
}
/**
 * One top-level schematic graphic circle.
 */
export interface SchematicGraphicCirclePlotRecord {
  uuid: string;
  kind: "graphic_circle";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
}
/**
 * One top-level schematic graphic rectangle.
 */
export interface SchematicGraphicRectanglePlotRecord {
  uuid: string;
  kind: "graphic_rectangle";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
}
/**
 * One canonical four-point top-level schematic graphic Bezier.
 */
export interface SchematicGraphicBezierPlotRecord {
  uuid: string;
  kind: "graphic_bezier";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
}
/**
 * One schematic rule area with source policy metadata.
 */
export interface SchematicRuleAreaPlotRecord {
  uuid: string;
  kind: "rule_area";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  shape: SchematicRuleAreaShape;
  locked: boolean;
  exclude_from_sim: boolean;
  in_bom: boolean;
  on_board: boolean;
  dnp: boolean;
}
/**
 * One decoded embedded schematic image.
 */
export interface SchematicImagePlotRecord {
  uuid: string;
  kind: "image";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  scale: number;
  image_format: SchematicImageFormat;
  width_nm: JavaScriptSafeInteger;
  height_nm: JavaScriptSafeInteger;
}
/**
 * One schematic table rendered as ordered text-box cell blocks.
 */
export interface SchematicTablePlotRecord {
  uuid: string;
  kind: "table";
  object_id: string;
  operation_count: number;
  operations: PlotterOperation[];
  cell_count: number;
}
/**
 * One placed schematic symbol, including body, pins, and visible fields.
 */
export interface SchematicSymbolInstancePlotRecord {
  uuid: string;
  kind: "symbol_instance";
  object_id: string;
  operation_count: number;
  operations: SchematicSymbolOperation[];
  lib_id: string;
  lib_name: string;
  reference: string;
  at_x_nm: JavaScriptSafeInteger;
  at_y_nm: JavaScriptSafeInteger;
  at_angle_deg: number;
  mirror: string | null;
  unit: number;
  convert: number;
  in_bom: boolean;
  on_board: boolean;
  dnp: boolean;
  exclude_from_sim: boolean;
  in_pos_files: boolean;
}
/**
 * Leading ownership marker for one placed-symbol pin group.
 */
export interface SchematicSymbolStartBlockOperation {
  kind: "StartBlock";
  index: number;
  label: string;
  data_uuid: string;
  data_ref: "symbol_pin";
  object_id: string;
  extra_attrs: SchematicSymbolPinBlockAttrs;
}
/**
 * Exact string metadata retained on one placed-symbol pin group.
 */
export interface SchematicSymbolPinBlockAttrs {
  [k: string]: string;
}
/**
 * Trailing ownership marker for one placed-symbol pin group.
 */
export interface SchematicSymbolEndBlockOperation {
  kind: "EndBlock";
  index: number;
}
/**
 * Replotted pins and fields for one overlapping placed symbol.
 */
export interface SchematicSymbolOverplotPlotRecord {
  uuid: string;
  kind: "symbol_overplot";
  object_id: string;
  operation_count: number;
  operations: SchematicSymbolOperation[];
  source_symbol_uuid: string;
  lib_id: string;
}
/**
 * One hierarchical sheet with its box, pins, visible fields, and DNP state.
 */
export interface SchematicSheetPlotRecord {
  uuid: string;
  kind: "sheet";
  object_id: string;
  operation_count: number;
  operations: SchematicSheetOperation[];
  sheet_name: string;
  sheet_file: string;
  at_x_nm: JavaScriptSafeInteger;
  at_y_nm: JavaScriptSafeInteger;
  size_x_nm: JavaScriptSafeInteger;
  size_y_nm: JavaScriptSafeInteger;
  dnp: boolean;
}
/**
 * Leading ownership marker for one hierarchical-sheet pin group.
 */
export interface SchematicSheetStartBlockOperation {
  kind: "StartBlock";
  index: number;
  label: string;
  data_uuid: string;
  data_ref: "sheet_pin";
  object_id: string;
  extra_attrs: SchematicSheetPinBlockAttrs;
}
/**
 * Exact string metadata retained on one hierarchical-sheet pin group.
 */
export interface SchematicSheetPinBlockAttrs {
  [k: string]: string;
}
/**
 * Trailing ownership marker for one hierarchical-sheet pin group.
 */
export interface SchematicSheetEndBlockOperation {
  kind: "EndBlock";
  index: number;
}
/**
 * Exact page extent of one schematic instance.
 */
export interface SchematicPlotCanvas {
  width_nm: JavaScriptSafeInteger;
  height_nm: JavaScriptSafeInteger;
}
/**
 * Explicit document viewport. Frozen MOD/SYM/PCB plot documents do not carry one.
 */
export interface NativeSvgViewport {
  min_x_nm: JavaScriptSafeInteger;
  min_y_nm: JavaScriptSafeInteger;
  width_nm: NativeSvgPositiveSafeInteger;
  height_nm: NativeSvgPositiveSafeInteger;
}
/**
 * Aggregate resource ceilings for one native SVG serialization.
 */
export interface NativeSvgRenderLimits {
  max_records: number;
  max_operations: number;
  max_points: CanonicalUint64Decimal;
  max_text_bytes: CanonicalUint64Decimal;
  max_image_encoded_bytes: CanonicalUint64Decimal;
  max_block_depth: number;
  max_svg_elements: CanonicalUint64Decimal;
  max_render_work: CanonicalUint64Decimal;
  max_svg_bytes: CanonicalUint64Decimal;
  max_result_bytes: CanonicalUint64Decimal;
}
