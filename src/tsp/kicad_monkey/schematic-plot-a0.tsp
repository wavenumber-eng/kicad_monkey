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

/** Finite nonnegative project text-offset ratio used by schematic plotting. */
@minValue(0)
@maxValue(1.7976931348623157e308)
scalar SchematicTextOffsetRatio extends float64;

/** Effective schematic plot width after KiCad's minimum-pen clamp. */
@minValue(84700)
@maxValue(9007199254740991)
scalar SchematicDefaultLineWidthNm extends safeint;

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
  text_offset_ratio: SchematicTextOffsetRatio;
  default_line_width_nm: SchematicDefaultLineWidthNm;
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
  max_labels: uint32;
  max_global_labels: uint32;
  max_hierarchical_labels: uint32;
  max_netclass_flags: uint32;
  max_netclass_flag_properties: uint32;
  max_texts: uint32;
  max_text_boxes: uint32;
  max_text_box_lines: uint32;
  max_polylines: uint32;
  max_arcs: uint32;
  max_circles: uint32;
  max_rectangles: uint32;
  max_beziers: uint32;
  max_rule_areas: uint32;
  max_images: uint32;
  max_tables: uint32;
  max_table_cells: uint32;
  max_table_cell_lines: uint32;
  max_image_data_parts: uint32;
  max_image_encoded_bytes: uint64;
  max_image_decoded_bytes: uint64;
  max_image_width_px: uint32;
  max_image_height_px: uint32;
  max_image_pixels: uint64;
  max_image_decode_work: uint64;
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

/** Signal-direction shapes preserved on global and hierarchical labels. */
enum SchematicLabelShape {
  input: "input",
  output: "output",
  bidirectional: "bidirectional",
  triState: "tri_state",
  passive: "passive",
  dot: "dot",
  round: "round",
  diamond: "diamond",
  rectangle: "rectangle",
}

/** Marker shapes admitted by schematic netclass/directive flags. */
enum SchematicNetclassFlagShape {
  round: "round",
  dot: "dot",
  diamond: "diamond",
  rectangle: "rectangle",
}

/** One local schematic label. */
model SchematicLabelPlotRecord {
  uuid: string;
  kind: "label";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  text: string;
}

/** One global schematic label and its optional decoration. */
model SchematicGlobalLabelPlotRecord {
  uuid: string;
  kind: "global_label";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  text: string;
  shape: SchematicLabelShape;
}

/** One hierarchical schematic label and its optional decoration. */
model SchematicHierarchicalLabelPlotRecord {
  uuid: string;
  kind: "hierarchical_label";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  text: string;
  shape: SchematicLabelShape;
}

/** One netclass/directive flag with its visible property text. */
model SchematicNetclassFlagPlotRecord {
  uuid: string;
  kind: "netclass_flag";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  at_x_nm: JavaScriptSafeInteger;
  at_y_nm: JavaScriptSafeInteger;
  shape: SchematicNetclassFlagShape;
  length_nm: JavaScriptSafeInteger;
}

/** One ordinary top-level schematic text annotation. */
model SchematicTextPlotRecord {
  uuid: string;
  kind: "text";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  text: string;
}

/** One schematic text box with its canonical outline and plotted lines. */
model SchematicTextBoxPlotRecord {
  uuid: string;
  kind: "text_box";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  text: string;
}

/** One top-level schematic graphic polyline. */
model SchematicGraphicPolylinePlotRecord {
  uuid: string;
  kind: "graphic_polyline";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
}

/** One top-level schematic graphic arc. */
model SchematicGraphicArcPlotRecord {
  uuid: string;
  kind: "graphic_arc";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
}

/** One top-level schematic graphic circle. */
model SchematicGraphicCirclePlotRecord {
  uuid: string;
  kind: "graphic_circle";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
}

/** One top-level schematic graphic rectangle. */
model SchematicGraphicRectanglePlotRecord {
  uuid: string;
  kind: "graphic_rectangle";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
}

/** One canonical four-point top-level schematic graphic Bezier. */
model SchematicGraphicBezierPlotRecord {
  uuid: string;
  kind: "graphic_bezier";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
}

/** Source-shape discriminator retained by a schematic rule area. */
enum SchematicRuleAreaShape {
  polyline: "polyline",
  rectangle: "rectangle",
  arc: "arc",
  circle: "circle",
  bezier: "bezier",
}

/** One schematic rule area with source policy metadata. */
model SchematicRuleAreaPlotRecord {
  uuid: string;
  kind: "rule_area";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  shape: SchematicRuleAreaShape;
  locked: boolean;
  exclude_from_sim: boolean;
  in_bom: boolean;
  on_board: boolean;
  dnp: boolean;
}

/** Decoded image formats admitted by the schematic image family. */
enum SchematicImageFormat {
  png: "png",
  jpeg: "jpeg",
  bmp: "bmp",
}

/** One decoded embedded schematic image. */
model SchematicImagePlotRecord {
  uuid: string;
  kind: "image";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  scale: float64;
  image_format: SchematicImageFormat;
  width_nm: JavaScriptSafeInteger;
  height_nm: JavaScriptSafeInteger;
}

/** One schematic table rendered as ordered text-box cell blocks. */
model SchematicTablePlotRecord {
  uuid: string;
  kind: "table";
  object_id: string;
  operation_count: uint32;
  operations: PlotterOperation[];
  cell_count: uint32;
}

/** Strict source-record vocabulary through the P5_062 schematic graphics. */
union SchematicPlotRecord {
  sheetHeader: SchematicSheetHeaderPlotRecord,
  wire: SchematicWirePlotRecord,
  bus: SchematicBusPlotRecord,
  busEntry: SchematicBusEntryPlotRecord,
  junction: SchematicJunctionPlotRecord,
  noConnect: SchematicNoConnectPlotRecord,
  label: SchematicLabelPlotRecord,
  globalLabel: SchematicGlobalLabelPlotRecord,
  hierarchicalLabel: SchematicHierarchicalLabelPlotRecord,
  netclassFlag: SchematicNetclassFlagPlotRecord,
  text: SchematicTextPlotRecord,
  textBox: SchematicTextBoxPlotRecord,
  graphicPolyline: SchematicGraphicPolylinePlotRecord,
  graphicArc: SchematicGraphicArcPlotRecord,
  graphicCircle: SchematicGraphicCirclePlotRecord,
  graphicRectangle: SchematicGraphicRectanglePlotRecord,
  graphicBezier: SchematicGraphicBezierPlotRecord,
  ruleArea: SchematicRuleAreaPlotRecord,
  image: SchematicImagePlotRecord,
  table: SchematicTablePlotRecord,
}

/** Strict schematic subset through the P5_062 graphic families. */
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
