/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Integer that remains exact when decoded as a JavaScript number.
 */
export type JavaScriptSafeInteger = number;

/**
 * Strict subset of kicad.plotter_ir.a0 emitted by the initial footprint slice.
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
 * One footprint record in the first typed plotter slice.
 */
export interface FootprintPlotRecord {
  uuid: string;
  kind: "footprint";
  object_id: string;
  operation_count: number;
  operations: ThickSegmentOperation[];
  name: string;
  layer: string;
  locked: boolean;
  placed: boolean;
  descr: string;
  tags: string;
  attr: string[];
}
/**
 * Solid footprint line operation supported by the first typed plotter slice.
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
 * Coordinate convention for the initial footprint plotter slice.
 */
export interface PlotterCoordinateSpace {
  unit: "nm";
  y_axis: "down";
}
