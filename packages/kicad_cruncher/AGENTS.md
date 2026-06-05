# KiCad Cruncher Agent Guide

`kicad-cruncher` is the public command-line workflow package built on the
public `kicad-monkey` parser/model/rendering package. Keep higher-level CLI and
artifact orchestration here; keep low-level KiCad parsing and source-model
behavior in `kicad-monkey`.

## Setup

Use `uv` for local development:

```bash
uv sync --extra test
```

Commit `uv.lock`. Do not hand-edit it.

## Test And Signoff

Run the package signoff before release-facing changes:

```bash
uv run rack run --all
uv run python -m build
uv run twine check dist/*
```

## Architecture Boundaries

- Public CLI commands live in dedicated `kicad_cruncher_cmd_*` modules.
- Shared command parsing and output helpers stay small and reusable.
- `kicad-monkey` owns KiCad file parsing, models, and base SVG rendering.
- `wn-geometer` owns hidden-line projection support used by assembly overlays.
- Output-producing commands write transient artifacts under `output/<command>/`
  by default.
- Durable command behavior belongs in docs, contracts, release notes, and Rack
  tests.

## Release Rules

- `main` should represent the latest released/tagged source.
- Public changes should merge through PRs with required CI.
- Release publication should trigger validation and PyPI publishing.
- Date-based versions are standard, for example `2026.6.4`.
- `CHANGELOG.md` and `docs/releases/<YYYY-MM-DD>.md` must mention the current
  package version.

## Local Secrets

Do not commit `.env` files, PyPI tokens, private corpora, customer data, or
generated manufacturing outputs.
