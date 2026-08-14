/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Stable ASCII identifier shared by manifests and oracle cases.
 */
export type StableTextId = string;
/**
 * Lowercase SHA-256 digest for one out-of-band font buffer.
 */
export type Sha256Hex = string;
/**
 * Four-byte OpenType variation or feature tag.
 */
export type OpenTypeTag = string;
/**
 * Finite float64 value, including fractional CFF/CFF2 design coordinates.
 */
export type FiniteFloat = number;

/**
 * Metadata for font buffers supplied out of band in matching numeric slots.
 */
export interface FontBundleManifestA0 {
  schema: "kicad_monkey.font_bundle.a0";
  type: "kicad_monkey.font_bundle";
  version: "a0";
  fonts: FontBundleEntry[];
}
/**
 * One font face whose bytes are supplied in a separate binary slot.
 */
export interface FontBundleEntry {
  id: StableTextId;
  slot: number;
  sha256: Sha256Hex;
  face_index: number;
  variations: FontVariationCoordinate[];
  aliases: string[];
  family?: string;
  style?: string;
  postscript_name?: string;
}
/**
 * One ordered OpenType variation coordinate.
 */
export interface FontVariationCoordinate {
  axis: OpenTypeTag;
  value: FiniteFloat;
}
