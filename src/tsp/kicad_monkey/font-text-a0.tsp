import "@typespec/json-schema";

using TypeSpec.JsonSchema;

namespace Wavenumber.KicadMonkey.Contracts.FontTextA0;

/** Lowercase SHA-256 digest for one out-of-band font buffer. */
@pattern("^[0-9a-f]{64}$")
scalar Sha256Hex extends string;

/** Four-byte OpenType variation or feature tag. */
@pattern("^[ -~]{4}$")
scalar OpenTypeTag extends string;

/** Stable ASCII identifier shared by manifests and oracle cases. */
@pattern("^[A-Za-z0-9][A-Za-z0-9._:-]*$")
scalar StableTextId extends string;

/** Integer that remains exact through JSON and JavaScript. */
@minValue(-9007199254740991)
@maxValue(9007199254740991)
scalar TextSafeInteger extends safeint;

/** Finite nonnegative tolerance transported as a JSON number. */
@minValue(0)
scalar NonNegativeFiniteFloat extends float64;

/** Finite float64 value, including fractional CFF/CFF2 design coordinates. */
@minValue(-1.7976931348623157e308)
@maxValue(1.7976931348623157e308)
scalar FiniteFloat extends float64;

/** Positive OpenType units-per-em value. */
@minValue(1)
scalar PositiveUint32 extends uint32;

/** One ordered OpenType variation coordinate. */
model FontVariationCoordinate {
  axis: OpenTypeTag;
  value: FiniteFloat;
}

/** One font face whose bytes are supplied in a separate binary slot. */
model FontBundleEntry {
  id: StableTextId;
  slot: uint32;
  sha256: Sha256Hex;
  face_index: uint32;
  variations: FontVariationCoordinate[];
  aliases: string[];
  family?: string;
  style?: string;
  postscript_name?: string;
}

/** Deterministic font request: explicit ID wins, otherwise aliases are matched. */
model FontSelection {
  font_id?: StableTextId;
  aliases: string[];
}

/** Selection metadata paired with a FontBundle and out-of-band buffers. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.font_resolution.request:a0")
@extension("title", "Font resolution request a0")
model FontResolutionRequest {
  schema: "kicad_monkey.font_resolution_request.a0";
  type: "kicad_monkey.font_resolution_request";
  version: "a0";
  selection: FontSelection;
}

/** Metadata for font buffers supplied out of band in matching numeric slots. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.font_bundle:a0")
@extension("title", "Font bundle manifest a0")
model FontBundleManifest {
  schema: "kicad_monkey.font_bundle.a0";
  type: "kicad_monkey.font_bundle";
  version: "a0";
  fonts: FontBundleEntry[];
}

enum TextDirection {
  left_to_right,
  right_to_left,
  top_to_bottom,
  bottom_to_top,
}

enum ShapingClusterLevel {
  monotone_graphemes,
  monotone_characters,
  characters,
}

enum DefaultIgnorablePolicy {
  normal,
  preserve,
  remove,
}

/** Explicit HarfBuzz buffer state; no ambient library defaults are implied. */
model ShapingBufferProperties {
  cluster_level: ShapingClusterLevel;
  beginning_of_text: boolean;
  end_of_text: boolean;
  default_ignorables: DefaultIgnorablePolicy;
  do_not_insert_dotted_circle: boolean;
  produce_unsafe_to_concat: boolean;
  produce_safe_to_insert_tatweel: boolean;
}

model ExactComparisonPolicy {
  mode: "exact";
}

/** Absolute tolerance in the enclosing record's declared coordinate unit. */
model AbsoluteToleranceComparisonPolicy {
  mode: "absolute_tolerance";
  absolute_tolerance: NonNegativeFiniteFloat;
}

union CoordinateComparisonPolicy {
  ExactComparisonPolicy,
  AbsoluteToleranceComparisonPolicy,
}

/**
 * HarfBuzz-compatible feature range over half-open UTF-8 byte offsets.
 * Non-global endpoints are UTF-8 code-point boundaries. A global range is
 * start=0 and end=4294967295. Rustybuzz receives these values unchanged after
 * UnicodeBuffer::push_str assigns char_indices clusters.
 */
model ShapingFeature {
  tag: OpenTypeTag;
  value: uint32;
  start: uint32;
  end: uint32;
}

/** Complete deterministic shaping input retained with an oracle record. */
model ShapingInput {
  font_id: StableTextId;
  font_sha256: Sha256Hex;
  face_index: uint32;
  variations: FontVariationCoordinate[];
  text: string;
  text_index_unit: "utf8_byte_offset";
  scale_x: TextSafeInteger;
  scale_y: TextSafeInteger;
  direction: TextDirection;
  script?: OpenTypeTag;
  language?: string;
  features: ShapingFeature[];
  buffer_properties: ShapingBufferProperties;
}

/** One shaped glyph in logical buffer order. */
model ShapedGlyph {
  glyph_id: uint32;
  /** UTF-8 byte offset into ShapingInput.text, as assigned by push_str. */
  cluster: uint32;
  x_advance: TextSafeInteger;
  y_advance: TextSafeInteger;
  x_offset: TextSafeInteger;
  y_offset: TextSafeInteger;
  unsafe_to_break: boolean;
  safe_to_insert_tatweel: boolean;
  unsafe_to_concat: boolean;
}

/** Intermediate shaping oracle, intentionally separate from glyph outlines. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.shaping_record:a0")
@extension("title", "Shaping record a0")
model ShapingRecord {
  schema: "kicad_monkey.shaping_record.a0";
  type: "kicad_monkey.shaping_record";
  version: "a0";
  case_id: StableTextId;
  comparison: ExactComparisonPolicy;
  input: ShapingInput;
  glyphs: ShapedGlyph[];
}

model OutlineMoveTo {
  kind: "move_to";
  x: FiniteFloat;
  y: FiniteFloat;
}

model OutlineLineTo {
  kind: "line_to";
  x: FiniteFloat;
  y: FiniteFloat;
}

model OutlineQuadTo {
  kind: "quad_to";
  control_x: FiniteFloat;
  control_y: FiniteFloat;
  x: FiniteFloat;
  y: FiniteFloat;
}

model OutlineCurveTo {
  kind: "curve_to";
  control1_x: FiniteFloat;
  control1_y: FiniteFloat;
  control2_x: FiniteFloat;
  control2_y: FiniteFloat;
  x: FiniteFloat;
  y: FiniteFloat;
}

model OutlineClose {
  kind: "close";
}

union OutlineCommand {
  OutlineMoveTo,
  OutlineLineTo,
  OutlineQuadTo,
  OutlineCurveTo,
  OutlineClose,
}

/** Raw glyph outline oracle in font units, separate from shaping and placement. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.outline_vector:a0")
@extension("title", "Outline vector a0")
model OutlineVector {
  schema: "kicad_monkey.outline_vector.a0";
  type: "kicad_monkey.outline_vector";
  version: "a0";
  case_id: StableTextId;
  coordinate_format: "font_design_units_f64";
  coordinate_comparison: CoordinateComparisonPolicy;
  font_id: StableTextId;
  font_sha256: Sha256Hex;
  face_index: uint32;
  variations: FontVariationCoordinate[];
  glyph_id: uint32;
  units_per_em: PositiveUint32;
  commands: OutlineCommand[];
}
