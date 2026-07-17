+++
type = "plan"
id = "kicad-cruncher-dev-std-cli-signoff"
status = "active"
title = "KiCad Cruncher Dev-Std, CLI Documentation, And Signoff Alignment"
created = "2026-07-17"
issue_refs = [
  "wavenumber-eng/kicad_cruncher#8",
]

[[steps]]
id = "branch-and-plan-tracking"
title = "Track the active plan on the kicad_cruncher feature branch"
status = "done"

[[steps]]
id = "baseline-audit"
title = "Capture current dev-std, CLI docs, plans, requirements, and release-governance gaps"
status = "done"
depends_on = ["branch-and-plan-tracking"]

[[steps]]
id = "plan-external-review"
title = "Move independent review to the pre-release gate per user direction"
status = "done"
depends_on = ["baseline-audit"]

[[steps]]
id = "existing-pr-reconciliation"
title = "Reconcile PR #9 dev-std signoff baseline with the expanded plan"
status = "done"
depends_on = ["plan-external-review"]

[[steps]]
id = "dependency-alignment"
title = "Update controlled dependencies to the latest approved public releases"
status = "done"
depends_on = ["existing-pr-reconciliation"]

[[steps]]
id = "dev-std-scope-expansion"
title = "Expand configured dev-std scopes for release-ready governance"
status = "done"
depends_on = ["dependency-alignment"]

[[steps]]
id = "cli-documentation-migration"
title = "Migrate CLI command documentation and manifest to dev-std docs.cli"
status = "done"
depends_on = ["dev-std-scope-expansion"]

[[steps]]
id = "legacy-plan-closeout"
title = "Move legacy active-plan content into durable docs and delete tracked plan files"
status = "done"
depends_on = ["cli-documentation-migration"]

[[steps]]
id = "requirements-and-release-governance"
title = "Add requirements and release-governance artifacts required by dev-std"
status = "done"
depends_on = ["legacy-plan-closeout"]

[[steps]]
id = "signoff-wiring"
title = "Wire expanded dev-std audit scopes into L99 signoff"
status = "done"
depends_on = ["requirements-and-release-governance"]

[[steps]]
id = "release-candidate-prep"
title = "Prepare kicad_cruncher release-candidate metadata after signoff passes"
status = "pending"
depends_on = ["signoff-wiring"]

[[steps]]
id = "design-doc-intent-audit"
title = "Audit CLI docs, design docs, ADRs, requirements, release docs, and tests against implementation intent"
status = "done"
depends_on = ["signoff-wiring"]

[[steps]]
id = "test-runtime-impact-audit"
title = "Record changed test coverage and runtime impact"
status = "done"
depends_on = ["signoff-wiring"]

[[steps]]
id = "external-review"
title = "Obtain final independent review after implementation and validation"
status = "active"
depends_on = [
  "design-doc-intent-audit",
  "test-runtime-impact-audit",
]

[[steps]]
id = "publish-authorization"
title = "Obtain explicit user authorization before any public release or PyPI publish"
status = "pending"
depends_on = ["external-review"]

[[exit_criteria]]
id = "ec-plan-reviewed"
title = "Independent review approves the scope, sequencing, and release gates"
status = "met"

[[exit_criteria]]
id = "ec-latest-dependencies"
title = "Controlled dependency lower bounds target latest approved public releases, including kicad-monkey >=2026.7.16"
status = "met"

[[exit_criteria]]
id = "ec-dev-std-latest"
title = "wn-dev-std lower bound targets the latest PyPI release and upstream-version audit passes"
status = "met"

[[exit_criteria]]
id = "ec-cli-docs"
title = "dev-std docs.cli passes and CLI parser, command manifest, README, and design docs agree"
status = "met"

[[exit_criteria]]
id = "ec-expanded-governance"
title = "Selected dev-std governance scopes pass, including docs.cli, docs.plans, docs.requirements, and docs.release"
status = "met"

[[exit_criteria]]
id = "design-doc-intent-audit"
title = "CLI docs, design docs, ADRs, requirements, contracts, release docs, and tests match implemented behavior"
status = "met"

[[exit_criteria]]
id = "test-runtime-impact-audit"
title = "New and changed tests are listed and runtime impact is reviewed"
status = "met"

[[exit_criteria]]
id = "external-review"
title = "Implementation and closeout have independent external review"
status = "pending"

[[exit_criteria]]
id = "ec-publish-authorization"
title = "No tag, GitHub release, or PyPI publish occurs without explicit user authorization"
status = "pending"
+++

# KiCad Cruncher Dev-Std, CLI Documentation, And Signoff Alignment

This plan covers the next `kicad_cruncher` release-prep pass after
`kicad-monkey 2026.7.16` was published on 2026-07-17.

The immediate objective is not to publish. The objective is to align
`kicad_cruncher` with the latest public `wn-dev-std`, migrate CLI
documentation to the dev-std CLI governance contract, wire that governance into
L99 signoff, and prepare a reviewed release candidate. Public publishing still
requires a separate explicit authorization step.

## Current Verified Baseline

Verified on 2026-07-17:

