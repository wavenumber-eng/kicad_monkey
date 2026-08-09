# ADR-010: Compiled Schematic Graph And Occurrence Policy

## Status

Accepted

## Date

2026-08-09

## Context

Downstream schematic visualization needs occurrence-scoped connectivity and
drawing evidence that cannot be recovered safely from flattened nets. KiCad
also resolves DNP, BOM, board, and simulation policy across the complete sheet
path, while older package code inspected only an immediate parent or omitted
the sheet state. Public issues #36, #37, #38, #41, and #42 exposed these related
compiler and Plotter IR gaps.

The downstream generic graph is the governed vocabulary, but `kicad-monkey`
must stay independently installable and cannot depend on a consumer model.

## Decision

`KiCadDesign.to_json()` embeds an accepted a0, variant-neutral compiled graph.
Its transport row names, type tokens, identity address semantics, reference
rules, and validator failure boundary mirror the governed generic graph. The
package keeps local typed DTO/allocator code and proves parity with shared
golden vectors and live cross-package tests.

Canonical unit-occurrence identity selects the complete realized
`sheet_path_uuids` path before a reusable placement UUID. This makes nested
reuse unambiguous and keeps an existing occurrence stable when a reused sibling
is added or removed. Terminal-bearing local-net identity derives from sorted
canonical terminal refs. Mutable names, sequential codes, drawing UUIDs, and
bare reused UUIDs are not semantic identity.

The graph is complete before assembly policy. DNP, BOM, board, simulation, and
variant state remain in the Design sidecar and existing netlist/variant
surfaces. One parent-first occurrence walker folds effective sheet policy
through all ancestors without pruning the graph.

The internal netlist model retains effectively off-board descendants so Design
JSON and policy consumers can inspect them. The KiCad S-expression exporter
omits those components and their net nodes, matching `kicad-cli`'s board-netlist
surface. This separates complete source evidence from format-specific export
filtering.

Directive markers, schematic rule areas, and DNP hierarchical sheets use
existing generic Plotter IR operations with source attribution. Recorder probes
already contained the needed line, circle, and polygon operations; the defect
was missing source-model-to-IR implementation, not missing recorder support.

Legacy `indexes` remain derived compatibility data during the downstream
cutover. They are not a second connectivity compiler or identity authority.

## Consequences

- KiCad is the only owner of KiCad source compilation and visible plotting
  facts; downstream packages project and validate rather than recompile them.
- The a0 graph cannot reinterpret identity inputs, cardinalities, or aggregate
  binding semantics. Such changes require a new schema/identity namespace.
- Scalar hierarchy bindings are supported. Aggregate bus/harness bindings are
  deferred to a later versioned contract.
- The durable contract lives in
  `docs/design/kicad-compiled-schematic-graph.md`; Plotter IR details live in
  `docs/design/kicad-plotter-ir.html`; release acceptance lives in
  `docs/requirements/2026-08-09-compiled-schematic-release.md` and package tests.
- Consumers may remove transitional graph synthesis only after they pin a
  released `kicad-monkey` carrying this contract and pass their own portability
  gates.
