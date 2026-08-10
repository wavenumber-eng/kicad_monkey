+++
type = "requirement"
id = "design-req-001-agent-compiled-graph-linkage"
domain = "design"
status = "active"
title = "Design review exposes occurrence-scoped compiled-graph navigation"
created = "2026-08-09"
issue_refs = ["wavenumber-eng/kicad_monkey#49"]
verification_status = "unverified"
design_refs = [
  "docs/design/cli/design.html",
  "docs/design/cli/megamaid.html",
  "docs/design/api/index.html",
]
+++

# DESIGN-REQ-001: Agent Compiled-Graph Linkage

Status: implemented; release pending

The `design`, `design-review`, and `dr` commands must write the exact
`compiled_schematic_graph` object returned in the single Design JSON producer
call as `<input-stem>_compiled_schematic_graph.json`. Cruncher validates and
serializes that object directly; it must not rebuild, translate, repair, or
allocate graph identity.

Every schematic SVG must embed the Monkey-owned page view and pass Monkey's
final SVG selector validation before bundle success. The manifest records the
graph schema/type/identity namespace, collection counts, linkage contract, and
per-page link/resolved-identity counts. `--no-indexes` retains this contract.

Megamaid reuses this writer under `design_review/` and advertises the one nested
graph from its top manifest. It must not emit a second semantic graph.

Acceptance requires an artifact-only consumer to traverse SVG element to graph
link, terminal, local net, connected terminals/components, and back from graph
targets to their owning page SVG. No join may depend on designator, displayed
net name, text, list position, DOM order, or geometry.
