# ADR-0004: Design Documentation And Test Ownership Signoff

Status: accepted
Date: 2026-05-31

## Context

Public commands, config formats, JSON outputs, dataclasses, and major
interfaces need durable design documentation so users and future maintainers can
understand intended behavior before changing it.

## Decision

Design documentation is part of release signoff.

Every public CLI command in `docs/contracts/command_manifest.v0.json` must have
a matching HTML design document:

- path: `docs/design/cli/<command-name>.html`;
- filename matches the command name exactly;
- document includes `data-command="<command-name>"`;
- document includes `usage`, `arguments`, `output`, and `tests` sections;
- document declares `data-config-contract="none"` or names the machine-readable
  config/output contract it uses.

`docs/design/index.html` is the master human and machine entry point.
`docs/design/styles.css` is the shared style file. Design HTML should remain
simple, monochrome, monospace, and easy to parse with text or HTML tooling.

Every public dataclass must have a machine-readable design section in
`docs/design/api/*.html`. Major public interfaces that are not dataclasses are
listed in `docs/contracts/interface_design_manifest.v0.json`.

## Consequences

`L99_signoff` fails when command design docs, interface design docs, or test
ownership links are missing.

