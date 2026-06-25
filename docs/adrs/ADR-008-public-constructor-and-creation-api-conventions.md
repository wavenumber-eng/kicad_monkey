# ADR-008: Public Constructor And Creation API Conventions

## Status

Accepted

## Date

2026-06-25

## Context

`kicad-monkey` exposes promoted public facade classes for KiCad documents and
project-level aggregates. Several of these classes predate the public API
contract and already use a path-or-empty constructor:

```python
pcb = KiCadPcb("board.kicad_pcb")      # parse from file
pcb = KiCadPcb()                       # empty model container
sch = KiCadSchematic("design.kicad_sch")
sch = KiCadSchematic()
```

The preferred public API style from ADR-001 is explicit facade classes with
named operations:

```python
pcb = KiCadPcb.from_file("board.kicad_pcb")
schematic = KiCadSchematic.from_file("design.kicad_sch")
```

New project scaffolding adds another kind of operation: create a coordinated
project folder containing a `.kicad_pro`, schematic, PCB, library tables, and
optional drawing-sheet references. That is more than constructing one in-memory
document model.

Without a shared convention, new APIs can drift into ambiguous constructor
overloads such as using the same class constructor for both an existing project
JSON facade and a writable multi-file project aggregate.

## Decision

Promoted public classes keep existing constructor semantics unless a breaking
API change is explicitly approved, documented, and tested.

Constructors are for direct object initialization only. They must not gain new
positional meanings that reinterpret existing positional arguments on promoted
public classes.

Use named class methods for alternate construction paths:

- `from_file(path)`, `from_text(text)`, `from_json_dict(data)`, and similar
  `from_*` methods load or parse existing source material.
- `new(...)` creates one blank, in-memory document model with KiCad-valid
  defaults when the bare constructor is intentionally lower-level or incomplete.
  For example, `KiCadPcb.new()` may return a blank board with a default layer
  stack and setup block, while `KiCadPcb()` remains the empty parser/model
  container used by tests and internal hydration.
- `create(...)` starts a higher-level creation workflow or aggregate that is
  bound to output intent, such as a new project directory assembled from
  multiple document models.

For project scaffolding, the canonical public shape is:

```python
project = KiCadProject.create("Demo", out_dir)
project.add_schematic()
project.add_pcb()
project.write_project()
```

`KiCadProject(...)` remains compatible with the existing project JSON facade
constructor shape. It must not be promoted as the primary new-project API by
adding positional `name, directory` semantics ahead of existing fields.

If a class needs both a blank KiCad-default document and a multi-file creation
workflow, those remain separate named operations. `new` describes the blank
document. `create` describes the workflow or aggregate.

## Consequences

- Public constructor behavior stays stable for downstream callers.
- New creation APIs are readable at call sites and align with ADR-001's named
  operation guidance.
- `KiCadPcb.new()` is acceptable when it means "blank KiCad-openable PCB
  document" rather than "create a project workflow."
- `KiCadProject.create(...)` is the consistent API for new project assembly.
- PRs that add or change promoted public construction behavior must update the
  relevant design API document and tests in the same change.
