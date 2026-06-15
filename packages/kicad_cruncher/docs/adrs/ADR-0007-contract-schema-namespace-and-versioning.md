# ADR-0007: Contract Schema Namespace And Versioning

## Status

Accepted.

## Context

`kicad-cruncher` emits JSON contracts for command configs, reports, daemon
payloads, manifests, and mutation requests. Early pre-release contracts mixed
`wn.`, `wavenumber.`, and `kicad_cruncher.` prefixes, and initial versions used
`v0`, `v1`, or `v2` depending on the feature.

That made generated artifacts harder to reason about and harder to test as
stable public contracts.

## Decision

New or renamed `kicad-cruncher` owned contract schema identifiers use:

- the `kicad_cruncher.` namespace
- an `.a0` initial version suffix

External or intentionally generic namespaces are exceptions only when explicitly
approved. Current approved exceptions are:

- `pcb.svg.manifest.a0`
- `geometry.planar_step.request.a0`

Config file names may stay command-oriented, but the `schema` value inside
generated JSON and JSONC artifacts must follow this policy.

## Consequences

Generated artifacts and tests must not introduce the old `wn.` or
`wavenumber.` prefixes on `kicad_cruncher` schema names.

Compatibility aliases are not added for pre-release schema spellings unless a
release explicitly requires them.