- Latest PyPI `wn-dev-std`: `2026.7.16`.
- Latest PyPI `kicad-monkey`: `2026.7.16`.
- Open PR #9 already wires `wn-dev-std>=2026.7.16` into L99 for the current
  configured scopes.
- Current configured scopes pass: `repo`, `ci`, `docs.design`, `docs.links`.
- `dev-std audit . --check-upstream-version --format json` reports the
  installed `wn-dev-std 2026.7.16` matches latest PyPI.

Current gaps when additional relevant scopes are run:

- `docs.cli` fails because `docs/contracts/command_manifest.a0.json` still uses
  the project-local `kicad_cruncher.command_manifest.a0` shape. The dev-std a0
  manifest expects schema `wn_dev_std.command_manifest.a0`, per-command
  `design_doc`, and no unsupported `requires_extra` field.
- `docs.plans` fails because three legacy plan files have no TOML front matter:
  `docs/plans/kicad-library-extraction-commands.md`,
  `docs/plans/kicad-plugin-daemon-framework.md`, and
  `docs/plans/pcb-svg-assembly-virtual-layer-fix.md`.
- `docs.requirements` fails because no requirements documents are present.
- `docs.release` fails because PyPI distribution governance requires
  `docs/governance/release.toml`.

## Execution Notes

Implemented on 2026-07-17:

- `kicad-monkey` now uses the lower-bound requirement
  `kicad-monkey>=2026.7.16`.
- `wn-dev-std` remains a lower-bound test/dev requirement:
  `wn-dev-std>=2026.7.16`.
- `docs/contracts/command_manifest.a0.json` now uses
  `wn_dev_std.command_manifest.a0`.
- Enabled dev-std scopes now include `docs.cli`, `docs.plans`,
  `docs.requirements`, and `docs.release`.
- The three legacy plan files were removed after their durable obligations were
  represented in existing design docs and new requirements.
- External review found that several unfinished obligations from the deleted
  active plans needed more explicit durable homes. They are now tracked in:
  `docs/plugin/requirements/plugin-req-001-daemon-live-validation.md`,
  `docs/plugin/requirements/plugin-req-002-footprint-hlr-daemon-tool.md`,
  `docs/schematic/requirements/schematic-req-001-clean-config-contract.md`,
  `docs/pcb-svg/requirements/pcb-svg-req-001-assembly-validation-and-selectors.md`,
  and `docs/library/requirements/library-req-001-extraction-hardening.md`.
- Runtime-impact validation was moved to
  `docs/research/2026-07-17-kicad-cruncher-dev-std-cli-signoff-validation.md`
  so it survives plan deletion.
- No package version bump, release note, tag, GitHub Release, or PyPI publish
  occurred. Green PR CI, external review, and explicit user authorization remain
  required before release.

Validation completed on 2026-07-17:

- `uv run dev-std audit . --format json`
- `uv run dev-std audit . --check-upstream-version --format json`
- `uv run pytest tests\L99_signoff -q` (`25 passed` in 6.86 s)
- `uv run pytest tests\L0_public_cli -q` (`49 passed` in 16.55 s)
- `uv run rack run L99` (`25 passed` in 7.00 s)

## Strategy

This work should proceed as a strict governance/documentation/signoff release
slice before any unrelated feature expansion.

1. Reconcile the already-open PR #9. Either merge it first as the dev-std
   baseline or continue from that branch, but do not lose its L99 signoff
   wiring.
2. Update `kicad-monkey` to `>=2026.7.16` and refresh `uv.lock`.
3. Expand `enabled_scopes` only when each newly enabled scope is made to pass.
4. Treat `docs.cli` as first-class release governance, not just a project-local
   test. The command manifest, CLI design index, per-command design docs,
   README command table, parser help, and L99 signoff must agree.
5. Close legacy active plans by moving durable content into ADRs, requirements,
   design docs, contracts, tests, and release notes, then delete the plan files.
6. Add release-governance and requirements artifacts before enabling
   `docs.release` and `docs.requirements`.
7. Wire the expanded dev-std audit into L99 so local tests and CI fail on
   governance drift.
8. Prepare release metadata only after the expanded governance gates pass.

## Slice 1: Existing PR Reconciliation

Expected work:

- Confirm whether PR #9 should be merged before this plan continues or whether
  this plan will supersede it on the same branch.
- Preserve the existing `wn-dev-std>=2026.7.16` test/dev dependency floor.
- Preserve the existing L99 `test_configured_dev_std_audit_scopes_pass` check.
- Ensure the branch history remains reviewable and commit-scoped.

Review gate:

- Reviewer confirms PR #9's baseline is either merged or intentionally folded
  into the new implementation branch.

## Slice 2: Dependency Alignment

Expected work:

- Update `kicad-monkey==2026.6.25` to `kicad-monkey>=2026.7.16`.
- Refresh controlled dependency checks in L99 so lower-bound dependencies are
  validated as minimums, not exact pins.
- Refresh CLI version output tests that report controlled dependency versions.
- Refresh `uv.lock`.
- Decide whether the next `kicad_cruncher` release target is `2026.7.17` or
  another date-based version under ADR-0001.

