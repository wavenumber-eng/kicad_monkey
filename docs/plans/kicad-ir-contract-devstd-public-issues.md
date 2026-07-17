+++
type = "plan"
id = "kicad-ir-contract-devstd-public-issues"
status = "active"
title = "KiCad IR Contract, Dev-Std Alignment, And Public Issue Work"
created = "2026-07-16"
issue_refs = [
  "wavenumber-eng/kicad_monkey#9",
  "wavenumber-eng/kicad_monkey#10",
  "wavenumber-eng/kicad_monkey#11",
  "wavenumber-eng/kicad_monkey#12",
]

[[steps]]
id = "branch-and-plan-tracking"
title = "Create feature branch and force-add the active plan file"
status = "done"

[[steps]]
id = "plan-external-review"
title = "Obtain external review of the plan before implementation"
status = "done"
depends_on = ["branch-and-plan-tracking"]

[[steps]]
id = "pre-plan-edit-reconciliation"
title = "Reconcile edits made before the official plan was created"
status = "done"
depends_on = ["plan-external-review"]

[[steps]]
id = "kicad-monkey-dev-std-signoff"
title = "Align kicad_monkey signoff with dev-std audit requirements"
status = "done"
depends_on = ["pre-plan-edit-reconciliation"]

[[steps]]
id = "dev-std-signoff-external-review"
title = "Obtain external review of dev-std signoff integration and deferred scopes"
status = "done"
depends_on = ["kicad-monkey-dev-std-signoff"]

[[steps]]
id = "ir-contract-and-reference"
title = "Document and validate the KiCad Plotter IR contract"
status = "done"
depends_on = ["dev-std-signoff-external-review"]

[[steps]]
id = "ir-contract-external-review"
title = "Obtain external review of the IR contract and canonical reference"
status = "done"
depends_on = ["ir-contract-and-reference"]

[[steps]]
id = "text-hyperlink-context"
title = "Investigate and add text hyperlink metadata through the IR context contract"
status = "done"
depends_on = ["ir-contract-external-review"]

[[steps]]
id = "text-hyperlink-external-review"
title = "Obtain external review of text hyperlink and IR context behavior"
status = "done"
depends_on = ["text-hyperlink-context"]

[[steps]]
id = "source-driven-preferences"
title = "Make KiCad preference setup source-driven and profile-neutral"
status = "done"
depends_on = ["text-hyperlink-external-review"]

[[steps]]
id = "preferences-external-review"
title = "Obtain external review of company-neutral preference setup"
status = "done"
depends_on = ["source-driven-preferences"]

[[steps]]
id = "design-json-pin-count-performance"
title = "Optimize design JSON pin-count generation without contract drift"
status = "done"
depends_on = ["preferences-external-review"]

[[steps]]
id = "performance-external-review"
title = "Obtain external review of the design JSON performance fix"
status = "done"
depends_on = ["design-json-pin-count-performance"]

[[steps]]
id = "kicad-monkey-release"
title = "Prepare the kicad_monkey public release candidate"
status = "done"
depends_on = ["performance-external-review"]

[[steps]]
id = "kicad-monkey-publish-authorization"
title = "Obtain explicit user authorization to publish the kicad_monkey public release"
status = "pending"
depends_on = ["kicad-monkey-release"]

[[steps]]
id = "kicad-cruncher-ir-export"
title = "Update kicad_cruncher after kicad_monkey release with public IR export support"
status = "pending"
depends_on = ["kicad-monkey-publish-authorization"]

[[steps]]
id = "kicad-cruncher-external-review"
title = "Obtain external review of kicad_cruncher IR export and dev-std signoff integration"
status = "pending"
depends_on = ["kicad-cruncher-ir-export"]

