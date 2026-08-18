import "@typespec/json-schema";

using TypeSpec.JsonSchema;
using Wavenumber.KicadMonkey.Contracts.CompiledSchematicGraphA0;
using Wavenumber.KicadMonkey.Contracts.PlotterIrA0;
using Wavenumber.KicadMonkey.Contracts.SourceBundleA0;

namespace Wavenumber.KicadMonkey.Contracts.NativeTransportA0;

/** File-system carrier for one zero-based source-bundle byte slot. */
model NativeFileSlot {
  slot: uint32;
  path: string;
}

/** Caller-selected resource ceilings for one native design-facts operation. */
model NativeDesignFactsLimits {
  max_sources: uint32;
  max_source_bytes: CanonicalUint64Decimal;
  max_total_source_bytes: CanonicalUint64Decimal;
  max_path_bytes: uint32;
  max_output_bytes: CanonicalUint64Decimal;
}

/** Metadata written into the version-E KiCad netlist header. */
model NativeNetlistMetadata {
  source_path: string;
  date: string;
  tool: string;
}

/** Supported native process error category. */
enum NativeErrorKind {
  request,
  path,
  io,
  resource_limit,
  core,
}

/** Exact native process handshake. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.native.handshake:a0")
@extension("title", "Native handshake a0")
model NativeHandshake {
  type: "kicad_monkey.native.handshake";
  version: "a0";
  engine_version: string;
  @minItems(1)
  @maxItems(1)
  operations: "design-facts"[];
}

/** Exact expanded native process handshake. The closed a0 handshake remains unchanged. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.native.handshake:a1")
@extension("title", "Native handshake a1")
model NativeHandshakeA1 {
  type: "kicad_monkey.native.handshake";
  version: "a1";
  engine_version: string;
  @extension("minItems", 2)
  @extension("maxItems", 2)
  operations: ["design-facts", "render-svg"];
}

/** Positive viewport dimension that remains exact in JavaScript. */
@minValue(1)
@maxValue(9007199254740991)
scalar NativeSvgPositiveSafeInteger extends safeint;

/** Explicit document viewport. Frozen MOD/SYM/PCB plot documents do not carry one. */
model NativeSvgViewport {
  min_x_nm: JavaScriptSafeInteger;
  min_y_nm: JavaScriptSafeInteger;
  width_nm: NativeSvgPositiveSafeInteger;
  height_nm: NativeSvgPositiveSafeInteger;
}

/** Aggregate resource ceilings for one native SVG serialization. */
model NativeSvgRenderLimits {
  max_records: uint32;
  max_operations: uint32;
  max_points: CanonicalUint64Decimal;
  max_text_bytes: CanonicalUint64Decimal;
  max_image_encoded_bytes: CanonicalUint64Decimal;
  max_block_depth: uint32;
  max_svg_elements: CanonicalUint64Decimal;
  max_render_work: CanonicalUint64Decimal;
  max_svg_bytes: CanonicalUint64Decimal;
  max_result_bytes: CanonicalUint64Decimal;
}

model NativeFootprintSvgDocument {
  kind: "footprint";
  value: FootprintPlotDocument;
}

model NativeSymbolSvgDocument {
  kind: "symbol";
  value: SymbolPlotDocument;
}

model NativeBoardSvgDocument {
  kind: "board";
  value: BoardPlotDocument;
}

model NativeSchematicSvgDocument {
  kind: "schematic";
  value: SchematicPlotDocument;
}

/** One explicitly discriminated frozen Phase-5 plot-document root. */
union NativeSvgPlotDocument {
  footprint: NativeFootprintSvgDocument,
  symbol: NativeSymbolSvgDocument,
  board: NativeBoardSvgDocument,
  schematic: NativeSchematicSvgDocument,
}

/** Strict request for deterministic, presentation-neutral base SVG. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.native.svg.request:a0")
@extension("title", "Native SVG render request a0")
model NativeSvgRenderRequest {
  type: "kicad_monkey.native.svg.request";
  version: "a0";
  profile: "plotter-base-a0";
  document: NativeSvgPlotDocument;
  viewport: NativeSvgViewport;
  limits: NativeSvgRenderLimits;
}

/** Strict result for deterministic, presentation-neutral base SVG. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.native.svg.result:a0")
@extension("title", "Native SVG render result a0")
model NativeSvgRenderResult {
  type: "kicad_monkey.native.svg.result";
  version: "a0";
  engine_version: string;
  profile: "plotter-base-a0";
  source_kind: "MOD" | "SYM" | "PCB" | "SCH";
  document_id: string;
  svg_utf8: string;
  svg_bytes: CanonicalUint64Decimal;
  @pattern("^[0-9a-f]{64}$")
  svg_sha256: string;
}

/** Strict request for the native compiled-graph and version-E netlist operation. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.native.design_facts.request:a0")
@extension("title", "Native design facts request a0")
model NativeDesignFactsRequest {
  type: "kicad_monkey.native.design_facts.request";
  version: "a0";
  bundle_root: string;
  manifest: SourceBundleManifest;
  file_slots: NativeFileSlot[];
  limits: NativeDesignFactsLimits;
  netlist: NativeNetlistMetadata;
}

/** Strict result for the native compiled-graph and version-E netlist operation. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.native.design_facts.result:a0")
@extension("title", "Native design facts result a0")
model NativeDesignFactsResult {
  type: "kicad_monkey.native.design_facts.result";
  version: "a0";
  engine_version: string;
  compiled_schematic_graph: CompiledSchematicGraph;
  kicad_netlist_version: "E";
  kicad_netlist: string;
}

/** Structured stderr payload for a failed native process operation. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.native.error:a0")
@extension("title", "Native error a0")
model NativeError {
  type: "kicad_monkey.native.error";
  version: "a0";
  kind: NativeErrorKind;
  message: string;
}
