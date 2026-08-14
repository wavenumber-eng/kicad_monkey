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

/** Canonical decimal wire encoding for an unsigned 64-bit byte count. */
@pattern("^(0|[1-9][0-9]{0,19})$")
scalar CanonicalUint64Decimal extends string;

/** Zero-based byte-buffer slot within one manifest. */
scalar SourceSlot extends uint32;

/** Metadata for one named byte buffer supplied out of band. */
model SourceBundleSource {
  path: string;
  kind: SourceKind;
  slot: SourceSlot;
  source_bytes: CanonicalUint64Decimal;
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
