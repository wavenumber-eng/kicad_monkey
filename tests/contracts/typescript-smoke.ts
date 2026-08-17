import type {
  CompiledSchematicGraphA0,
  FootprintEditRequestA0,
  FootprintPlotDocumentA0,
  FootprintPlotRequestA0,
  FootprintReadRequestA0,
  SchematicPlotDocumentA0,
  SchematicPlotRequestA0,
  SchematicPlotResultA0,
  SExpressionBuildRequestA0,
  SExpressionScanRequestA0,
} from "../../src/ts/kicad_monkey/contracts/generated/index.js";

const compiledGraph = {
  schema: "kicad_monkey.compiled_schematic_graph.a0",
  type: "sch.compiled_schematic_graph",
  identity_namespace: "sch.compiled_schematic_graph.a0",
  unit_definitions: [],
  page_definitions: [],
  unit_occurrences: [],
  page_occurrences: [],
  hierarchy_occurrences: [],
  component_occurrences: [],
  local_net_occurrences: [],
  terminal_occurrences: [],
  hierarchy_terminal_bindings: [],
  graphical_artifact_links: [],
} satisfies CompiledSchematicGraphA0;

void compiledGraph;

const buildRequest = {
  type: "kicad_monkey.sexpr_build.request",
  version: "a0",
  root: {
    kind: "list",
    children: [{ kind: "atom", text: "footprint" }],
  },
  max_output_bytes: "4096",
  max_depth: 16,
  max_nodes: 64,
} satisfies SExpressionBuildRequestA0;

const scanRequest = {
  type: "kicad_monkey.sexpr_scan.request",
  version: "a0",
  selector: { heads: ["footprint"] },
  max_source_bytes: "4096",
  max_depth: 16,
  max_selected_forms: 8,
} satisfies SExpressionScanRequestA0;

void buildRequest;
void scanRequest;

const footprintRead = {
  type: "kicad_monkey.footprint_read.request",
  version: "a0",
  max_source_bytes: "1048576",
  max_depth: 64,
  max_properties: 128,
  max_pads: 4096,
} satisfies FootprintReadRequestA0;

const footprintEdit = {
  ...footprintRead,
  type: "kicad_monkey.footprint_edit.request",
  property_name: "Value",
  value: "R_0603",
  max_output_bytes: "1048576",
} satisfies FootprintEditRequestA0;

void footprintRead;
void footprintEdit;

const footprintPlot = {
  type: "kicad_monkey.footprint_plot.request",
  version: "a0",
  document_id: "Demo",
  max_source_bytes: "1048576",
  max_output_bytes: "1048576",
  max_depth: 64,
  max_metadata_forms: 128,
  max_text_carriers: 4096,
  max_text_bytes: "1048576",
  max_operations: 4096,
  max_points: 16384,
} satisfies FootprintPlotRequestA0;

const footprintPlotDocument = {
  schema: "kicad.plotter_ir.a0",
  source_kind: "MOD",
  total_operations: 0,
  records: [],
  document_id: "Demo",
  coordinate_space: { unit: "nm", y_axis: "down" },
  version: 20260206,
  generator: "pcbnew",
  generator_version: "10.0",
} satisfies FootprintPlotDocumentA0;

