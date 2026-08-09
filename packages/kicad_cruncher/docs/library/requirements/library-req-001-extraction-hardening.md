+++
type = "requirement"
id = "library-req-001-extraction-hardening"
domain = "library"
status = "active"
title = "Library extraction and project-local relinking hardening remains tracked"
created = "2026-07-17"
issue_refs = ["wavenumber-eng/kicad_cruncher#8", "wavenumber-eng/kicad_monkey#25"]
verification_status = "unverified"
design_refs = [
  "docs/design/cli/project-lib.html",
  "docs/design/cli/lib-extract.html",
  "docs/design/cli/megamaid.html",
  "docs/design/cli/health.html",
  "docs/contracts/library_extraction_bundle.a0.schema.json",
  "docs/contracts/source_relink.a0.schema.json",
]
+++

# Library Extraction Hardening

The current durable design docs keep `project-lib`, `lib-extract`,
`megamaid`, and `health` as distinct public workflows. `project-lib` now has
an explicit `--relink-dry-run` and `--relink-sources` path for source schematic
and PCB library-reference relinking, with schematic cache-link validation,
embedded cache-unit name validation, embedded cache-body validation, guarded
cache-link repair, a `source_relink.json` report, and an optional KiCad CLI
before/after ERC hygiene gate for apply-mode validation. The following
obligations recovered from the deleted
`docs/plans/kicad-library-extraction-commands.md` plan remain active:

- Keep source mutation opt-in and test-covered.
- Continue hardening embedded asset extraction as new KiCad payload containers
  are found, including board-level `embedded_files`, footprint-level
  `embedded_files`, schematic images, PCB images, and worksheet bitmaps.
- Before changing release-facing behavior in this feature area, audit
  `project-lib`, `lib-extract`/`library-extract`, `megamaid`, `health`, and
  their aliases for command names, help text, output layout, manifest schemas,
  model extraction defaults, embedded payload manifest entries, symbol
  extraction semantics, footprint dedupe semantics, library-table behavior,
  durable docs, and focused regression coverage.

Completed workflow-boundary decisions and current output contracts remain in
the command design docs and `library_extraction_bundle.a0` schema.
