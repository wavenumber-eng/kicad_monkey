+++
type = "plan"
id = "sexpr-projection-parser"
status = "active"
title = "KiCad Monkey Projection Parser And Domain API"
created = "2026-06-16"

[[steps]]
id = "sexpr-projection-foundation"
title = "Add generic S-expression projection scanner foundation"
status = "done"

[[steps]]
id = "pcb-projection-api"
title = "Add PCB projection API with same-type domain object hydration"
status = "done"
depends_on = ["sexpr-projection-foundation"]

[[steps]]
id = "projection-corpus-tests"
title = "Add projection parity and corpus coverage tests"
status = "done"
depends_on = ["pcb-projection-api"]

[[steps]]
id = "projection-docs"
title = "Move projection parser decisions into ADRs, design docs, and public API docs"
status = "done"
depends_on = ["projection-corpus-tests"]

[[steps]]
id = "design-doc-intent-audit"
title = "Audit projection design docs, ADRs, requirements, and release notes against implementation"
status = "pending"
depends_on = ["projection-docs"]

[[steps]]
id = "test-runtime-impact-audit"
title = "Audit projection tests and runtime impact"
status = "pending"
depends_on = ["projection-corpus-tests"]

[[steps]]
id = "external-review"
title = "Obtain independent external review before projection plan closeout"
status = "pending"
depends_on = [
  "design-doc-intent-audit",
  "test-runtime-impact-audit",
]

[[exit_criteria]]
id = "projection-api"
title = "Projection APIs return same public domain objects as the full parser"
status = "met"

[[exit_criteria]]
id = "projection-corpus-tests"
title = "Projection parser behavior is covered by focused and corpus tests"
status = "met"

[[exit_criteria]]
id = "design-doc-intent-audit"
title = "Projection design docs, ADRs, requirements, and release notes match implemented behavior"
status = "pending"

[[exit_criteria]]
id = "test-runtime-impact-audit"
title = "Projection test additions and runtime impact are reviewed"
status = "pending"

[[exit_criteria]]
id = "external-review"
title = "Projection plan closeout has independent external review"
status = "pending"
+++

# KiCad Monkey Projection Parser And Domain API Plan

Status: active implementation

This plan tracks the `kicad_monkey` parser/projection work needed before
continuing broader `kicad_cruncher` library-extraction command iteration. Per
WN development standards, this file is temporary: final decisions should move
into code, design docs, ADRs, contracts, release notes, and tests before
release.

## Context

Large KiCad PCB and schematic files are currently parsed through the full
S-expression and OOP model path even when a caller only needs a narrow subset of
objects. On the 4-ch backplane board, full PCB parsing costs about 10 to 11
seconds, and most of that cost is allocating tokens and a complete nested list
tree for a large file.

The low-level projection scanner now provides source spans for selected forms
without materializing the whole file. The next slice is a higher-level PCB API
that hydrates selected source spans into the same public domain object classes
that callers get from `KiCadPcb`.

## Goals

- Add a PCB projection API that selects and hydrates PCB sub-objects without a
  full board parse.
- Make projected object-family methods return the same public object classes as
  the full parser, such as `Footprint`, `Via`, `Segment`, `GrText`, `Zone`, and
  `Layer`.
- Keep exact source spans available for every projected object for diagnostics,
  source-aware patching, and health checks.
- Cover every PCB top-level sub-object family that the current typed parser can
  hydrate.
- Test against both stripped 4-ch backplane and Speedy processing module corpus
  boards.
- Preserve the existing `KiCadPcb` full parser as the authoritative complete
  edit and round-trip model.

## Non-Goals

- Do not replace `KiCadPcb`, `KiCadSchematic`, or existing typed OOP classes.
- Do not make PCB-specific regex scanners the public abstraction.
- Do not make object-family projection methods return summary-only records.
  Inventory summaries can exist, but they are secondary helpers.
- Do not start with a C++ rewrite. Native code can be evaluated later if the
  Python projection scanner still leaves important workflows too slow.
- Do not update external applications such as BOM Cruncher in this slice.

