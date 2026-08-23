+++
type = "requirement"
id = "release-req-001-governed-pypi-release-signoff"
domain = "release"
status = "implemented"
title = "Python and native release candidates pass governed release signoff"
created = "2026-07-17"
issue_refs = ["wavenumber-eng/kicad_cruncher#8"]
design_refs = [
  "docs/release-process.md",
  "docs/governance/release.toml",
  "docs/design/rust-cli-phase7-audit.html",
  "docs/contracts/rust_cli_windows_x64.md",
]

[[verification_refs]]
kind = "local_pytest"
target = "tests/L99_signoff/test_L99_001_release_signoff.py::test_configured_dev_std_audit_scopes_pass"
rationale = "L99 release signoff runs the configured dev-std governance scopes, including CLI, plan, requirement, and release governance."

[[verification_refs]]
kind = "local_pytest"
target = "tests/L99_signoff/test_L99_001_release_signoff.py::test_dev_std_upstream_version_is_current"
rationale = "L99 release signoff checks the configured standard against the latest upstream dev-std release."

[[verification_refs]]
kind = "local_pytest"
target = "tests/L3_public_workflows/test_L3_012_rust_cli_install.py::test_installed_rust_cli_runs_design_without_python"
rationale = "L3 installs and executes the Rust design CLI, then verifies the commit/version/hash-bound Windows candidate and its rejection paths."
+++

# Governed Python And Native Release Signoff

Before a `kicad-cruncher` release candidate can be considered release-ready,
the package-configured dev-std audit must pass for repository, design-doc,
link, CLI, plan, requirement, and release-governance scopes. The monorepo-root
signoff separately owns the CI audit. The Python package, Rust crate, release
tag, native archive, and candidate manifest must name one exact Cruncher
version and source commit.

The universal Python distribution is published through PyPI. The promoted
Windows x64 design CLI is a companion GitHub Release archive. Immutable tags
for both Python packages authorize one manual GitHub Actions release dispatch,
which uses PyPI Trusted Publishing and promotes only the exact successful CI
candidates. Public PyPI filenames and SHA256 digests are checked against those
candidates before GitHub Releases are created and the native archive is
attached. Local Twine upload is fallback only and does not substitute for
candidate verification.
