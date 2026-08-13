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

/** Fill values shared by plotter operation producers. */
enum PlotterFill {
  noFill: "NO_FILL",
  filledShape: "FILLED_SHAPE",
}

/** Semantic roles allowed on shared circle and segment drill operations. */
enum PlotterDrillRole {
  padDrill: "pad_drill",
  npthHole: "npth_hole",
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
  layer: string;
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
  layer: string;
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
  layer: string;
}

/** Circular pad flash shared by footprint and PCB producers. */
model FlashPadCircleOperation {
  kind: "FlashPadCircle";
  index: uint32;
  x: JavaScriptSafeInteger;
  y: JavaScriptSafeInteger;
  diameter_nm: JavaScriptSafeInteger;
  layers: string[];
  mask_margin_nm: JavaScriptSafeInteger;
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
