/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Canonical decimal wire encoding for an unsigned 64-bit byte count.
 */
export type CanonicalUint64Decimal = string;

/**
 * Strict result for deterministic, presentation-neutral base SVG.
 */
export interface NativeSVGRenderResultA0 {
  type: "kicad_monkey.native.svg.result";
  version: "a0";
  engine_version: string;
  profile: "plotter-base-a0";
  source_kind: "MOD" | "SYM" | "PCB" | "SCH";
  document_id: string;
  svg_utf8: string;
  svg_bytes: CanonicalUint64Decimal;
  svg_sha256: string;
}
