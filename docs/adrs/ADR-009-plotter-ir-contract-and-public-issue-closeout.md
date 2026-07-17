# ADR-009: Plotter IR Contract And Public Issue Closeout

## Status

Accepted

## Date

2026-07-17

## Context

The 2026-07-16 release adds public behavior for the Plotter IR
contract and public issues #9, #10, #11, and #12. The implementation was planned
and reviewed in a developer working plan, but ADR-003 treats active plans and
research notes as transient material. Durable release records belong in ADRs,
requirements, contracts, design docs, tests, and release notes.

The main decisions that need to survive plan closeout are:

- the IR contract is published as `kicad.plotter_ir.a0`;
- new IR files emit the a0 schema id while readers accept legacy
  `kicad.plotter_ir.v1` inputs;
- optional `context` objects are allowed at document, record, and operation
  level, with `context.hyperlink.href` standardized for linked text;
- KiCad preference setup reads supplied source files and avoids profile-specific
  defaults in `kicad_monkey`;
- design JSON component pin counts are computed from one terminal index without
  changing the public JSON contract;
- release publishing remains separate from review approval and requires explicit
  user authorization.

## Decision

Close the public-issue working plan by deleting the tracked plan and review
summary after moving durable content into public artifacts.

The durable artifact set for this work is:

- `docs/contracts/kicad_plotter_ir_a0.schema.json`;
- `docs/design/kicad-plotter-ir.html`;
- `docs/requirements/2026-07-16-public-issue-requirements.html`;
- `docs/releases/2026-07-16.md`;
- `CHANGELOG.md`;
- regression tests covering schema validation, hyperlink context, source-driven
  preferences, pin-count behavior, release signoff, and dev-std audit wiring.

`kicad_cruncher` may receive its IR export workflow only after a published
`kicad_monkey` release is available. The cruncher branch may still carry
signoff and dev-std baseline changes before that release.

## Consequences

- Public package artifacts retain the contract, design, requirement, and release
  record without shipping active planning notes.
- Review evidence that was useful during implementation is summarized in durable
  requirements and release notes instead of remaining as a research file.
- Future public issue plans should be closed the same way: move decisions into
  durable docs, delete transient plan/research notes, then push for PR and CI
  validation when authorized.
- Publishing, tagging, and creating a GitHub release still require a separate
  explicit authorization step.