[[steps]]
id = "design-doc-intent-audit"
title = "Audit design docs, ADRs, requirements, contracts, and release notes against implementation"
status = "pending"
depends_on = [
  "ir-contract-external-review",
  "text-hyperlink-external-review",
  "preferences-external-review",
  "performance-external-review",
  "kicad-cruncher-external-review",
]

[[steps]]
id = "test-runtime-impact-audit"
title = "Audit new and changed tests and record runtime impact"
status = "pending"
depends_on = [
  "ir-contract-and-reference",
  "text-hyperlink-context",
  "source-driven-preferences",
  "design-json-pin-count-performance",
  "kicad-cruncher-ir-export",
]

[[steps]]
id = "external-review"
title = "Obtain final independent external review after all slice reviews"
status = "pending"
depends_on = [
  "design-doc-intent-audit",
  "test-runtime-impact-audit",
]

[[exit_criteria]]
id = "ec-plan-external-review"
title = "Initial plan review approves scope, sequencing, and review gates"
status = "met"

[[exit_criteria]]
id = "ec-kicad-monkey-first"
title = "kicad_monkey work is completed, reviewed, signed off, and released before kicad_cruncher implementation starts"
status = "pending"

[[exit_criteria]]
id = "ec-ir-contract-reference"
title = "The KiCad Plotter IR has an accepted JSON Schema and canonical HTML reference suitable for downstream IR-to-DWG scene conversion"
status = "pending"

[[exit_criteria]]
id = "ec-public-issues"
title = "Issues #9, #10, #11, and #12 are addressed or explicitly deferred with reviewed rationale"
status = "pending"

[[exit_criteria]]
id = "ec-dev-std-signoff"
title = "kicad_monkey and kicad_cruncher signoff include dev-std audit coverage appropriate to their release state"
status = "pending"

[[exit_criteria]]
id = "design-doc-intent-audit"
title = "Design docs, ADRs, requirements, contracts, and release notes match implemented behavior"
status = "pending"

[[exit_criteria]]
id = "test-runtime-impact-audit"
title = "New and changed tests are listed and runtime impact is reviewed"
status = "pending"

[[exit_criteria]]
id = "external-review"
title = "All major slices and final closeout have independent external review"
status = "pending"

[[exit_criteria]]
id = "ec-publish-authorization"
title = "Public release publish occurs only after explicit user authorization"
status = "pending"
+++

# KiCad IR Contract, Dev-Std Alignment, And Public Issue Work

This plan covers a release-oriented update across `kicad_monkey` and
`kicad_cruncher`, with `kicad_monkey` completed and publicly released before
`kicad_cruncher` consumes the new behavior.

The plan is intentionally gated. Implementation must not start until the plan
receives external review. Each major slice also requires external review before
the next major slice starts.

The external reviewer for this plan means an independent review context that is
not the executor currently making the changes. It may be a separate agent
session, a designated review tool, or a human reviewer. The executor must not
self-approve any gate labeled external review.

## Background

`kicad_monkey` already exposes a JSON-serializable graphics rendering IR through
`KiCadPlotterDocument`, `KiCadPlotterRecord`, `KiCadPlotterOp`,
`schematic_to_ir()`, `KiCadDesign.to_schematic_instance_ir()`, `pcb_to_ir()`,
and `render_ir_to_svg()`. The IR has been used internally with a special KiCad
CLI oracle build to validate SVG generation.

The next public use case is broader: `toolz/data_models` will need a canonical
reference for a KiCad IR to DWG scene converter. That consumer should not have
to reverse-engineer the Python implementation or existing tests. The IR needs
contract documentation, a JSON Schema, focused validation, and clear guidance
for downstream scene/viewer tooling.

The plan also folds in three public or install-facing issues that should land
in the same release window:

- `wavenumber-eng/kicad_monkey#9`: preserve text hyperlink metadata through
  schematic IR.
- `wavenumber-eng/kicad_monkey#10`: make KiCad preference setup source-driven
  and company/profile-neutral.