## Hard API Contract

Projection object-family methods must return normal `kicad_monkey` domain
objects:

```python
projection = KiCadPcbProjection.from_file(board_path)

footprint = next(projection.footprints())
assert isinstance(footprint, Footprint)

via = next(projection.vias())
assert isinstance(via, Via)
```

When a projection is built from a fully parsed board, the projected objects
should be the exact instances owned by that board:

```python
board = KiCadPcb.from_file(board_path)
projection = KiCadPcbProjection.from_board(board)

assert next(projection.footprints()) is board.footprints[0]
```

When a projection is built from a file, objects are same-type and same-behavior
objects hydrated from exact source spans. They are not the same Python object
identity as a separately parsed `KiCadPcb`.

Projection source metadata should be available through sidecar lookup APIs:

```python
span = projection.source_span(footprint)
text = projection.source_text(footprint)
sexp = projection.source_sexp(footprint)
```

## Proposed API

Add a PCB projection module:

- `src/py/kicad_monkey/kicad_pcb_projection.py`

Public classes:

- `KiCadPcbProjection`
- `ProjectedSource`
- optional later: `KiCadProjectionCollection`

First public methods:

```python
KiCadPcbProjection.from_file(path)
KiCadPcbProjection.from_board(board)

projection.source_span(obj)
projection.source_text(obj)
projection.source_sexp(obj)

projection.layers()
projection.nets()
projection.properties()
projection.variants()
projection.stackup()
projection.title_block()
projection.setup_sexp()
projection.embedded_fonts()
projection.embedded_files()

projection.footprints()
projection.pads()
projection.model_references()

projection.gr_texts()
projection.gr_lines()
projection.gr_rects()
projection.gr_arcs()
projection.gr_circles()
projection.gr_polys()
projection.gr_curves()
projection.gr_text_boxes()

projection.images()
projection.barcodes()
projection.tables()
projection.zones()
projection.dimensions()
projection.segments()
projection.vias()
projection.arcs()
projection.groups()
projection.generated_items()
projection.unknown_elements()
```

The `pads()` and `model_references()` helpers are nested projections under
footprints. `pads()` should return normal `Pad` objects and preserve parent
footprint source context. `model_references()` can return inventory records
because KiCad model references are not currently standalone board object
classes in the same way as footprints or vias.

## PCB Projection Coverage Target

The first implementation should cover every PCB object family that can
currently be hydrated through existing typed factories without special-case text
parsing.

| KiCad form | Full parser class/list | Projection method |
| --- | --- | --- |
| `net` | `Net` / `nets` | `nets()` |
| `property` | `BoardProperty` / `properties` | `properties()` |
| `variant` inside `variants` | `BoardVariant` / `variants` | `variants()` |
| `gr_text` | `GrText` / `gr_texts` | `gr_texts()` |
| `gr_line` | `GrLine` / `gr_lines` | `gr_lines()` |
| `gr_rect` | `GrRect` / `gr_rects` | `gr_rects()` |
| `gr_arc` | `GrArc` / `gr_arcs` | `gr_arcs()` |
| `gr_circle` | `GrCircle` / `gr_circles` | `gr_circles()` |
| `gr_poly` | `GrPoly` / `gr_polys` | `gr_polys()` |
| `gr_curve` | `GrCurve` / `gr_curves` | `gr_curves()` |
| `gr_text_box` | `GrTextBox` / `gr_text_boxes` | `gr_text_boxes()` |
| `barcode` | `Barcode` / `barcodes` | `barcodes()` |
| `image` | `Image` / `images` | `images()` |
| `table` | `Table` / `tables` | `tables()` |
| `footprint` and `module` | `Footprint` / `footprints` | `footprints()` |
| `zone` | `Zone` / `zones` | `zones()` |
| `dimension` | `Dimension` / `dimensions` | `dimensions()` |
| `segment` | `Segment` / `segments` | `segments()` |
| `via` | `Via` / `vias` | `vias()` |
| `arc` | `Arc` / `arcs` | `arcs()` |
| `group` | `Group` / `groups` | `groups()` |
| `generated` | `GeneratedObject` / `generated_items` | `generated_items()` |
| `file` inside `embedded_files` | `EmbeddedFile` / `embedded_files` | `embedded_files()` |

