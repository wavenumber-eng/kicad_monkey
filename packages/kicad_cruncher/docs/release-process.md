# Release Process

`kicad-cruncher` publishes from the `wavenumber-eng/kicad_monkey` monorepo when
a GitHub Release is published for a matching package-qualified tag. Monkey and
Cruncher keep independent versions and PyPI projects.

## Preflight

Before tagging, confirm the release commit on `main` has:

- `packages/kicad_cruncher/pyproject.toml`,
  `packages/kicad_cruncher/src/py/kicad_cruncher/_version.py`, and
  `packages/kicad_cruncher/src/rs/kicad-cruncher-cli/Cargo.toml` set to the
  same date version, such as `2026.6.25`.
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
uv run --package kicad-cruncher --extra test pytest -q tests\L3_public_workflows\test_L3_012_rust_cli_install.py
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

The root `Publish` workflow runs on the release event. It verifies the tag,
Python and Rust package metadata, changelog, and dated release note agree; runs
Rack; builds and checks the Python distributions; and runs both installed
console smokes. The exact commit/version-bound Windows x64 Rust archive is
verified and attached to the GitHub Release before the Python distributions
are published through PyPI Trusted Publishing/OIDC.

Watch the run:

```powershell
gh run list --repo wavenumber-eng/kicad_monkey --workflow release.yml --limit 5
$runId = gh run list --repo wavenumber-eng/kicad_monkey --workflow release.yml --limit 1 --json databaseId --jq ".[0].databaseId"
gh run watch $runId --repo wavenumber-eng/kicad_monkey
```

Before the first monorepo publish, configure the `kicad-cruncher` PyPI trusted
publisher for repository `wavenumber-eng/kicad_monkey`, workflow
`release.yml`, and the authorized `pypi` GitHub environment.

## Coordinated Two-Package Publish

When one reviewed commit changes both distributions, create both annotated
package-qualified tags at that commit, but do not create the GitHub Releases
manually. From the Actions page, run the `Publish` workflow on `main` with the
two exact versions and confirmation text `publish-both`.

The authorized lane validates both tags against the selected `main` commit,
publishes Monkey first, waits for the exact PyPI version, verifies a no-cache
install and the compiled-graph API, tests the Cruncher wheel against that
public Monkey artifact, verifies the exact Rust CLI candidate, publishes
Cruncher, and creates both GitHub Releases with the native archive and
manifest attached to the Cruncher release.
The GitHub environment approval remains the publication boundary. Do not use
the coordinated lane when only one distribution needs a release.

## Recovery

If the release workflow fails before PyPI upload, including during native
candidate verification or attachment, fix the source on `main`, delete the
failed GitHub Release, then create a new package version/tag. Do not move a
publicly presented release tag after publication.

If PyPI upload succeeded, that version is immutable. Make a new date version or
same-day fourth-component version, such as `2026.6.25.1`.

## Workspace Consumers

After the package is on PyPI, update downstream runtime pins in
`appz/config/runtime-tools.jsonc` so WN workspace setup installs the new
`kicad-cruncher` version.
