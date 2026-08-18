import "@typespec/json-schema";

using TypeSpec.JsonSchema;
using Wavenumber.KicadMonkey.Contracts.CompiledSchematicGraphA0;
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
