# Quality Signoff Status

Status: public-release bootstrap audit
Last updated: 2026-06-18

## Passing Gates

- `L99_signoff` checks date-version metadata, changelog coverage, public
  package metadata, sdist boundaries, and the promoted public API contract.
- `L99_signoff` checks design-doc entry points, the major-interface manifest,
  promoted public class design sections, and Rack test ownership links.
- `L99_signoff` runs `ruff check` across `src/py/kicad_monkey`, L99 signoff
  tests, the promoted API contract test, and the corpus packaging script.
- `L99_signoff` runs a Ruff C901 complexity ratchet across
  `src/py/kicad_monkey`; existing hotspots are baselined and regressions fail
  signoff.
- `L99_signoff` runs package-wide pyright against `src/py/kicad_monkey` through
  `pyrightconfig.json`.
- `tests/corpus/kicad.zip` is the public test-corpus transport. The loose
  corpus mirror is ignored locally and extracted on demand by test helpers.
- CI prepares the corpus archive, runs Rack L0 and L99, builds the package,
  runs `twine check`, and verifies installed-package imports.
- The 2026-06-18 release adds targeted KiCad object readers, cleaned library
  extraction metadata normalization, and current KiCad CLI validation for
  generated symbol libraries.
- The 2026-06-02 release adds focused design-review coverage for PCB SVG
  parity, enriched SVG metadata, and repeated schematic sheet instances.

## Active Quality Ratchet

Ruff, the Ruff C901 complexity ratchet, and pyright are installed in the test
extra and remain release-signoff tools. The source package is now ruff-clean
and pyright-clean, and both are hard-gated by L99.

Current complexity baseline:

```text
max C901 complexity: 27
functions over 10: 129
functions over 20: 18
functions over 30: 0
functions over 50: 0
```

Known remaining package-wide ruff work is in older non-L99 tests and any future
developer-only scripts. Package pyright is at zero diagnostics under
`typeCheckingMode = "standard"` with `reportUnsupportedDunderAll` suppressed
for the intentionally broad lazy package export table.

Current local package pyright run:

```text
uv run --extra test pyright
-> 0 errors, 0 warnings, 0 informations
```

Ongoing release expectations:

1. Keep `src/py/kicad_monkey` package-wide ruff and pyright clean.
2. Keep new or modified functions at C901 complexity 10 or lower unless the
   complexity ratchet is intentionally reviewed.
3. Reduce the existing complexity baseline opportunistically during refactors.
4. Keep package pyright at zero while downstream consumers move to the public
   API surface.
5. Add conformance contracts under `docs/contracts/` for any stable JSON,
   corpus manifest, or cruncher-facing output that leaves the package.
6. Use the first `kicad-cruncher` integration pass to decide which provisional
   `__all__` exports graduate into the promoted public contract.
