/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Lowercase SHA-256 digest for one out-of-band font buffer.
 */
export type Sha256Hex = string;
/**
 * Four-byte OpenType variation or feature tag.
 */
export type OpenTypeTag = string;
/**
 * Integer that remains exact through JSON and JavaScript.
 */
export type TextSafeInteger = number;
export type TextDirection = "left_to_right" | "right_to_left" | "top_to_bottom" | "bottom_to_top";
export type ShapingClusterLevel = "monotone_graphemes" | "monotone_characters" | "characters";
export type DefaultIgnorablePolicy = "normal" | "preserve" | "remove";

/**
 * Intermediate shaping oracle, intentionally separate from glyph outlines.
 */
export interface ShapingRecordA0 {
  schema: "kicad_monkey.shaping_record.a0";
  type: "kicad_monkey.shaping_record";
  version: "a0";
  case_id: string;
  comparison: ExactComparisonPolicy;
  input: ShapingInput;
  glyphs: ShapedGlyph[];
}
export interface ExactComparisonPolicy {
  mode: "exact";
}
/**
 * Complete deterministic shaping input retained with an oracle record.
 */
export interface ShapingInput {
  font_id: string;
  font_sha256: Sha256Hex;
  face_index: number;
  variations: FontVariationCoordinate[];
  text: string;
  scale_x: TextSafeInteger;
  scale_y: TextSafeInteger;
  direction: TextDirection;
  script?: OpenTypeTag;
  language?: string;
  features: ShapingFeature[];
  buffer_properties: ShapingBufferProperties;
}
/**
 * One ordered OpenType variation coordinate.
 */
export interface FontVariationCoordinate {
  axis: OpenTypeTag;
  value: number;
}
/**
 * HarfBuzz-compatible feature range over input scalar indices.
 */
export interface ShapingFeature {
  tag: OpenTypeTag;
  value: number;
  start: number;
  end: number;
}
/**
 * Explicit HarfBuzz buffer state; no ambient library defaults are implied.
 */
export interface ShapingBufferProperties {
  cluster_level: ShapingClusterLevel;
  beginning_of_text: boolean;
  end_of_text: boolean;
  default_ignorables: DefaultIgnorablePolicy;
  do_not_insert_dotted_circle: boolean;
  produce_unsafe_to_concat: boolean;
  produce_safe_to_insert_tatweel: boolean;
}
/**
 * One shaped glyph in logical buffer order.
 */
export interface ShapedGlyph {
  glyph_id: number;
  cluster: number;
  x_advance: TextSafeInteger;
  y_advance: TextSafeInteger;
  x_offset: TextSafeInteger;
  y_offset: TextSafeInteger;
  unsafe_to_break: boolean;
  safe_to_insert_tatweel: boolean;
  unsafe_to_concat: boolean;
}
