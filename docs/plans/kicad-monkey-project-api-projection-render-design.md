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
id = "fresh-performance-baseline"
title = "Capture fresh parser, projection, and full-parse decomposition baselines on this branch"
status = "pending"
depends_on = ["current-api-inventory"]

[[steps]]
id = "documentation-guidance-audit"
title = "Audit API documentation and user guidance for full-model versus projected reads"
status = "pending"
depends_on = ["current-api-inventory", "fresh-performance-baseline"]

[[steps]]
id = "render-pipeline-architecture-research"
title = "Research whether file-level IR or SVG APIs can safely use projection or partial materialization"
status = "pending"
depends_on = ["current-api-inventory", "fresh-performance-baseline"]

[[steps]]
id = "native-acceleration-options-research"
title = "Evaluate pure-Python pull-parser, C, C++, and Cython parser/tokenizer acceleration options"
status = "pending"
depends_on = ["current-api-inventory", "fresh-performance-baseline"]

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
id = "public-issue-response"
title = "Prepare and, with explicit authorization, post public guidance on issues #16 and #17"
status = "pending"
depends_on = ["behavior-performance-signoff", "design-doc-intent-audit"]

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
depends_on = ["external-review", "kicad-cruncher-handoff", "public-issue-response"]

[[exit_criteria]]
id = "ec-dependent-on-performance-closeout"
title = "The parser/projection optimization plan is closed before this plan executes implementation"
status = "met"

[[exit_criteria]]
id = "ec-api-inventory-complete"
title = "Full-model, projection, targeted-reader, schematic, project, IR, and SVG entry points are inventoried"
status = "pending"

[[exit_criteria]]
id = "ec-fresh-baselines-captured"
title = "Fresh parser/projection baselines and full-parse decomposition are captured on this branch before implementation comparison"
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
title = "Pure-Python pull-parser, C, C++, and Cython parser/tokenizer acceleration options are compared with platform, packaging, ABI, and maintenance tradeoffs"
status = "pending"

[[exit_criteria]]
id = "design-doc-intent-audit"
title = "Checker-required alias: ADRs, design docs, requirements, and release notes match selected API behavior"
status = "pending"

[[exit_criteria]]
id = "ec-design-doc-intent-audit"
title = "ADRs, design docs, requirements, and release notes match selected API behavior"
status = "pending"

[[exit_criteria]]
id = "test-runtime-impact-audit"
title = "Checker-required alias: test additions and runtime impact are reviewed with recorded evidence"
status = "pending"

[[exit_criteria]]
id = "ec-test-runtime-impact-audit"
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
id = "ec-public-issue-response-authorized"
title = "Any public issue comment on #16 or #17 is prepared from durable docs and posted only after explicit authorization"
status = "pending"

[[exit_criteria]]
id = "external-review"
title = "Checker-required alias: external review is complete before release preparation or publish authorization"
status = "pending"

[[exit_criteria]]
id = "ec-external-review"
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

The prior closeout intentionally deleted plan logs and benchmark JSON files.
This plan must re-measure any performance facts it depends on. In particular,
native acceleration research must recreate the full-parse decomposition that
motivates the architecture question: raw regex scanning, full token production,
and `parse_sexp()` generic-tree construction on a large public corpus board
such as Jumperless. Historical review numbers are useful as a target for
sanity-checking only; they are not durable evidence for this plan.

This plan keeps both `ec-*` exit criteria and three checker-required unprefixed
exit criteria (`design-doc-intent-audit`, `test-runtime-impact-audit`, and
`external-review`). The unprefixed IDs are retained solely because the current
dev-std plan audit requires them.

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
- Evaluate pure-Python pull-parser architecture before native acceleration,
  then compare C, C++, and Cython options against that zero-packaging-cost
  baseline.
- Prepare a downstream `kicad_cruncher` plan only after Monkey API decisions
  have landed.