- `wavenumber-eng/kicad_monkey#11`: avoid repeated net-terminal scans when
  computing design JSON component pin counts.
- `wavenumber-eng/kicad_monkey#12`: align with current `wn-dev-std`,
  especially JSON contract and schema documentation expectations.

## Strategy

The work proceeds in this order:

1. Create feature branch workspaces for the affected repositories.
2. Force-add this active plan file because `docs/plans/` is ignored locally for
   release-artifact hygiene. Active plan tracking belongs in git history;
   public source distributions still exclude active planning files through
   package build configuration.
3. Reconcile the pre-plan `pyproject.toml` edits onto the feature branches.
4. Review and approve this plan.
5. Finish all `kicad_monkey` work.
6. Run focused tests, Rack signoff, and dev-std audit coverage for
   `kicad_monkey`.
7. Prepare a `kicad_monkey` release candidate and obtain external review.
8. Publish a public `kicad_monkey` release only after explicit user
   authorization for that publish action.
9. Update `kicad_cruncher` to depend on and expose the released IR behavior.
10. Run focused tests, Rack signoff, and dev-std audit coverage for
    `kicad_cruncher`.

This order keeps the parser/model/rendering contract in `kicad_monkey`, where
it belongs, and avoids implementing a `kicad_cruncher` command against an
unreleased or unstable IR contract.

The major implementation slices are intentionally serialized even where they do
not have a strict technical dependency. This keeps review scope bounded and
ensures each public issue has a completed review record before the next issue
slice starts. The `kicad_monkey` release candidate must include all completed
slice review gates, not just the last implementation slice.

Dev-std plan hygiene requires the design-doc intent audit, test runtime impact
audit, and external-review exit criteria to use the same identifiers as their
corresponding closeout steps. Those identifiers are namespaced by section in the
checker.

## Pre-Plan Workspace Edits

Before this official plan was created, two `pyproject.toml` files were edited:

- `kicad_monkey/pyproject.toml`
- `kicad_cruncher/pyproject.toml`

Those edits added:

- `standard_version = "2026.7.16"` under `[tool.wn_dev_std]`;
- `jsonschema>=4.22.0` under test/dev dependencies.

The first implementation step must explicitly reconcile these edits. The
reviewed outcome may keep, revise, or revert them. No feature slice should
treat them as already accepted plan output.

## Slice 1: Dev-Std Signoff Integration

`kicad_monkey` and `kicad_cruncher` both need release signoff that includes
dev-std audit coverage. This slice decides the initial audit scopes and avoids
turning broad governance migration into uncontrolled side work.

Expected work:

- Declare the current `wn-dev-std` standard version after review.
- Decide which dev-std scopes are mandatory for this release.
- Add signoff wiring so Rack or release scripts include the selected dev-std
  audit checks.
- Keep broad governance failures such as legacy plan format, ADR migration,
  domain registry, requirements, and build/test-strategy documents either in
  scope with explicit tasks or deferred with reviewed rationale.

Reviewed initial signoff scopes:

- `kicad_monkey`: `repo`, `ci`, `docs.design`, `docs.links`, `docs.plans`.
- `kicad_cruncher`: `repo`, `ci`, `docs.design`, `docs.links`.

Deferred governance scopes must be revisited before release closeout with exact
scope names, especially `docs.release` for both PyPI packages and `docs.cli`
plus `docs.plans` for `kicad_cruncher`.

Review gate:

- An external reviewer confirms the chosen dev-std scopes are sufficient for
  this release and that any deferred governance migration is explicit.

## Slice 2: KiCad Plotter IR Contract And Canonical Reference

`kicad_monkey` owns the public IR. This slice makes the existing IR a durable
contract.

Expected work:

- Add a JSON Schema for the current `kicad.plotter_ir.a0` payload.
- Add detailed accepted HTML documentation for the complete IR.
- Cover document fields, records, operation envelope, operation kinds, units,
  coordinate system, colors, text, images, grouping, source references,
  normalized JSON, and forward compatibility.
