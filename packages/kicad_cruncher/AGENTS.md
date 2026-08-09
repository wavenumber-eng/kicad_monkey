# KiCad Cruncher Agent Guide

`kicad-cruncher` is the public command-line workflow package built on the
public `kicad-monkey` parser/model/rendering package. Keep higher-level CLI and
artifact orchestration here; keep low-level KiCad parsing and source-model
behavior in `kicad-monkey`.

This package lives in the `wavenumber-eng/kicad_monkey` monorepo under
`packages/kicad_cruncher/`. Development resolves Monkey from the shared `uv`
workspace. Published artifacts still depend on the public `kicad-monkey`
distribution and must not contain local path or direct-URL dependencies.

## Setup

Use `uv` for local development:

```bash
uv sync --extra test
```

Commit the repository-root `uv.lock`. Do not create a package-local lockfile.

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

- `main` should stay release-ready; version tags identify released source.
- Public changes should merge through PRs with required CI.
- Publishing uses the package-qualified tag `kicad-cruncher-v<version>` from
  the repository root and triggers validation and PyPI trusted publishing.
- Date-based versions are standard, for example `2026.6.4`.
- `CHANGELOG.md` and `docs/releases/<YYYY-MM-DD>.md` must mention the current
  package version.

## Local Secrets

Do not commit `.env` files, PyPI tokens, private corpora, customer data, or
generated manufacturing outputs.
