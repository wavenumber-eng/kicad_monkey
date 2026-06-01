# KiCad Cruncher

`kicad-cruncher` is a cross-platform command-line application for KiCad design
workflows. It consumes the public `kicad-monkey` package and keeps higher-level
CLI behavior outside the core parser package.

The first public command is `design`, which writes KiCad-native design JSON from
a `.kicad_pro` project or `.kicad_sch` schematic.

## Install

Install `uv` first if it is not already available:

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/install.ps1 | iex"
```

On macOS or Linux:

```bash
curl -LsSf https://astral.sh/uv/install.sh | sh
```

Install as a uv tool:

```powershell
uv tool install kicad-cruncher
uv tool update-shell
kicad-cruncher --help
```

During local development:

```powershell
uv sync --extra test
uv run kicad-cruncher --help
uv run python -m kicad_cruncher version
```

## Commands

Run `kicad-cruncher <command> --help` for command-specific options.

| Command | Purpose | Status |
| --- | --- | --- |
| `design` | Generate KiCad-native design JSON with project metadata, hierarchy, components, nets, variants, and optional lookup indexes. | Public |
| `version` | Print `kicad-cruncher` and controlled dependency versions. | Public |

The `design` command writes to `./output/design/` by default:

```powershell
kicad-cruncher design board.kicad_pro
kicad-cruncher design board.kicad_sch --no-indexes
kicad-cruncher design -o output/design
```

## Output Layout

Output-producing commands follow the same directory policy:

- when `-o/--output` is omitted, write artifacts under `./output/<command>/`;
- when `-o/--output` is supplied, use that directory directly;
- command modules own filenames inside their command output directory.

## Tests

Run the Rack suite:

```powershell
uv run --extra test rack run --all
```

Run build and installed-console smoke tests:

```powershell
uv run --extra test python -m build
uv run --extra test twine check dist/*
uv run --extra test python tests\support_scripts\install_test.py
```

Rack is the primary local gate. `L0_public_cli` covers startup and command
manifest alignment, `L3_public_workflows` covers fixture-backed command
behavior, and `L99_signoff` covers versioning, docs, contracts, source hygiene,
ruff, and pyright.

## Architecture Docs

- `docs/adrs/` records accepted architecture decisions.
- `docs/design/` records durable interface, command, data-flow, and format
  design notes.
- `docs/contracts/` stores stable manifests and future schemas for public JSON
  or config formats.

## Release Policy

Versioning, tagging, release, and traceability are defined in
`docs/adrs/ADR-0001-versioning-tagging-release-policy.md`. The intended release
workflow is GitHub Actions plus PyPI Trusted Publishing/OIDC. Local Twine upload
is fallback only.
