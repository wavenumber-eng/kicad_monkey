# KiCad Cruncher Dev-Std CLI Signoff Validation

Date: 2026-07-17

Scope:

- dev-std CLI governance migration
- expanded dev-std release-signoff scopes
- legacy active-plan closeout into durable requirements
- lower-bound dependency alignment for `kicad-monkey` and `wn-dev-std`

Local validation:

- `uv run dev-std audit . --scope docs.cli --format json` passed.
- `uv run dev-std audit . --scope docs.plans --format json` passed.
- `uv run dev-std audit . --scope docs.requirements --format json` passed.
- `uv run dev-std audit . --scope docs.release --format json` passed.
- `uv run dev-std audit . --format json` passed.
- `uv run dev-std audit . --check-upstream-version --format json` passed.
- `uv run pytest tests\L99_signoff -q` passed: 25 tests in 6.86 seconds.
- `uv run pytest tests\L0_public_cli -q` passed: 49 tests in 16.55 seconds.
- `uv run rack run L99` passed: 25 L99 tests in 7.00 seconds.
- `git diff --check` passed.

Runtime impact:

The changed local gates add one upstream-version dev-std audit to L99. The
observed full L99 runtime remained under 8 seconds locally after the added
audit and expanded governance scopes.

Release state:

No package version bump, tag, GitHub Release, or PyPI publish occurred. The
release-candidate gate still requires green PR CI, external review, and
explicit user authorization.
