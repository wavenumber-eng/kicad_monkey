+++
type = "plan"
id = "kicad-monkey-project-api-projection-render-design"
status = "active"
title = "KiCad Monkey Project API Projection And Render Design"
created = "2026-07-17"

[[steps]]
id = "branch-and-plan-bootstrap"
title = "Create dependent branch and active dev-std plan"
status = "done"

[[steps]]
id = "current-api-inventory"
title = "Inventory current project, PCB, projection, IR, and SVG APIs"
status = "active"
depends_on = ["branch-and-plan-bootstrap"]

[[steps]]
id = "documentation-guidance-audit"
title = "Audit API documentation and user guidance for full-model versus projected reads"
status = "pending"
depends_on = ["current-api-inventory"]

[[steps]]
id = "render-pipeline-architecture-research"
title = "Research whether file-level IR or SVG APIs can safely use projection or partial materialization"
status = "pending"
depends_on = ["current-api-inventory"]

[[steps]]
id = "native-acceleration-options-research"
title = "Evaluate C, C++, Cython, and pure-Python parser/tokenizer acceleration options"
status = "pending"
depends_on = ["current-api-inventory"]

[[steps]]
id = "independent-research-review"
title = "Have an independent agent redo the API, documentation, render, and native-acceleration research before implementation"
status = "pending"
depends_on = [
  "documentation-guidance-audit",
  "render-pipeline-architecture-research",
  "native-acceleration-options-research",
]

[[steps]]
id = "api-design-decision"
title = "Select API and documentation changes from reviewed research"
status = "pending"
depends_on = ["independent-research-review"]

[[steps]]
id = "durable-design-doc-updates"
title = "Update ADRs, design docs, requirements, and release notes for the selected API direction"
status = "pending"
depends_on = ["api-design-decision"]

[[steps]]
id = "api-guidance-implementation"
title = "Implement accepted documentation, guidance, and public API contract updates"
status = "pending"
depends_on = ["durable-design-doc-updates"]

[[steps]]
id = "file-level-ir-svg-api-implementation"
title = "Implement accepted file-level IR or SVG APIs, if the design decision selects them"
status = "pending"
depends_on = ["durable-design-doc-updates"]

[[steps]]
id = "native-acceleration-followup-decision"
title = "Record whether native tokenizer or parser acceleration remains research-only or becomes a separate implementation plan"
status = "pending"
depends_on = ["native-acceleration-options-research", "api-design-decision"]

[[steps]]
id = "design-doc-intent-audit"
title = "Audit ADRs, design docs, requirements, and release notes against selected API behavior"
status = "pending"
depends_on = [
  "api-guidance-implementation",
  "file-level-ir-svg-api-implementation",
  "native-acceleration-followup-decision",
]

[[steps]]
id = "behavior-performance-signoff"
title = "Validate behavior, API contracts, and performance impact for accepted changes"
status = "pending"
depends_on = [
  "api-guidance-implementation",
  "file-level-ir-svg-api-implementation",
  "native-acceleration-followup-decision",
]

[[steps]]
id = "test-runtime-impact-audit"
title = "Audit test coverage and runtime impact for accepted API or render changes"
status = "pending"
depends_on = ["behavior-performance-signoff"]

[[steps]]
id = "kicad-cruncher-handoff"
title = "Prepare downstream kicad_cruncher guidance and follow-on plan after Monkey API work lands"
status = "pending"
depends_on = ["behavior-performance-signoff"]

[[steps]]
id = "external-review"
title = "Obtain external review before any release preparation"
status = "pending"
depends_on = [
  "behavior-performance-signoff",
  "design-doc-intent-audit",
  "test-runtime-impact-audit",
]

[[steps]]
id = "closeout-artifacts"
title = "Close the active plan after review by moving durable decisions out of docs/plans"
status = "pending"
depends_on = ["external-review", "kicad-cruncher-handoff"]

[[exit_criteria]]
id = "ec-dependent-on-performance-closeout"
title = "The parser/projection optimization plan is closed before this plan executes implementation"
status = "met"

