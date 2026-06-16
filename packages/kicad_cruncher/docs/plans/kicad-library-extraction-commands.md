# KiCad Library Extraction Commands Plan

Status: active implementation notes

This plan tracks the current quick-iteration work on `project-lib`, `megamaid`,
and related library extraction support. Per WN development standards, this file
is temporary: final decisions should move into code, design docs, ADRs, and
tests before release.

## Current Focus

- Use the new `kicad_monkey` generic S-expression projection parser in the
  library and health workflows before releasing the next `kicad_monkey`
  version.
- Iterate on `project-lib` behavior for project-local symbol and footprint
  library generation.
- Keep `megamaid` behavior stable until the project-local workflow settles.
- Defer broad signoff and exhaustive regression expansion while command
  behavior is still changing quickly.

## Core Parser Dependency

The core parser slice now lives in `kicad_monkey`, not in command-local
scanners in `kicad_cruncher`. The `kicad_monkey` S-expression projection API
provides generic form-span selection for PCB, schematic, symbol library, and
footprint library files before full OOP parsing.

`kicad_cruncher` should consume that API for:

- `health` asset and model-reference scans;
- `project-lib` project-local library extraction;
- later `megamaid` library-ingestion extraction if a cleaned-flow bottleneck
  remains;
- any quick inventory needed before full design review or SVG conversion.

## Deferred Completion Pass

Before release, audit every new or changed command in this feature area:

- `project-lib`
- `megamaid`
- `health`
- aliases that route to those commands

The audit should verify:

- command names, aliases, help text, and default output layout;
- manifest schemas, counts, and relative paths;
- model extraction defaults and explicit model-export options;
- symbol extraction semantics for metadata-preserving and cleaned flows;
- footprint dedupe semantics for project-local and library-ingestion flows;
- project `sym-lib-table` and `fp-lib-table` update behavior;
- generated design docs and ADRs match final behavior;
- focused L0/L2/L3/L99 regression coverage exists for the final public
  contracts.
