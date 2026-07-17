# KiCad Cruncher Large-Board Profile Follow-Ups

Date: 2026-07-17

Scope:

- large-board `design`/`design-review`/`dr` profiling
- `kicad-monkey>=2026.7.17` dependency floor refresh
- command-scoped PCB review SVG cache decision
- deferred upstream and API work

Accepted Cruncher-local work:

- `design`/`design-review`/`dr` should reuse a command-scoped cached PCB render
  state while writing per-copper-layer PCB review SVGs.
- The cache must preserve the direct render path's enriched metadata, root
  `viewBox`, width, height, coordinate framing, review styling output, and
  drill/slot record counts.
- Cache lifetime stays within a single command invocation.

Measured implementation evidence:

- 4-ch-backplane, 4 copper layers: direct SVG loop 10.170 seconds; cached SVG
  loop 7.336 seconds; loop ratio 1.39x; total command-model ratio 1.10x.
- Speedy Processing Module, 10 copper layers: direct SVG loop 21.739 seconds;
  cached SVG loop 6.700 seconds; loop ratio 3.24x; total command-model ratio
  1.83x.
- In both measurements, SVG byte totals and drill/slot record counts matched.

Deferred work:

- `design_json` remains the largest measured `design/dr` stage on the 4-ch
  public corpus run. It is intentionally deferred because the design JSON file
  is part of the current command output contract.
- Future work may add an explicit narrower output mode or an upstream
  `kicad-monkey` improvement for design JSON materialization, but existing
  full-output commands must not silently replace full design JSON with
  projection or targeted reads.
- Parser, projection, pull-parser, native tokenizer/parser, and IR/SVG
  materialization changes belong in future `kicad-monkey` work unless Cruncher
  first defines a narrower public command contract.

Validation:

- Focused parity test checks cached PCB review rendering against the direct
  `pcb.to_svg(...)` path for styled SVG equality, drill/slot counts, `viewBox`,
  width, and height.
- Large-board timing evidence is recorded under `rack_results/` during local
  research and summarized in the active plan logs.
