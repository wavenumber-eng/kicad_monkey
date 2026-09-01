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
contracts through one exhaustive borrowed adapter; it does not materialize
`serde_json::Value`. The frozen native a0 function remains an adapter and the
WASM/native crates remain dependency leaves.

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
- Generated family operation unions are reconciled by one exhaustive typed
  adapter, so adding a union arm becomes a compile/test failure rather than a
  silently drifting manual mapping.
- Alexandria owns product themes, UI state, cache policy, and GUI interaction;
  Monkey owns truthful source identity, basic presentation controls, and
  deterministic SVG.
- Durable details and test ownership live in
  `docs/design/rust-direct-svg-preview.html` and
  `docs/requirements/2026-09-01-native-rust-svg-preview.md`.
