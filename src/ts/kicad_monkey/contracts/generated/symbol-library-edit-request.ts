/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

export type SymbolBooleanField = "in_bom" | "on_board";

/**
 * Focused source-preserving edit of one symbol boolean field.
 */
export interface SymbolLibraryEditRequestA0 {
  type: "kicad_monkey.symbol_library_edit.request";
  version: "a0";
  symbol_name: string;
  field: SymbolBooleanField;
  value: boolean;
  max_source_bytes: string;
  max_output_bytes: string;
  max_depth: number;
  max_symbols: number;
  max_metadata_forms: number;
  max_subsymbols: number;
  max_pins: number;
}
