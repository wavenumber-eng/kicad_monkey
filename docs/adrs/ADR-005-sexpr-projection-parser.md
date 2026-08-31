# ADR-005: S-Expression Projection Parser

## Status

Accepted

## Date

2026-06-16

## Context

KiCad PCB, schematic, symbol, and footprint files are S-expression documents.
`kicad_monkey` currently supports a full parser path that lexes an entire file,
builds a nested list tree, and then materializes a typed OOP model such as
`KiCadPcb` or `KiCadSchematic`.

That full path is appropriate for round-trip editing, SVG and plotter-IR
conversion, netlisting, geometry operations, and design-review workflows. It is
unnecessarily expensive for workflows that only need a selected subset of
forms, such as:

- BOM and PnP extraction from PCB footprints;
- 3D model reference health checks;
- project-local symbol and footprint extraction;
- quick route/netclass inventories;
- schematic symbol or sheet metadata extraction.

On a large real-world 4-ch backplane board, full PCB parsing takes roughly 11
seconds, while scanning top-level footprint spans without materializing the
full tree takes well under a second. Command-local regex scanners can exploit
that fact, but they duplicate KiCad syntax handling and do not form a reusable
foundation.

## Decision

`kicad_monkey` will add a generic S-expression projection parser layer. The
projection parser selects complete S-expression forms by head, path, and depth
before the full nested list tree or typed OOP model is materialized.

The new layer is generic to KiCad S-expression files. It does not know about
PCB footprints, schematic symbols, model references, or command workflows.
Typed projection helpers may be built above it, but the first public boundary
is the source span and selector API.

The existing full parser and OOP models remain the authoritative edit and
round-trip path. Downstream tools should choose projection parsing when they
need source selection or lightweight summaries, and full OOP parsing when they
need complete object behavior.

## Consequences

The parser stack becomes explicitly layered:

1. `kicad_sexpr` projection scanner for source spans and selected forms.
2. Lightweight typed projections for workflow-specific summaries.
3. Existing full S-expression tree parser.
4. Existing typed OOP document models.

Downstream command-line, BOM, PCB, and visualization preflight tools should
prefer projection parsing for narrow scans.

Projection parsing must be covered by parser-only tests for KiCad syntax edge
cases: nested forms, quoted strings, escaped quotes, comments, source offsets,
path matching, and selected-form reparsing.

Future native-code acceleration can be considered if the projection scanner is
still too slow after Python implementation, but a native rewrite of the typed
OOP model is not part of this decision.

## 2026-07-17 Performance Update

The projection parser remains the public source-span layer, but large-board
workloads now make the performance constraints explicit:

- selected spans must preserve exact offsets, line/column metadata, source
  text, and reparsed S-expression behavior even when internal indexes are used;
- line-column rebasing, direct-child span discovery, and PCB net lookup tables
  may be cached as private immutable-source accelerators;
- lexer and projection scanner hot paths remain pure Python for this effort,
  with the lexer using compiled regex token discovery, lazy quoted-string
  unescape, and grouped numeric token classification;
- public GitHub issue and pull-request reports may guide research, but
  implementation code is independently written in this repository.

Future native accelerators, pull-parser architecture, or direct typed parsing
remain separate design decisions and are not part of this accepted update.

## 2026-08-31 Native Rust Performance Update

The public projection boundary remains unchanged. The native Rust
implementation now uses two private, independently measured accelerators:

- the lexer scans structural ASCII and token runs by byte, then uses the
  Unicode-aware path only when exact scalar-column accounting requires it;
- projection frames borrow unquoted heads in memory, retain only the owned
  active head required by streaming input, and derive paths from the active
  frame stack instead of cloning an owned path for every visited form.

Owned `FormSpan` heads and paths are still materialized at selection time, so
the public source-span, selector, ordering, resource-limit, diagnostic, native,
streaming, and WASM contracts do not change. `StructuralIndex` keeps its
specialized indexed path lookup rather than routing exact stored-span queries
through the generic iterator matcher.

The accepted same-host evidence compares exact, independently buildable
commits: B0 `72dea9973eade13034ef043dd62792006fda9722`, lexer L
`beb40f62abab2e6f64f3daaab36f14899114bf8e`, and final P
`697a685ef0843dc0b6a73e8288e1599a049d93d4`. B0 to L improved the
non-collecting lexer median by 2.30x. L to P reduced sparse in-memory
allocation calls from 1,600,631 to 628 and streaming calls from 1,600,632 to
300,630; sparse scan medians improved 5.05x and 2.74x respectively. The full
Speedy native median improved from 7.618 seconds at B0 to 5.028 seconds at P
with structured-artifact and SVG parity.

These results ratify the two internal changes, not portable performance
guarantees. A borrowed or arena-backed public tree, selector tries,
source-order sort removal, `memchr`, direct typed parsing, and a Python lexer
rewrite remain separate decisions requiring their own evidence.
