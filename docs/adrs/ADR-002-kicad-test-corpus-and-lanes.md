# ADR-002: KiCad Test Corpus Layout And Lane Model

## Status

Accepted

## Context

The old KiCad test setup mixed repo-local `tests/test_cases/...` fixtures with
broader corpus data. The current package needs a corpus layout that works both
from the checked-in archive and from an externally supplied corpus root.

KiCad also needs a lane model so smoke runs and heavier validations are
predictable.

## Decision

KiCad persistent file-backed fixtures resolve from the ZIP carrier selected by
`KM_CORPUS`. When unset, `tests/corpus/kicad.zip` is authoritative. A directory
containing `kicad/` is accepted only for fixture-authoring workflows; runtime
tests consume a verified, immutable, content-addressed extraction exposed
internally as `KM_CORPUS_ROOT`. There is no implicit loose-tree fallback.

Shared corpus layout rule:
- `kicad/common/...` for reusable shared fixtures
- `kicad/<topic>/...` for focused feature or stratum assets

Preferred case shape:
- `input/`
- `reference_output/`
- `output/`

`output/` is transient and should remain local or temp-backed, not authoritative
shared corpus data. Runtime tests and visual tools treat `KM_CORPUS_ROOT` as
read-only and write generated review artifacts under the owning stratum's
ignored `output/` tree. Fixture changes require an explicit writable authoring
root followed by rebuilding and reviewing the ZIP.

Real-world project domain tagging is file-role based. Assembly-procedure
projects with names ending in `_assembly` are promoted for schematic
rendering/IR and project parsing, but their `.kicad_pcb` files are intentionally
empty or header-only. They should not be expected to participate in `board_svg`
or PCB SVG review lanes unless a real board file is later added.

Lane model:
- `fast` is the default lane
- `full` is the broader routine-validation lane
- `strict` is the heavier or stricter validation lane

KiCad CLI oracle tools are test dependencies, not source fixtures. Local
resolvers may use an explicit executable environment variable, a build-tree
executable, or a restored immutable binary
cache entry. Cache metadata lives in `tools/kicad-cli/MANIFEST.toml`, and
restored bundles must be verified by SHA-256 before use.

Generated binary dependency caches use a stable object layout under
`deps/v1/<project>/<dependency>/...`. The `kicad-monkey` KiCad CLI cache stores
patched CLI bundles under `deps/v1/kicad-monkey/kicad-cli/...`, with sidecar
manifest and checksum objects so CI and developer machines can restore the same
oracle tool without rebuilding KiCad.

The package-local corpus archive also has stable object-storage metadata. CI
must not require Git LFS object downloads during checkout. Instead, it restores
`tests/corpus/kicad.zip` from the public object URL recorded in
`tests/corpus/kicad.archive.toml` and verifies the recorded size and SHA-256.
The restored archive is ignored locally and is not a tracked Git or Git LFS
object. `KICAD_MONKEY_CORPUS_URL` may override the manifest URL for local
testing or an emergency reroute.

## Consequences

- Moving the shared KiCad corpus should require changing only `KM_CORPUS`.
- Moving the package-local corpus archive transport should require updating
  `tests/corpus/kicad.archive.toml`, not workflow logic.
- New persistent fixtures should not be introduced under repo-local `tests/test_cases/...` unless they are synthetic/local-only by design.
- Focused feature boards should not pollute the broad common board corpus when that would widen unrelated parser-equivalency sweeps.
- Future strata should record durable corpus ownership in package-local
  manifests or design docs before release.
- KiCad CLI oracle tests should skip cleanly when the required patched CLI is
  unavailable, but release verification should run them from a verified staged
  or restored tool bundle when the oracle proves numerical behavior.
