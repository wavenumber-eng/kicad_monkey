import "@typespec/json-schema";

using TypeSpec.JsonSchema;

namespace Wavenumber.KicadMonkey.Contracts.SexprA0;

enum DiagnosticPhase {
  lex,
  tree,
  build,
}

/** Zero-based UTF-8 byte offset with one-based source coordinates. */
model SourcePosition {
  offset: uint64;
  line: uint64;
  column: uint64;
}

/** Stable operation diagnostic shared by native, Python, and browser adapters. */
model Diagnostic {
  phase: DiagnosticPhase;
  code: string;
  message: string;
  position?: SourcePosition;
  token?: string;
}

/** Generic structural selector; source bytes are always supplied out of band. */
model Selector {
  heads?: string[];
  paths?: string[][];
  min_depth?: uint32;
  max_depth?: uint32;
  prune_heads?: string[];
}

/** One complete selected form in the original UTF-8 byte buffer. */
model FormSpan {
  head?: string;
  path: string[];
  depth: uint32;
  start_offset: uint64;
  end_offset: uint64;
  line: uint64;
  column: uint64;
  end_line: uint64;
  end_column: uint64;
}

/** Metadata envelope for a byte-buffer structural scan operation. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.sexpr_scan.request:a0")
@extension("title", "S-expression scan request a0")
model ScanRequest {
  type: "kicad_monkey.sexpr_scan.request";
  version: "a0";
  selector: Selector;
  max_source_bytes: uint64;
  max_depth: uint32;
  max_selected_forms: uint32;
}

/** Result envelope; selected source bytes remain in the caller-owned input buffer. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.sexpr_scan.result:a0")
@extension("title", "S-expression scan result a0")
model ScanResult {
  type: "kicad_monkey.sexpr_scan.result";
  version: "a0";
  source_bytes: uint64;
  forms: FormSpan[];
  diagnostics: Diagnostic[];
}

enum NodeKind {
  list,
  atom,
  quoted,
  integer,
  float,
}

/** Portable generic S-expression tree node used only by explicit build operations. */
model Node {
  kind: NodeKind;
  text?: string;
  integer?: int64;
  float?: float64;
  children?: Node[];
}

/** Deterministic generic-tree build request. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.sexpr_build.request:a0")
@extension("title", "S-expression build request a0")
model BuildRequest {
  type: "kicad_monkey.sexpr_build.request";
  version: "a0";
  root: Node;
  max_output_bytes: uint64;
  max_depth: uint32;
  max_nodes: uint32;
}

/** Build metadata; output UTF-8 bytes are returned out of band. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.sexpr_build.result:a0")
@extension("title", "S-expression build result a0")
model BuildResult {
  type: "kicad_monkey.sexpr_build.result";
  version: "a0";
  output_bytes: uint64;
  diagnostics: Diagnostic[];
}
