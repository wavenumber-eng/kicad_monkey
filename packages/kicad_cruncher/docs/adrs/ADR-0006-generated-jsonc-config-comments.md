# ADR-0006: Generated JSONC Config Comments

## Status

Accepted.

## Context

Public command configs are edited by humans and by agents. Bare JSON defaults
make it too easy to miss valid string values, enum modes, units, coordinate
assumptions, and removed field names. Hand-written comment blocks also drift
from the parser when fields are added.

## Decision

Commands that generate editable configuration must generate JSONC from
structured payload data plus structured comment metadata. This is the rule for
all command configs, including BOM/PnP/JLC, pcb-svg, pcb clean, and
pcb-layer-step. Generated JSONC must include:

- a top-level behavior summary;
- comments for every supported top-level and nested config field emitted in the
  default template;
- explicit option lists for every string/enum field;
- clear terminology that matches the parser and public schema;
- no documentation of removed compatibility aliases as preferred usage.

Runtime parsers must require the current schema id for public config files.
When the project intentionally breaks a config contract, parsers must reject
removed fields with actionable replacement messages instead of silently
accepting old spelling or probing old default file names.

## Consequences

The default config output becomes part of the public interface. Signoff tests
must parse generated JSONC defaults and verify option comments are present for
all config producers. Schemas and CLI design docs must use the same vocabulary
as the generated config. For pcb-layer-step v2, color and body policy lives
under `features.*`, pad highlighting lives under
`features.component_pads.highlight_rules`, and STEP body ids use
`step_body_name`.
