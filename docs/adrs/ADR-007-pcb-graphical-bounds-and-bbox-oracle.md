# ADR-007: PCB Graphical Bounds And Bbox Oracle

## Status

Accepted

## Date

2026-06-19

## Context

PCB graphical domain objects are used by source-model workflows as well as by
rendering workflows. GitHub issue #2 exposed that split graphical classes such
as `GrLine` could be rendered through polygon conversion but did not all provide
`get_bounds()` support. Existing SVG tests did not catch this because PCB SVG
generation is now IR-backed and computes renderer bounds independently of
`KiCadPcb.get_bounds()`.

Bounds are numerically sensitive. Defining correctness as "whatever polygon
flattening returned" would make source-model behavior depend on renderer
approximation details and would not match KiCad for every shape.

## Decision

`KiCadPcb.get_bounds()` is a source-model API. For board graphical shapes, its
normative behavior follows KiCad-style source geometry bounds:

- compute a shape-specific box from the source geometry;
- expand that box by half the non-negative stroke width;
- normalize the result before returning it.

The graphical mappings are:

| Domain object | KiCad shape behavior |
| --- | --- |
| `GrLine` | segment |
| `GrRect` | rectangle |
| `GrCircle` | circle |
| `GrArc` | arc with cardinal extrema |
| `GrPoly` | polygon points |
| `GrCurve` | KiCad-compatible flattened Bezier point list |

Bezier graphical bounds intentionally follow KiCad's approximated point-list
behavior instead of defining a mathematically exact cubic-extrema box. Oracle
tests may allow a small Bezier-only tolerance when KiCad and `kicad-monkey`
flatten the same curve with slightly different point sets.

Numerical proof uses two complementary lanes:

- L0 analytic tests compute independent expected values for one-shape cases,
  board aggregation, layer filtering, and empty-board behavior.
- L3 oracle tests compare `kicad-monkey` graphical bounds against a patched
  `kicad-cli pcb export bbox` command that emits KiCad-computed per-item
  bounding boxes as JSON.

The patched CLI command emits schema `kicad.pcb_bbox.v1` and is resolved through
the shared KiCad CLI resolver with a `pcb_bbox` capability gate. Machines
without the patched CLI skip oracle tests cleanly; release verification should
run them against a staged or cache-restored bundle.

PCB rendering remains a separate contract. `KiCadPcb.to_svg()` delegates to the
plotter IR renderer, and the removed direct `to_svg_elements()` path should not
be reintroduced as a second canonical PCB SVG implementation.

## Consequences

Source-model bounds tests can catch crashes or numerical drift even when the IR
renderer still produces valid SVG.

Renderer tests continue to prove KiCad SVG compatibility and viewBox behavior,
but they are not the sole proof for `get_bounds()`.

The package keeps a reusable shape-bounds helper for board graphics instead of
deriving all source-model bounds from renderer polygons.

The KiCad CLI bbox oracle is a generated test dependency. Its manifest and
cache metadata live with other tool resolver inputs, and restored bundles must
be checksum-verified before use.
