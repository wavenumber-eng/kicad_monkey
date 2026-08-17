# KiCad Monkey Design Docs

Design notes describe public interfaces, signoff policies, data contracts, and
test ownership rules that are too detailed for ADRs.

The master HTML entry point is `index.html`. Public API and interface design
sections live under `api/`, and all design HTML uses `styles.css`.

`L99_signoff` enforces:

- `docs/design/index.html`, `docs/design/api/index.html`, and
  `docs/design/styles.css` exist;
- every promoted public class in `kicad_monkey.kicad_api_contract` has a
  `data-interface` design section;
- every major interface in
  `docs/contracts/interface_design_manifest.v0.json` has a design section;
- every design section records rationale, purpose, test requirements, working
  definition, and Rack test ownership.

Current design notes:

- `rust-standard.html` - provisional Rust workspace, safety, dependency, and
  review policy for the native/WASM port.
- `rust-sexpr-l0-review.html` - accepted Rust S-expression foundation review,
  evidence, corrections, and retained parser-promotion gates.
- `rust-sexpr-l1-review.html` - accepted parser-only corpus parity review and
  the remaining performance and memory gates before typed-reader expansion.
- `rust-sexpr-phase1-measurement.html` - accepted named-corpus release timing
  and peak-memory method, Windows evidence, and retained Linux ratification.
- `rust-sexpr-phase1-promotion.html` - accepted parser correctness, safety,
  API-boundary, and deferred-performance decision authorizing typed readers.
- `rust-footprint-phase2-slice.html` - accepted first source-backed footprint
  reader and unknown-form-preserving focused writer boundary.
- `rust-footprint-plotter-phase2-slice.html` - review-ready shared plotter
  operation vocabulary plus non-text footprint graphics, standard pads,
  patterned strokes, drills, generated contracts, native, and WASM boundary.
- `rust-symbol-plotter-phase2-slice.html` - library-symbol body geometry, body
  text, and pin geometry/labels using the shared plotter operation vocabulary.
- `rust-symbol-library-phase2-slice.html` - typed symbol-library iteration and
  source-preserving write-back boundary.
- `rust-phase2-boundary-review.html` - review packet for the first typed native
  boundary, the completed WASM feasibility proof, and the native-first
  continuation decision.
- `rust-compiled-schematic-graph-phase4-contract.html` - accepted TypeSpec
  authority, generated transport projections, and deterministic identity
  vectors for the Phase 4 schematic compiler.
- `rust-compiled-schematic-graph-phase4-native.html` - native deterministic
  identity allocation and linear semantic graph validation over generated DTOs.
- `rust-source-bundle-phase4-slice.html` - named byte ownership, portable path
  validation, one-scan schematic definitions, and repeated hierarchy occurrences.
- `rust-schematic-connectivity-phase4-slice.html` - typed schematic connection
  carriers and deterministic 100-nm-grid wire connectivity over source bundles.
- `rust-schematic-writer-phase4-slice.html` - exact owned schematic writeback,
  transactional placed-symbol property edits, and promoted semantic reparse.
- `rust-worksheet-phase4-slice.html` - source-ordered modern and legacy
  worksheet semantics, bounded lazy item decoding, and exact owned writeback.
- `rust-project-phase4-slice.html` - insertion-ordered project JSON semantics,
  full restored-corpus parity, and transactional exact/canonical writeback.
- `rust-netlist-phase4-slice.html` - native resolved netlist model, project
  net-class enrichment, and bounded KiCad version-E S-expression output.
- `rust-phase4-exit-audit.html` - explicit accepted-corpus, compiler,
  writer, malformed-graph, and netlist/compiled-graph Phase 4 exit mapping.
- `rust-kicad-version-compatibility.html` - exact stable-release format and
  operation evidence matrix policy plus opt-in nightly observation rules.
- `rust-font-text-contracts.html` - TypeSpec-owned out-of-band font bundles,
  deterministic selection, and independently attributable shaping and outline
  oracle records.
- `rust-native-text-shaping.html` - accepted native HarfRust shaping parity,
  fixed records, resource policy, and explicit version-bound flag evidence.
- `rust-native-font-outlines.html` - accepted bounded native TTF/gvar/CFF
  outline extraction and deterministic FontTools parity evidence.
- `rust-native-render-cache.html` - accepted native shaping, outline, curve
  decomposition, placement, and bounded KiCad cache-parity ladder.
- `rust-board-plotter-phase5-slice.html` - accepted bounded native board
  text, text-box, table, five-style dimension, and embedded-footprint
  Plotter-IR parity with
  explicit outline-bridge deferrals and independent resource ceilings.
- `rust-schematic-plotter-phase5-slice.html` - accepted bounded native
  schematic page-header, worksheet, and connectivity Plotter-IR foundation
  with a distinct strict generated document contract.
- `rust-schematic-annotations-phase5-slice.html` - accepted bounded native schematic
  labels, netclass flags, text, and text-box Plotter-IR with explicit drawing
  settings and deterministic caller-supplied font metrics.
- `rust-schematic-graphics-phase5-slice.html` - accepted bounded native schematic
  graphics, rule-area, embedded-image, and table Plotter-IR with strict image
  decoding and shared deterministic font resources.
- `rust-schematic-symbols-phase5-slice.html` - accepted bounded placed schematic
  symbols, typed pin ownership, occurrence fields, transforms, DNP rendering,
  and overlap overplots with deterministic font resources.
- `rust-schematic-sheets-phase5-slice.html` - accepted bounded hierarchical
  sheet boxes, typed pin ownership and decorations, visible fields, and DNP
  rendering with deterministic font resources.
- `rust-phase5-exit-audit.html` - accepted inventory and bounded closure order
  for the remaining native text, footprint, symbol, board, and schematic
  Plotter-IR surfaces before the native application phase.
- `kicad-stroke-webfont.html` - ownership, licensing, generation, and package
  contract for the KiCad Newstroke webfont bundle.
- `../guides/project-workflows.html` - user-facing workflow and read-path
  guidance for choosing full model, projection, targeted reader, project, IR,
  and SVG APIs.
- `quality-signoff-status.md` - current release-gate status and quality-tool
  ratchet plan.
- `kicad-plotter-ir.html` - canonical JSON rendering IR contract reference
  for scene conversion, SVG rendering, and validation.
- `../requirements/2026-07-17-performance-optimization-requirements.html` -
  parser/projection performance optimization requirements, measured evidence,
  and release gates for the `2026.7.17` release.
- `../requirements/2026-07-16-public-issue-requirements.html` - public-issue
  requirements and acceptance evidence for the `2026.7.16` release.
- `library-megamaid-extraction.html` - KiCad project library extraction,
  metadata, model repair, and test ownership design.
- `sexpr-projection-parser.html` - generic S-expression form-span selection
  design for fast project scans and lightweight projections.
- `api/` - public API class and major-interface design docs.

Design-doc and test-ownership signoff is defined in
`docs/adrs/ADR-003-design-doc-and-test-ownership-signoff.md`.
