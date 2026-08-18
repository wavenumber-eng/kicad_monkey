/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Exact native process handshake.
 */
export interface NativeHandshakeA0 {
  type: "kicad_monkey.native.handshake";
  version: "a0";
  engine_version: string;
  /**
   * @minItems 1
   * @maxItems 1
   */
  operations: ["design-facts"];
}
