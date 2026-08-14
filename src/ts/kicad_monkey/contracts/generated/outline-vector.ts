/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Lowercase SHA-256 digest for one out-of-band font buffer.
 */
export type Sha256Hex = string;
/**
 * Four-byte OpenType variation or feature tag.
 */
export type OpenTypeTag = string;
export type OutlineCommand = OutlineMoveTo | OutlineLineTo | OutlineQuadTo | OutlineCurveTo | OutlineClose;
/**
 * Integer that remains exact through JSON and JavaScript.
 */
export type TextSafeInteger = number;

/**
 * Raw glyph outline oracle in font units, separate from shaping and placement.
 */
export interface OutlineVectorA0 {
  schema: "kicad_monkey.outline_vector.a0";
  type: "kicad_monkey.outline_vector";
  version: "a0";
  font_id: string;
  font_sha256: Sha256Hex;
  face_index: number;
  variations: FontVariationCoordinate[];
  glyph_id: number;
  units_per_em: number;
  commands: OutlineCommand[];
}
/**
 * One ordered OpenType variation coordinate.
 */
export interface FontVariationCoordinate {
  axis: OpenTypeTag;
  value: number;
}
export interface OutlineMoveTo {
  kind: "move_to";
  x: TextSafeInteger;
  y: TextSafeInteger;
}
export interface OutlineLineTo {
  kind: "line_to";
  x: TextSafeInteger;
  y: TextSafeInteger;
}
export interface OutlineQuadTo {
  kind: "quad_to";
  control_x: TextSafeInteger;
  control_y: TextSafeInteger;
  x: TextSafeInteger;
  y: TextSafeInteger;
}
export interface OutlineCurveTo {
  kind: "curve_to";
  control1_x: TextSafeInteger;
  control1_y: TextSafeInteger;
  control2_x: TextSafeInteger;
  control2_y: TextSafeInteger;
  x: TextSafeInteger;
  y: TextSafeInteger;
}
export interface OutlineClose {
  kind: "close";
}