- Explain how downstream consumers should use the IR for scene conversion,
  including the future `toolz/data_models` KiCad IR to DWG scene converter.
- Define an optional `context` mechanism in the contract/schema.
- State which levels can carry context: document, record, operation, or a
  narrower subset.
- State which context keys are standardized now and how consumers should treat
  unknown keys.
- Use root `schema = "kicad.plotter_ir.a0"` per user decision on
  2026-07-16. Do not add root `type` plus `version` unless reviewed later.
- Add schema validation tests using a real JSON Schema validator.

Review gate:

- An external reviewer confirms the schema and HTML reference are sufficiently
  complete for a downstream agent to implement an IR-to-DWG scene converter
  without reading the implementation first.

## Slice 3: Text Hyperlink Metadata And IR Context

Issue `#9` asks for KiCad schematic text hyperlink metadata to survive through
`to_schematic_instance_ir()`. It is not yet known whether the current parser or
IR supports this data.

Expected work:

- Investigate KiCad's source representation for text hyperlinks.
- Determine whether `kicad_monkey` currently parses the hyperlink metadata.
- If missing, add source-model support without changing unrelated text
  behavior.
- Preserve hyperlink data through `schematic_to_ir()` and
  `KiCadDesign.to_schematic_instance_ir()`.
- Represent hyperlink metadata through the reviewed IR `context` mechanism or
  another reviewed contract field.
- Update the IR schema and reference docs to describe hyperlink metadata.
- Add regression tests for linked text and ordinary text.

Compatibility requirements:

- Existing text IR without hyperlinks remains valid.
- Geometry rendering must not require hyperlink context.
- Unknown context keys must not break existing consumers.

Review gate:

- An external reviewer confirms the hyperlink behavior is non-breaking and is
  documented at the right contract layer.

## Slice 4: Source-Driven KiCad Preference Setup

Issue `#10` covers install/control-repo integration. `kicad_monkey` must not
contain Wavenumber, TE, ET, or any other company/profile-specific preference
assumptions.

Expected work:

- Make `setup_kicad_preferences(preferences_source=...)` data-driven from the
  supplied source directory.
- Copy provided color theme files without assuming a company name.
- Apply `appearance.color_theme` and related settings from source preference
  JSON files where present.
- Support source files such as `eeschema.json`, `pcbnew.json`, `fpedit.json`,
  `pl_editor.json`, and `gerbview.json` when present.
- Keep current Wavenumber setup working by reading existing source files rather
  than hard-coding Wavenumber defaults.
- Log actual source-derived settings rather than fixed profile names.
- Keep profile-specific payloads in install/control/libz repositories, not in
  `kicad_monkey`.

Compatibility requirements:

- Existing callers that pass only `preferences_source` and `config_paths` keep
  working.
- The shared helper remains generic and profile-neutral.

Review gate:

- An external reviewer confirms the implementation has no company-specific
  behavior or misleading logs in `kicad_monkey`.

## Slice 5: Design JSON Pin-Count Performance

Issue `#11` reports avoidable repeated work in `kicad_design_to_json()`.
Component `classification.pin_count` should not rescan every net terminal once
per component.

Expected work:

- Replace per-component terminal rescans with a one-pass lookup keyed by
  component reference.
- Preserve the generated design JSON contract exactly.
- Count unique pins once per designator.
- Preserve case-sensitive reference matching.
- Preserve current behavior for empty pin values.
- Report zero for components with no terminal records.
- Add regression tests for duplicate terminal records and disconnected
  components.
- Add focused performance coverage if it can be stable in routine test lanes.

Review gate:

- An external reviewer confirms the change preserves the public JSON contract
  and removes the repeated-scan behavior.

## Slice 6: kicad_monkey Release

