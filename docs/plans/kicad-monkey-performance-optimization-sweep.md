+++
type = "plan"
id = "kicad-monkey-performance-optimization-sweep"
status = "active"
title = "KiCad Monkey Performance Optimization Sweep"
created = "2026-07-17"

[[steps]]
id = "branch-and-plan-bootstrap"
title = "Create optimization branch and active dev-std plan"
status = "done"

[[steps]]
id = "primary-performance-research"
title = "Prepare primary performance research findings for independent review"
status = "done"
depends_on = ["branch-and-plan-bootstrap"]

[[steps]]
id = "baseline-benchmark-harness"
title = "Create reproducible public or synthetic performance baselines"
status = "done"
depends_on = ["primary-performance-research"]

[[steps]]
id = "independent-performance-research"
title = "Have an independent agent redo the performance research before implementation"
status = "done"
depends_on = [
  "primary-performance-research",
  "baseline-benchmark-harness",
]

[[steps]]
id = "optimization-candidate-selection"
title = "Select implementation candidates from independent research and benchmarks"
status = "done"
depends_on = [
  "independent-performance-research",
  "baseline-benchmark-harness",
]

[[steps]]
id = "net-resolution-optimization"
title = "Reimplement net resolution lookup optimization from first principles"
status = "done"
depends_on = ["optimization-candidate-selection"]

[[steps]]
id = "projection-span-optimization"
title = "Reimplement nested projection span rebasing optimization from first principles"
status = "done"
depends_on = ["optimization-candidate-selection"]

[[steps]]
id = "sexpr-lexer-tokenizer-optimization"
title = "Reimplement the S-expression lexer/tokenizer hot path in pure Python"
status = "done"
depends_on = ["optimization-candidate-selection"]

[[steps]]
id = "direct-child-span-cache-optimization"
title = "Cache direct child spans per parent and filter by head"
status = "done"
depends_on = ["optimization-candidate-selection"]

[[steps]]
id = "broader-hot-path-sweep"
title = "Investigate and implement additional parser/projection hot-path optimizations"
status = "done"
depends_on = ["optimization-candidate-selection"]

[[steps]]
id = "behavior-and-contract-signoff"
title = "Verify behavior, contracts, and public API stability after optimizations"
status = "done"
depends_on = [
  "net-resolution-optimization",
  "projection-span-optimization",
  "sexpr-lexer-tokenizer-optimization",
  "direct-child-span-cache-optimization",
  "broader-hot-path-sweep",
]

[[steps]]
id = "performance-signoff"
title = "Record measured performance impact against reproducible baselines"
status = "pending"
depends_on = ["behavior-and-contract-signoff"]

[[steps]]
id = "design-doc-intent-audit"
title = "Audit ADRs, design docs, requirements, and release notes against accepted optimizations"
status = "pending"
depends_on = ["performance-signoff"]

[[steps]]
id = "test-runtime-impact-audit"
title = "Audit test coverage and runtime impact for the optimization effort"
status = "pending"
depends_on = ["performance-signoff"]

[[steps]]
id = "closeout-artifacts"
title = "Move durable decisions and results into ADRs, design docs, requirements, and release notes"
status = "pending"
depends_on = [
  "design-doc-intent-audit",
  "test-runtime-impact-audit",
]

[[steps]]
id = "external-review"
title = "Obtain external review before release preparation"
status = "pending"
depends_on = [
  "design-doc-intent-audit",
  "test-runtime-impact-audit",
  "closeout-artifacts",
]

[[exit_criteria]]
id = "ec-no-direct-pr-acceptance"
title = "Contributor PRs are used as research input only; implementation is independently rewritten on this branch"
status = "pending"

[[exit_criteria]]
id = "ec-independent-research-complete"
title = "A separate agent independently reproduces the research and checks for additional optimization targets"
status = "pending"

[[exit_criteria]]
id = "ec-public-baselines"
title = "Performance baselines are reproducible with public corpus data or synthetic fixtures"
status = "pending"

[[exit_criteria]]
id = "ec-behavior-preserved"
title = "Optimized paths preserve parser, projection, source-span, and net-resolution behavior"
status = "pending"

[[exit_criteria]]
id = "ec-performance-impact-recorded"
title = "Measured before/after performance impact is recorded for each accepted optimization"
status = "pending"

[[exit_criteria]]
id = "design-doc-intent-audit"
title = "ADRs, design docs, requirements, and release notes match accepted optimized behavior"
status = "pending"

