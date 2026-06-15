# Changelog

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
