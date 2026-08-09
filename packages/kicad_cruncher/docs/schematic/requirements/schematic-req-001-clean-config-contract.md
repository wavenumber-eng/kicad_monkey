+++
type = "requirement"
id = "schematic-req-001-clean-config-contract"
domain = "schematic"
status = "active"
title = "Schematic cleanup requires a documented JSONC config contract before mutation behavior"
created = "2026-07-17"
issue_refs = ["wavenumber-eng/kicad_cruncher#8"]
verification_status = "unverified"
design_refs = [
  "docs/design/cli/schematic.html",
]
+++

# Schematic Clean Config Contract

`kicad-cruncher schematic clean` is currently a deferred command group. Before
schematic cleanup becomes public mutation behavior, it must define a documented
`schematic.clean.config.a0` JSONC contract and tests for the generated default
config.

The future cleanup contract must cover:

- parameter coalescing;
- generated-field and cruft deletion policy;
- safe schematic formatting normalization;
- explicit dry-run and apply behavior; and
- the same CLI/config/daemon/plugin separation already used by PCB Clean.

This requirement preserves the active deferred item from the deleted
`docs/plans/kicad-plugin-daemon-framework.md` plan.