[[exit_criteria]]
id = "ec-api-inventory-complete"
title = "Full-model, projection, targeted-reader, project, IR, and SVG entry points are inventoried"
status = "pending"

[[exit_criteria]]
id = "ec-guidance-clear"
title = "Documentation clearly distinguishes full materialization, source projection, SVG/3D projection, and rendering"
status = "pending"

[[exit_criteria]]
id = "ec-render-design-reviewed"
title = "Any file-level IR/SVG API proposal is reviewed for correctness, draw order, layer filtering, bounding boxes, enrichment metadata, and test coverage"
status = "pending"

[[exit_criteria]]
id = "ec-native-options-evaluated"
title = "C, C++, Cython, and pure-Python parser/tokenizer acceleration options are compared with platform, packaging, ABI, and maintenance tradeoffs"
status = "pending"

[[exit_criteria]]
id = "design-doc-intent-audit"
title = "ADRs, design docs, requirements, and release notes match selected API behavior"
status = "pending"

[[exit_criteria]]
id = "test-runtime-impact-audit"
title = "Test additions and runtime impact are reviewed with recorded evidence"
status = "pending"

[[exit_criteria]]
id = "ec-independent-review-complete"
title = "A separate agent independently repeats the research and reviews the selected design before implementation"
status = "pending"

[[exit_criteria]]
id = "ec-implementation-behavior-preserved"
title = "Accepted changes preserve existing parser, projection, IR, SVG, and project API behavior unless a reviewed contract update says otherwise"
status = "pending"

[[exit_criteria]]
id = "ec-signoff-green"
title = "Relevant L0/L1/L99, dev-std audit, and targeted performance checks pass"
status = "pending"

[[exit_criteria]]
id = "ec-cruncher-deferred"
title = "No kicad_cruncher implementation begins until Monkey API decisions land and a downstream plan is created"
status = "pending"

[[exit_criteria]]
id = "external-review"
title = "External review is complete before release preparation or publish authorization"
status = "pending"

[[exit_criteria]]
id = "ec-closeout-docs"
title = "Durable decisions are moved to ADRs, design docs, requirements, release notes, contracts, or tests before plan deletion"
status = "pending"
+++

# KiCad Monkey Project API Projection And Render Design

Status: active planning and research gate. Implementation is not started.

This plan is dependent on the closed
`kicad-monkey-performance-optimization-sweep` work. That optimization effort
made projection and lexer paths faster, but it intentionally deferred larger
API-design questions:

- whether project-level guidance makes the fast projection APIs discoverable;
- whether current docs clearly tell users when `KiCadPcb`, `KiCadDesign`,
  `to_ir()`, and `to_svg()` fully materialize a board or design;
- whether new file-level IR/SVG APIs should exist for workflows that do not
  require a mutable full `KiCadPcb`;
- whether native tokenizer/parser acceleration should be pursued, and through
  which implementation strategy.

Per ADR-003 and `AGENTS.md`, this is a working artifact. Durable outcomes must
move into ADRs, design docs, requirements, release notes, contracts, or tests
before closeout. No tag, release, PyPI upload, publish workflow, or downstream
`kicad_cruncher` implementation is part of this plan without explicit
authorization.

## Definitions

This plan must keep these terms distinct:

- **Full model**: `KiCadPcb`, `KiCadSchematic`, and `KiCadDesign` object
  graphs created by the full S-expression parser and typed factories.
- **Source projection**: `SexpFormSpan`, targeted readers, and
  `KiCadPcbProjection` APIs that select source forms and hydrate only selected
  typed objects.
- **Plotter IR**: the render intermediate representation used for KiCad SVG
  output and downstream scene conversion.
- **SVG/3D projection**: visual projection concepts such as layer-filtered SVG
  output or Geometer/STEP HLR projection. These are not the same as source
  projection.

## Goals

- Produce a clear API decision record for when callers should use full
  materialization, source projection, targeted readers, project APIs, IR, and
  SVG renderers.
- Audit and improve docs so performance-sensitive users can choose the
  smallest correct API.
- Determine whether new file-level IR/SVG APIs can provide meaningful speedups
  without weakening rendering correctness.
