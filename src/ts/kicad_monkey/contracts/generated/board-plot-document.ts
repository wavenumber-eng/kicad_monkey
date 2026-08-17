/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

export type BoardPlotRecord =
  | BoardGraphicPlotRecord
  | TrackSegmentPlotRecord
  | TrackArcPlotRecord
  | ViaPlotRecord
  | TablePlotRecord
  | ZoneFillPlotRecord
  | BoardTextPlotRecord
  | BoardTextBoxPlotRecord;
/**
 * Board graphic record kinds promoted in the first board slice.
 */
export type BoardGraphicRecordKind = "gr_line" | "gr_arc" | "gr_circle" | "gr_rect" | "gr_poly" | "gr_curve";
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
/**
 * Via construction kinds mirrored from the established producer.
 */
export type BoardViaType = "through" | "blind" | "buried" | "micro";
/**
 * Stringified boolean metadata mirrored from the established producer.
 */
export type PlotterStringBool = "true" | "false";

/**
 * Strict board graphics, text, tracks, vias, and authored-zone-fill subset of
 * kicad.plotter_ir.a0. Producers and consumers must run generated semantic
 * validation after structural decoding.
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
/**
 * Typed authored render cache mirrored from `(render_cache ...)` forms. The
 * promoted producers only forward file caches, so `source` is pinned to
 * `existing_file_cache`; `knockout` appears when the knockout background
 * restructure replaced the polygons.
 */
export interface TextRenderCache {
  schema: "kicad.render_cache.v1";
  unit: "nm";
  coordinate_space: "board";
  text: string;
  angle: number;
  source: "existing_file_cache";
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
 * Coordinate convention for the footprint plotter slice.
 */
export interface PlotterCoordinateSpace {
  unit: "nm";
  y_axis: "down";
}
