# ADR-0001: Versioning, Tagging, And Release Policy

Status: accepted
Date: 2026-05-31

## Context

`kicad-cruncher` is a public CLI package intended for PyPI distribution. Users
and downstream tools need a simple way to identify exactly which release they
are running.

## Decision

Use date-based versions in the form `YYYY.M.D` with an optional fourth build
component for same-day rebuilds. New monorepo release tags use
`kicad-cruncher-vYYYY.M.D`.

Release notes live under `docs/releases/YYYY-MM-DD.md`, and `CHANGELOG.md`
contains a matching `## YYYY.M.D` entry. The package exposes the version through
`kicad_cruncher.__version__`, `kicad_cruncher.version()`, `kicad-cruncher
--version`, and `kicad-cruncher version`.

Publishing a GitHub Release for a matching package-qualified tag, such as
`kicad-cruncher-v2026.8.9`, triggers the repository-level GitHub Actions
workflow and PyPI Trusted Publishing/OIDC. Local Twine upload is a fallback
only. Historical standalone `vYYYY.M.D` tags remain in the retired repository.

## Consequences

CI and release workflows fail when the pushed tag, package metadata, changelog,
and dated release note disagree.
