/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * KiCad source role within one schematic compiler input bundle.
 */
export type SourceKind = "project" | "schematic" | "symbol_library" | "symbol_table" | "worksheet" | "other";
/**
 * Zero-based byte-buffer slot within one manifest.
 */
export type SourceSlot = number;
/**
 * Canonical decimal wire encoding for an unsigned 64-bit byte count.
 */
export type CanonicalUint64Decimal = string;

/**
 * Portable inventory for a named multi-file schematic compiler input.
 */
export interface SourceBundleManifestA0 {
  schema: "kicad_monkey.source_bundle_manifest.a0";
  type: "kicad_monkey.source_bundle_manifest";
  version: "a0";
  root_schematic_path: string;
  project_path?: string;
  sources: SourceBundleSource[];
}
/**
 * Metadata for one named byte buffer supplied out of band.
 */
export interface SourceBundleSource {
  path: string;
  kind: SourceKind;
  slot: SourceSlot;
  source_bytes: CanonicalUint64Decimal;
}
