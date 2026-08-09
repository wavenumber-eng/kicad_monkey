+++
type = "requirement"
id = "release-req-001-governed-pypi-release-signoff"
domain = "release"
status = "implemented"
title = "PyPI release candidates pass governed release signoff"
created = "2026-07-17"
issue_refs = ["wavenumber-eng/kicad_cruncher#8"]
design_refs = [
  "docs/release-process.md",
  "docs/governance/release.toml",
]

[[verification_refs]]
kind = "local_pytest"
target = "tests/L99_signoff/test_L99_001_release_signoff.py::test_configured_dev_std_audit_scopes_pass"
rationale = "L99 release signoff runs the configured dev-std governance scopes, including CLI, plan, requirement, and release governance."

[[verification_refs]]
kind = "local_pytest"
target = "tests/L99_signoff/test_L99_001_release_signoff.py::test_dev_std_upstream_version_is_current"
rationale = "L99 release signoff checks the configured standard against the latest upstream dev-std release."
+++

# Governed PyPI Release Signoff

Before a `kicad-cruncher` release candidate can be considered release-ready, the
configured dev-std audit must pass for repository, CI, design-doc, link, CLI,
plan, requirement, and release-governance scopes.

The release channel is PyPI. The release process remains tag-driven through the
GitHub Actions release workflow and PyPI Trusted Publishing. Local Twine upload
is fallback only.
