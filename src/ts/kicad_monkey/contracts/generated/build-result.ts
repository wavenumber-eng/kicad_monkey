/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

export type DiagnosticPhase = "lex" | "tree" | "build";

/**
 * Build metadata; output UTF-8 bytes are returned out of band.
 */
export interface SExpressionBuildResultA0 {
  type: "kicad_monkey.sexpr_build.result";
  version: "a0";
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
