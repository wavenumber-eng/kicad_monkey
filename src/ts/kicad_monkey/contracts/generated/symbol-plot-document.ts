/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

export type SymbolPlotRecord = SymbolHeaderPlotRecord | LibSubsymbolPlotRecord;
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
export type PlotterDrillRole = "pad_drill" | "npth_hole";
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
 * Four pad-local trapezoid corners.
 *
 * @minItems 4
 * @maxItems 4
 */
export type PlotterQuad = [PlotterPoint, PlotterPoint, PlotterPoint, PlotterPoint];

/**
 * Strict non-text library-symbol subset of kicad.plotter_ir.a0.
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
 * Circular pad flash shared by footprint and PCB producers.
 */
export interface FlashPadCircleOperation {
  kind: "FlashPadCircle";
  index: number;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  diameter_nm: JavaScriptSafeInteger;
  layers: string[];
  mask_margin_nm: JavaScriptSafeInteger;
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
/**
 * Coordinate convention for the footprint plotter slice.
 */
export interface PlotterCoordinateSpace {
  unit: "nm";
  y_axis: "down";
}
