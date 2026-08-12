/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

export type DiagnosticPhase = "lex" | "tree" | "build";

/**
 * Typed footprint facts; KiCad source bytes remain in the caller-owned input.
 */
export interface FootprintReadResultA0 {
  type: "kicad_monkey.footprint_read.result";
  version: "a0";
  name: string;
  source_bytes: string;
  properties: FootprintProperty[];
  pad_count: number;
  diagnostics: Diagnostic[];
}
/**
 * One decoded top-level KiCad footprint property.
 */
export interface FootprintProperty {
  name: string;
  value: string;
}
/**
 * Stable operation diagnostic shared by native, Python, and browser adapters.
 */
export interface Diagnostic {
  phase: DiagnosticPhase;
  code: string;
  message: string;
  position?: SourcePosition;
  token?: string;
}
/**
 * Zero-based UTF-8 byte offset with one-based source coordinates.
 */
export interface SourcePosition {
  offset: string;
  line: string;
  column: string;
}
