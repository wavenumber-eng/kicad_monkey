# CI and release ownership

The repository has three CI scopes and one release path.

| Invariant | Authoritative owner |
| --- | --- |
| Workflow and documentation integrity | `workflow-validation` on every change |
| Python APIs, dependency compatibility, packages, and installed entry points | Linux `python-provider` |
| Windows native-provider selection on dependency pull requests | `windows-provider-smoke` |
| KiCad-oracle parity | `phase5-exit.yml`, in parallel with other full gates |
| Native providers, exact Python distributions, and Rust CLI migration | the single `windows-release-candidates.yml` job |
| Publication | `release.yml`, consuming artifacts from the exact successful main CI run |

Pull requests containing only documentation or workflow edits use `fast`.
Python or dependency-only pull requests use `python`. Rust, native, corpus,
contract, mixed, or unknown changes use `full`. On `main`, every Python or
dependency change is promoted to `full` so an exact release candidate is built
once. A manual CI dispatch is always `full`.

Release never rebuilds a distribution. It infers both versions from the tagged
main commit, locates that commit's successful `CI` run, verifies the manifests
and hashes, and publishes those bytes with `skip-existing`. Retrying the same
workflow is the recovery operation; there is no separate recovery graph and no
operator-supplied run ID.
