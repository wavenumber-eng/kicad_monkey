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
}
