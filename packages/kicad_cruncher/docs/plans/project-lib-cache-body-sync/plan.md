+++
type = "plan"
id = "project-lib-cache-body-sync"
status = "active"
created = "2026-07-31"
issue_refs = []
design_refs = [
  "docs/design/cli/project-lib.html",
  "docs/contracts/source_relink.a0.schema.json",
  "docs/library/requirements/library-req-001-extraction-hardening.md",
  "docs/plans/project-lib-cache-consistency/plan.md",
]

[[steps]]
id = "root-cause-reproduction"
title = "Capture the Rev B lib_symbol_mismatch failure as a durable regression target"
status = "done"

[[steps]]
id = "contract-design"
title = "Extend the source relink contract for embedded cache body synchronization and validation"
status = "done"
depends_on = ["root-cause-reproduction"]

[[steps]]
id = "cache-body-relink"
title = "Relink embedded schematic cache body fields that KCR changes in generated local symbol libraries"
status = "done"
depends_on = ["contract-design"]

[[steps]]
id = "cache-body-validation"
title = "Detect cache/library body mismatches that would produce KiCad lib_symbol_mismatch warnings"
status = "done"
depends_on = ["cache-body-relink"]

[[steps]]
id = "project-erc-gate"
title = "Add a KiCad CLI project ERC hygiene gate for library-resolution warning types"
status = "done"
depends_on = ["cache-body-validation"]

[[steps]]
id = "focused-tests"
title = "Add L0 and focused L3 coverage for cache body relink, validation reports, and apply-mode blocking"
status = "done"
depends_on = ["cache-body-validation"]

[[steps]]
id = "speedy-rev-b-validation"
title = "Run fixed KCR on a clean Speedy Rev B branch and verify KiCad CLI plus manual GUI inspection"
status = "done"
depends_on = ["project-erc-gate", "focused-tests"]

[[steps]]
id = "signoff"
title = "Run focused tests, full L0, targeted L3, ruff, pyright, diff checks, and dev-std plan audit"
status = "done"
depends_on = ["speedy-rev-b-validation"]

[[steps]]
id = "design-doc-intent-audit"
title = "Audit design docs, ADRs, contracts, and requirements against implementation"
status = "done"
depends_on = ["signoff"]

[[steps]]
id = "test-runtime-impact-audit"
title = "Audit new test runtime impact"
status = "done"
depends_on = ["signoff"]

[[steps]]
id = "external-review"
title = "Obtain external review of the cache body synchronization approach and test evidence"
status = "done"
depends_on = ["signoff", "design-doc-intent-audit", "test-runtime-impact-audit"]

[[steps]]
id = "release-prep"
title = "Prepare and publish a date-versioned kicad-cruncher release after validation and approval"
status = "active"
depends_on = ["external-review"]

[[exit_criteria]]
id = "cache-body-invariant"
title = "Generated local symbol library definitions and embedded schematic cache definitions agree for all KCR-mutated fields"
status = "met"

[[exit_criteria]]
id = "no-loader-regression"
title = "Placed symbol cache links and multipart unit names remain exact KiCad-loader-safe matches"
status = "met"

[[exit_criteria]]
id = "no-library-hygiene-erc"
title = "KiCad CLI ERC after localization reports zero library-resolution, footprint-link, missing-unit, and lib_symbol_mismatch findings"
status = "met"

[[exit_criteria]]
id = "speedy-rev-b-ready"
title = "Speedy Rev B local-library output opens in KiCad 10.0.5 without symbol-cache/library mismatch warnings caused by KCR"
status = "met"

[[exit_criteria]]
id = "design-doc-intent-audit"
title = "Design docs, ADRs, contracts, and requirements match implementation"
status = "met"

[[exit_criteria]]
id = "test-runtime-impact-audit"
title = "New tests are listed and runtime impact is reviewed"
status = "met"

[[exit_criteria]]
id = "external-review"
title = "Independent external review is complete"
status = "met"

[[exit_criteria]]
id = "release-published"
title = "A corrected kicad-cruncher release is published and the installed kcr shim resolves to that release"
status = "pending"
+++

# Project-local library cache body synchronization

## Objective

Make `kcr project-lib --relink-sources --repair-cache-links` produce a KiCad
project-local library conversion that is clean at the KiCad symbol-cache level.
After localization, live schematic and PCB references must point at the generated
local libraries, embedded schematic cache symbol names and multipart unit names
must remain valid, and the embedded cache body must match the generated local
symbol library for every field KCR changes.

## Current Failure

`kicad-cruncher 2026.7.31` fixed the loader-breaking cache alias and unit-prefix
bugs, but it still leaves KiCad `lib_symbol_mismatch` warnings on Speedy Rev B.
The observed Rev B run produced:

- `80` generated symbols and `39` generated footprints.
- `1806` source relinks.
- `cache_link_validation.ok = true`.
- `cache_unit_validation.ok = true`.
- `0` non-local live placed schematic refs, placed schematic footprint refs, or
  PCB footprint refs.
- KiCad 10.0.5 ERC after localization: `622` violations, including `433`
  `lib_symbol_mismatch` warnings.

The immediate representative mismatch is a localized generated symbol library
definition with:

```scheme
(property "Footprint" "11-10084__speedy_processing_module__B:R0402_0.40MM_HD")
```

while the embedded schematic cache copy for the same symbol still contains:

```scheme
(property "Footprint" "wavenumber:R0402_0.40MM_HD")
```

That mismatch is not a live-placement link error, but KiCad compares the
embedded cache symbol against the library symbol referenced by the placed
`lib_id` and warns that the schematic copy does not match the library copy.

## Required Invariants

KCR must preserve all of these invariants in one source relink plan:

- Every placed schematic `lib_id` that maps to an extracted symbol uses the
  generated local symbol library nickname.
- Every placed schematic `Footprint` property that maps to an extracted
  footprint uses the generated local footprint library nickname.
- Every PCB footprint instance that maps to an extracted footprint uses the
  generated local footprint library nickname.
- Every embedded schematic cache parent symbol name that KCR localizes has
  direct child unit symbol names that use the localized member-name prefix.
- Every placed symbol cache lookup key, using `lib_name` when present and
  `lib_id` otherwise, exactly matches an embedded cache symbol name in the same
  schematic.
- Every embedded schematic cache field that KCR mutates in the generated local
  symbol library is mutated the same way in the embedded cache copy.

## Implementation Direction

Keep the implementation targeted. The first fix should not replace whole
embedded `lib_symbols` blocks from generated `.kicad_sym` files because the
schematic cache format has KiCad-schematic-specific atoms such as
`exclude_from_sim`, `in_pos_files`, `show_name`, and `do_not_autoplace`. Whole
block replacement risks losing or normalizing data that KiCad intentionally
keeps in schematic files.

Instead, extend the existing relink planner to operate on parent symbols under
`(kicad_sch (lib_symbols ...))` and apply the same reference rewrite rules that
are used elsewhere:

- Relink cache parent symbol names and direct child unit names as `.31` already
  does.
- Relink embedded cache parent `property "Footprint"` values through the same
  `footprint_member_map` used for placed schematic symbols.
- Only relink fields that are known to be generated-local-library mutations.
  Do not rewrite `ki_fp_filters` unless the generated symbol library also
  rewrites that field.
- Add distinct report change kinds such as `schematic_cache_symbol_footprint`
  so reviewers can separate embedded cache body edits from placed source edits.
- Preserve source newline style and avoid unrelated formatting churn.

If later investigation shows KiCad compares additional generated-library
mutations, add them explicitly with tests. Do not silently normalize arbitrary
property bodies.

## Validation Design

Add a cache body validation report to `source_relink.json`. It should be strict
enough to catch the Rev B failure before release:

- Report embedded cache symbol, file path, property name, old value, expected
  value, and whether the issue is repairable.
- Validate the planned post-relink text, not only the original text.
- Apply mode must block rather than write a partially localized project if
  cache-link, cache-unit, or cache-body validation has unresolved remaining
  issues.
- Update `docs/contracts/source_relink.a0.schema.json` and the `project-lib`
  design doc for the new validation field and change kinds.

Add a KiCad CLI project ERC hygiene gate for validation runs. This does not need
to treat ordinary electrical findings as failures; it should classify and fail
on library-hygiene findings only:

- `lib_symbol_issues`
- `footprint_link_issues`
- `lib_symbol_mismatch`
- text containing missing symbol, missing unit, not found, or invalid symbol
  unit-name wording

For Speedy Rev B, the expected post-localization target is `0` library-hygiene
ERC findings. In the current KiCad 10.0.5 environment, the remaining ordinary
electrical findings should be the same `189` non-library ERC findings observed
before and after the `.31` relink attempt.

## Test Plan

Add L0 synthetic coverage for:

- A placed symbol whose generated local symbol library has a localized
  `Footprint` property while the embedded cache initially still references the
  external footprint nickname.
- Multiple placed instances sharing one cache symbol so one cache body rewrite
  removes multiple KiCad mismatch warnings.
- Multipart symbols where cache parent names, unit child names, `lib_name`, and
  cache body `Footprint` values all change in the same plan.
- Dry-run report shape and apply-mode blocking when a cache body mismatch is
  not repairable.
- Idempotency: rerunning `--relink-sources --repair-cache-links` after a clean
  apply must produce no additional source changes.

Add focused L3 coverage using the existing redistributable KiCad corpus fixture
if it can demonstrate the mismatch. If not, keep the public L3 test targeted at
general project-local idempotency and use Speedy Rev B as the private validation
gate documented in the plan log.

## Speedy Validation

Use a disposable Speedy branch starting from `origin/main`. Run the fixed local
KCR checkout against `tracks/B` first:

```powershell
uv run python -m kicad_cruncher project-lib `
  tracks\B\11-10084__speedy_processing_module__B.kicad_pro `
  --relink-dry-run --repair-cache-links --validate-kicad-cli

uv run python -m kicad_cruncher project-lib `
  tracks\B\11-10084__speedy_processing_module__B.kicad_pro `
  --relink-sources --repair-cache-links --validate-kicad-cli
```

Then run KiCad CLI ERC against the localized Rev B schematic and classify the
JSON results. Release is blocked unless the library-hygiene class count is zero
and manual KiCad GUI inspection opens the hierarchical schematic without symbol
load errors or cache mismatch prompts caused by the localization.

## Release

After implementation, focused tests, Speedy Rev B validation, dev-std plan
audit, external review, and explicit publish approval, prepare a new
date-versioned `kicad-cruncher` release. Reinstall the uv tool and rerun Rev B
with the published package before considering the Speedy project patch ready for
commit.
