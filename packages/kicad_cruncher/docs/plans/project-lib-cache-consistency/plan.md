+++
type = "plan"
id = "project-lib-cache-consistency"
status = "active"
created = "2026-07-30"
issue_refs = ["wavenumber-eng/kicad_cruncher#8", "wavenumber-eng/kicad_monkey#25"]
design_refs = [
  "docs/design/cli/project-lib.html",
  "docs/contracts/source_relink.a0.schema.json",
  "docs/library/requirements/library-req-001-extraction-hardening.md",
]

[[steps]]
id = "work"
title = "Execute plan work"
status = "done"

[[steps]]
id = "design-doc-intent-audit"
title = "Audit design docs, ADRs, and requirements against implementation"
status = "done"
depends_on = ["work"]

[[steps]]
id = "test-runtime-impact-audit"
title = "Audit new test runtime impact"
status = "done"
depends_on = ["work"]

[[steps]]
id = "external-review"
title = "Obtain independent external review"
status = "done"
depends_on = ["work", "design-doc-intent-audit", "test-runtime-impact-audit"]

[[exit_criteria]]
id = "signoff"
title = "Focused signoff passes"
status = "met"

[[exit_criteria]]
id = "design-doc-intent-audit"
title = "Design docs, ADRs, and requirements match implementation"
status = "met"

[[exit_criteria]]
id = "test-runtime-impact-audit"
title = "New tests are listed and runtime impact is reviewed"
status = "met"

[[exit_criteria]]
id = "external-review"
title = "Independent external review is complete"
status = "met"

[[steps]]
id = "dev-std-baseline"
title = "Verify kicad-monkey and kicad-cruncher target the latest wn-dev-std release"
status = "done"

[[steps]]
id = "branch-setup"
title = "Use a dedicated KCR fix branch and a disposable Speedy Rev B validation branch"
status = "done"

[[steps]]
id = "scanner-baseline"
title = "Confirm project-lib uses kicad-monkey project traversal that ignores KiCad history, autosave, and backup folders"
status = "done"
depends_on = ["dev-std-baseline"]

[[steps]]
id = "cache-link-validator"
title = "Add exact placed lib_name to embedded lib_symbols consistency validation"
status = "done"
depends_on = ["scanner-baseline"]

[[steps]]
id = "relink-reporting"
title = "Expose cache-link validation results in source relink reports and apply-mode failure behavior"
status = "done"
depends_on = ["cache-link-validator"]

[[steps]]
id = "repair-option"
title = "Add a guarded repair/localization option only if it preserves lib_name and cache symbol names as a pair"
status = "done"
depends_on = ["cache-link-validator"]

[[steps]]
id = "unit-tests"
title = "Add synthetic tests for the IC1 failure pattern and KiCad alias cache edge cases"
status = "done"
depends_on = ["cache-link-validator", "relink-reporting"]

[[steps]]
id = "speedy-rev-b-validation"
title = "Run the fixed KCR workflow against a Speedy Rev B validation branch and inspect in KiCad"
status = "done"
depends_on = ["unit-tests", "repair-option"]

[[steps]]
id = "release-prep"
title = "Prepare changelog, version bump, build artifacts, and PyPI release for a new kicad-cruncher version"
status = "active"
depends_on = ["speedy-rev-b-validation", "design-doc-intent-audit", "test-runtime-impact-audit", "external-review"]

[[exit_criteria]]
id = "dev-std-current"
title = "kicad-monkey and kicad-cruncher target the latest reviewed wn-dev-std release"
status = "met"

[[exit_criteria]]
id = "cache-link-invariant"
title = "KCR detects or prevents placed lib_name values that do not exactly match embedded lib_symbols cache names"
status = "met"

[[exit_criteria]]
id = "cache-unit-invariant"
title = "KCR detects or prevents embedded cache unit names that KiCad's schematic loader rejects"
status = "met"

[[exit_criteria]]
id = "speedy-rev-b-validation"
title = "Speedy Rev B validation branch opens cleanly enough for manual KiCad inspection with no IC1 missing-sub-symbol regression"
status = "met"

[[exit_criteria]]
id = "release-published"
title = "A new kicad-cruncher package version is built, verified, and published after validation"
status = "pending"
+++

# Project-local library cache consistency

Harden kicad-cruncher project-lib so source relinking and any schematic cache localization preserve KiCad's placed symbol to embedded library symbol invariants. The work uses kicad-monkey 2026.7.28 or newer for project scan hygiene, validates against a Speedy Rev B test branch before release, and publishes a new kicad-cruncher PyPI release only after focused signoff and manual KiCad inspection.

## Baseline

The local kicad-monkey checkout is on `fix/project-file-scan-history` and already declares `wn-dev-std>=2026.7.18` with `standard_version = "2026.7.18"`. The local kicad-cruncher checkout is based on `feat/project-lib-relink`, depends on `kicad-monkey>=2026.7.28`, and also declares `wn-dev-std>=2026.7.18` with `standard_version = "2026.7.18"`. PyPI currently resolves `wn-dev-std 2026.7.18` through `uvx --from wn-dev-std==2026.7.18 dev-std --version`.

The installed user `kcr` resolves to a uv tool environment with `kicad-cruncher 2026.7.17` and `kicad-monkey 2026.7.28`. That means the KiCad `.history` and backup traversal fix is present in the current installed runtime, but the cache-link validation described here is not.

## Problem

The relink workflow must not leave KiCad schematics in a state where a placed symbol contains `(lib_name "Alias")` while the embedded cache contains only `(symbol "local:Alias" ...)`. KiCad 10.0.5 treats those aliases as missing units for multi-part symbols such as the Zynq IC1. kicad-monkey's tolerant lookup can mask the issue because it may resolve by suffix, so KCR needs an exact KiCad-facing validation step.

## Implementation Notes

The default `project-lib` workflow should keep source mutation explicit and reviewable. Cache-localization or cache-repair behavior must be opt-in unless the implementation is purely diagnostic. If KCR rewrites embedded schematic cache symbol names, it must update all matching placed `lib_name` values in the same schematic at the same time. Apply mode should fail rather than produce local-library links that KiCad cannot resolve.

The validation report should be durable enough for the Speedy workflow: file path, reference designator when present, placed `lib_name`, exact-match status, and any prefixed/suffix candidate cache names. Dry-run can report pre-existing issues. Apply mode should either repair them through an explicit option or stop with a clear error.

## Speedy Validation

Use a disposable branch in `D:\prj\magnitude_instruments\speedy\speedy_processing_module` starting from clean Rev B baseline state. Run KCR from the local Cruncher checkout with `uv run python -m kicad_cruncher`, not the installed `kcr` shim, until the package is released. Validate with generated `source_relink.json`, KiCad CLI ERC where useful, and manual KiCad inspection before publishing.

## Release

After focused tests, Speedy Rev B validation, external review, and release-facing signoff pass, prepare a new date-versioned `kicad-cruncher` release, publish it to PyPI, and reinstall the uv tool so `kcr --version` reports the released version and `kicad-monkey 2026.7.28` or newer.
