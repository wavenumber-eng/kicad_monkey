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
- `rust-symbol-plotter-phase2-slice.html` - non-text library-symbol body and
  pin geometry using the shared plotter operation vocabulary.
- `rust-symbol-library-phase2-slice.html` - typed symbol-library iteration and
  source-preserving write-back boundary.
- `rust-phase2-boundary-review.html` - review packet for the first typed
  native/WASM boundary and its unresolved artifact/copy-ownership findings.
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
