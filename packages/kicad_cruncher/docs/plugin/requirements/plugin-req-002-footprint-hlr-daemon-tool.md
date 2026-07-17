+++
type = "requirement"
id = "plugin-req-002-footprint-hlr-daemon-tool"
domain = "plugin"
status = "active"
title = "Footprint HLR daemon and plugin flows remain deferred"
created = "2026-07-17"
issue_refs = ["wavenumber-eng/kicad_cruncher#8"]
verification_status = "unverified"
design_refs = [
  "docs/design/cli/daemon.html",
  "docs/design/cli/plugin.html",
  "docs/design/cli/pcb-svg.html",
]
+++

# Footprint HLR Daemon Tool

The future Footprint HLR tool must reuse existing `pcb-svg` pose and
`wn-geometer` projection behavior rather than implementing a second projection
path in the daemon or plugin shim.

Deferred obligations recovered from the deleted plugin-daemon plan:

- Add daemon preview and apply flows for footprint-local HLR.
- Preserve selected-footprint defaults and target-layer preview behavior.
- Add fixture-backed tests that validate daemon request and response shape
  without requiring a live KiCad editor.
- Keep live KiCad IPC validation optional outside default CI until a stable
  runner exists.

The durable command-line behavior remains the source of truth. Daemon endpoints
and KiCad IPC actions are adapters around shared command logic.
