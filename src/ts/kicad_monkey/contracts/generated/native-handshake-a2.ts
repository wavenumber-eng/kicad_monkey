/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Exact native process handshake with the bounded a1 design-facts operation.
 */
export interface NativeHandshakeA2 {
  type: "kicad_monkey.native.handshake";
  version: "a2";
  engine_version: string;
  /**
   * @minItems 3
   * @maxItems 3
   */
  operations: ["design-facts", "render-svg", "design-facts-a1"];
}
