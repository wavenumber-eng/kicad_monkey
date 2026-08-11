# ADR-0008: Altium Asset Conversion Ownership

## Status

Accepted.

## Context

`kicad-monkey` historically exposed `PartKiCadConverter`, which combined
application-shaped Part field access, workspace output policy, `kicad-cli`
subprocess orchestration, optional cleanup filters, and direct destination
replacement. The only known production consumers are Lib Cruncher's HTTP and
batch Altium-to-KiCad workflows.

That surface violates the parser package boundary, permits application models
to cross into a public file-format package, silently selects the first output
from multi-entry footprint libraries, and deletes existing output before a
replacement has passed normalization and validation.

## Decision

The conversion path is split into three layers:

- `kicad-monkey` owns dependency-light KiCad source parsing and format-level
  normalization. Its direct pad-size normalizer matches KiCad's native rule:
  if either direct size axis is nonpositive, both axes become 0.001 mm.
- `kicad-cruncher` owns typed, model-agnostic `kicad-cli` execution, exact
  native asset selection, staging, mandatory normalization, discretionary
  cleanup, validation, diagnostics, and atomic publication.
- downstream applications own workspace selection, provenance and overwrite
  policy, business-model projection, user workflow behavior, and persistence.

Public conversion requests and results contain explicit paths, native keys,
policies, stages, and diagnostics. They never contain an ALX Part, Part-shaped
facade, company workspace setting, or application field-name dictionary.

Mandatory compatibility normalization and output validation are not optional
filters. Existing destinations are not removed before a staged replacement is
ready to publish. Multi-entry inputs require an exact native key and never
fall back to the first generated asset.

`kicad-cruncher` may depend on the documented public `kicad-monkey` API.
`kicad-monkey` must not depend on `kicad-cruncher`.

## Consequences

Lib Cruncher must project canonical CAD options into explicit conversion
requests and persist successful results through its governed mutation layer.
`PartKiCadConverter` and the Lib Cruncher Part adapter are removed after the
new public executor is released and the consumer has cut over.

Conversion tests must inject failures at each stage and prove existing output
is unchanged. Public fixtures remain synthetic; private library corpus results
are recorded only as aggregate validation evidence.
