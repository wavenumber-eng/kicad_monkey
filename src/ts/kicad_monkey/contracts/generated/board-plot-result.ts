/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

export type DiagnosticPhase = "lex" | "tree" | "build";

/**
 * Metadata paired with a separate board plotter-IR JSON byte buffer.
 */
export interface BoardPlotResultA0 {
  type: "kicad_monkey.board_plot.result";
  version: "a0";
  output_bytes: string;
  total_operations: number;
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
