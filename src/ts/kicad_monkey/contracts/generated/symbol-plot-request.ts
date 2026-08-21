/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Resource-bounded library-symbol plot operation. Source bytes are out of band.
 */
export interface SymbolPlotRequestA0 {
  type: "kicad_monkey.symbol_plot.request";
  version: "a0";
  symbol_name: string;
  unit?: number;
  style: number;
  source_path?: string;
  document_id?: string;
  max_source_bytes: string;
  max_output_bytes: string;
  max_depth: number;
  max_symbols: number;
  max_subsymbols: number;
  max_operations: number;
  max_points: number;
  max_text_carriers?: number;
  max_text_bytes?: string;
  /**
   * Exact-case project variables used only by library-symbol body text.
   */
  text_variables?: SymbolTextVariable[];
}
/**
 * One exact-case project-sidecar variable for library-symbol body text.
 */
export interface SymbolTextVariable {
  name: string;
  value: string;
}
