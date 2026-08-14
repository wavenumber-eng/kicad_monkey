/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

export type NumericComparisonPolicy = ExactComparisonPolicy | AbsoluteToleranceComparisonPolicy;
/**
 * Finite nonnegative tolerance transported as a JSON number.
 */
export type NonNegativeFiniteFloat = number;
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
 * Raw glyph outline oracle in font units, separate from shaping and placement.
 */
export interface OutlineVectorA0 {
  schema: "kicad_monkey.outline_vector.a0";
  type: "kicad_monkey.outline_vector";
  version: "a0";
  case_id: string;
  coordinate_format: "font_design_units_f64";
  comparison: NumericComparisonPolicy;
  font_id: string;
  font_sha256: Sha256Hex;
  face_index: number;
  variations: FontVariationCoordinate[];
  glyph_id: number;
  units_per_em: number;
  commands: OutlineCommand[];
}
export interface ExactComparisonPolicy {
  mode: "exact";
}
/**
 * Absolute tolerance in the enclosing record's declared coordinate unit.
 */
export interface AbsoluteToleranceComparisonPolicy {
  mode: "absolute_tolerance";
  absolute_tolerance: NonNegativeFiniteFloat;
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
  x: number;
  y: number;
}
export interface OutlineLineTo {
  kind: "line_to";
  x: number;
  y: number;
}
export interface OutlineQuadTo {
  kind: "quad_to";
  control_x: number;
  control_y: number;
  x: number;
  y: number;
}
export interface OutlineCurveTo {
  kind: "curve_to";
  control1_x: number;
  control1_y: number;
  control2_x: number;
  control2_y: number;
  x: number;
  y: number;
}
export interface OutlineClose {
  kind: "close";
}
