# ADR-010: Projection-First Copper Geometry Document

## Status

Proposed

## Date

2026-07-17

## Context

Polygon consumers currently load a complete `KiCadPcb`, build
`kicad.plotter_ir.a0`, discard unrelated graphics/text records, and reconstruct
copper polygons from plotter operations. That path is appropriate for 2D
rendering but duplicates geometry work for clipping, triangulation, DRC, CAM,
and 3D board visualization.

KiCad 10 also stores board-element nets by name without a top-level ordinal net
table. A geometry contract must therefore treat names as authoritative while
retaining source ordinals only for older files.

## Decision

Add `kicad.copper_geometry.a0` and
`emit_pcb_copper_geometry(source, *, curve_tolerance_mm=0.005)`.

Path inputs use `KiCadPcbProjection.from_file()`. Extraction requests only
layers, stackup, routing, filled zones, and footprints. For path-backed
projection inputs the implementation may use a pads-only footprint parse and
direct filled-polygon nanometre extraction; those are internal optimizations
and are not part of the public contract. `KiCadPcb` and `KiCadPcbProjection`
instances remain accepted for callers that already own either model.

The coordinate contract is:

- integer nanometres;
- board-right X and board-down Y;
- unclosed rings;
- explicit outer and hole roles;
- deterministic family, feature, layer, and net ordering;
- caller-selected positive curve tolerance recorded in the document.

Net names are authoritative. Pre-v10 ordinals are optional compatibility
metadata. Copper layers and nets use dense document-local indexes.

Supported feature kinds are `track`, `track_arc`, `via`, `pad`, and
`zone_fill`. Drill kinds are `via`, `plated_pad`, and `npth_pad`. Drill
records retain round/oval dimensions, centers, plating intent, layer spans,
and pad/component identity needed for cap holes and barrels.

The contract is separate from Plotter IR. It does not construct
`KiCadPlotterDocument` and excludes board graphics, text, images, mask/paste,
3D component models, triangulation, tiling, compression, and downstream
Clipper/GLB protocols.

The implementation builds on the projection performance work landed in
PR #21 (`2026.7.17`).

## Validation

Synthetic tests cover routing, filled zones, supported pad shapes, footprint
placement, multilayer expansion, plated/NPTH and oval drills, KiCad 9 ordinal
nets, KiCad 10 name-only nets, deterministic serialization, JSON Schema, and
projection/full-board parity.

Corpus parity passes on `4-ch-backplane` and `speedy_processing_module`.

On JTYU-OBC (production 160 mm tiles):

| Metric | Current Plotter IR path | Copper document path |
| --- | ---: | ---: |
| Native end-to-end median | 35.507 s | 16.593 s |
| `copper_emit` / IR front half | 28.067 s before slim opts | 10.676 s after |
| Python peak RSS | ~1.6–2.0 GiB | ~0.68–0.71 GiB |
| Source features / barrels | 20,919 / 1,974 | 20,919 / 1,974 |
| GLB bytes | 12.19 MB | 8.04 MB |

A production Prism Docker integration that avoids full `KiCadPcb` hydration
completes the same board in about 67 s under `linux/amd64` emulation, versus
about 100–122 s for the Plotter IR path in the same container.

## Consequences

- Geometry consumers can bypass full-board hydration and Plotter IR.
- The JSON document is reusable and renderer-independent.
- Changing ring semantics, units, ordering, or net authority requires a new
  schema revision.
- Downstream adapters remain responsible for tiling, boolean clipping,
  triangulation, mesh construction, and rendering.
- Plotter IR remains the canonical 2D rendering contract.
