/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

export type DiagnosticPhase = "lex" | "tree" | "build";

/**
 * Typed symbol-library facts retaining source order.
 */
export interface SymbolLibraryReadResultA0 {
  type: "kicad_monkey.symbol_library_read.result";
  version: "a0";
  source_bytes: string;
  symbols: SymbolSummary[];
  diagnostics: Diagnostic[];
}
/**
 * One source-backed top-level symbol summary.
 */
export interface SymbolSummary {
  name: string;
  extends?: string;
  in_bom: boolean;
  on_board: boolean;
  power: boolean;
  power_kind?: string;
  property_count: number;
  subsymbol_count: number;
  pin_count: number;
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
