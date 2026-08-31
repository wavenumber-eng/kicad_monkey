# Rust S-Expression Performance Requirements

## Status

Accepted on 2026-08-31 for the native Rust lexer and projection scanner.

## Required behavior

- Preserve every exported lexer, tree, selector, span, and resource-limit API.
- Preserve token kinds, source slices, byte offsets, one-based lines,
  Unicode-scalar columns, errors, and exact limit boundaries.
- Return identical source-ordered, owned spans from memory and streaming
  scanners, including quoted, escaped, Unicode, headless, pruned, and KiCad
  teardrop forms.
- Preserve Python-reference, native, WASM, corpus roundtrip, StructuralIndex,
  and Cruncher consumer behavior.
- Do not modify the reviewed corpus archive or authoritative fixture output.

## Required evidence

Acceptance uses clean, exact, independently buildable refs with identical
benchmark harnesses: B0 `72dea9973eade13034ef043dd62792006fda9722`, lexer L
`beb40f62abab2e6f64f3daaab36f14899114bf8e`, and final P
`697a685ef0843dc0b6a73e8288e1599a049d93d4`. Timing probes use alternating
paired order, raw rounds, and a distribution-free 95% median sign interval.
Allocation probes run separately with a known-allocation control and reset the
counter immediately around one warmed scan.

The promotion targets are decision gates rather than portable SLOs: at least
2x lexer improvement; no unexplained regression greater than 5%; at least 90%
and 70% allocation-call reductions for sparse memory and stream scanners; at
least 15% sparse-scan improvement; select-all within 5%; and at least 5%
cumulative native Speedy improvement with artifact parity.

## Accepted results

| Comparison | Result | Decision |
|---|---:|---|
| B0 to L non-collecting lexer | 2.302x median; 2.292x confidence lower bound | pass |
| L to P sparse memory scan | 5.053x median; 5.035x lower bound | pass |
| L to P sparse stream scan | 2.740x median; 2.704x lower bound | pass |
| L to P memory allocation calls | 1,600,631 to 628 (99.96% reduction) | pass |
| L to P stream allocation calls | 1,600,632 to 300,630 (81.22% reduction) | pass |
| L to P select-all medians | 1.043x to 1.188x across tiers/scanners | pass |
| B0 to P native Speedy median | 7.618 s to 5.028 s (1.515x) | pass |

The final Speedy run retained semantic parity for all 35 artifacts and 29
SVGs, did not mutate the reviewed source tree, and measured Rust at 15.44x the
Python median on that run. The three paired B0-to-P Rust ratios were 1.479x,
1.515x, and 1.578x. Full locked Rust tests, Python/Rust shared vectors,
exact resource boundaries, corpus roundtrip/determinism, real Node-hosted
WASM, Clippy, formatting, native Cruncher source-install behavior, Rack L99,
and the repository dev-std audit passed. The CI artifact-manifest release gate
was not locally applicable because its CI-owned manifest was intentionally
absent.

## Test runtime impact

Shared projection vectors and focused boundary cases add only subsecond work
to their existing Python and Rust test binaries. Allocation, paired timing,
large-corpus, and three-round Speedy probes remain explicit advisory/manual
evidence and are not added to ordinary fast Rack execution. Complete corpus
parity and consumer signoff remain intentionally slow release/review gates.

## Deferred work

This acceptance does not authorize a borrowed public tree, arena allocation,
selector tries, final source-order sort removal, `memchr`, direct typed
parsing, or a Python lexer rewrite. Each requires a separately scoped change
and its own consumer and performance evidence.
