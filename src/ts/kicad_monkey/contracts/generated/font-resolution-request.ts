/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Selection metadata paired with a FontBundle and out-of-band buffers.
 */
export interface FontResolutionRequestA0 {
  schema: "kicad_monkey.font_resolution_request.a0";
  type: "kicad_monkey.font_resolution_request";
  version: "a0";
  selection: FontSelection;
}
/**
 * Deterministic font request: explicit ID wins, otherwise aliases are matched.
 */
export interface FontSelection {
  font_id?: string;
  aliases: string[];
}
