# ADR-0009: Pure-Rust Canonical CLI

Status: accepted
Date: 2026-08-20

## Context

Phase 6 proved that selected `kicad-cruncher design` facts can be supplied by
the Rust Monkey implementation through a bounded native sidecar. The installed
console script and most design-review assembly nevertheless remain Python.
Adding more Python commands over the sidecar would improve access to individual
facts without reaching the intended product boundary.

The design-review bundle is the highest-value real consumer of the parser,
compiled graph, netlist, Plotter-IR, and SVG work. It is therefore a better
migration driver than isolated graph or netlist facade commands.

## Decision

The canonical `kicad-cruncher` process will become a Rust executable. It will
compose the Rust Monkey crates directly and will not invoke Python or the
`kicad-monkey-native` transport process to perform its core work.

The first complete vertical slice is the existing `design`, `design-review`,
and `dr` workflow. The current Python implementation is retained as an oracle
during migration. Promotion requires parity for the contracted Design JSON,
compiled graph, both netlists, enriched schematic and PCB SVGs, manifest,
README, safe paths, and transactional publication behavior.

The executable and workflow presentation belong to the Cruncher package.
Reusable KiCad parsing, source models, graph and netlist construction,
round-trip behavior, Plotter-IR, and base SVG rendering remain Monkey-owned.
Monkey must not depend on Cruncher.

## Consequences

- New Phase 7 CLI implementation work is Rust-first; no new Python facade
  command is accepted as the target architecture.
- The Python CLI remains available as the full cross-platform command
  distribution and as migration coverage for the promoted design slice.
- A new Cruncher-owned Rust crate is required in the Cargo workspace.
- Platform artifact construction must install the Rust executable without
  workspace paths. The companion universal wheel continues to carry the normal
  public `kicad-monkey` distribution dependency; the native executable composes
  the reviewed Monkey Rust crates at build time.
- Other commands migrate after the design-review vertical slice establishes the
  executable, packaging, and cross-package acceptance pattern.

## Implementation Status

The Windows x64 cutover for `design`, `design-review`, `dr`, and version was
accepted on 2026-08-20. Releases attach hash-bound `kicad-cruncher.exe` and
`kcr.exe` artifacts built from one Rust dispatch implementation. An isolated
install smoke rejects workspace-path leakage, runs a reviewed ZIP-corpus design
without Python in the runtime environment, and verifies the exact archive
before release publication. Commands outside this slice remain explicitly
available through `python -m kicad_cruncher`.
