#!/usr/bin/env python
"""Contract tests for the compact CI and release policy."""

from __future__ import annotations

import hashlib
import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

import yaml

from check_release_artifact_paths import ArtifactEntry, find_path_leaks
from classify_ci_scope import classify
from universal_release_candidate import verify, write
from verify_phase6_release_candidates import verify as verify_phase6
from verify_pypi_release import (
    compare_compatible_release_files,
    compare_release_files,
)


ROOT = Path(__file__).resolve().parents[1]
WORKFLOWS = ROOT / ".github" / "workflows"


class ScopeTests(unittest.TestCase):
    def test_docs_and_workflow_changes_are_fast(self) -> None:
        self.assertEqual(
            classify(["docs/index.html"], event_name="pull_request"), "fast"
        )
        self.assertEqual(
            classify([".github/workflows/ci.yml"], event_name="push"), "fast"
        )
        self.assertEqual(
            classify(
                [
                    "docs/index.html",
                    "packages/kicad_cruncher/docs/release-process.md",
                    "packages/kicad_cruncher/README.md",
                ],
                event_name="pull_request",
            ),
            "fast",
        )

    def test_dependency_pull_request_is_python_but_main_is_full(self) -> None:
        paths = ["pyproject.toml", "packages/kicad_cruncher/pyproject.toml", "uv.lock"]
        self.assertEqual(classify(paths, event_name="pull_request"), "python")
        self.assertEqual(classify(paths, event_name="push"), "full")

    def test_native_unknown_and_manual_changes_are_full(self) -> None:
        self.assertEqual(
            classify(["src/rs/native.rs"], event_name="pull_request"), "full"
        )
        self.assertEqual(
            classify(["hatch_build.py"], event_name="pull_request"), "full"
        )
        self.assertEqual(classify(["surprise.bin"], event_name="pull_request"), "full")
        self.assertEqual(
            classify(
                ["packages/kicad_cruncher/surprise.bin"],
                event_name="pull_request",
            ),
            "full",
        )
        self.assertEqual(
            classify(
                ["packages/kicad_cruncher/tests/rack.toml"],
                event_name="pull_request",
            ),
            "full",
        )
        self.assertEqual(
            classify(
                [
                    "packages/kicad_cruncher/tests/support_scripts/"
                    "rust_cli_install_test.py"
                ],
                event_name="pull_request",
            ),
            "full",
        )
        self.assertEqual(classify([], event_name="workflow_dispatch"), "full")

    def test_mixed_change_escalates(self) -> None:
        self.assertEqual(
            classify(["docs/index.html", "Cargo.lock"], event_name="pull_request"),
            "full",
        )


class WorkflowTests(unittest.TestCase):
    def test_workflows_parse(self) -> None:
        for path in WORKFLOWS.glob("*.yml"):
            with self.subTest(path=path.name):
                self.assertIsInstance(
                    yaml.safe_load(path.read_text(encoding="utf-8")), dict
                )

    def test_retired_duplicate_workflows_are_absent(self) -> None:
        retired = {
            "phase6-exit.yml",
            "phase6-native-design-facts.yml",
            "phase6-native-full-cli.yml",
            "phase6-native-physical-provider.yml",
            "phase6-native-svg.yml",
            "phase7-rust-cli.yml",
        }
        self.assertFalse(retired & {path.name for path in WORKFLOWS.glob("*.yml")})

    def test_release_is_one_idempotent_exact_run_publisher(self) -> None:
        release = (WORKFLOWS / "release.yml").read_text(encoding="utf-8")
        self.assertNotIn("candidate_run_id", release)
        self.assertNotIn("phase7_run_id", release)
        self.assertNotIn("recover-", release)
        self.assertIn("skip-existing: true", release)
        self.assertIn("head_sha", release)
        self.assertIn("run-id:", release)
        self.assertNotIn("uv run", release)

    def test_ci_has_only_three_scopes_and_one_native_candidate_call(self) -> None:
        ci = (WORKFLOWS / "ci.yml").read_text(encoding="utf-8")
        self.assertIn("fast, python, or full", ci)
        self.assertEqual(ci.count("windows-release-candidates.yml"), 1)
        self.assertIn(
            "uv run --no-project --with pyyaml python scripts/test_ci_policy.py", ci
        )
        self.assertEqual(ci.count("check_release_artifact_paths.py"), 1)
        self.assertIn('--forbid "${HOME}"', ci)
        self.assertIn("git status --porcelain --untracked-files=all", ci)
        candidates = (WORKFLOWS / "windows-release-candidates.yml").read_text(
            encoding="utf-8"
        )
        self.assertIn("--remap-path-prefix=$cargoHome=.cargo", candidates)
        self.assertEqual(candidates.count("check_release_artifact_paths.py"), 2)
        self.assertIn("git status --porcelain --untracked-files=all", candidates)
        self.assertIn("$build = Join-Path", candidates)
        self.assertNotIn("uv build --sdist --out-dir $dist", candidates)

    def test_release_verifies_public_bytes_before_creating_releases(self) -> None:
        release_path = WORKFLOWS / "release.yml"
        release = release_path.read_text(encoding="utf-8")
        self.assertEqual(release.count("verify_pypi_release.py"), 4)
        self.assertEqual(release.count("--pre-upload --attempts 1"), 2)
        self.assertLess(
            release.index("verify_pypi_release.py kicad-cruncher"),
            release.index('gh release create "${MONKEY_TAG}"'),
        )
        workflow = yaml.safe_load(release_path.read_text(encoding="utf-8"))
        for job_name in ("publish-monkey", "publish-cruncher"):
            publish_steps = workflow["jobs"][job_name]["steps"]
            preflight = next(
                index
                for index, step in enumerate(publish_steps)
                if "--pre-upload" in step.get("run", "")
            )
            upload = next(
                index
                for index, step in enumerate(publish_steps)
                if str(step.get("uses", "")).startswith("pypa/gh-action-pypi-publish")
            )
            self.assertLess(preflight, upload)
        steps = workflow["jobs"]["create-releases"]["steps"]
        phase6_download = next(
            index
            for index, step in enumerate(steps)
            if step.get("with", {}).get("name")
            == "phase6-windows-x64-candidates"
        )
        phase6_verify = next(
            index
            for index, step in enumerate(steps)
            if "verify_phase6_release_candidates.py" in step.get("run", "")
        )
        self.assertLess(phase6_download, phase6_verify)


