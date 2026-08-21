/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Exact expanded native process handshake. The closed a0 handshake remains unchanged.
 */
export interface NativeHandshakeA1 {
  type: "kicad_monkey.native.handshake";
  version: "a1";
  engine_version: string;
  /**
   * @minItems 2
   * @maxItems 2
   */
  operations: ["design-facts", "render-svg"];
}
