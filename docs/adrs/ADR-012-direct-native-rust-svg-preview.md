# ADR-012: Direct Native Rust SVG Preview Boundary

## Status

Accepted

## Date

2026-09-01

## Context

The frozen Phase-5 Plotter-IR contracts and Phase-6 native SVG transport can
render footprints, library symbols, boards, and schematic pages. Reusable Rust
consumers still have to copy assembly code from the WASM adapter or construct a
JSON-valued native request. Alexandria, the Rust successor to lib-0cruncher,
needs linked-library previews from an exact pinned Git revision without Python,
WASM, an executable sidecar, or a JSON bridge.

## Decision

`kicad-monkey-core` owns four bounded, typed projectors into the existing a0
contract documents. It also owns complete schematic-page composition from a
`SourceBundle` and a caller-selected concrete occurrence. A source filename is
not a sufficient selector when one child source occurs more than once.

`kicad-monkey-svg` owns a validated `SvgRenderContextA1`, explicit and fitted
viewport policies, one effective-operation visibility stream, bounds, and four
family-specific typed render functions. The typed path traverses generated
contracts through exhaustive typed adapters; it does not materialize
`serde_json::Value`. Each original generated operation container is preflighted
against caller ceilings before bounded normalized geometry is materialized.
The frozen native a0 function remains an adapter and the WASM/native crates
remain dependency leaves.

The first profile remains `plotter-base-a0`. Viewer controls are explicit and
bounded. They do not claim the established Python `oracle` or `enriched`
semantics. PCB and footprint layer visibility distinguishes exact pad copper
membership, resolved via apertures, via drill spans, plated through-board pad
drills, and NPTH physical holes. Bounds and SVG emission consume the same
filtered stream and effective styles.

Alexandria initially pins every directly used Monkey crate to one exact Git
`rev` and commits its resolved lockfile. This repository proves Git resolution
with a temporary external consumer; it does not commit a self-referential
future revision.

## Consequences

- Core and SVG remain usable without WASM, native transport, Cruncher, or GUI
  dependencies.
- Frozen a0 wire schemas and default SVG vectors do not change.
- New context, fit, and visibility behavior is additive and fail-closed.
- Generated family operation unions are reconciled by exhaustive typed
  adapters, so adding a union arm becomes a compile/test failure rather than a
  silently drifting manual mapping; preflight runs on the generated containers
  before any adapter-owned point/cache vectors are allocated.
- Alexandria owns product themes, UI state, cache policy, and GUI interaction;
  Monkey owns truthful source identity, basic presentation controls, and
  deterministic SVG.
- Durable details and test ownership live in
  `docs/design/rust-direct-svg-preview.html` and
  `docs/requirements/2026-09-01-native-rust-svg-preview.md`.

## 2026-09-02 browser-correctness amendment

The original implementation made the typed direct renderer byte-identical to
native a0 by writing nanometres directly into SVG user-unit tokens. That is not
a valid browser-preview compatibility strategy: Chromium clamps very large CSS
font sizes before later geometric transforms can compensate. Issue #78 was
therefore reopened.

Typed direct rendering now keeps all public geometry and viewport metadata in
nanometres but serializes SVG dimensions, coordinates, paths, transforms,
strokes, dashes, text sizes, and images in millimetres. The frozen native a0
transport alone uses its retained compatibility serializer so its
request/result schema, 29 successful SVG hashes, and error order do not change.
The public Rust `render_svg` adapter continues to return typed direct-renderer
bounds, warnings, metrics, and structured errors. WASM remains a typed
projection leaf and does not render SVG.

Uncached browser-font text honors both nominal X and Y font sizes and receives
deterministic robust estimated fit bounds plus a structured warning instead of
invalidating all bounds. Retained contour geometry remains exact. Cruncher PCB
and schematic production now consume the typed browser-safe renderer and
enrichment no longer applies a second scale. Pinned Chromium and pinned-Git
consumer gates own the browser and downstream contracts.
