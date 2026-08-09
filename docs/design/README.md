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
