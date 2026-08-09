# ADR-0005: Plugin Daemon Ownership

Status: accepted
Date: 2026-06-05

## Context

The first KiCad IPC plugin prototype lived in `appz` so workspace setup could
prove installation, KiCad IPC discovery, browser UI, selected-footprint access,
and undoable editor mutation. That location is no longer the right public
boundary. Wavenumber needs a family of open-source KiCad tools with matching
CLI, daemon, plugin, config, docs, and tests.

## Decision

Public KiCad IPC plugin architecture belongs in `kicad-cruncher`.

`kicad-monkey` remains the reusable low-level KiCad parsing, source-model, and
rendering package. `wn-geometer` remains the geometry/projection package.
`appz` may call public commands for Wavenumber workstation setup, but must not
own durable plugin architecture, plugin implementation code, plugin installers,
or plugin planning docs.

Every plugin GUI tool must have a matching CLI command. The CLI command owns
the durable behavior contract, accepts JSONC config when the behavior is
configurable, and produces deterministic reports for tests and automation.
Daemon routes and KiCad IPC actions are adapters over the same shared Python
logic.

For live KiCad editor mutations, the daemon returns deterministic mutation
requests and the KiCad IPC plugin applies them under KiCad's commit/undo model.
The daemon may apply direct file mutations only for explicit file-mode CLI or
automation workflows; it must not write a board file behind an open KiCad editor.

KiCad plugin bundle identifiers and user-facing plugin names are Wavenumber
branded because the installed plugin package identifies the Wavenumber-owned
tool family. Runtime contracts, command/config schemas, daemon state, mutation
requests, reports, and future board-persistent metadata use the generic
`kicad_cruncher.*` schema namespace and `KICAD_CRUNCHER_*` field/env names. The
appz prototype `ALX_HLR_META` field and `wavenumber.kicad_footprint_hlr.*`
metadata schema are not carried forward into new public `kicad-cruncher`
plugin behavior.

## Consequences

The daemon is a localhost router, state holder, and no-build browser UI server.
It must not become a second implementation of cleanup, HLR, or config behavior.

The first release-facing tool is `pcb clean`. `schematic clean`, Footprint HLR,
viewer/inspection, and SVG config tooling follow after the PCB cleanup pattern
is proven through CLI, daemon, plugin, and Rack tests.

New board-persistent metadata must be added deliberately and tested before a
public release, because KiCad plugin identifiers and hidden fields are difficult
to rename once user boards contain them.
