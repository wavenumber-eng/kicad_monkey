import "@typespec/json-schema";
import "./sexpr-a0.tsp";

using TypeSpec.JsonSchema;
using Wavenumber.KicadMonkey.Contracts.SexprA0;

namespace Wavenumber.KicadMonkey.Contracts.SymbolLibraryA0;

/** One source-backed top-level symbol summary. */
model SymbolSummary {
  name: string;
  `extends`?: string;
  in_bom: boolean;
  on_board: boolean;
  power: boolean;
  power_kind?: string;
  property_count: uint32;
  subsymbol_count: uint32;
  pin_count: uint32;
}

/** Controls a typed symbol-library summary read; source bytes are out of band. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.symbol_library_read.request:a0")
@extension("title", "Symbol library read request a0")
model SymbolLibraryReadRequest {
  type: "kicad_monkey.symbol_library_read.request";
  version: "a0";
  max_source_bytes: uint64;
  max_depth: uint32;
  max_symbols: uint32;
  max_metadata_forms: uint32;
  max_subsymbols: uint32;
  max_pins: uint32;
}

/** Typed symbol-library facts retaining source order. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.symbol_library_read.result:a0")
@extension("title", "Symbol library read result a0")
model SymbolLibraryReadResult {
  type: "kicad_monkey.symbol_library_read.result";
  version: "a0";
  source_bytes: uint64;
  symbols: SymbolSummary[];
  diagnostics: Diagnostic[];
}

enum SymbolBooleanField {
  in_bom,
  on_board,
}

/** Focused source-preserving edit of one symbol boolean field. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.symbol_library_edit.request:a0")
@extension("title", "Symbol library edit request a0")
model SymbolLibraryEditRequest {
  type: "kicad_monkey.symbol_library_edit.request";
  version: "a0";
  symbol_name: string;
  field: SymbolBooleanField;
  value: boolean;
  max_source_bytes: uint64;
  max_output_bytes: uint64;
  max_depth: uint32;
  max_symbols: uint32;
  max_metadata_forms: uint32;
  max_subsymbols: uint32;
  max_pins: uint32;
}

/** Edit metadata paired with a separate resulting KiCad byte buffer. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.symbol_library_edit.result:a0")
@extension("title", "Symbol library edit result a0")
model SymbolLibraryEditResult {
  type: "kicad_monkey.symbol_library_edit.result";
  version: "a0";
  changed: boolean;
  output_bytes: uint64;
  diagnostics: Diagnostic[];
}
