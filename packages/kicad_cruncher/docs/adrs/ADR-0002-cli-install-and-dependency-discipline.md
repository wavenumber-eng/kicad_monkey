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

- `kicad-monkey==2026.6.13`
- `wn-geometer==2026.6.10`
- `openpyxl>=3.1.0`
- `colorama>=0.4.6`

`wn-geometer` is included for the public `pcb-svg` command's assembly HLR
projection path. It remains a pinned controlled Wavenumber dependency.
`openpyxl` is included for public BOM, PnP, and JLC XLSX workbook outputs.

The top-level CLI stays an orchestrator. Command behavior lives in
command-specific modules named `kicad_cruncher_cmd_<command>.py`.

Output-producing commands accept `-o/--output` as an output directory. When the
option is omitted, commands write under `./output/<command>/`. Passing
`-o/--output` replaces the whole command output directory.

## Consequences

Adding private workspace packages or heavy visualization dependencies to the
public runtime requires a new ADR or explicit design-doc update.
New output-producing commands should use the shared output-directory helper so
the default layout stays consistent across commands.
