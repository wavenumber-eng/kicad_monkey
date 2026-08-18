/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Supported native process error category.
 */
export type NativeErrorKind = "request" | "path" | "io" | "resource_limit" | "core";

/**
 * Structured stderr payload for a failed native process operation.
 */
export interface NativeErrorA0 {
  type: "kicad_monkey.native.error";
  version: "a0";
  kind: NativeErrorKind;
  message: string;
}
