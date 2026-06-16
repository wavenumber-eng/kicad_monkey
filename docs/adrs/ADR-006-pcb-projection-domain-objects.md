# ADR-006: PCB Projection Domain Objects

## Status

Accepted

## Date

2026-06-16

## Context

ADR-005 introduced a generic S-expression projection parser that can select
complete KiCad source forms without building a full file tree. The next design
choice is what higher-level PCB projection APIs should return.

Summary-only records would be fast and simple, but they would force consumers
to learn a second representation for footprints, vias, tracks, graphics, and
board metadata. That would reduce reuse across `kicad_monkey`, `kicad_cruncher`,
project health checks, library extraction, and future PCB visualization tools.

The hard consumer requirement is that projected PCB objects can be used in the
same way as objects obtained from `KiCadPcb`.

## Decision

`KiCadPcbProjection` object-family methods return the same public domain object
classes used by the full `KiCadPcb` parser.

Examples:

- `footprints()` returns `Footprint` objects;
- `vias()` returns `Via` objects;
- `segments()` returns `Segment` objects;
- `gr_lines()` returns `GrLine` objects;
- `layers()` returns `Layer` objects.

When a projection is created from a loaded `KiCadPcb`, object-family methods
return the exact board-owned object instances. When a projection is created
from a file, object-family methods hydrate same-type objects from exact source
spans.

Source metadata is kept as sidecar data rather than changing the domain object
types. Callers can recover exact source spans, text, and S-expressions through
projection lookup methods such as `source_span()`, `source_text()`, and
`source_sexp()`.

Inventory helpers may still return helper records when KiCad does not have a
standalone board object class for the item. The first example is
`PcbModelReference`, which joins a footprint-owned `Model` object with parent
footprint and source-span context.

## Consequences

Projection consumers can use normal domain-object APIs and avoid a parallel
summary object model.

The projection implementation must use the same parser factories as the full
board parser. If a future PCB sub-object cannot be hydrated by the same parser
factory, it should not be promoted through the projection API until parser
coverage is added.

Net-bound projected objects must resolve numeric net references the same way as
the full parser. This applies to pads, zones, segments, vias, and route arcs.

Real-board parity tests are required for the stripped 4-ch backplane corpus
board and Speedy processing module corpus board. These tests compare projected
object-family counts and representative fields against a full `KiCadPcb` parse.