const footprintGraphicDocument = {
  ...footprintPlotDocument,
  total_operations: 4,
  records: [{
    uuid: "",
    kind: "footprint",
    object_id: "Graphics",
    operation_count: 4,
    operations: [
      {
        kind: "ArcThreePoint",
        index: 0,
        start_x: 1_000_000,
        start_y: 0,
        mid_x: 0,
        mid_y: 1_000_000,
        end_x: -1_000_000,
        end_y: 0,
        fill: "NO_FILL",
        width_nm: 100_000,
        layer: "F.Fab",
      },
      {
        kind: "PlotPoly",
        index: 1,
        points: [[0, 0], [1_000_000, 0], [0, 1_000_000]],
        fill: "FILLED_SHAPE",
        width_nm: 100_000,
        layer: "F.Cu",
      },
      {
        kind: "FlashPadRoundRect",
        index: 2,
        x: 2_000_000,
        y: 0,
        size_x_nm: 1_500_000,
        size_y_nm: 800_000,
        corner_radius_nm: 200_000,
        orient_deg: 45,
        layers: ["F.Cu", "F.Mask"],
        mask_margin_nm: 50_000,
      },
      {
        kind: "FlashPadCustom",
        index: 3,
        x: 0,
        y: 0,
        size_x_nm: 2_000_000,
        size_y_nm: 1_000_000,
        orient_deg: 0,
        polygons: [[[-1_000_000, -500_000], [1_000_000, -500_000], [0, 500_000]]],
        polygon_widths_nm: [50_000],
        anchor_shape: "rect",
        layers: ["F.Cu", "F.Mask"],
        mask_margin_nm: 0,
      },
    ],
    name: "Graphics",
    layer: "F.Cu",
    locked: false,
    placed: false,
    descr: "",
    tags: "",
    attr: [],
  }],
} satisfies FootprintPlotDocumentA0;

void footprintPlot;
void footprintPlotDocument;
void footprintGraphicDocument;

const schematicPlot = {
  type: "kicad_monkey.schematic_plot.request",
  version: "a0",
  document_id: "Demo",
  sheet_index: 1,
  sheet_count: 1,
  sheet_path: "/",
  sheet_name: "Root",
  worksheet_mode: "default",
  text_offset_ratio: 0.15,
  default_line_width_nm: 152_400,
  max_source_bytes: "1048576",
  max_worksheet_bytes: "1048576",
  max_output_bytes: "1048576",
  max_depth: 64,
  max_parse_nodes: 100_000,
  max_selected_forms: 10_000,
  max_records: 10_000,
  max_operations: 100_000,
  max_points: 1_000_000,
  max_input_points: 1_000_000,
  max_text_bytes: "1048576",
  max_metadata_bytes: "1048576",
  max_wires: 10_000,
  max_buses: 10_000,
  max_bus_entries: 10_000,
  max_junctions: 10_000,
  max_no_connects: 10_000,
  max_labels: 10_000,
  max_global_labels: 10_000,
  max_hierarchical_labels: 10_000,
  max_netclass_flags: 10_000,
  max_netclass_flag_properties: 10_000,
  max_texts: 10_000,
  max_text_boxes: 10_000,
  max_text_box_lines: 100_000,
  max_text_variables: 128,
  max_text_variable_bytes: "65536",
  max_worksheet_items: 10_000,
  max_worksheet_repeats: 10_000,
  max_worksheet_point_sets: 10_000,
  max_worksheet_points: 100_000,
  max_worksheet_bitmap_data_parts: 10_000,
  max_worksheet_bitmap_encoded_bytes: "1048576",
  max_worksheet_bitmap_decoded_bytes: "1048576",
  max_worksheet_bitmap_width_px: 4096,
  max_worksheet_bitmap_height_px: 4096,
  max_worksheet_bitmap_pixels: "16777216",
  max_worksheet_bitmap_decode_work: "16777216",
} satisfies SchematicPlotRequestA0;

const schematicPlotDocument = {
  schema: "kicad.plotter_ir.a0",
  source_kind: "SCH",
  total_operations: 0,
  records: [],
  document_id: "Demo",
  canvas: {
    width_nm: 297_000_000,
    height_nm: 210_000_000,
  },
  coordinate_space: { unit: "nm", y_axis: "down" },
} satisfies SchematicPlotDocumentA0;

const schematicPlotResult = {
  type: "kicad_monkey.schematic_plot.result",
  version: "a0",
  output_bytes: "0",
  total_operations: 0,
  diagnostics: [],
} satisfies SchematicPlotResultA0;

void schematicPlot;
void schematicPlotDocument;
void schematicPlotResult;
