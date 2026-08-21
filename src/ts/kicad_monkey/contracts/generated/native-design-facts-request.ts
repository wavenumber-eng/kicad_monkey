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
 * Strict request for the native compiled-graph and version-E netlist operation.
 */
export interface NativeDesignFactsRequestA0 {
  type: "kicad_monkey.native.design_facts.request";
  version: "a0";
  bundle_root: string;
  manifest: SourceBundleManifestA0;
  file_slots: NativeFileSlot[];
  limits: NativeDesignFactsLimits;
  netlist: NativeNetlistMetadata;
}
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
/**
 * File-system carrier for one zero-based source-bundle byte slot.
 */
export interface NativeFileSlot {
  slot: number;
  path: string;
}
/**
 * Caller-selected resource ceilings for one native design-facts operation.
 */
export interface NativeDesignFactsLimits {
  max_sources: number;
  max_source_bytes: CanonicalUint64Decimal;
  max_total_source_bytes: CanonicalUint64Decimal;
  max_path_bytes: number;
  max_output_bytes: CanonicalUint64Decimal;
}
/**
 * Metadata written into the version-E KiCad netlist header.
 */
export interface NativeNetlistMetadata {
  source_path: string;
  date: string;
  tool: string;
}
