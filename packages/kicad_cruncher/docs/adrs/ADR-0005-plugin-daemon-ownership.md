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

## Consequences

The daemon is a localhost router, state holder, and no-build browser UI server.
It must not become a second implementation of cleanup, HLR, or config behavior.

The first release-facing tool is `pcb clean`. `schematic clean`, Footprint HLR,
viewer/inspection, and SVG config tooling follow after the PCB cleanup pattern
is proven through CLI, daemon, plugin, and Rack tests.
