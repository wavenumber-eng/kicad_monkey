# KiCad compiled schematic graph contract

Status: accepted a0 producer contract (2026-08-09).

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

Identity allocation is a package-local copy of the governed generic allocator.
The normalized source selector and canonical owner refs use the governed
address shape while the producer remains independently installable. Definition
identity uses the portable schematic source path and source UUID. Unit
occurrence identity uses the complete realized `sheet_path_uuids` path before
the reusable placement UUID and retains the released a0 unowned address. The
same nested placement inside two reused parent occurrences therefore remains
distinct, and adding or removing a reused sibling cannot replace the surviving
occurrence. The full root-UUID-prefixed instance path remains source
provenance. Local-net
identity is topology-derived from sorted canonical terminal refs within the
page occurrence. Only a terminal-free graphical island falls back to sorted,
scoped drawing selectors. Consequently, editing a wire UUID cannot replace a
terminal-bearing local net.

Component-pin terminal identity uses the placed-pin UUID plus its stable pin
designator. Label and sheet-entry terminals use their authored object UUID
without the mutable displayed name; renaming a matched hierarchy boundary
therefore preserves both endpoint IDs, its topology-derived local-net IDs, and
the binding ID.

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
The same rule applies to non-hidden pins whose shaft and text both produce no
rendered operations, including zero-length power pins with suppressed name and
number text. Overplot passes use separate suffixed SVG ids, leaving the primary
source-owned pin id as the unique canonical graph selector without changing
visual geometry.

Enriched schematic SVGs expose a page-scoped projection of these rows through
`kicad_monkey.schematic.svg.compiled_graph_view.a0`. Monkey resolves a concrete
`KiCadSchematicInstance` to exactly one page occurrence and validates that
every projected `(page_occurrence_ref, artifact_key, element_id)` selector is
present exactly once in that page SVG. The SVG view is a navigation index; the
compiled graph remains the semantic authority.

The a0 contract supports scalar sheet-entry to child-port bindings. Aggregate
bus and harness member bindings require a later versioned contract. Missing
scalar boundary matches fail closed through governed resolution diagnostics;
the producer validator enforces role, page/unit ownership, direction, and
binding-or-diagnostic completeness.

## Compatibility

The accepted a0 wire shape is closed and strict. Adding a field, source-identity
selector, enum value, or row family requires a new schema token plus an explicit
consumer migration; readers must not silently accept fields that their generated
projection cannot represent. Removing or reinterpreting fields, changing identity
inputs, changing collection cardinalities, or adding aggregate binding semantics
likewise requires a new schema token. Existing `indexes` data remains a derived
compatibility view during consumer cutover and is not an independent connectivity
truth.

## Validation ownership

Package-local L0 tests own synthetic hierarchy, policy inheritance, identity,
relationship, and invalid-graph behavior. L3 acceptance builds the graph from
Yoshi, Taillight, Speedy, and Jumperless to cover single-page, repeated-page,
multipart, scalar hierarchy, global-label, bus, bus-entry, and scoped drawing
evidence. Shared invalid-vector tests in the downstream generic model verify
that both validators reject wrong-type refs, wrong-owner refs, inverse
membership gaps, and hierarchy cycles at the same boundary.

The source producer and generic projector deliberately keep local copies of the
transport DTOs and identity allocator so `kicad-monkey` has no consumer-model
dependency.
Contract parity is governed by golden identity vectors and live serialized
graph tests in both repositories.
