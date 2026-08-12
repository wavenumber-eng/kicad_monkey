/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

export type DiagnosticPhase = "lex" | "tree" | "build";

/**
 * Result envelope; selected source bytes remain in the caller-owned input buffer.
 */
export interface SExpressionScanResultA0 {
  type: "kicad_monkey.sexpr_scan.result";
  version: "a0";
  source_bytes: string;
  forms: FormSpan[];
  diagnostics: Diagnostic[];
}
/**
 * One complete selected form in the original UTF-8 byte buffer.
 */
export interface FormSpan {
  head?: string;
  path: string[];
  depth: number;
  start_offset: string;
  end_offset: string;
  line: string;
  column: string;
  end_line: string;
  end_column: string;
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
