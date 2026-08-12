/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

export type DiagnosticPhase = "lex" | "tree" | "build";

/**
 * Edit metadata; the resulting KiCad UTF-8 bytes are returned out of band.
 */
export interface FootprintEditResultA0 {
  type: "kicad_monkey.footprint_edit.result";
  version: "a0";
  changed: boolean;
  output_bytes: string;
  diagnostics: Diagnostic[];
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
