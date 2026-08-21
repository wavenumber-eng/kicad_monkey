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
export type TextDirection = "left_to_right" | "right_to_left" | "top_to_bottom" | "bottom_to_top";
/**
 * Nonempty text token whose detailed grammar is owned by the consumer API.
 */
export type NonEmptyText = string;
export type ShapingClusterLevel = "monotone_graphemes" | "monotone_characters" | "characters";
export type DefaultIgnorablePolicy = "normal" | "preserve" | "remove";
/**
 * Integer that remains exact through JSON and JavaScript.
 */
export type TextSafeInteger = number;

/**
 * Intermediate shaping oracle, intentionally separate from glyph outlines.
 */
export interface ShapingRecordA0 {
  schema: "kicad_monkey.shaping_record.a0";
  type: "kicad_monkey.shaping_record";
  version: "a0";
  case_id: StableTextId;
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
  font_id: StableTextId;
  font_sha256: Sha256Hex;
  face_index: number;
  variations: FontVariationCoordinate[];
  text: string;
  text_index_unit: "utf8_byte_offset";
  /**
   * HarfBuzz hb_font_set_scale x value.
   */
  scale_x: number;
  /**
   * HarfBuzz hb_font_set_scale y value.
   */
  scale_y: number;
  direction: TextDirection;
  script?: OpenTypeTag;
  language?: NonEmptyText;
  features: ShapingFeature[];
  buffer_properties: ShapingBufferProperties;
}
/**
 * One ordered OpenType variation coordinate.
 */
export interface FontVariationCoordinate {
  axis: OpenTypeTag;
  value: FiniteFloat;
}
/**
 * HarfBuzz-compatible feature range over half-open UTF-8 byte offsets.
 * Non-global endpoints are UTF-8 code-point boundaries. A global range is
 * start=0 and end=4294967295. HarfRust receives these values unchanged after
 * UnicodeBuffer::push_str assigns char_indices clusters.
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
  /**
   * UTF-8 scalar-start byte offset into ShapingInput.text, as assigned by push_str.
   */
  cluster: number;
  x_advance: TextSafeInteger;
  y_advance: TextSafeInteger;
  x_offset: TextSafeInteger;
  y_offset: TextSafeInteger;
  unsafe_to_break: boolean;
  safe_to_insert_tatweel: boolean;
  unsafe_to_concat: boolean;
}
