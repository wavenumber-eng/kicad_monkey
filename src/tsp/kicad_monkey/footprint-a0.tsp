import "@typespec/json-schema";
import "./sexpr-a0.tsp";

using TypeSpec.JsonSchema;
using Wavenumber.KicadMonkey.Contracts.SexprA0;

namespace Wavenumber.KicadMonkey.Contracts.FootprintA0;

/** One decoded top-level KiCad footprint property. */
model FootprintProperty {
  name: string;
  value: string;
}

/** Controls a typed footprint summary read; source bytes are supplied out of band. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.footprint_read.request:a0")
@extension("title", "Footprint read request a0")
model FootprintReadRequest {
  type: "kicad_monkey.footprint_read.request";
  version: "a0";
  max_source_bytes: uint64;
  max_depth: uint32;
  max_properties: uint32;
  max_pads: uint32;
}

/** Typed footprint facts; KiCad source bytes remain in the caller-owned input. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.footprint_read.result:a0")
@extension("title", "Footprint read result a0")
model FootprintReadResult {
  type: "kicad_monkey.footprint_read.result";
  version: "a0";
  name: string;
  source_bytes: uint64;
  properties: FootprintProperty[];
  pad_count: uint32;
  diagnostics: Diagnostic[];
}

/** Focused source-preserving property edit; source bytes are supplied out of band. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.footprint_edit.request:a0")
@extension("title", "Footprint edit request a0")
model FootprintEditRequest {
  type: "kicad_monkey.footprint_edit.request";
  version: "a0";
  property_name: string;
  value: string;
  max_source_bytes: uint64;
  max_output_bytes: uint64;
  max_depth: uint32;
  max_properties: uint32;
  max_pads: uint32;
}

/** Edit metadata; the resulting KiCad UTF-8 bytes are returned out of band. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.footprint_edit.result:a0")
@extension("title", "Footprint edit result a0")
model FootprintEditResult {
  type: "kicad_monkey.footprint_edit.result";
  version: "a0";
  changed: boolean;
  output_bytes: uint64;
  diagnostics: Diagnostic[];
}
