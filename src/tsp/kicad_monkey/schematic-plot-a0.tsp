import "@typespec/json-schema";
import "./plotter-ir-a0.tsp";

using TypeSpec.JsonSchema;
using Wavenumber.KicadMonkey.Contracts.SexprA0;

namespace Wavenumber.KicadMonkey.Contracts.PlotterIrA0;

/** Selection of the drawing-sheet byte sidecar supplied out of band. */
enum SchematicWorksheetMode {
  default: "default",
  provided: "provided",
}

/** One exact-case project text variable supplied by the caller. */
model SchematicTextVariable {
  name: string;
  value: string;
}

/** Resource-bounded schematic plot operation. Source bytes are out of band. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.schematic_plot.request:a0")
@extension("title", "Schematic plot request a0")
model SchematicPlotRequest {
  type: "kicad_monkey.schematic_plot.request";
  version: "a0";
  source_path?: string;
  document_id?: string;
  @minValue(1)
  sheet_index: uint32;
  @minValue(1)
  sheet_count: uint32;
  sheet_path: string;
  sheet_name: string;
  worksheet_mode: SchematicWorksheetMode;
  text_variables?: SchematicTextVariable[];
  max_source_bytes: uint64;
  max_worksheet_bytes: uint64;
  max_output_bytes: uint64;
  max_depth: uint32;
  max_parse_nodes: uint32;
  max_selected_forms: uint32;
  max_records: uint32;
  max_operations: uint32;
  max_points: uint32;
  max_input_points: uint32;
  max_text_bytes: uint64;
  max_metadata_bytes: uint64;
  max_wires: uint32;
  max_buses: uint32;
  max_bus_entries: uint32;
  max_junctions: uint32;
  max_no_connects: uint32;
  max_text_variables: uint32;
  max_text_variable_bytes: uint64;
  max_worksheet_items: uint32;
  max_worksheet_repeats: uint32;
  max_worksheet_point_sets: uint32;
  max_worksheet_points: uint32;
  max_worksheet_bitmap_data_parts: uint32;
  max_worksheet_bitmap_encoded_bytes: uint64;
  max_worksheet_bitmap_decoded_bytes: uint64;
  max_worksheet_bitmap_width_px: uint32;
  max_worksheet_bitmap_height_px: uint32;
  max_worksheet_bitmap_pixels: uint64;
  max_worksheet_bitmap_decode_work: uint64;
}

/** Metadata paired with separate schematic plotter-IR JSON bytes. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.schematic_plot.result:a0")
@extension("title", "Schematic plot result a0")
model SchematicPlotResult {
  type: "kicad_monkey.schematic_plot.result";
  version: "a0";
  output_bytes: uint64;
  total_operations: uint32;
  diagnostics: Diagnostic[];
}

/** Exact page extent of one schematic instance. */
model SchematicPlotCanvas {
  width_nm: JavaScriptSafeInteger;
  height_nm: JavaScriptSafeInteger;
}

/** Typed title-block metadata carried by the leading sheet-header record. */
model SchematicPlotTitleBlock {
  title: string;
  date: string;
  rev: string;
  company: string;
  comments: Record<string>;
}

/** Leading paper, title-block, background, and worksheet record. */
model SchematicSheetHeaderPlotRecord {
  uuid: string;
  kind: "sheet_header";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  paper_size: string;
  paper_width_mm: float64 | null;
  paper_height_mm: float64 | null;
  paper_portrait: boolean;
  sheet_width_nm: JavaScriptSafeInteger;
  sheet_height_nm: JavaScriptSafeInteger;
  version: JavaScriptSafeInteger;
  generator: string;
  generator_version: string;
  title_block?: SchematicPlotTitleBlock;
}

/** One schematic wire polyline. */
model SchematicWirePlotRecord {
  uuid: string;
  kind: "wire";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
}

/** One schematic bus polyline. */
model SchematicBusPlotRecord {
  uuid: string;
  kind: "bus";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
}

/** One schematic bus-entry segment. */
model SchematicBusEntryPlotRecord {
  uuid: string;
  kind: "bus_entry";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
}

/** One schematic junction marker. */
model SchematicJunctionPlotRecord {
  uuid: string;
  kind: "junction";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  /** Authored junction color; null preserves an authored transparent color. */
  color?: string | null;
}

/** One schematic no-connect cross. */
model SchematicNoConnectPlotRecord {
  uuid: string;
  kind: "no_connect";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
}

/** Strict source-record vocabulary for the P5_060 schematic foundation. */
union SchematicPlotRecord {
  sheetHeader: SchematicSheetHeaderPlotRecord,
  wire: SchematicWirePlotRecord,
  bus: SchematicBusPlotRecord,
  busEntry: SchematicBusEntryPlotRecord,
  junction: SchematicJunctionPlotRecord,
  noConnect: SchematicNoConnectPlotRecord,
}

/** Strict schematic-foundation subset of kicad.plotter_ir.a0. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.schematic_plot.document:a0")
@extension("title", "Schematic plot document a0")
model SchematicPlotDocument {
  schema: "kicad.plotter_ir.a0";
  source_kind: "SCH";
  total_operations: uint32;
  records: SchematicPlotRecord[];
  source_path?: string;
  document_id: string;
  canvas: SchematicPlotCanvas;
  coordinate_space: PlotterCoordinateSpace;
}
