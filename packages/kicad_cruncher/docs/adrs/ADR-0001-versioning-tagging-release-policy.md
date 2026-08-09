# ADR-0001: Versioning, Tagging, And Release Policy

Status: accepted
Date: 2026-05-31

## Context

`kicad-cruncher` is a public CLI package intended for PyPI distribution. Users
and downstream tools need a simple way to identify exactly which release they
are running.

## Decision

Use date-based versions in the form `YYYY.M.D` with an optional fourth build
component for same-day rebuilds. Release tags use `vYYYY.M.D`.

Release notes live under `docs/releases/YYYY-MM-DD.md`, and `CHANGELOG.md`
contains a matching `## YYYY.M.D` entry. The package exposes the version through
`kicad_cruncher.__version__`, `kicad_cruncher.version()`, `kicad-cruncher
--version`, and `kicad-cruncher version`.

Pushing a matching version tag, such as `v2026.6.25`, triggers GitHub Actions
publishing with PyPI Trusted Publishing/OIDC. A GitHub Release may be created
for release-note presentation, but it is not the publish trigger. Local Twine
upload is a fallback only.

## Consequences

CI and release workflows fail when the pushed tag, package metadata, changelog,
and dated release note disagree.
