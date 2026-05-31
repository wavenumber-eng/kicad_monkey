# ADR-0003: Documentation Lifecycle And Release Boundary

Status: accepted
Date: 2026-05-31

## Context

Development plans and research notes are useful while building a feature, but
public packages need durable documentation that describes the accepted behavior.

## Decision

Durable public documentation lives in `README.md`, `CONTRIBUTING.md`,
`docs/adrs/`, `docs/design/`, `docs/contracts/`, and `docs/releases/`.

Developer-only plans and research notes are excluded from source distributions
through the hatch build configuration. Completed plan outcomes should move into
ADRs, design docs, contracts, or release notes before release.

## Consequences

Release signoff checks that plans and research docs are not packaged as public
artifacts.

