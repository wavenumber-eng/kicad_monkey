import "@typespec/json-schema";

using TypeSpec.JsonSchema;

namespace Wavenumber.KicadMonkey.Contracts.SourceBundleA0;

/** KiCad source role within one schematic compiler input bundle. */
enum SourceKind {
  project,
  schematic,
  symbol_library,
  symbol_table,
  worksheet,
  other,
}

/** Metadata for one named byte buffer supplied out of band. */
model SourceBundleSource {
  path: string;
  kind: SourceKind;
  slot: uint32;
  source_bytes: uint64;
}

/** Portable inventory for a named multi-file schematic compiler input. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.source_bundle_manifest:a0")
@extension("title", "Source bundle manifest a0")
model SourceBundleManifest {
  schema: "kicad_monkey.source_bundle_manifest.a0";
  type: "kicad_monkey.source_bundle_manifest";
  version: "a0";
  root_schematic_path: string;
  project_path?: string;
  sources: SourceBundleSource[];
}