Review gate:

- Reviewer confirms controlled dependency lower bounds match released public packages
  and no prerelease package is referenced.

## Slice 3: Dev-Std Scope Expansion

Expected work:

- Keep `standard_version = "2026.7.16"`.
- Add candidate scopes after their migrations pass:
  `docs.cli`, `docs.plans`, `docs.requirements`, and `docs.release`.
- Consider whether `docs.adrs`, `docs.surfaces`, `docs.traceability`,
  `docs.test_strategy`, and `docs.artifacts` are in scope for this release or
  should remain deferred with explicit rationale.
- Add `--check-upstream-version` coverage to L99 or an equivalent signoff check
  so the package notices when the configured standard is stale.

Review gate:

- Reviewer confirms the selected scopes are appropriate for the release and any
  deferred scopes are named explicitly.

## Slice 4: CLI Documentation Migration

Expected work:

- Migrate `docs/contracts/command_manifest.a0.json` to the dev-std command
  manifest schema `wn_dev_std.command_manifest.a0`.
- Add `design_doc` for every command and remove unsupported fields such as
  `requires_extra`.
- Keep or adapt existing L99 tests so they validate both dev-std CLI
  governance and project-specific command inventory invariants.
- Ensure the CLI design index, per-command docs, README command table, parser
  help, command modules, and command manifest remain synchronized.
- Keep every public command design doc accepted and complete:
  usage, arguments, output, tests, and config contract.

Review gate:

- Reviewer confirms `dev-std audit . --scope docs.cli --format json` passes and
  L99 still catches project-specific CLI drift.

## Slice 5: Legacy Plan Closeout

Expected work:

- Audit the three legacy plan files and identify durable content that must be
  preserved.
- Move durable decisions into ADRs, requirements, design docs, contracts,
  release notes, or tests.
- Delete closed/transient plan files from `docs/plans`.
- Run `dev-std audit . --scope docs.plans --format json` and confirm zero
  invalid legacy plans remain.

Review gate:

- Reviewer confirms no meaningful implementation or design decision was lost
  when deleting the legacy plans.

## Slice 6: Requirements And Release Governance

Expected work:

- Add requirements docs for the dev-std/CLI/release-signoff work.
- Add `docs/governance/release.toml` with the package's PyPI release policy,
  trusted-publishing assumptions, tag/version rules, release-note requirements,
  and no-publish-without-authorization rule.
- Update ADRs or design docs if the CLI manifest migration changes the public
  documentation contract.
- Update release notes and changelog when the release target version is chosen.

Review gate:

- Reviewer confirms `docs.requirements` and `docs.release` pass and that release
  policy matches current GitHub/PyPI workflow behavior.

## Slice 7: Signoff Wiring

Expected work:

- Update L99 signoff constants so required dev-std scopes include the expanded
  scope set.
- Ensure L99 fails if `dev-std audit .` fails, including `docs.cli`.
- Ensure L99 or a companion release test checks upstream standard freshness.
- Run focused L99 tests, then Rack L99.

Review gate:

- Reviewer confirms signoff would fail if CLI docs or release governance drift.

## Slice 8: Release Candidate Prep

Expected work:

- Update package version only after the target version is decided.
- Update changelog and dated release notes.
- Run focused tests, Rack signoff, dev-std audits, build, `twine check`, and
  installed import tests.
- Confirm no tag, GitHub release, or PyPI publish occurs until explicit user
  authorization.

Review gate:

- Reviewer confirms the release candidate is ready and publish authorization
  remains a separate user decision.

## Validation Plan

Required focused validation:

- `uv run dev-std audit . --scope docs.cli --format json`
- `uv run dev-std audit . --scope docs.plans --format json`
- `uv run dev-std audit . --scope docs.requirements --format json`
- `uv run dev-std audit . --scope docs.release --format json`
- `uv run dev-std audit . --check-upstream-version --format json`
- `uv run pytest tests\L99_signoff -q`

Required release-candidate validation:

- `uv run dev-std audit . --format json`
- `uv run rack run L99`
- Relevant CLI workflow lanes if parser, manifest, or CLI docs change behavior
- Package build
- `twine check`
- Installed import/version smoke test

## Open Decisions

- Whether PR #9 should merge first or be superseded by this plan branch.
- Target `kicad_cruncher` release version. If released on 2026-07-17, the
  likely date-based version is `2026.7.17`.
- Whether to enable only `docs.cli`, `docs.plans`, `docs.requirements`, and
  `docs.release`, or broaden to additional dev-std documentation scopes.
- Whether the command manifest should remain at `docs/contracts/command_manifest.a0.json`
  or move/rename if dev-std requires a different canonical path later.
- Whether any deleted legacy plan content deserves a new ADR versus a
  requirements or design-doc entry.

## Closeout Procedure

Before this plan is removed from `docs/plans`:

- Mark each exit criterion as met or blocked with reviewed rationale.
- Move durable decisions into ADRs, requirements, design docs, contracts, tests,
  changelog, and release notes.
- Record changed tests and runtime impact.
- Obtain final independent review.
- Delete this active plan file after durable closeout artifacts are committed.
