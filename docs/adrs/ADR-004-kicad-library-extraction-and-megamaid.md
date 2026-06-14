# ADR-004: KiCad Library Extraction And Megamaid Workflow

## Status

Accepted

## Date

2026-06-14

## Context

KiCad projects in active hardware work can contain a mix of HTTP library
parts, project-local symbols and footprints, vendor imports, embedded board
assets, and external `${KICAD*_3DMODEL_DIR}` model references. This is workable
while designing interactively, but it is not enough for repeatable handoff,
canonical library promotion, or long-term archival.

The canonical library flow needs to extract verified project assets into forms
that downstream library tooling can import into part records. The project
handoff flow also needs local project libraries that preserve useful metadata
and can be inspected or edited by an engineer before any schematic or PCB
relinking.

KiCad has useful primitives for HTTP libraries, folder-based symbol libraries,
footprint libraries, embedded board files, and CLI validation. It does not
provide a single project decomposition command equivalent to the existing
Altium `megamaid` workflow.

## Decision

`kicad_monkey` owns the KiCad-format primitives for project asset scanning,
symbol extraction, footprint extraction, metadata stripping, embedded model
inspection, and 3D model payload repair.

`kicad_cruncher` owns the user-facing workflow commands. `megamaid` composes
the primitives into a cleaned downstream library-ingestion bundle;
`project-lib` composes the same primitives into a metadata-preserving
project-local review bundle.

Downstream library tooling remains the owner of part-record creation and update
semantics. `kicad_cruncher` emits JSON contracts with raw and canonicalized
metadata so that tooling can decide how to create library parts.

The first implementation is non-destructive:

- it does not modify source schematic or PCB files;
- it does not rewrite `sym-lib-table` or `fp-lib-table`;
- it does not relink symbols or footprints;
- it produces extracted libraries, model assets, JSON manifests, and
  diagnostics for inspection.

Two extraction policies are required:

- canonical extraction strips symbols and footprints down to KiCad
  required/default fields plus raw geometry, pin, pad, and model data;
- project-local extraction preserves metadata and creates assets suitable for
  local project editing.

Part identity metadata is preserved in JSON. `mpn` is preferred when present.
`cad-reference` is the CAD key and fallback identity for parts that do not
cleanly map to an MPN. Field aliases and final part-record import behavior
belong to the downstream alias resolver and import flow.

KiCad CLI is an optional validation oracle. When available, tests should use
KiCad-native readers to validate extracted `.kicad_sym`, folder symbol library,
`.kicad_mod`, and `.pretty` outputs. Parser-level assertions remain required so
failures are precise and do not depend solely on KiCad CLI availability.

## Consequences

`kicad_monkey` will need stable scan/extract/strip/model-repair APIs before
`kicad_cruncher megamaid` and `kicad_cruncher project-lib` can become released
workflows.

The 4-ch backplane corpus fixture is the first real-world regression target for
this work because it contains hierarchy, design blocks, off-board sheet
behavior, local libraries, vendor libraries, external 3D model references, and
embedded 3D model payloads.

Future mutation features such as library-table management or schematic/PCB
relinking must be explicit opt-in workflow steps with dry-run behavior and
tests.
