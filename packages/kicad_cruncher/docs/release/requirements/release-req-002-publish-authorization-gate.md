+++
type = "requirement"
id = "release-req-002-publish-authorization-gate"
domain = "release"
status = "active"
title = "Publishing requires external review and explicit user authorization"
created = "2026-07-17"
issue_refs = ["wavenumber-eng/kicad_cruncher#8"]
verification_status = "unverified"
design_refs = [
  "docs/release-process.md",
  "docs/governance/release.toml",
]
+++

# Publish Authorization Gate

No `kicad-cruncher` tag, GitHub Release, or PyPI publish may occur from this
work until the implementation has an external review and the user explicitly
authorizes release.

This requirement is intentionally active and process-verified. It remains
unverified until the external review and authorization happen for a specific
release candidate.
