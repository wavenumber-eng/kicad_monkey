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

Immutable package-qualified tags for both packages, such as
`kicad-monkey-v2026.8.22` and `kicad-cruncher-v2026.8.22`, authorize a
coordinated release from their shared commit. One manual repository-level
GitHub Actions workflow dispatch promotes the exact successful CI candidates
through PyPI Trusted Publishing/OIDC and then creates the GitHub Releases.
Local Twine upload is a fallback only. Historical standalone `vYYYY.M.D` tags
remain in the retired repository.

## Consequences

CI and release workflows fail when either authorizing tag, package metadata,
changelog, dated release note, or selected CI commit disagrees.
