# Release Process

`kicad-monkey` and the established Python `kicad-cruncher` package publish
together from one reviewed commit in the `wavenumber-eng/kicad_monkey`
monorepo. They keep independent versions, tags, release notes, and PyPI
projects. The separately packaged Windows Rust CLI is attached to Cruncher's
GitHub Release.

## Preflight

Before tagging, confirm the release commit on `main` has:

- matching Cruncher versions in `pyproject.toml`, `_version.py`, the Rust CLI,
  L99 signoff, changelog, and dated release note;
- matching Monkey metadata, changelog, and dated release note;
- the configured dev-std and Rack signoffs passing;
- explicit authorization to publish; and
- one successful full `CI` run for the exact commit.

The full main run is authoritative. Its Linux Python-provider job produces the
universal Monkey wheel. Its single Windows candidate job runs all Phase 6 and
Phase 7 native gates once, then emits the Monkey and Cruncher sdists, Windows
Monkey wheel, universal Cruncher wheel, and Rust CLI archive. Release does not
rebuild any of them.

## Publish both packages

Create both annotated package-qualified tags at the same reviewed main commit:

```powershell
git checkout main
git pull --ff-only
git tag -a kicad-monkey-v2026.9.7 -m "Release kicad-monkey 2026.9.7"
git tag -a kicad-cruncher-v2026.9.7 -m "Release kicad-cruncher 2026.9.7"
git push origin kicad-monkey-v2026.9.7 kicad-cruncher-v2026.9.7
```

From the Actions page, run `Publish coordinated release` on `main` and select
`publish`. That is the only publish or recovery action. The workflow infers
both versions and tags, locates the exact commit's successful `CI` run, verifies every
manifest and hash, publishes Monkey, verifies it publicly, publishes the
Python Cruncher package, then creates any missing GitHub Releases and attaches
the tested Rust CLI archive.

Watch the run:

```powershell
gh run list --repo wavenumber-eng/kicad_monkey --workflow release.yml --limit 5
$runId = gh run list --repo wavenumber-eng/kicad_monkey --workflow release.yml --limit 1 --json databaseId --jq ".[0].databaseId"
gh run watch $runId --repo wavenumber-eng/kicad_monkey
```

PyPI Trusted Publishing uses workflow `release.yml` with the
`pypi-kicad-monkey` and `pypi-kicad-cruncher` GitHub environments.

## Recovery

Rerun the same failed workflow. Before each `skip-existing` PyPI upload, the
workflow rejects any existing filename or digest that is not an exact subset
of the CI candidates; afterward it requires the complete public set to match.
Public package checks are repeatable, and GitHub Release creation is
create-if-missing with a clobber-safe Rust asset upload. There is no recovery
workflow and no candidate run ID to find or enter.

Published PyPI versions remain immutable. If source bytes must change, create a
new date version or a same-day fourth-component version such as
`2026.9.7.1`; do not move a published tag.

## Workspace consumers

After both packages are public, update the downstream pins in
`appz/config/runtime-tools.jsonc`. The workspace continues to install the
Python `kicad-cruncher` CLI; the Rust CLI remains a separate release artifact.
