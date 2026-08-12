/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Non-text footprint graphics promoted by the second plotter slice.
 */
export type FootprintGraphicOperation =
  ThickSegmentOperation | ArcThreePointOperation | CircleOperation | RectOperation | PlotPolyOperation;
/**
 * Integer that remains exact when decoded as a JavaScript number.
 */
export type JavaScriptSafeInteger = number;
/**
 * Fill values emitted by promoted footprint graphics.
 */
export type PlotterFill = "NO_FILL" | "FILLED_SHAPE";
/**
 * Footprint polygon point stream.
 *
 * @minItems 2
 * @maxItems 2
 */
export type PlotterPoint = [JavaScriptSafeInteger, JavaScriptSafeInteger];

/**
 * Strict non-text footprint subset of kicad.plotter_ir.a0.
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
 * One footprint record in the promoted non-text graphics slice.
 */
export interface FootprintPlotRecord {
  uuid: string;
  kind: "footprint";
  object_id: string;
  operation_count: number;
  operations: FootprintGraphicOperation[];
  name: string;
  layer: string;
  locked: boolean;
  placed: boolean;
  descr: string;
  tags: string;
  attr: string[];
}
/**
 * Solid or decomposed footprint stroke segment.
 */
export interface ThickSegmentOperation {
  kind: "ThickSegment";
  index: number;
  start_x: JavaScriptSafeInteger;
  start_y: JavaScriptSafeInteger;
  end_x: JavaScriptSafeInteger;
  end_y: JavaScriptSafeInteger;
  width_nm: JavaScriptSafeInteger;
  layer: string;
}
/**
 * Solid three-point footprint arc.
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
  layer: string;
}
/**
 * Footprint circle represented by center and diameter.
 */
export interface CircleOperation {
  kind: "Circle";
  index: number;
  cx: JavaScriptSafeInteger;
  cy: JavaScriptSafeInteger;
  diameter_nm: JavaScriptSafeInteger;
  fill: PlotterFill;
  width_nm: JavaScriptSafeInteger;
  layer: string;
}
/**
 * Footprint rectangle with square corners.
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
  layer: string;
}
/**
 * Footprint polygon operation.
 */
export interface PlotPolyOperation {
  kind: "PlotPoly";
  index: number;
  points: PlotterPoint[];
  fill: PlotterFill;
  width_nm: JavaScriptSafeInteger;
  layer: string;
}
/**
 * Coordinate convention for the footprint plotter slice.
 */
export interface PlotterCoordinateSpace {
  unit: "nm";
  y_axis: "down";
}
