import "@typespec/json-schema";
import "./sexpr-a0.tsp";

using TypeSpec.JsonSchema;
using Wavenumber.KicadMonkey.Contracts.SexprA0;

namespace Wavenumber.KicadMonkey.Contracts.PlotterIrA0;

/** Integer that remains exact when decoded as a JavaScript number. */
@minValue(-9007199254740991)
@maxValue(9007199254740991)
scalar JavaScriptSafeInteger extends safeint;

/** Resource-bounded first footprint plotter operation. Source bytes are out of band. */
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

/** Coordinate convention for the initial footprint plotter slice. */
model PlotterCoordinateSpace {
  unit: "nm";
  y_axis: "down";
}

/** Solid footprint line operation supported by the first typed plotter slice. */
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

/** One footprint record in the first typed plotter slice. */
model FootprintPlotRecord {
  uuid: string;
  kind: "footprint";
  object_id: string;
  operation_count: uint32;
  operations: ThickSegmentOperation[];
  name: string;
  layer: string;
  locked: boolean;
  placed: boolean;
  descr: string;
  tags: string;
  attr: string[];
}

/** Strict subset of kicad.plotter_ir.a0 emitted by the initial footprint slice. */
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
