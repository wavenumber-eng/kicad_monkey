# 2026-07-16 Public Issue Review Summary

This note records the external review outcomes for the active
`kicad-ir-contract-devstd-public-issues` plan. It is developer-only evidence
and remains excluded from release artifacts by the sdist policy.

## Slice Reviews

- Plan review: approved after branch tracking, forced plan inclusion,
  publish-authorization separation, and exit-criteria hygiene were clarified.
- Dev-std signoff review: approved configured audit scopes and documented
  deferred governance scopes.
- Plotter IR contract review: approved `kicad.plotter_ir.a0`, the JSON Schema,
  accepted HTML reference, and legacy `kicad.plotter_ir.v1` reader
  compatibility.
- Text hyperlink review: approved preserving KiCad `href` metadata as
  `context.hyperlink.href` on text operations without requiring renderers to
  understand hyperlink context.
- Source-driven preferences review: initially rejected because a local text
  editor fallback could clobber command-style editors; approved after the
  fallback was removed from preference setup and regression tests were added.
- Design JSON pin-count review: initially rejected because empty designators
  no longer matched prior exact-reference behavior; approved after indexing
  terminals by exact designator, including the empty string.
- Release-candidate review: initially rejected because the publish workflow
  parsed alpha versions with `map(int, version.split("."))`; approved after the
  workflow used `parse_version(...).release_date` and L99 covered that path.

## Current Release Boundary

The release candidate is prepared as `2026.7.16a0`, but no publish action is
authorized. Publishing, tagging, and creating a GitHub release remain blocked
until explicit user authorization.
