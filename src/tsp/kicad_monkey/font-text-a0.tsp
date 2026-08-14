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
  input: ShapingInput;
  glyphs: ShapedGlyph[];
}

model OutlineMoveTo {
  kind: "move_to";
  x: TextSafeInteger;
  y: TextSafeInteger;
}

model OutlineLineTo {
  kind: "line_to";
  x: TextSafeInteger;
  y: TextSafeInteger;
}

model OutlineQuadTo {
  kind: "quad_to";
  control_x: TextSafeInteger;
  control_y: TextSafeInteger;
  x: TextSafeInteger;
  y: TextSafeInteger;
}

model OutlineCurveTo {
  kind: "curve_to";
  control1_x: TextSafeInteger;
  control1_y: TextSafeInteger;
  control2_x: TextSafeInteger;
  control2_y: TextSafeInteger;
  x: TextSafeInteger;
  y: TextSafeInteger;
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
  font_id: string;
  font_sha256: Sha256Hex;
  face_index: uint32;
  variations: FontVariationCoordinate[];
  glyph_id: uint32;
  units_per_em: uint32;
  commands: OutlineCommand[];
}
