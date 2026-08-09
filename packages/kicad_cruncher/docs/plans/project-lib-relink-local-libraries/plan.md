+++
type = "plan"
id = "project-lib-relink-local-libraries"
status = "active"
created = "2026-07-28"
issue_refs = ["wavenumber-eng/kicad_monkey#25", "wavenumber-eng/kicad_cruncher#8"]
design_refs = [
  "docs/design/cli/project-lib.html",
  "docs/contracts/source_relink.a0.schema.json",
  "docs/library/requirements/library-req-001-extraction-hardening.md",
]

[[steps]]
id = "work"
title = "Execute plan work"
status = "pending"

[[steps]]
id = "design-doc-intent-audit"
title = "Audit design docs, ADRs, and requirements against implementation"
status = "pending"
depends_on = ["work"]

[[steps]]
id = "test-runtime-impact-audit"
title = "Audit new test runtime impact"
status = "pending"
depends_on = ["work"]

[[steps]]
id = "external-review"
title = "Obtain independent external review"
status = "pending"
depends_on = ["work", "design-doc-intent-audit", "test-runtime-impact-audit"]

[[exit_criteria]]
id = "signoff"
title = "Focused signoff passes"
status = "pending"

[[exit_criteria]]
id = "design-doc-intent-audit"
title = "Design docs, ADRs, and requirements match implementation"
status = "pending"

[[exit_criteria]]
id = "test-runtime-impact-audit"
title = "New tests are listed and runtime impact is reviewed"
status = "pending"

[[exit_criteria]]
id = "external-review"
title = "Independent external review is complete"
status = "pending"

[[steps]]
id = "monkey-scanner"
title = "Patch kicad-monkey project file traversal to ignore KiCad history and generated backup folders"
status = "done"

[[steps]]
id = "extraction-maps"
title = "Expose deterministic generated symbol and footprint relink maps for project-lib output"
status = "done"

[[steps]]
id = "relink-option"
title = "Add explicit KCR project-lib source relink dry-run and apply paths"
status = "done"

[[steps]]
id = "speedy-validation"
title = "Validate extraction and relink behavior on Speedy rev A and rev B copies"
status = "done"

[[steps]]
id = "dev-std-2026-07-18"
title = "Update KCR governance and signoff to wn-dev-std 2026.7.18"
status = "done"
+++

# Project-local library relink hardening

Harden KCR project-lib for project-local library extraction and explicit source relinking. The workflow must ignore KiCad history and backup artifacts, keep source mutation opt-in and reviewable, preserve generated symbol and footprint metadata, and validate against the Speedy rev A/B local-library use case.
