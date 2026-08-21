/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Controls a typed symbol-library summary read; source bytes are out of band.
 */
export interface SymbolLibraryReadRequestA0 {
  type: "kicad_monkey.symbol_library_read.request";
  version: "a0";
  max_source_bytes: string;
  max_depth: number;
  max_symbols: number;
  max_metadata_forms: number;
  max_subsymbols: number;
  max_pins: number;
}
