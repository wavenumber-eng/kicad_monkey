# ADR-0002: CLI Install And Dependency Discipline

Status: accepted
Date: 2026-05-31

## Context

`kicad-cruncher` is the higher-level command package above `kicad-monkey`. It
should be easy to install as a user tool and should not pull private workspace
packages into the public runtime.

## Decision

The public install path is `uv tool install kicad-cruncher`. Runtime
dependencies must stay public, pinned where they are controlled Wavenumber
packages, and justified by command behavior.

The first runtime dependency set is intentionally narrow:

- `kicad-monkey==2026.5.31`
- `colorama>=0.4.6`

The top-level CLI stays an orchestrator. Command behavior lives in
command-specific modules named `kicad_cruncher_cmd_<command>.py`.

## Consequences

Adding private workspace packages or heavy visualization dependencies to the
public runtime requires a new ADR or explicit design-doc update.
