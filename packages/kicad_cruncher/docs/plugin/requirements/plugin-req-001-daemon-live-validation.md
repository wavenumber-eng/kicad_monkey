+++
type = "requirement"
id = "plugin-req-001-daemon-live-validation"
domain = "plugin"
status = "active"
title = "Daemon and plugin apply paths require live validation before production-ready status"
created = "2026-07-17"
issue_refs = ["wavenumber-eng/kicad_cruncher#8"]
verification_status = "unverified"
design_refs = [
  "docs/design/cli/daemon.html",
  "docs/design/cli/plugin.html",
  "docs/design/cli/pcb.html",
]
+++

# Daemon And Plugin Live Validation

The baseline daemon, plugin installer, plugin action, PCB Clean CLI path, daemon
PCB Clean endpoint, no-build tool center, daemon discovery state, and mocked
KiCad IPC apply adapter are implemented. Before the live KiCad workflow is
called production-ready, the remaining manual validation obligations recovered
from the deleted `docs/plans/kicad-plugin-daemon-framework.md` plan must be
completed:

- Validate the daemon web UI direct-file apply path on copied real boards.
- Validate live KiCad editor apply from the installed plugin through the daemon
  mutation request path.
- Validate that a fresh public install can install the plugin, start or discover
  the daemon from the toolbar action, and execute cleanup from KiCad on a copied
  validation board.
- Validate daemon auto-start from the packaged command on a normal workstation
  install.
- Defer shared no-build UI primitive factoring until a second real tool tab
  needs the shared modal, toast, panel, or command-surface behavior.

These obligations were active roadmap items, not release-complete behavior.
They remain outside default CI until a stable live KiCad runner exists.
