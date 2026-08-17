/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Strict source-record vocabulary through the P5_061 schematic annotations.
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
  | SchematicTextBoxPlotRecord;
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
 * Strict schematic subset through the P5_061 annotation families.
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
 * Exact page extent of one schematic instance.
 */
export interface SchematicPlotCanvas {
  width_nm: JavaScriptSafeInteger;
  height_nm: JavaScriptSafeInteger;
}
/**
 * Coordinate convention for the footprint plotter slice.
 */
export interface PlotterCoordinateSpace {
  unit: "nm";
  y_axis: "down";
}
