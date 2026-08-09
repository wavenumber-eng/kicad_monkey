# Release Process

`kicad-cruncher` publishes from the `wavenumber-eng/kicad_monkey` monorepo when
a GitHub Release is published for a matching package-qualified tag. Monkey and
Cruncher keep independent versions and PyPI projects.

## Preflight

Before tagging, confirm the release commit on `main` has:

- `packages/kicad_cruncher/pyproject.toml` and
  `packages/kicad_cruncher/src/py/kicad_cruncher/_version.py` set to the same
  date version, such as `2026.6.25`.
- Cruncher's L99 release-signoff test updated for that version and release
  date.
- Cruncher's `CHANGELOG.md` with a matching `## YYYY.M.D` section.
- Cruncher's `docs/releases/YYYY-MM-DD.md` with the matching release note.
- The configured dev-std audit, including CLI, plan, requirement, and release
  governance scopes, passing in L99 signoff.
- External review approval for the release candidate.
- Explicit user authorization to publish.
- Main CI passing for the release commit.

Recommended local gate:

```powershell
cd packages\kicad_cruncher
uv run --package kicad-cruncher --extra test rack run --all
uv run --package kicad-cruncher --extra test python -m build
uv run --package kicad-cruncher --extra test twine check dist/*
uv run --package kicad-cruncher --extra test python tests\support_scripts\install_test.py
cd ..\..
uv run --extra test python tests\support_scripts\toolchain_install_test.py
```

## Publish

Create and push an annotated package-qualified tag, then publish its GitHub
Release:

```powershell
git checkout main
git pull --ff-only
git tag -a kicad-cruncher-v2026.8.9 -m "Release kicad-cruncher 2026.8.9"
git push origin kicad-cruncher-v2026.8.9
gh release create kicad-cruncher-v2026.8.9 `
  --repo wavenumber-eng/kicad_monkey `
  --title "kicad-cruncher 2026.8.9" `
  --notes-file packages/kicad_cruncher/docs/releases/2026-08-09.md
```

The root `Publish` workflow runs on the release event. It verifies the tag, package
metadata, changelog, and dated release note agree, then runs Rack, builds the
package, checks the distributions, runs the installed-console smoke test, and
publishes through PyPI Trusted Publishing/OIDC.

Watch the run:

```powershell
gh run list --repo wavenumber-eng/kicad_monkey --workflow release.yml --limit 5
$runId = gh run list --repo wavenumber-eng/kicad_monkey --workflow release.yml --limit 1 --json databaseId --jq ".[0].databaseId"
gh run watch $runId --repo wavenumber-eng/kicad_monkey
```

Before the first monorepo publish, configure the `kicad-cruncher` PyPI trusted
publisher for repository `wavenumber-eng/kicad_monkey`, workflow
`release.yml`, and the authorized `pypi` GitHub environment.

## Recovery

If the release workflow fails before PyPI upload, fix the source on `main`,
delete the failed GitHub Release, then create a new package version/tag. Do not
move a publicly presented release tag after publication.

If PyPI upload succeeded, that version is immutable. Make a new date version or
same-day fourth-component version, such as `2026.6.25.1`.

## Workspace Consumers

After the package is on PyPI, update downstream runtime pins in
`appz/config/runtime-tools.jsonc` so WN workspace setup installs the new
`kicad-cruncher` version.
