# Release Process

`kicad-cruncher` publishes to PyPI from version tag pushes. A GitHub Release is
optional release-note presentation; it is not the publish trigger.

## Preflight

Before tagging, confirm the release commit on `main` has:

- `pyproject.toml` and `src/py/kicad_cruncher/_version.py` set to the same
  date version, such as `2026.6.25`.
- `tests/L99_signoff/test_L99_001_release_signoff.py` updated for that version
  and release date.
- `CHANGELOG.md` with a matching `## YYYY.M.D` section.
- `docs/releases/YYYY-MM-DD.md` with the matching release note.
- The configured dev-std audit, including CLI, plan, requirement, and release
  governance scopes, passing in L99 signoff.
- External review approval for the release candidate.
- Explicit user authorization to publish.
- Main CI passing for the release commit.

Recommended local gate:

```powershell
uv run --extra test rack run --all
uv run --extra test python -m build
uv run --extra test twine check dist/*
uv run --extra test python tests\support_scripts\install_test.py
```

## Publish

Create and push an annotated tag whose name exactly matches the package version:

```powershell
git checkout main
git pull --ff-only
git tag -a v2026.6.25 -m "Release kicad-cruncher 2026.6.25"
git push origin v2026.6.25
```

The `Publish` workflow runs on the tag push. It verifies the tag, package
metadata, changelog, and dated release note agree, then runs Rack, builds the
package, checks the distributions, runs the installed-console smoke test, and
publishes through PyPI Trusted Publishing/OIDC.

Watch the run:

```powershell
gh run list --repo wavenumber-eng/kicad_cruncher --workflow release.yml --limit 5
$runId = gh run list --repo wavenumber-eng/kicad_cruncher --workflow release.yml --limit 1 --json databaseId --jq ".[0].databaseId"
gh run watch $runId --repo wavenumber-eng/kicad_cruncher
```

## GitHub Release

After PyPI publish succeeds, create the GitHub Release from the existing tag if
you want a release page:

```powershell
gh release create v2026.6.25 --title "kicad-cruncher 2026.6.25" --notes-file docs/releases/2026-06-25.md
```

## Recovery

If the tag workflow fails before PyPI upload, fix the source on `main`, then
delete and recreate that tag at the corrected commit.

If PyPI upload succeeded, that version is immutable. Make a new date version or
same-day fourth-component version, such as `2026.6.25.1`.

## Workspace Consumers

After the package is on PyPI, update downstream runtime pins in
`appz/config/runtime-tools.jsonc` so WN workspace setup installs the new
`kicad-cruncher` version.
