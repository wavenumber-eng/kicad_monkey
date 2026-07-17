+++
type = "requirement"
id = "cli-req-001-public-command-documentation"
domain = "cli"
status = "implemented"
title = "Public CLI commands have governed documentation and inventory signoff"
created = "2026-07-17"
issue_refs = ["wavenumber-eng/kicad_cruncher#8"]
design_refs = [
  "docs/design/cli/index.html",
  "docs/contracts/command_manifest.a0.json",
]

[[verification_refs]]
kind = "local_pytest"
target = "tests/L99_signoff/test_L99_002_design_docs.py::test_cli_command_inventory_matches_parser_manifest_and_design_docs"
rationale = "The L99 design-doc signoff verifies parser help, command manifest, CLI design index, and per-command design docs expose the same command set."

[[verification_refs]]
kind = "local_pytest"
target = "tests/L99_signoff/test_L99_002_design_docs.py::test_cli_commands_have_matching_design_docs"
rationale = "The L99 design-doc signoff verifies every public command has an accepted design doc with usage, argument, output, test, and config-contract sections."
+++

# Public CLI Command Documentation

Every public root command declared by `kicad-cruncher` must have a governed
manifest entry, a row in the CLI design index, an accepted per-command design
document, README command-table coverage, and parser help visibility.

The command manifest uses `wn_dev_std.command_manifest.a0` so dev-std can audit
the public CLI inventory directly. Project-specific L99 tests continue to check
the stricter local invariant that parser help, README, the CLI index, command
modules, and per-command design docs stay synchronized.