After the `kicad_monkey` slices pass review, prepare the public release
candidate. Publishing is a separate externally visible action and requires
explicit user authorization after reviewer approval.

Expected work:

- Update release notes and changelog.
- Run focused tests for changed areas.
- Run Rack signoff.
- Run the selected dev-std audit scopes.
- Confirm the public distribution includes the new contract and documentation
  artifacts and excludes active planning files as required by release policy.

Review gate:

- An external reviewer confirms the release candidate is ready.
- The user explicitly authorizes the public publish action after reviewer
  approval. Reviewer approval alone is not publish authorization.

## Slice 7: kicad_cruncher IR Export And Signoff

`kicad_cruncher` should consume the released `kicad_monkey` behavior. If no
public command exists for exporting IR JSON, add one after the `kicad_monkey`
release.

Expected work:

- Update the `kicad-monkey` dependency to the released version.
- Add or update a public IR export command, likely `kicad-cruncher ir`.
- Export schematic instance IR JSON for `.kicad_pro` and `.kicad_sch` inputs.
- Export PCB IR JSON for `.kicad_pcb` inputs and project inputs with boards.
- Emit a manifest that lists generated IR artifacts and source context.
- Add CLI design docs and command manifest entries.
- Link output contracts back to the `kicad_monkey` IR schema and reference.
- Update `kicad_cruncher` signoff so it includes appropriate dev-std audit
  coverage.

Review gate:

- An external reviewer confirms command UX, artifact layout, manifest shape,
  contract linkage, and dev-std signoff integration.

## Validation Plan

Validation should be slice-focused during implementation and full enough before
release.

Expected focused validation:

- IR serializer and schema validation tests.
- Schematic text hyperlink parser/model/IR tests.
- Preference setup tests with at least two generic source-profile fixtures.
- Design JSON pin-count regression tests.
- `kicad_cruncher` IR export CLI tests after the `kicad_monkey` release.

Expected release validation:

- `kicad_monkey` Rack signoff.
- `kicad_monkey` selected dev-std audit scopes.
- Public package build validation for `kicad_monkey`.
- `kicad_cruncher` Rack signoff after dependency update.
- `kicad_cruncher` selected dev-std audit scopes.
- Public package build validation for `kicad_cruncher` if a release is being
  prepared.

## Documentation Deliverables

Expected durable `kicad_monkey` documentation:

- IR contract JSON Schema under `docs/contracts/`.
- Accepted HTML reference under `docs/design/`.
- Updates to `docs/contracts/README.md`.
- Updates to API/interface design docs where the IR context or hyperlink
  surface changes public behavior.
- ADR or requirement docs if review determines the context mechanism or
  source-driven preferences need durable decision records.
- Release notes and changelog entries.

Expected durable `kicad_cruncher` documentation:

- CLI design doc for IR export.
- Command manifest update.
- Contract README update if needed.
- Release notes and changelog entries if a release is prepared.

## Open Decisions

- Whether the IR keeps only root `schema = "kicad.plotter_ir.a0"` or also adds
  root `type` and `version` in a later version.
- Whether the optional `context` object is allowed on all operations or only
  selected operation kinds.
- Whether document-level and record-level context should be introduced in the
  same version as operation-level context.
- Exact field name and shape for hyperlink metadata.
- Exact `kicad_cruncher` command spelling and output layout.
- Which dev-std audit scopes are mandatory for this release versus deferred
  governance migration.
- Target `kicad_monkey` release version and target `kicad_cruncher` release
  version under the date-based versioning policy.

## Closeout Procedure

Before this plan is removed from active `docs/plans`:

- Move durable decisions into design docs, ADRs, requirements, contracts, tests,
  and release notes.
- Mark each exit criterion as met or blocked with reviewed rationale.
- Record new or changed tests and runtime impact.
- Obtain final external review after all slice reviews.
- Run `dev-std audit . --scope docs.plans` from each repo whose plan/signoff
  state is being closed out.