class CandidateTests(unittest.TestCase):
    def test_release_artifacts_reject_wsl_and_machine_specific_paths(self) -> None:
        clean = ArtifactEntry(Path("clean.whl"), "package/module.py", b"portable")
        leaked = [
            ArtifactEntry(Path("bad.whl"), "native.exe", b"/mnt/c/eli/work"),
            ArtifactEntry(Path("bad.whl"), "metadata.json", b"file:///tmp/build"),
            ArtifactEntry(Path("bad.whl"), "native.exe", b"C:\\eli\\worktree"),
            ArtifactEntry(
                Path("bad.whl"),
                "wide-native.exe",
                r"C:\eli\wide-worktree".encode("utf-16-le"),
            ),
            ArtifactEntry(
                Path("bad.whl"),
                "wide-native-be.exe",
                "/mnt/d/wide-worktree".encode("utf-16-be"),
            ),
        ]
        self.assertEqual(find_path_leaks([clean], [r"C:\eli"]), [])
        findings = find_path_leaks(leaked, [r"C:\eli"])
        self.assertEqual(
            {label for _, _, label in findings},
            {"WSL mount path", "local file URL", "explicit local root"},
        )

    def test_public_release_requires_the_exact_candidate_files_and_hashes(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            wheel = root / "example-1.0-py3-none-any.whl"
            sdist = root / "example-1.0.tar.gz"
            wheel.write_bytes(b"wheel")
            sdist.write_bytes(b"sdist")
            payload = {
                "info": {"version": "1.0"},
                "urls": [
                    {
                        "filename": path.name,
                        "digests": {
                            "sha256": hashlib.sha256(path.read_bytes()).hexdigest()
                        },
                    }
                    for path in (wheel, sdist)
                ],
            }
            compare_release_files(root, payload, expected_version="1.0")
            partial_payload = {"info": payload["info"], "urls": payload["urls"][:1]}
            compare_compatible_release_files(
                root, partial_payload, expected_version="1.0"
            )
            payload["urls"][0]["digests"]["sha256"] = "0" * 64
            with self.assertRaises(SystemExit):
                compare_release_files(root, payload, expected_version="1.0")
            with self.assertRaises(SystemExit):
                compare_compatible_release_files(
                    root, payload, expected_version="1.0"
                )

    def test_universal_candidate_is_commit_bound_and_rejects_tampering(self) -> None:
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            wheel = root / "kicad_monkey-2026.8.22-py3-none-any.whl"
            wheel.write_bytes(b"tested wheel")
            write(
                root,
                "a" * 40,
                run_id="1234",
                workflow="CI",
                version="2026.8.22",
            )
            self.assertEqual(
                verify(root, "a" * 40, expected_version="2026.8.22"),
                wheel.resolve(),
            )
            wheel.write_bytes(b"changed wheel")
            with self.assertRaises(SystemExit):
                verify(root, "a" * 40, expected_version="2026.8.22")

    def test_phase6_candidate_versions_and_hashes_are_enforced(self) -> None:
        names = {
            "monkey_sdist": "kicad_monkey-2026.8.22.tar.gz",
            "monkey_windows_x64_wheel": (
                "kicad_monkey-2026.8.22-py3-none-win_amd64.whl"
            ),
            "cruncher_sdist": "kicad_cruncher-2026.8.22.tar.gz",
            "cruncher_universal_wheel": ("kicad_cruncher-2026.8.22-py3-none-any.whl"),
        }
        with TemporaryDirectory() as temporary:
            root = Path(temporary)
            artifacts = []
            for role, name in names.items():
                path = root / name
                path.write_bytes(role.encode())
                artifacts.append(
                    {
                        "role": role,
                        "filename": name,
                        "bytes": path.stat().st_size,
                        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
                    }
                )
            (root / "phase6-release-candidate-a0.json").write_text(
                json.dumps(
                    {
                        "schema": "kicad_monkey.phase6_release_candidate.a0",
                        "platform": "windows-x64",
                        "git_sha": "b" * 40,
                        "source": {"workflow": "CI", "run_id": "5678"},
                        "versions": {
                            "monkey": "2026.8.22",
                            "cruncher": "2026.8.22",
                        },
                        "artifacts": artifacts,
                    }
                ),
                encoding="utf-8",
            )
            verify_phase6(
                root,
                git_sha="b" * 40,
                monkey_version="2026.8.22",
                cruncher_version="2026.8.22",
            )
            (root / names["cruncher_universal_wheel"]).write_bytes(b"tampered")
            with self.assertRaises(SystemExit):
                verify_phase6(root, git_sha="b" * 40)


if __name__ == "__main__":
    unittest.main()
