# KiCad Library Extraction Commands Plan

Status: active implementation notes

This plan tracks the current quick-iteration work on `project-lib`,
Lib Cruncher import extraction, `megamaid`, and related library extraction
support. Per WN development standards, this file is temporary: final decisions
should move into code, design docs, ADRs, and tests before release.

## Current Focus

- Use the new `kicad_monkey` generic S-expression projection parser in the
  library and health workflows before releasing the next `kicad_monkey`
  version.
- Iterate on `project-lib` behavior for project-local symbol and footprint
  library generation, local library table registration, and opt-in source
  relinking.
- Split the Lib Cruncher import-oriented extraction workflow from the broader
  `megamaid` project-dissection workflow.
- Defer broad signoff and exhaustive regression expansion while command
  behavior is still changing quickly.

## Target Commands

Keep three workflows distinct:

- `project-lib`: metadata-preserving project-local KiCad library export. This
  creates local symbol and footprint libraries from the current project,
  registers the local `sym-lib-table` and `fp-lib-table` entries, embeds
  resolvable 3D models into the generated footprint library when requested or
  by final default policy, and provides an explicit dry-run/patch path to
  relink schematic symbols and PCB footprints to the generated local libraries.
  Source mutation must be opt-in and test-covered.
- `lib-extract` / `library-extract`: import-oriented export for Lib Cruncher
  and Alexandria library-system ingestion. This strips project-specific KiCad
  property metadata out of symbol and footprint files, writes reusable raw CAD
  assets, and writes JSON metadata suitable for a future Lib Cruncher import
  tool. The JSON should align with the `alx.part.a0` / `alx.cad_library.a0`
  direction: part identity, raw fields, canonical fields, symbol asset refs,
  footprint asset refs, and optional 3D model refs or payload files. This mode
  should optionally extract 3D models.
- `megamaid`: aggressive project dissection, analogous to Altium Cruncher's
  `megamaid`. This should include the library extraction outputs plus broader
  project artifacts: design JSON, netlist JSON, KiCad S-expression netlists,
  schematic SVGs, PCB review SVGs, BOM/PnP artifacts when those workflows are
  stable, all embedded file payloads, embedded 3D models, schematic images, PCB
  images, worksheet bitmaps, and a comprehensive manifest.

Do not collapse these into one default behavior. `megamaid` may reuse the
library extraction primitives, but it is not just an alias for `lib-extract`.

## Core Parser Dependency

The core parser slice now lives in `kicad_monkey`, not in command-local
scanners in `kicad_cruncher`. The `kicad_monkey` S-expression projection API
provides generic form-span selection for PCB, schematic, symbol library, and
footprint library files before full OOP parsing.

`kicad_cruncher` should consume that API for:

- `health` asset and model-reference scans;
- `project-lib` project-local library extraction;
- `lib-extract` library-ingestion extraction;
- `megamaid` full project dissection, including all embedded payload discovery;
- any quick inventory needed before full design review or SVG conversion.

## Current Gaps

- `project-lib` currently creates libraries and updates library tables, but it
  does not yet relink source schematic symbols or PCB footprints to the local
  libraries.
- `lib-extract` is now a first-class Lib Cruncher import-oriented workflow,
  with `library-extract` as its alias. `megamaid` is now a separate broader
  project-dissection workflow.
- `library_extraction.json` now has a documented
  `kicad_cruncher.library_extraction_bundle.a0` contract. The emitted
  `raw_fields` maps are the raw KiCad parameter/property bags, and
  `canonical_fields` carries case-insensitive derived aliases such as `mpn`,
  `mfg`, `value`, `description`, and `cad-reference` for future import tools.
- Embedded STEP/STP model extraction exists, footprint generation can embed
  resolvable external STEP/STP models, and `megamaid` now writes a general
  `embedded_assets/` export plus a nested `design_review/` bundle with design
  JSON, netlist JSON, KiCad S-expression netlist, schematic SVGs, and PCB
  review SVGs. Continue hardening edge cases as new KiCad payload containers
  are found.
- The missing embedded-asset surface should cover at least:
  - board-level `embedded_files` payloads, including non-model files;
  - footprint-level `embedded_files` payloads;
  - schematic `(image ...)` payloads;
  - PCB `(image ...)` payloads;
  - worksheet `(bitmap ...)` payloads from `.kicad_wks` files.

## Deferred Completion Pass

Before release, audit every new or changed command in this feature area:

- `project-lib`
- `lib-extract` / `library-extract`
- `megamaid`
- `health`
- aliases that route to those commands

The audit should verify:

- command names, aliases, help text, and default output layout;
- workflow boundaries between project-local export and Lib Cruncher
  import-oriented export, and between Lib Cruncher export and aggressive
  `megamaid` project dissection;
- manifest schemas, counts, and relative paths;
- model extraction defaults and explicit model-export options;
- embedded file/image/bitmap extraction defaults and manifest entries;
- symbol extraction semantics for metadata-preserving and cleaned flows;
- footprint dedupe semantics for project-local and library-ingestion flows;
- project `sym-lib-table` and `fp-lib-table` update behavior;
- generated design docs and ADRs match final behavior;
- focused L0/L2/L3/L99 regression coverage exists for the final public
  contracts.
