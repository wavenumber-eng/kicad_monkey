# ADR-011: KiCad Monkey And KiCad Cruncher Monorepo

## Status

Accepted

## Date

2026-08-09

## Context

`kicad-monkey` is the public KiCad parser, source-model, round-trip, compiled
fact, and basic rendering package. `kicad-cruncher` is the public command and
artifact-workflow package built on that library. They are separate PyPI
distributions with a deliberate one-way dependency, but they have historically
lived in separate Git repositories.

Most substantive work crosses that repository boundary. A typical change adds
or corrects Monkey source behavior, publishes an intermediate Monkey release,
updates Cruncher's dependency floor, adds the command/workflow behavior, and
then publishes Cruncher. Both repositories also carry closely related CI,
documentation governance, release preparation, and trusted-publishing logic.
The source split therefore makes one product change harder to review and test
without providing useful implementation independence.

## Decision

Use `wavenumber-eng/kicad_monkey` as the single active development repository
for both packages. Keep the existing Monkey project at the repository root and
place Cruncher under `packages/kicad_cruncher/` in the same `uv` workspace.
Import the standalone Cruncher Git history without squashing it.

The distribution and import boundaries do not change:

- distribution `kicad-monkey`, import package `kicad_monkey`;
- distribution `kicad-cruncher`, import package `kicad_cruncher`, console
  commands `kicad-cruncher` and `kcr`.

Dependency direction remains one way. Cruncher may depend on reviewed public
Monkey APIs; Monkey must not import or depend on Cruncher. Parsing,
source-model, round-trip, compiled-fact, and base-rendering behavior remains in
Monkey. CLI parsing, command configuration, daemon/plugin behavior, artifact
packaging, and higher-level workflow orchestration remains in Cruncher.

During development, Cruncher resolves Monkey from the workspace so one pull
request can implement and validate both halves. Built Cruncher artifacts must
retain ordinary public package dependency metadata and must not contain a
workspace path, direct source URL, or bundled Monkey source. Clean artifact
tests install the two wheels with the repository absent.

The packages keep independent versions and may release independently. New
monorepo tags are package-qualified:

- `kicad-monkey-v<version>`;
- `kicad-cruncher-v<version>`.

When both packages release, automation publishes Monkey first, verifies the
exact public PyPI artifact with a no-cache install, validates Cruncher against
that public release, and then publishes Cruncher. Each package retains its own
release notes, artifact checks, authorization environment, and provenance.

The standalone `wavenumber-eng/kicad_cruncher` repository remains available
through cutover. After the first monorepo-based Cruncher release is publicly
verified, transferable open issues move to KiCad Monkey. A final default-branch
commit removes active source and publishing automation and leaves one
relocation `README.md`. The old repository is then archived. Its Git history,
tags, and GitHub releases remain available as historical release provenance.

The Wavenumber installer treats KiCad Monkey as the single development source
repository while continuing to install the public `kicad-cruncher`
distribution for ordinary users. Package names and commands do not change.

## Consequences

- Cross-package changes can be implemented, reviewed, and tested atomically
  before either distribution is published.
- Repository CI becomes responsible for both package-local gates and explicit
  live-source/artifact compatibility gates.
- Release automation is more complex because one repository owns two trusted
  publishers and independent versions.
- Package-qualified tags are required because both former repositories used
  the same `vYYYY.M.D` tag namespace.
- Cruncher history remains inspectable in the active repository, while the
  archived standalone repository preserves original hashes and releases.
- Repository colocation does not authorize semantic or ownership leakage
  between the packages.

## Validation

The migration is accepted only when:

- Monkey and Cruncher package-local Rack gates pass from one checkout on
  Windows and Linux;
- live Monkey source passes Cruncher command/workflow tests;
- both wheel and sdist contents remain package-specific;
- an isolated environment can install Monkey's wheel and then Cruncher's wheel
  without source-tree leakage;
- public `kicad-cruncher` installation still exposes `kicad-cruncher`, `kcr`,
  and `python -m kicad_cruncher`;
- installer setup, update, worktree, Windows replay, and WSL/POSIX replay gates
  pass with one canonical development repository;
- the first monorepo-based Cruncher release is installed and verified before
  the standalone repository is retired.
