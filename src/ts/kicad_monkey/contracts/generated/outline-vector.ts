/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Stable ASCII identifier shared by manifests and oracle cases.
 */
export type StableTextId = string;
export type CoordinateComparisonPolicy = ExactComparisonPolicy | AbsoluteToleranceComparisonPolicy;
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
/**
 * Finite float64 value, including fractional CFF/CFF2 design coordinates.
 */
export type FiniteFloat = number;
/**
 * Positive OpenType units-per-em value.
 */
export type PositiveUint32 = number;
export type OutlineCommand = OutlineMoveTo | OutlineLineTo | OutlineQuadTo | OutlineCurveTo | OutlineClose;

/**
 * Raw glyph outline oracle in font units, separate from shaping and placement.
 */
export interface OutlineVectorA0 {
  schema: "kicad_monkey.outline_vector.a0";
  type: "kicad_monkey.outline_vector";
  version: "a0";
  case_id: StableTextId;
  coordinate_format: "font_design_units_f64";
  coordinate_comparison: CoordinateComparisonPolicy;
  font_id: StableTextId;
  font_sha256: Sha256Hex;
  face_index: number;
  variations: FontVariationCoordinate[];
  glyph_id: number;
  units_per_em: PositiveUint32;
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
  value: FiniteFloat;
}
export interface OutlineMoveTo {
  kind: "move_to";
  x: FiniteFloat;
  y: FiniteFloat;
}
export interface OutlineLineTo {
  kind: "line_to";
  x: FiniteFloat;
  y: FiniteFloat;
}
export interface OutlineQuadTo {
  kind: "quad_to";
  control_x: FiniteFloat;
  control_y: FiniteFloat;
  x: FiniteFloat;
  y: FiniteFloat;
}
export interface OutlineCurveTo {
  kind: "curve_to";
  control1_x: FiniteFloat;
  control1_y: FiniteFloat;
  control2_x: FiniteFloat;
  control2_y: FiniteFloat;
  x: FiniteFloat;
  y: FiniteFloat;
}
export interface OutlineClose {
  kind: "close";
}