[[exit_criteria]]
id = "test-runtime-impact-audit"
title = "Test additions and runtime impact are reviewed with recorded evidence"
status = "pending"

[[exit_criteria]]
id = "ec-signoff-green"
title = "L0/L99/dev-std signoff and relevant corpus or benchmark checks pass"
status = "pending"

[[exit_criteria]]
id = "ec-closeout-docs"
title = "Durable decisions and release-facing status are moved out of this active plan before release"
status = "pending"

[[exit_criteria]]
id = "external-review"
title = "External review is complete before any release preparation or publish authorization"
status = "pending"
+++

# KiCad Monkey Performance Optimization Sweep

Status: active planning and research gate

This plan tracks a `kicad_monkey` optimization effort focused on PCB parser and
projection performance. It is a working artifact only. Per ADR-003 and
`AGENTS.md`, durable results must move into ADRs, design docs, requirements,
release notes, contracts, or tests before release, and this plan must be
deleted at closeout.

## Strategy

Issues #16 and #17 and PRs #18 and #19 are treated as external research inputs,
not implementation to accept directly. Do not merge, cherry-pick, copy, or port
the public PR code suggestions; accepted optimizations must be independently
rewritten on `feature/performance-optimization-sweep` after primary research is
recorded and a fresh independent research pass confirms the hot paths, checks
for additional optimization opportunities, and validates reproducible
baselines.

No implementation slice should start until `primary-performance-research`,
`baseline-benchmark-harness`, `independent-performance-research`, and
`optimization-candidate-selection` are complete. Reviewer approval of
individual implementation PRs is not release authorization; release preparation
requires the `external-review` gate and explicit publish authorization.

## Initial Research Snapshot

The following notes capture the first-pass reconnaissance only. They should be
reproduced independently before implementation.

| Input | Claim | First-pass observation |
| --- | --- | --- |
| Issue #16 / PR #18 | Cache PCB net ordinal/name maps during net resolution instead of rebuilding maps per net-bound object. | Synthetic net-heavy board showed full parse best time improving from 0.2996s to 0.2153s and projection segments/vias from 0.3505s to 0.2601s. |
| Issue #17 / PR #19 | Rebase nested PCB projection source spans by arithmetic instead of whole-file prefix newline scans. | Synthetic nested-model board showed model span attachment best time improving from 0.5116s to 0.3595s while preserving checked start line/column. |
| PR #18 + PR #19 combined | The two optimizations should compose cleanly. | Temporary merge-tree and throwaway combined worktree showed a clean textual merge and combined synthetic best times of 0.2142s, 0.2612s, and 0.3579s for the same probes. |

Current GitHub workflow state at first pass: PR #18 and PR #19 CI and PR
hygiene runs were `action_required`, so there was no remote CI evidence to use
as acceptance evidence. Both contributor PRs also included attribution text that
should not be copied into this branch.

## Independent Research Findings

The second-agent research pass is recorded in
`docs/plans/logs/2026-07-17T081739-0400.md`. Candidate selection should treat
that log as the controlling research update over the first-pass cProfile
interpretation.

Corrections from the independent pass:

- Use wall-clock microbenchmarks for candidate selection. cProfile inflates
  pure-Python scanner/lexer frames more than C-level `str.count` and dict work,
  so the raw profile text is useful for call paths but not final ranking.
- KiCad v10 boards can omit the top-level net table and carry name-only
  `(net "GND")` references. Net-map caching yields little or no benefit on
  those boards and behavior tests need a v10 name-only fixture.
- Nested span line/column rebasing is the highest value-per-effort first
  optimization. On 4-ch, `str.count("\n", 0, offset)` accounts for most of the
  nested metadata wall time.
- The S-expression lexer/tokenizer is the largest total-leverage candidate for
  full parse and broad projection hydration. It should be a first-class slice,
  not only part of the broader sweep.
- Direct child-span caching per parent is real, cheap, and second-order.
- Net-map caching remains safe and useful, but its average corpus impact is
  smaller than the first-pass issue data implied.

Accepted candidate order after the independent pass:

1. Nested projection source-span rebasing.
2. Pure-Python S-expression lexer/tokenizer hot-path rewrite.
3. Direct child-span cache per parent with head filtering.
4. Net lookup map reuse for v8/v9-style net tables plus the
   `resolve_net_ref()` double-table-build fix.
5. Broader hot-path sweep only after the first four slices land or are
   explicitly rejected.

## Goals

- Establish reproducible baseline timings for large-board PCB parse and
  projection workflows using public corpus fixtures or synthetic boards.
