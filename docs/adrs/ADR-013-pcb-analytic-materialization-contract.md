# ADR-013: PCB Analytic Materialization Contract

## Status

Proposed

## Date

2026-10-13

## Context

Consumers of a board need its physical facts: what material exists, on which
layer of the stack, on which net, and how it is transformed. Viewers,
fabrication writers (Gerber, IPC-2581), DRC and analysis tools, and
simulators all need the same facts. The Plotter IR (ADR-009) describes what
KiCad plots, not the board's material. A copper-only polygon contract would
fix a polygonization tolerance at the boundary and would have to be extended
separately for mask, paste, silkscreen, holes and the stack.

Issue 20 proposed the pipeline `KiCad source -> resolve physical intent ->
analytic materialization -> adapters`, a closed set of analytic shapes with
one composed affine per operation, material operations on physical slices,
holes as their own operations, resolved net assignment, a simple stack, and
selective materialization whose result equals the matching subset of the
complete one.

## Decision

`kicad-monkey-core` owns `board_materialization`, a producer that resolves a
board's physical intent into `kicad_monkey.materialization.a0`, a
TypeSpec-owned contract generated into `kicad-monkey-contracts` like the other
native contracts.

- Geometry is analytic: `disk_2d`, `rect_2d`, `rounded_rect_2d`,
  `chamfered_rect_2d`, `trapezoid_2d`, `capsule_2d`, `annulus_2d`,
  `swept_path_2d` and `planar_region_2d`, with line, arc and cubic segments,
  each under one `Affine2D`. The producer performs no Boolean operations,
  offsetting or tessellation; removed material is written as subtract
  operations and consumers lower at their own tolerance.
- Operations add or subtract one material on one stack slice, each with a
  resolved net key. Operations on different nets are never merged. Holes are
  separate operations with plating, the conductor slices they span, IPC-4761 protection,
  back-drills and post-machining.
- KiCad object kinds and source identity live only in a separate binding
  table keyed by operation index.
- The stack follows the KiCad stackup; unknown physical values stay absent.
- A request selects products, slices, nets and a region. A selective result
  equals the complete result filtered to its scope; results are
  byte-deterministic.
- `kicad-monkey-materialization-pack` defines the canonical packed encoding:
  a JSON header and chunked columns per (slice, tile). It depends only on the
  contracts crate, so readers need no KiCad parser.
- Rules that KiCad resolves (mask and paste margins, conditional flashing,
  via protection, net classes, differential pairs) follow KiCad's own
  source, and cases not modelled are reported as diagnostics.

Native and WASM transport operations and a Python API are deferred until the
contract is agreed. The design document is
`docs/design/rust-pcb-analytic-materializer.html`; the contract summary is
`docs/contracts/materialization.md`.

## Consequences

- One model serves every consumer; adapters lower it instead of
  reinterpreting KiCad objects.
- The contract is additive to existing contracts; no frozen a0 schema
  changes.
- Consumers own Boolean resolution and lowering, and must not assume a
  tolerance from the producer.
- Facts KiCad does not store (plating thickness, plated-edge geometry, body
  height, per-region rigid-flex stackups) stay absent rather than guessed.
- Changes to the contract after agreement follow the usual versioning: new
  optional fields are additive, anything else is a new version.

## Alternatives Considered

- A copper-only polygon contract: faster to define, but lossy and not
  extensible to the rest of the board.
- Resolving Booleans in the producer: fixes one tolerance and loses
  structure that DRC and analysis use.
- KiCad object kinds in the geometry: couples consumers to KiCad's authoring
  model.
