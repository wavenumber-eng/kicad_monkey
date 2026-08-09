# Changelog

## Unreleased

- Extend the bundled CC0 KiCad Newstroke table through U+2BFF, adding Greek,
  mathematical, arrow, and technical-symbol coverage without changing the
  original ASCII glyph strings.
- Add a deterministic KiCad Stroke webfont package with Light, Regular, Bold,
  and italic faces in TTF, OTF, WOFF, and WOFF2 formats, plus CSS, a blue
  phosphor engineering demo, provenance metadata, and package synchronization
  checks.

## 2026.8.1

- Fix zone fill emission: unfilled copper zones now emit a bare `(fill ...)`
  element instead of `(fill no ...)`. KiCad's parser accepts only a bare `yes`
  token inside `fill`, so boards written with the old form failed to load in
  KiCad ("Failed to load board"). Parsing is unchanged; legacy files carrying
  `(fill no ...)` are repaired on re-emit.

## 2026.7.28

- Harden project-local library extraction so project scans ignore KiCad
  `.history`, autosave, and backup folders instead of extracting stale
  schematic or board data.
- Traverse the active schematic hierarchy from the project root sheet and use
  the project-stem board for project-local extraction and asset inventory.
- Add deterministic symbol and footprint output-member maps for downstream
  tools that relink source schematics and PCB footprints to generated local
  libraries, including duplicate footprint-name cases.
- Update the configured dev-std release audit floor to `2026.7.18`.

## 2026.7.17

- Improve large-board PCB parser and projection performance with pure-Python
  S-expression regex-tokenizer changes, projection source-span line-column
  indexing, direct child-span caching, and PCB net lookup reuse.
- Preserve parser, projection, source metadata, and KiCad v10 name-only net
  reference behavior while adding focused L0 and corpus coverage for the
  optimized paths.
- Record public/synthetic benchmark evidence and keep public PRs #18 and #19
  as research inputs only; implementation is independently rewritten.
- Document when to use full model APIs, `KiCadPcbProjection`, targeted readers,
  and render APIs; clarify that PCB render `layers=` filters output after full
  IR construction, with a dedicated project workflow guide.

## 2026.7.16

- Add the `kicad.plotter_ir.a0` JSON Schema and canonical HTML reference for
  the KiCad Plotter IR used by SVG rendering and downstream scene conversion.
- Preserve schematic text hyperlinks through IR `context.hyperlink.href` while
  keeping SVG rendering independent of hyperlink metadata.
- Make KiCad preference setup source-driven from the supplied preference
  directory, including color themes and app-specific JSON files, with neutral
  setup defaults.
- Optimize design JSON component `classification.pin_count` generation with a
  one-pass terminal index while preserving the existing JSON contract.
- Align release signoff with the configured dev-std audit scopes.
- Add durable ADR and requirements closeout artifacts for the public issue work
  and remove transient plan/research notes from tracked release content.

## 2026.6.25

- Add a new-project assembly surface to `KiCadProject`: start a writable blank
  project with `KiCadProject.create(name, directory)`, attach pieces through the
  object model with `add_schematic`, `set_worksheet`, `add_pcb`,
  `add_symbol_library` / `add_footprint_library` / `ensure_library_tables`, and
  write the whole folder with `write_project`.
- Add ADR-008 and update the design docs to make constructor compatibility and
  the `from_*` / `new` / `create` naming split explicit.
- Add `EmbeddedFile.from_worksheet` and `KiCadSchematic.embed_worksheet` for
  packing `.wks` drawing sheets (zstd + SHA-256) as KiCad embedded files.
- Build `KiCadPcb.new()`'s default board entirely through the object model:
  the standard layer set is composed from `Layer` objects and the default
  `setup` block from an s-expression builder, replacing the hard-coded template.
- Model the schematic `(embedded_files)` block so embedded worksheets, fonts,
  and models survive a parse/serialize round-trip (previously dropped on emit).
- Add `KICAD_PAGE_SIZES`, `KICAD_PAGE_DIMENSIONS_MM`, and `kicad_page_size_label`
  as the single source of truth for standard schematic page sizes.

## 2026.6.19

- Fix `KiCadPcb.get_bounds()` for split PCB graphical shapes by adding
  KiCad-style source-geometry bounds for lines, rectangles, circles, arcs,
  polygons, and Bezier curves.
- Add analytic and KiCad-backed oracle tests for graphical PCB bounds,
  including package-local corpus coverage for the 4-channel backplane fixture.
- Add a patched `kicad-cli pcb export bbox` oracle path, manifest wiring, and
  R2-backed restore tooling for the bbox-capable KiCad CLI bundle.
- Keep PCB SVG output canonical through the plotter IR renderer and remove the
  old direct `KiCadPcb.to_svg_elements()` surface.

