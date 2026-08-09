# KiCad compiled schematic graph contract

Status: experimental a0 producer contract (2026-08-08).

`KiCadDesign.to_json()` embeds a complete, variant-neutral schematic graph at
`compiled_schematic_graph`. The section has:

- `schema`: `kicad_monkey.compiled_schematic_graph.a0`
- `type`: `sch.compiled_schematic_graph`
- `identity_namespace`: `sch.compiled_schematic_graph.a0`

The collection names and row `type` tokens intentionally match the downstream
compiled-graph cardinalities: unit and page definitions, unit and
page occurrences, hierarchy occurrences, component occurrences, local-net
occurrences, terminal occurrences, scalar hierarchy-terminal bindings, and
scoped graphical-artifact links.

## Identity boundary

All graph row IDs are deterministic UUIDv7 values allocated from a portable
project scope, a governed object type, and source occurrence selectors. KiCad
UUIDs and paths remain provenance in `source_identity`; they are not canonical
runtime identity. Display names, designators, net names, sequential net codes,
list positions, and drawing geometry do not identify semantic rows.

Identity allocation is a package-local copy of the governed generic allocator:
the normalized source selector and canonical owner refs are wrapped exactly as
they are in `data_models`, without a runtime dependency on Appz. Definition
identity uses the portable schematic source path and source UUID. Occurrence
identity uses KiCad's realized instance path. The public
`sheet_path_uuids` value is the cross-package occurrence selector, while the
full root-UUID-prefixed instance path remains source provenance. Local-net
identity is topology-derived from sorted canonical terminal refs within the
page occurrence. Only a terminal-free graphical island falls back to sorted,
scoped drawing selectors. Consequently, editing a wire UUID cannot replace a
terminal-bearing local net.

The downstream importer preserves producer graph UUIDs and maps optional Design
component, pin, and net refs exactly once from `source_identity` selectors. It
must not fall back to
designator, displayed net name, bare source UUID, or drawing geometry. A pin
number may only select a Design pin after its owning component occurrence has
already resolved by source identity.

## Variant and policy boundary

The graph contains all realized source occurrences, including sheets and
symbols marked DNP, excluded from BOM, or excluded from the board. Those flags
do not filter graph topology or change graph identity. Raw and effective DNP,
BOM, board, simulation, and assembly-variant state belong to the KiCad Design
sidecar and are applied by the consumer/viewer.

## Drawing boundary

`sch.graphical_artifact_link` selects an element by the tuple
`page_occurrence_ref + artifact_key + element_id`. Bare drawing IDs are not
globally resolvable. Links target component, hierarchy, terminal, or local-net
occurrences only when the KiCad compiler has authoritative ownership evidence.
Global labels are page-port terminals. Because aggregate interface rows are
deferred in a0, buses and bus entries link to their owning page occurrence
rather than being misrepresented as one scalar local net.
Hidden pins have no pin-level link. A stacked-pin drawing shared by multiple
scalar terminals is deliberately left unlinked rather than made ambiguous.

The a0 contract supports scalar sheet-entry to child-port bindings. Aggregate
bus and harness member bindings require a later versioned contract. Missing
scalar boundary matches fail closed through governed resolution diagnostics;
the producer validator enforces role, page/unit ownership, direction, and
binding-or-diagnostic completeness.

## Compatibility

Additive fields may be introduced within a0. Removing or reinterpreting fields,
changing identity inputs, changing collection cardinalities, or adding aggregate
binding semantics requires a new schema token. Existing `indexes` data remains
a derived compatibility view during consumer cutover and is not an independent
connectivity truth.
