# KiCad Monkey S-Expression Projection Parser Plan

Status: active planning

This plan tracks the next `kicad_monkey` core parser work needed before
continuing broader `kicad_cruncher` library-extraction command iteration. Per
WN development standards, this file is temporary: final decisions should move
into code, design docs, ADRs, contracts, release notes, and tests before
release.

## Context

Large KiCad PCB and schematic files are currently parsed through the full
S-expression and OOP model path even when a caller only needs a narrow subset of
objects. On the 4-ch backplane board, full PCB parsing costs about 11 seconds,
and most of that cost is allocating tokens and a complete nested list tree for a
large file.

That full path is appropriate for round-trip editing, SVG/IR conversion,
geometry work, and complete design review. It is too expensive for workflows
that only need selected top-level or nested S-expression forms, such as BOM
extraction, asset health checks, project-local library generation, model
reference scanning, netclass inventory, and quick visualization preflight.

## Goals

- Add a generic S-expression projection API that can select matching forms
  before the full tree or typed OOP model is materialized.
- Keep the projection layer file-format generic so it can serve PCB, schematic,
  footprint library, symbol library, worksheet, and future KiCad S-expression
  files.
- Use this core API as the foundation for faster typed summaries such as PCB
  footprint summaries, model-reference summaries, embedded-file summaries, and
  schematic symbol summaries.
- Preserve the existing full parser and typed OOP model as the authoritative
  edit/round-trip path.
- Make downstream tools choose the smallest parse model that fits the task.
- Produce the normal `kicad_monkey` durable artifacts for the new parser lane:
  ADRs, design documentation, contract documentation if public schemas are
  introduced, release notes, and focused regression tests.

## Non-Goals

- Do not replace `KiCadPcb`, `KiCadSchematic`, or existing typed OOP classes.
- Do not make PCB-specific regex scanners the public abstraction.
- Do not start with a C++ rewrite. Native code can be evaluated later if the
  Python projection scanner still leaves important workflows too slow.
- Do not change `kicad_cruncher` command contracts until the core parser API is
  stable enough to consume.

## Proposed Core API

The new low-level API should live in or near `kicad_sexpr.py` and expose
read-only spans for selected forms:

```python
@dataclass(frozen=True)
class SexpFormSpan:
    head: str | None
    path: tuple[str, ...]
    depth: int
    start_offset: int
    end_offset: int
    line: int
    column: int
    source_path: Path | None = None

    def text(self) -> str: ...
    def parse(self) -> list: ...


@dataclass(frozen=True)
class SexpSelector:
    heads: frozenset[str] | None = None
    paths: frozenset[tuple[str, ...]] | None = None
    min_depth: int | None = None
    max_depth: int | None = None
    prune_heads: frozenset[str] = frozenset()
```

Expected public functions:

```python
iter_sexp_form_spans(text: str, selector: SexpSelector | None = None)
iter_sexp_file_form_spans(path: Path | str, selector: SexpSelector | None = None)
parse_sexp_span(span: SexpFormSpan)
```

The scanner should understand KiCad S-expression syntax well enough to be
trusted for file selection:

- quoted strings and escaped quotes;
- line comments using KiCad's comment rules;
- nested parenthesis depth;
- the first atom/string after an opening parenthesis as the form head;
- stable offsets and source locations.

## Projection Layers

After the generic span API exists, add typed projection helpers that parse only
the selected forms they need:

- PCB:
  - footprint summary records for BOM/PnP/project health;
  - model reference summaries;
  - embedded file summaries;
  - board setup summaries such as `aux_axis_origin`;
  - optional route primitive summaries for quick netclass and visualization
    preflight.
- Schematic:
  - placed symbol summaries;
  - sheet summaries;
  - `lib_symbols` summaries;
  - metadata/property summaries for library extraction.
- Libraries:
  - single symbol and footprint form selection from folder libraries;
  - metadata-preserving and metadata-stripping extraction support.

These projection helpers should return small dataclasses, not partially
initialized full OOP objects.

## Relationship To Object Query

`KiCadObjectCollection` remains the typed OOP query view. The projection API can
reuse the same style by exposing selected spans or summaries through a read-only
queryable collection, but it should not depend on full OOP parsing.

The intended layering is:

1. `kicad_sexpr`: generic form span selection.
2. Projection modules: small file-format summaries built from selected forms.
3. Typed OOP modules: full edit/round-trip objects.
4. Downstream apps: choose projection or full OOP based on workflow needs.

## Candidate Consumers

- `appz/bom_cruncher`: parse only board setup and footprint summaries.
- `kicad_cruncher health`: scan footprint/model/embedded-file state quickly.
- `kicad_cruncher project-lib`: extract project-local library inputs without
  one-off regex scanners.
- `kicad_cruncher megamaid`: reuse the same project scan primitives when the
  command behavior is revisited.
- `toolz/pcb_cruncher` netclass/SVG workflows: select only nets, net-bound
  route primitives, layers, zones, and relevant footprint pads when possible.
- KiCad Monkey SVG/IR preflight: quickly inventory required object families
  before full render conversion.

## Implementation Phases

1. Write the ADR for the parser split: full OOP parse versus projection parse,
   public API ownership, and downstream command boundaries.
2. Write the design document for S-expression projection scanning, selector
   semantics, source spans, and intended consumer patterns.
3. Add parser-only tests for `SexpFormSpan` and `SexpSelector`.
4. Implement the generic character-level span scanner.
5. Verify selector behavior on small synthetic PCB and schematic examples.
6. Benchmark against the 4-ch backplane corpus fixture.
7. Replace existing private footprint and embedded-file regex scans with the
   generic span scanner.
8. Add PCB footprint/setup/model summary projections.
9. Move BOM Cruncher and health/project-lib scans to projection APIs where
   appropriate.
10. Audit public exports, docs, and contract naming before release.

## Documentation Deliverables

Before this leaves planning status, `kicad_monkey` should have:

- an ADR describing why projection parsing exists alongside the existing full
  parser and OOP model;
- a design document for the `kicad_sexpr` projection API, selector behavior,
  source span ownership, and error handling;
- a design document or design-doc update for PCB projection summaries if they
  become public API;
- public API contract updates if new root-level exports are promoted;
- release notes naming the performance-driven parser lane and first consumers;
- test ownership notes covering parser fixtures, 4-ch backplane regression
  expectations, and downstream consumer coverage.

## Test Plan

Focused tests should cover:

- balanced and unbalanced S-expression handling;
- quoted strings containing parentheses;
- escaped quotes inside strings;
- comments and whitespace preservation around offsets;
- head and path filtering;
- max-depth and prune behavior;
- span text reparses to the same selected form;
- 4-ch backplane performance and count expectations.

Final signoff should include existing parser round-trip tests, projection tests,
the complexity ratchet, and focused consumer workflow tests after command
behavior settles.

## Open Decisions

- Whether `SexpFormSpan.text()` should hold the source text or require a source
  accessor to avoid retaining large file buffers.
- Whether path filters should match exact paths only or support glob-like
  wildcards.
- Whether projections should expose `KiCadObjectCollection` directly or a
  sibling collection type for source spans and summaries.
- Where the first public contract boundary belongs: core span API only, or span
  API plus PCB footprint summary records.
