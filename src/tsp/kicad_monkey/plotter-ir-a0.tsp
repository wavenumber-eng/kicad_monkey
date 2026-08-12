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

/** Fill values emitted by promoted footprint graphics. */
enum PlotterFill {
  noFill: "NO_FILL",
  filledShape: "FILLED_SHAPE",
}

/** Solid or decomposed footprint stroke segment. */
model ThickSegmentOperation {
  kind: "ThickSegment";
  index: uint32;
  start_x: JavaScriptSafeInteger;
  start_y: JavaScriptSafeInteger;
  end_x: JavaScriptSafeInteger;
  end_y: JavaScriptSafeInteger;
  width_nm: JavaScriptSafeInteger;
  layer: string;
}

/** Solid three-point footprint arc. */
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
  layer: string;
}

/** Footprint circle represented by center and diameter. */
model CircleOperation {
  kind: "Circle";
  index: uint32;
  cx: JavaScriptSafeInteger;
  cy: JavaScriptSafeInteger;
  diameter_nm: JavaScriptSafeInteger;
  fill: PlotterFill;
  width_nm: JavaScriptSafeInteger;
  layer: string;
}

/** Footprint rectangle with square corners. */
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
  layer: string;
}

/** Footprint polygon point stream. */
@minItems(2)
@maxItems(2)
model PlotterPoint is Array<JavaScriptSafeInteger>;

/** Footprint polygon operation. */
model PlotPolyOperation {
  kind: "PlotPoly";
  index: uint32;
  points: PlotterPoint[];
  fill: PlotterFill;
  width_nm: JavaScriptSafeInteger;
  layer: string;
}

/** Non-text footprint graphics promoted by the second plotter slice. */
union FootprintGraphicOperation {
  thickSegment: ThickSegmentOperation,
  arcThreePoint: ArcThreePointOperation,
  circle: CircleOperation,
  rect: RectOperation,
  plotPoly: PlotPolyOperation,
}

/** One footprint record in the promoted non-text graphics slice. */
model FootprintPlotRecord {
  uuid: string;
  kind: "footprint";
  object_id: string;
  operation_count: uint32;
  operations: FootprintGraphicOperation[];
  name: string;
  layer: string;
  locked: boolean;
  placed: boolean;
  descr: string;
  tags: string;
  attr: string[];
}

/** Strict non-text footprint subset of kicad.plotter_ir.a0. */
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
