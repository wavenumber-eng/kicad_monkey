/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

export type DiagnosticPhase = "lex" | "tree" | "build";

/**
 * Edit metadata paired with a separate resulting KiCad byte buffer.
 */
export interface SymbolLibraryEditResultA0 {
  type: "kicad_monkey.symbol_library_edit.result";
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