- Prepare public issue guidance for #16 and #17 after durable docs land,
  including citable public/synthetic numbers and the correct projection API
  guidance, but post it only with explicit authorization.

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
- Do not treat a native tokenizer or parser as a fork of the Python behavior.
  The pure-Python toolkit remains the reference implementation; any native
  option must be a validated accelerator behind the same contracts and corpus
  tests.

## Research Questions

- Which public and provisional APIs force full file or project materialization?
- Which existing projection and targeted-reader APIs are appropriate for
  large-board inventories, diagnostics, source-span lookups, model-reference
  scans, footprint/pad summaries, and route/net scans?
- Does schematic-side source projection need a public analogue, or should
  schematic netlisting and IR continue to use the full schematic model until a
  separate design justifies partial schematic readers?
- Where do docs or command examples imply that IR/SVG or project APIs are
  partial when they currently are not?
- What fresh baseline measurements on this branch describe full
  materialization, source projection, raw regex scanning, token production, and
  generic tree construction before any implementation comparison?
- Can a file-level `pcb_file_to_ir(...)`, `render_pcb_file_to_svg(...)`, or
  equivalent facade build a correct layer-filtered document without first
  constructing a complete `KiCadPcb`?
- If file-level render APIs are possible, what correctness rules must they
  preserve: draw order, bounding box, layer filtering, zones, pads, holes,
  enrichment metadata, source IDs, project variables, and stable output?
- Should a partial render path be public API, private acceleration, or rejected
  until a streaming typed parser exists?
- For native acceleration, which approach has the best cost/risk profile:
  a pure-Python pull parser that avoids token-list materialization, a C
  tokenizer, C++ tokenizer/parser aligned with a future native core, or Cython
  over current Python structures?
- What wheel, ABI, CI, Windows/macOS/Linux, source-distribution, and fallback
  requirements would native acceleration introduce?
- How does KiCad's direction away from SWIG bindings toward kiapi/protobuf
  affect any C++ integration or parser-alignment argument?

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

Conditional implementation steps still need an explicit terminal record. If
`api-design-decision` rejects file-level IR/SVG APIs, mark
`file-level-ir-svg-api-implementation` done only after recording the rejection
rationale in durable docs or a plan log; a done status in that case means
"closed by reviewed rejection", not "implemented".

The `native-acceleration-options-research` step must start from fresh
measurements on this branch. It should compare pure-Python pull parsing first,
because that attacks the same token-list/materialization floor without wheel or
ABI cost. Native options must then beat or complement that baseline while
preserving the Python reference implementation and golden corpus behavior.

## Validation Plan

Minimum validation for documentation-only changes:

- `uv run dev-std audit . --format json`
- `uv run --extra test python tests/rack.py run L99_signoff`
- focused documentation link checks if docs move or entry points change

Minimum validation for accepted API or render changes:

- focused L0 tests for new public/provisional APIs;
- corpus tests for projection/full-model parity where applicable;
- SVG/IR oracle or structural tests for render output stability;
- fresh baselines on this branch before implementation comparison;
- performance probe comparing full-materialization and file-level paths on
  synthetic and public-corpus cases; do not cite deleted closeout JSON files by
  path;
- `uv run --extra test python tests/rack.py run L0_foundation`;
- `uv run --extra test python tests/rack.py run L99_signoff`;
- `uv run dev-std audit . --format json`.

## Public Issue Communication

Issues #16 and #17 should receive a public response after durable docs and
guidance land. The response should summarize the accepted performance results
using citable public or synthetic numbers, explain that public PRs were used as
research input rather than accepted directly, and point reporters to the
correct APIs for large-board scans such as `KiCadPcbProjection` and
`iter_kicad_objects_from_file`. Posting on GitHub is a shared-state action and
requires explicit authorization.

## Downstream Handoff

After Monkey API decisions land, prepare a separate `kicad_cruncher` plan to:

- update cruncher docs and CLI guidance to point to the correct Monkey APIs;
- decide whether cruncher commands can use any new file-level Monkey APIs;
- keep render-heavy commands honest about full-board materialization;
- avoid duplicating Monkey parsing logic in cruncher.

That downstream plan is intentionally out of scope here.
