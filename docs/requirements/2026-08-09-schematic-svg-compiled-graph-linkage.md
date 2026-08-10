# 2026-08-09 Schematic SVG Compiled-Graph Linkage

Status: implemented; release pending

## Requirement

KiCad Monkey enriched schematic SVG must provide an occurrence-scoped,
machine-validated navigation surface from source-owned SVG ids to the accepted
compiled schematic graph. The graph remains variant-neutral and authoritative;
the SVG view must not allocate semantic identity or infer connectivity from
names, text, geometry, or DOM structure.

For every compiled graph drawing link emitted for `sch.dwg_scene`, the selected
element id must occur exactly once in the owning page SVG. Non-rendered pins
must not claim drawing evidence. Repeated overplot rendering must not duplicate
the primary canonical id. Reused sheet occurrences must resolve through their
distinct page occurrence even when their source object ids are shared.

## Acceptance

- L0 owns page resolution, deterministic forward/reverse indexes, reused-page
  scoping, schema validation, wrong-page rejection, and missing/ambiguous SVG
  selector rejection.
- L0 renderer/netlist tests own zero-length non-rendered pin suppression and
  overplot id uniqueness.
- L3 rendering owns actual graph-to-SVG selector resolution across the public
  real-project manifest.
- Semantic component, terminal, local-net, hierarchy, and binding identities
  remain unchanged; only incorrect drawing-evidence rows are suppressed.

The public helpers and constants are exported from `kicad_monkey`; downstream
workflow packages may package and validate the projection without importing
KiCad source-model internals.
