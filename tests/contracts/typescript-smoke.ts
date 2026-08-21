import type {
  CompiledSchematicGraphA0,
  FootprintEditRequestA0,
  FootprintPlotDocumentA0,
  FootprintPlotRequestA0,
  FootprintReadRequestA0,
  NativeDesignFactsRequestA0,
  NativeDesignFactsRequestA1,
  NativeDesignFactsResultA0,
  NativeDesignFactsResultA1,
  NativeErrorA0,
  NativeHandshakeA0,
  NativeHandshakeA1,
  NativeHandshakeA2,
  NativeSVGRenderRequestA0,
  NativeSVGRenderResultA0,
  SchematicPlotDocumentA0,
  SchematicPlotRequestA0,
  SchematicPlotResultA0,
  SExpressionBuildRequestA0,
  SExpressionScanRequestA0,
} from "../../src/ts/kicad_monkey/contracts/generated/index.js";

const nativeHandshake = {
  type: "kicad_monkey.native.handshake",
  version: "a0",
  engine_version: "0.1.0",
  operations: ["design-facts"],
} satisfies NativeHandshakeA0;

const nativeError = {
  type: "kicad_monkey.native.error",
  version: "a0",
  kind: "resource_limit",
  message: "bounded failure",
} satisfies NativeErrorA0;

void nativeHandshake;
void nativeError;

const nativeHandshakeA1 = {
  type: "kicad_monkey.native.handshake",
  version: "a1",
  engine_version: "0.1.0",
  operations: ["design-facts", "render-svg"],
} satisfies NativeHandshakeA1;

type NativeHandshakeA1OperationsAreExact = NativeHandshakeA1["operations"] extends [
  "design-facts",
  "render-svg",
]
  ? ["design-facts", "render-svg"] extends NativeHandshakeA1["operations"]
    ? true
    : false
  : false;
const nativeHandshakeA1OperationsAreExact: NativeHandshakeA1OperationsAreExact = true;
// @ts-expect-error Native a1 operation order is a closed wire tuple.
const reversedNativeHandshakeA1Operations: NativeHandshakeA1["operations"] = ["render-svg", "design-facts"];

void nativeHandshakeA1;
void nativeHandshakeA1OperationsAreExact;
void reversedNativeHandshakeA1Operations;

const nativeHandshakeA2 = {
  type: "kicad_monkey.native.handshake",
  version: "a2",
  engine_version: "0.1.0",
  operations: ["design-facts", "render-svg", "design-facts-a1"],
} satisfies NativeHandshakeA2;

type NativeHandshakeA2OperationsAreExact = NativeHandshakeA2["operations"] extends [
  "design-facts",
  "render-svg",
  "design-facts-a1",
]
  ? ["design-facts", "render-svg", "design-facts-a1"] extends NativeHandshakeA2["operations"]
    ? true
    : false
  : false;
const nativeHandshakeA2OperationsAreExact: NativeHandshakeA2OperationsAreExact = true;
// @ts-expect-error Native a2 operation order is a closed wire tuple.
const reversedNativeHandshakeA2Operations: NativeHandshakeA2["operations"] = ["design-facts-a1", "render-svg", "design-facts"];

void nativeHandshakeA2;
void nativeHandshakeA2OperationsAreExact;
void reversedNativeHandshakeA2Operations;

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

const nativeDesignFactsRequest = {
  type: "kicad_monkey.native.design_facts.request",
  version: "a0",
  bundle_root: "C:/bundle",
  manifest: {
    schema: "kicad_monkey.source_bundle_manifest.a0",
    type: "kicad_monkey.source_bundle_manifest",
    version: "a0",
    root_schematic_path: "root.kicad_sch",
    sources: [{ path: "root.kicad_sch", kind: "schematic", slot: 0, source_bytes: "0" }],
  },
  file_slots: [{ slot: 0, path: "root.kicad_sch" }],
  limits: {
    max_sources: 1,
    max_source_bytes: "1048576",
    max_total_source_bytes: "1048576",
    max_path_bytes: 4096,
    max_output_bytes: "8388608",
  },
  netlist: { source_path: "root.kicad_sch", date: "", tool: "kicad-monkey-native" },
} satisfies NativeDesignFactsRequestA0;