Board setup and stack helpers should also cover:

- `layers()` returning `Layer` objects;
- `stackup()` returning `Stackup | None`;
- `title_block()` returning `TitleBlock | None`;
- `setup_sexp()` returning the exact setup S-expression;
- `embedded_fonts()` returning the board-level embedded-fonts flag;
- `unknown_elements()` returning source-aware `UnknownElement` objects or
  source-span diagnostics.

Projected route, pad, and zone objects must resolve numeric net references the
same way the full parser does. If a projection method hydrates net-bound
objects without resolving `NetRef` name/ordinal data, it is incomplete.

## Implementation Phases

1. Keep the generic `SexpSelector` and `SexpFormSpan` API as the foundation.
2. Add `KiCadPcbProjection.from_file()` with source text ownership, projection
   cache, and sidecar source metadata.
3. Add `KiCadPcbProjection.from_board()` so projections can wrap a full
   `KiCadPcb` and return the exact board-owned object instances.
4. Implement top-level object hydration for the full parser table:
   nets/properties/graphics/images/tables/footprints/dimensions/groups/generated.
5. Implement net-aware hydration for zones, segments, vias, arcs, and nested
   footprint pads.
6. Implement board stack/setup helpers: layers, stackup, title block, setup
   S-expression, embedded fonts, embedded files.
7. Implement nested footprint helpers for pads and 3D model references.
8. Add corpus count/core-field parity tests against full `KiCadPcb` for every
   supported object family.
9. Add performance smoke tests on 4-ch backplane and Speedy processing module.
10. Update ADR/design docs/public exports after API names settle.
11. Only after the PCB surface is stable, add `KiCadSchematicProjection` using
    the same same-type object and source-span rules.

## Test Plan

Focused tests should cover:

- object-family projection methods return the same public classes as
  `KiCadPcb.from_file()`;
- projected object counts match full-parser object counts for every supported
  PCB object family;
- projected object core fields match full-parser object core fields for
  representative examples;
- projected net-bound route, pad, and zone objects resolve net references the
  same way as the full parser;
- source spans are available for projected objects and round-trip to the exact
  selected S-expression form;
- `from_board()` projections return object identities owned by the source
  `KiCadPcb`;
- 4-ch backplane projection coverage and performance expectations;
- Speedy processing module projection coverage and performance expectations.

Primary real-board fixtures:

- `tests/corpus/.unpacked/kicad/projects/4-ch-backplane/input/4-ch-backplane.kicad_pcb`
- `tests/corpus/.unpacked/kicad/projects/speedy_processing_module/input/11-10084__speedy_processing_module__B.kicad_pcb`

Final signoff should include existing parser round-trip tests, projection tests,
the complexity ratchet, and focused consumer workflow tests after command
behavior settles.

## Documentation Deliverables

Before this leaves planning status, `kicad_monkey` should have:

- an ADR describing why projection parsing exists alongside the existing full
  parser and OOP model;
- a design document for the `kicad_sexpr` projection API;
- a design document update for the PCB projection API and same-type object
  contract;
- public API contract updates if new root-level exports are promoted;
- release notes naming the performance-driven parser lane and first consumers;
- test ownership notes covering parser fixtures, 4-ch backplane regression
  expectations, Speedy regression expectations, and downstream consumer
  coverage.

## Open Decisions

- Whether object-family projection methods should stream fresh objects on each
  call or cache hydrated objects per projection instance by default.
- Whether source metadata should be stored in a weak sidecar map keyed by object
  identity or on private attributes attached to hydrated objects.
- Whether projection collections need a query API similar to
  `KiCadObjectCollection`, or whether iterators plus source lookup are enough.
- Whether source text should be retained by each span or owned once by the
  projection instance to reduce memory use.
- Whether path filters need glob-like wildcards after the first exact-path API.
