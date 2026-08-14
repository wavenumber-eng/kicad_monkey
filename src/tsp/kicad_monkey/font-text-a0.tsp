import "@typespec/json-schema";

using TypeSpec.JsonSchema;

namespace Wavenumber.KicadMonkey.Contracts.FontTextA0;

/** Lowercase SHA-256 digest for one out-of-band font buffer. */
@pattern("^[0-9a-f]{64}$")
scalar Sha256Hex extends string;

/** Four-byte OpenType variation or feature tag. */
@pattern("^[ -~]{4}$")
scalar OpenTypeTag extends string;

/** Integer that remains exact through JSON and JavaScript. */
@minValue(-9007199254740991)
@maxValue(9007199254740991)
scalar TextSafeInteger extends safeint;

/** Finite nonnegative tolerance transported as a JSON number. */
@minValue(0)
scalar NonNegativeFiniteFloat extends float64;

/** One ordered OpenType variation coordinate. */
model FontVariationCoordinate {
  axis: OpenTypeTag;
  value: float64;
}

/** One font face whose bytes are supplied in a separate binary slot. */
model FontBundleEntry {
  id: string;
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
  font_id?: string;
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

union NumericComparisonPolicy {
  ExactComparisonPolicy,
  AbsoluteToleranceComparisonPolicy,
}

/** HarfBuzz-compatible feature range over input scalar indices. */
model ShapingFeature {
  tag: OpenTypeTag;
  value: uint32;
  start: uint32;
  end: uint32;
}

/** Complete deterministic shaping input retained with an oracle record. */
model ShapingInput {
  font_id: string;
  font_sha256: Sha256Hex;
  face_index: uint32;
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

/** One shaped glyph in logical buffer order. */
model ShapedGlyph {
  glyph_id: uint32;
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
  case_id: string;
  comparison: ExactComparisonPolicy;
  input: ShapingInput;
  glyphs: ShapedGlyph[];
}

model OutlineMoveTo {
  kind: "move_to";
  x: float64;
  y: float64;
}

model OutlineLineTo {
  kind: "line_to";
  x: float64;
  y: float64;
}

model OutlineQuadTo {
  kind: "quad_to";
  control_x: float64;
  control_y: float64;
  x: float64;
  y: float64;
}

model OutlineCurveTo {
  kind: "curve_to";
  control1_x: float64;
  control1_y: float64;
  control2_x: float64;
  control2_y: float64;
  x: float64;
  y: float64;
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
  case_id: string;
  coordinate_format: "font_design_units_f64";
  comparison: NumericComparisonPolicy;
  font_id: string;
  font_sha256: Sha256Hex;
  face_index: uint32;
  variations: FontVariationCoordinate[];
  glyph_id: uint32;
  units_per_em: uint32;
  commands: OutlineCommand[];
}