## 2026.6.18

- Add a generic targeted KiCad object reader for extracting typed schematic,
  symbol, board, footprint, model, and embedded-file objects without first
  materializing a full file model.
- Promote symbol and footprint text extractors onto the targeted reader API
  while preserving object-level return types for local library generation.
- Add reusable parameter alias normalization for canonical MPN, manufacturer,
  value, and description fields in library extraction metadata.
- Fix hidden graphical symbol text serialization so generated symbol libraries
  validate with current `kicad-cli`.
- Extend real-world corpus coverage for library extraction, schematic
  hierarchy, board review, and SVG/IR promotion gates.

## 2026.6.15

- Add KiCad project library extraction primitives for metadata-preserving
  project-local libraries and cleaned library-ingestion bundles.
- Add project asset and 3D model scanning helpers used by higher-level
  `kicad-cruncher` health diagnostics.
- Optimize extraction paths for large KiCad projects by scanning narrow
  S-expression blocks before constructing full model objects.
- Keep KiCad CLI validation helpers available for generated symbol and
  footprint libraries.

## 2026.6.13

- Fix KiCad netlist parity for hierarchical designs with sheet-level board
  exclusion: off-board child sheet contents are omitted while parent-side sheet
  pin nets remain connected.
- Align KiCad S-expression netlist component metadata with `kicad-cli`,
  including shown field text, blank `~` normalization, `libsource`,
  `sheetpath`, `tstamps`, property rows, and multi-unit `units` blocks.
- Improve multi-unit symbol handling with KiCad-like duplicate reference
  suppression, instance timestamp ordering, unit names, common pins, stacked pin
  expansion, and unit pin ordering.
- Add the sanitized `4-ch-backplane` real-world fixture to the packed corpus as
  an active netlist, schematic SVG/IR, and PCB SVG/IR regression case for the
  hierarchy/design-block issue that exposed the old parser drift.
- Refresh strict KiCad CLI oracle coverage for netlist projects and document the
  remaining expected metadata-only xfails separately from structural netlist
  parity.

## 2026.6.10

- Fix pin-name markup rendering in symbol SVG output: `~{...}` overbar,
  `_{...}` subscript, and `^{...}` superscript are now parsed and rendered
  instead of being drawn literally (GitHub issue #1). Markup works for both
  the KiCad stroke font and TTF-faced pin fonts, with bar position, glyph
  scaling, and baseline offsets matching `kicad-cli sym export svg`.
- Align the default symbol SVG theme with KiCad CLI output (body fill,
  outline stroke widths, pin-number color). Custom theme overrides remain
  available and unchanged.
- Add stroke-font markup unit coverage, TTF-face overbar regression cases,
  and strict element-level symbol SVG parity tests against the KiCad CLI
  reference output.
- Pin the staged KiCad CLI oracle builds in `tools/kicad-cli/MANIFEST.toml`
  so test references resolve deterministically instead of by file mtime.
- Refresh the redistributable test corpus archive: add overbar markup
  fixtures (stroke and TTF variants), exclude regenerable runtime products
  (`output/`, `_stage/`, `.kicad_prl`) from packaging and hygiene checks,
  and remove editor/backup debris.

## 2026.6.3

- Publish the 2026-06-03 public package build for downstream KiCad SVG and
  design-review consumers.
- Carry forward the 2026.6.2 enriched PCB/SVG metadata and schematic instance
  API surface as the current audited `kicad-monkey` release.

## 2026.6.2

- Harden PCB SVG rendering against KiCad CLI oracle output, including custom
  pads, NPTH mask apertures, filled polygons, track arcs, stroke widths,
  render-cache fill rules, dimensions, and review-layer naming.
- Add strict PCB SVG structural-oracle tests and promote additional synthetic
  and real-world SVG parity cases.
- Add PCB SVG render profiles and enriched PCB SVG metadata for components,
  pads, vias, tracks, zones, drills, stackup, project variables, and net
  linkage.
- Add enriched schematic SVG metadata with design JSON, view-local net indexes,
  and SVG-to-net linkage for schematic review workflows.
- Normalize design and schematic hierarchy contract revisions to `a0`.
- Add the public schematic hierarchy instance API for enumerating repeated
  sheet instances, parent/child navigation, source-file usage lookup, and
  per-instance schematic IR rendering.
- Add `uv.lock` for reproducible uv-based development and CI workflows.
- Refresh README developer examples for loading designs, extracting netlists,
  rendering SVG, and mutating KiCad model objects.

## 2026.5.31

- First public release of `kicad-monkey`.
- Establish the `2026.5.31` date-versioned package baseline.
- Include the public parser/model/rendering API surface, release signoff gates,
  and redistributable KiCad corpus archive needed for initial `kicad-cruncher`
  integration work.