const nativeDesignFactsResult = {
  type: "kicad_monkey.native.design_facts.result",
  version: "a0",
  engine_version: "0.1.0",
  compiled_schematic_graph: compiledGraph,
  kicad_netlist_version: "E",
  kicad_netlist: '(export (version "E"))',
} satisfies NativeDesignFactsResultA0;

void nativeDesignFactsRequest;
void nativeDesignFactsResult;

const nativeDesignFactsRequestA1 = {
  ...nativeDesignFactsRequest,
  version: "a1",
  resource_profile: "design-facts-bounded-a1",
} satisfies NativeDesignFactsRequestA1;

const nativeDesignFactsResultA1 = {
  ...nativeDesignFactsResult,
  version: "a1",
  resource_profile: "design-facts-bounded-a1",
  source_snapshot_sha256: "0000000000000000000000000000000000000000000000000000000000000000",
  kicad_netlist_bytes: "22",
  kicad_netlist_sha256: "fdb9de537cd653ab43fed2d1406a0cbf818365b1aad3b4c859bd60240288290b",
} satisfies NativeDesignFactsResultA1;

void nativeDesignFactsRequestA1;
void nativeDesignFactsResultA1;

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

const nativeSvgRequest = {
  type: "kicad_monkey.native.svg.request",
  version: "a0",
  profile: "plotter-base-a0",
  document: { kind: "footprint", value: footprintPlotDocument },
  viewport: { min_x_nm: 0, min_y_nm: 0, width_nm: 1_000_000, height_nm: 1_000_000 },
  limits: {
    max_records: 1,
    max_operations: 1,
    max_points: "10",
    max_text_bytes: "100",
    max_image_encoded_bytes: "100",
    max_block_depth: 1,
    max_svg_elements: "10",
    max_render_work: "10000",
    max_svg_bytes: "10000",
    max_result_bytes: "20000",
  },
} satisfies NativeSVGRenderRequestA0;

// @ts-expect-error A footprint wrapper cannot carry a PCB document.
const mismatchedNativeSvgDocument: NativeSVGRenderRequestA0["document"] = { kind: "footprint", value: { ...footprintPlotDocument, source_kind: "PCB" } };

const nativeSvgResult = {
  type: "kicad_monkey.native.svg.result",
  version: "a0",
  engine_version: "0.1.0",
  profile: "plotter-base-a0",
  source_kind: "MOD",
  document_id: "demo",
  svg_utf8: "<svg/>\n",
  svg_bytes: "7",
  svg_sha256: "0000000000000000000000000000000000000000000000000000000000000000",
} satisfies NativeSVGRenderResultA0;

void nativeSvgRequest;
void nativeSvgResult;
void mismatchedNativeSvgDocument;

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
  max_polylines: 10_000,
  max_arcs: 10_000,
  max_circles: 10_000,
  max_rectangles: 10_000,
  max_beziers: 10_000,
  max_rule_areas: 10_000,
  max_images: 10_000,
  max_tables: 10_000,
  max_table_cells: 100_000,
  max_table_cell_lines: 100_000,
  max_image_data_parts: 10_000,
  max_image_encoded_bytes: "1048576",
  max_image_decoded_bytes: "1048576",
  max_image_width_px: 4096,
  max_image_height_px: 4096,
  max_image_pixels: "16777216",
  max_image_decode_work: "16777216",
  max_symbols: 10_000,
  max_symbol_overplots: 10_000,
  max_symbol_properties: 100_000,
  max_symbol_pins: 100_000,
  max_library_symbols: 10_000,
  max_library_subsymbols: 100_000,
  max_library_pins: 100_000,
  max_symbol_overlap_checks: "1000000",
  max_sheets: 10_000,
  max_sheet_properties: 100_000,
  max_sheet_pins: 100_000,
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
