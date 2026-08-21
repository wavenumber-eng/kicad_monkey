# Contributing

`kicad-monkey` accepts direct public pull requests once CI is enabled.

Use `uv` for local development and test commands. Library install
documentation should prefer `pip install kicad-monkey` or normal project
dependencies; application workflows belong in downstream packages such as
`kicad-cruncher`.

Use the GitHub issue templates for bugs and feature requests. Include the exact
API call or workflow, package version, and only reproduction files that can be
shared publicly.

## Contribution Workflow

Open an issue or GitHub Discussion before starting a public PR, unless the
change is a small documentation typo or an obviously isolated bug fix.

For any public API, JSON, corpus format, generated artifact, dependency, or
workflow change, contributors should agree on the design first. That discussion
should settle intended behavior, compatibility impact, docs updates, contracts,
and required tests before a PR is considered ready for review.

PRs that change public behavior should link the issue or discussion where the
design was agreed and include the matching design-doc, contract, and test
updates.

## Commit Messages And Human Signoff

Commit messages, PR summaries, and signoff notes should be concise, factual,
and limited to what changed, why it changed, and how it was validated. Do not
use emojis, decorative prefixes, or marketing-style language.

Every PR signoff should identify the responsible human by name or GitHub user
ID. If an AI coding agent materially assisted with the change, include that as
an implementation note, not as the accountable signoff.

Before opening a PR:

1. Keep changes focused on one API, contract, corpus, or infrastructure area.
2. Add or update tests for every public behavior change.
3. Update docs for public APIs, JSON outputs, corpus formats, and stable
   generated artifacts.
4. Justify every new public feature and dependency in the commit, PR, or linked
   plan.
5. Run package tests and signoff locally.

Minimize external dependencies. A new dependency must explain why the standard
library and existing project dependencies are not enough, whether it is
runtime/optional/test-only, its license compatibility, and the expected
packaging impact.

Design documentation is release-signoff material:

- every promoted public class in `kicad_monkey.kicad_api_contract` needs a
  `docs/design/api/*.html` section;
- interface docs must cover rationale, purpose, test requirements, working
  definition, and Rack test ownership;
- stable JSON outputs, corpus manifest formats, and cruncher-facing contracts
  need docs under `docs/contracts/` plus conformance tests;
- broad package-root `__all__` exports are provisional until they are promoted
  into the public API contract.

Expected local checks:

```powershell
uv run --extra test python scripts/kicad_corpus_archive.py restore --check-zip
uv run --extra test python tests/rack.py run L0_foundation
uv run --extra test python tests/rack.py run L1_029
uv run --extra test python tests/rack.py run L99_signoff
uv run --extra test python -m build
uv run --extra test twine check dist/*
```

Run narrower targeted tests while developing, then run the signoff gates before
opening a PR. Corpus-backed tests use `tests/corpus/kicad.zip` unless
`KM_CORPUS` points at an external reviewed `kicad.zip` archive.

Release decisions, compatibility policy, and public contract changes should be
recorded in `docs/adrs/`.