- Reproduce and re-evaluate the net lookup and nested span rebasing hot paths
  before coding.
- Search for additional high-impact parser/projection hot paths before
  narrowing implementation scope.
- Reimplement accepted optimizations from first principles on this branch.
- Preserve public behavior, promoted API contracts, JSON/corpus contracts, and
  source metadata semantics unless a later reviewed slice explicitly changes
  them.
- Record measured performance impact in release-facing artifacts before
  release.

## Non-Goals

- Do not merge, cherry-pick, copy, or port contributor PRs #18 or #19.
- Do not publish a release from this plan branch.
- Do not change promoted public API exports as part of a private internal
  optimization unless a separate design/contract update is approved.
- Do not use proprietary board data as the only evidence for a speedup.
- Do not introduce native extensions, new runtime dependencies, or format
  contract changes in this effort. The lexer work is a pure-Python rewrite
  first; native implementations are deferred to a future discussion after
  measured Python results exist.

## Research Questions

- Where does time go for `KiCadPcb.from_file()` on large public or synthetic
  PCB files after the projection-parser work already in `2026.7.16`?
- Where does time go for `KiCadPcbProjection.from_file()` followed by common
  family hydration calls such as `footprints()`, `pads()`, `segments()`,
  `vias()`, `zones()`, and `model_references()`?
- How much work comes from repeated S-expression tree scans, repeated net map
  construction, nested span rebasing, source text slicing/parsing, object
  allocation, and IR/render handoff paths?
- Which measurements are stable enough to guard as regression tests, and which
  should remain advisory research notes?
- What public corpus fixture or synthetic generator gives a useful benchmark
  without shipping proprietary board content?

## Candidate Hot Paths

The independent research pass should at least inspect:

- net reference resolution for pads, zones, segments, vias, and arcs;
- projection source-span rebasing for nested pads and model references;
- repeated `find_all_elements()` traversals during full PCB parse;
- projection top-level span indexing and direct-child span caching;
- S-expression lexer/tokenizer throughput, especially per-span projection
  hydration and full-board parse;
- repeated `span.text()` slicing and `span.parse()` work for nested projection
  families;
- `NetRef.resolve_name()` / `resolve_ordinal()` call patterns;
- downstream IR/render entry points only after parser/projection costs are
  understood.

## Implementation Constraints

- Public issue and PR discussion may inform problem statements, measurements,
  and tests, but implementation code must be derived from local analysis and
  written in this branch.
- Optimizations must preserve `NetRef` fallback behavior:
  ordinal-only refs resolve names when present, name-only refs resolve ordinals
  when present, and unresolved refs remain unresolved.
- Projection source metadata must preserve exact `start_offset`, `end_offset`,
  `line`, `column`, `end_line`, `end_column`, `source_text()`, and
  `source_sexp()` behavior.
- Projection remains a read-oriented interface; if caching assumes immutable
  net tables or source text, that assumption must be documented in design notes
  or guarded in code.
- Behavior tests must include both v8/v9-style top-level net tables and v10
  name-only object net references.
- Pad and model source-span tests must assert that index-matched spans
  correspond to the matched object text, not only that a span exists.
- Tests must use public corpus fixtures or synthetic inputs.
- Any durable public contract or behavior change requires matching design,
  contract, and conformance updates in the same slice.

## Validation Plan

Research validation:

- independent agent review of #16, #17, #18, and #19;
- profiling or timing records for `main` before implementation;
- before/after synthetic or corpus benchmarks for each accepted optimization;
  final evidence should use at least three timing rounds;
- review note identifying additional candidates considered and rejected.

Publicly citable baseline evidence should use synthetic fixtures, WREN, and
Jumperless. The 4-ch backplane and Speedy boards can remain local/internal
research references but should not be the only release-facing evidence.

Implementation validation:

- focused L0 tests for each optimized path;
- behavior parity tests for full-board parse and projection hydration;
- source-span assertions including offsets, start line/column, and end
  line/column;
- net-resolution assertions for pads, segments, vias, arcs, and zones;
- `uv run dev-std audit . --format json`;
- `uv run --extra test python tests/rack.py run L0_foundation`;
- `uv run --extra test python tests/rack.py run L99_signoff`;
- broader Rack/corpus checks if shared parser behavior changes.

## Closeout Expectations

Before release preparation, close this plan by:

- moving durable performance decisions into ADRs or design docs;
- recording benchmark results in a release-facing note;
- adding release notes for accepted optimizations;
- deleting this active plan and any transient research records that are not
  intended to ship;
- obtaining external review and explicit release authorization.