- Implement accepted API/doc changes after independent research review.
- Evaluate native tokenizer/parser acceleration options across C, C++, Cython,
  and pure-Python architecture changes, including per-platform packaging and
  maintenance costs.
- Prepare a downstream `kicad_cruncher` plan only after Monkey API decisions
  have landed.

## Non-Goals

- Do not rewrite `KiCadPcb.to_ir()` or `KiCadPcb.to_svg()` to hide partial
  parsing behind an already materialized `KiCadPcb`; those methods start after
  the full parse cost has already been paid.
- Do not add native extensions in this plan unless a later reviewed design
  explicitly changes scope. The required native step is evaluation and
  decision capture.
- Do not start `kicad_cruncher` implementation before Monkey API changes land.
- Do not break existing full parser, round-trip, IR, SVG, or projection
  contracts for performance.
- Do not accept third-party code by copying, cherry-picking, or porting it
  without a separate reviewed decision.

## Research Questions

- Which public and provisional APIs force full file or project materialization?
- Which existing projection and targeted-reader APIs are appropriate for
  large-board inventories, diagnostics, source-span lookups, model-reference
  scans, footprint/pad summaries, and route/net scans?
- Where do docs or command examples imply that IR/SVG or project APIs are
  partial when they currently are not?
- Can a file-level `pcb_file_to_ir(...)`, `render_pcb_file_to_svg(...)`, or
  equivalent facade build a correct layer-filtered document without first
  constructing a complete `KiCadPcb`?
- If file-level render APIs are possible, what correctness rules must they
  preserve: draw order, bounding box, layer filtering, zones, pads, holes,
  enrichment metadata, source IDs, project variables, and stable output?
- Should a partial render path be public API, private acceleration, or rejected
  until a streaming typed parser exists?
- For native acceleration, which approach has the best cost/risk profile:
  a C tokenizer, C++ tokenizer/parser aligned with KiCad's eventual parser
  direction, Cython over current Python structures, or a pure-Python pull
  parser that avoids token-list materialization?
- What wheel, ABI, CI, Windows/macOS/Linux, source-distribution, and fallback
  requirements would native acceleration introduce?

## Candidate API Directions

These are candidates to investigate, not accepted designs:

1. Documentation-only guidance that makes existing APIs clear.
2. File-level inventory helpers that wrap `KiCadPcbProjection` for common
   large-board diagnostics.
3. File-level IR/SVG helpers for selected layers or selected object families.
4. A reusable render cache API for callers rendering many views from one
   already-loaded board.
5. A pull-token parser or streaming typed parser architecture that reduces
   full-parse token object and generic tree materialization.
6. Native tokenizer/parser acceleration as a separate future plan.

## Implementation Gates

Implementation must not start until:

- the current API inventory is complete;
- documentation and render-pipeline research are recorded;
- native acceleration options are compared;
- an independent agent repeats the research and reviews the selected API
  direction;
- durable design docs or ADRs describe the chosen public behavior.

If research concludes that no new IR/SVG partial API is safe yet, the
implementation slice should be limited to documentation, guidance, API naming,
and deferred-work records.

## Validation Plan

Minimum validation for documentation-only changes:

- `uv run dev-std audit . --format json`
- `uv run --extra test python tests/rack.py run L99_signoff`
- focused documentation link checks if docs move or entry points change

Minimum validation for accepted API or render changes:

- focused L0 tests for new public/provisional APIs;
- corpus tests for projection/full-model parity where applicable;
- SVG/IR oracle or structural tests for render output stability;
- performance probe comparing full-materialization and file-level paths on
  synthetic and public-corpus cases;
- `uv run --extra test python tests/rack.py run L0_foundation`;
- `uv run --extra test python tests/rack.py run L99_signoff`;
- `uv run dev-std audit . --format json`.

## Downstream Handoff

After Monkey API decisions land, prepare a separate `kicad_cruncher` plan to:

- update cruncher docs and CLI guidance to point to the correct Monkey APIs;
- decide whether cruncher commands can use any new file-level Monkey APIs;
- keep render-heavy commands honest about full-board materialization;
- avoid duplicating Monkey parsing logic in cruncher.

That downstream plan is intentionally out of scope here.
