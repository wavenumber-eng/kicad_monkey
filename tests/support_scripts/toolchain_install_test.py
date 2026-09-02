"""Validate the two monorepo wheels together without source-tree leakage."""

from __future__ import annotations

import argparse
import email.parser
import hashlib
import json
import os
import platform
import subprocess
import sys
import tarfile
import tempfile
import zipfile
import xml.etree.ElementTree as ET
from pathlib import Path


REPOSITORY_ROOT = Path(__file__).resolve().parents[2]

_INSTALLED_SMOKE_SCHEMATIC = """(kicad_sch
  (version 20250114)
  (generator "eeschema")
  (generator_version "9.0")
  (uuid "11111111-2222-4333-8444-555555555555")
  (paper "A4")
  (wire
    (pts (xy 10 10) (xy 20 10))
    (stroke (width 0) (type default))
    (uuid "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee")
  )
)
"""

_INSTALLED_SMOKE_PCB = """(kicad_pcb
  (version 20241229)
  (generator "pcbnew")
  (generator_version "9.0")
  (general (thickness 1.6) (legacy_teardrops no))
  (paper "A4")
  (layers
    (0 "F.Cu" signal)
    (31 "B.Cu" signal)
    (44 "Edge.Cuts" user)
  )
  (gr_rect
    (start 0 0)
    (end 20 10)
    (stroke (width 0.1) (type solid))
    (fill none)
    (layer "Edge.Cuts")
    (uuid "aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee")
  )
)
"""


def _latest_wheel(dist_dir: Path, prefix: str) -> Path:
    wheels = sorted(
        dist_dir.glob(f"{prefix}-*.whl"), key=lambda path: path.stat().st_mtime
    )
    if not wheels:
        raise SystemExit(f"No {prefix} wheel found in {dist_dir}")
    return wheels[-1]


def _latest_sdist(dist_dir: Path, prefix: str) -> Path:
    sdists = sorted(
        dist_dir.glob(f"{prefix}-*.tar.gz"), key=lambda path: path.stat().st_mtime
    )
    if not sdists:
        raise SystemExit(f"No {prefix} sdist found in {dist_dir}")
    return sdists[-1]


def _metadata(wheel: Path) -> tuple[list[str], list[str]]:
    with zipfile.ZipFile(wheel) as archive:
        names = archive.namelist()
        metadata_name = next(
            name for name in names if name.endswith(".dist-info/METADATA")
        )
        payload = archive.read(metadata_name).decode("utf-8")
    parsed = email.parser.Parser().parsestr(payload)
    return names, parsed.get_all("Requires-Dist", [])


def _sdist_names(sdist: Path) -> list[str]:
    with tarfile.open(sdist, "r:gz") as archive:
        return archive.getnames()


def _unique_sdist_payload(sdist: Path, relative_name: str) -> bytes:
    """Read one package-root-relative regular member and reject duplicates."""
    expected_parts = tuple(relative_name.split("/"))
    with tarfile.open(sdist, "r:gz") as archive:
        matches = [
            member
            for member in archive.getmembers()
            if member.isfile()
            and tuple(Path(member.name).parts[1:]) == expected_parts
        ]
        if len(matches) != 1:
            raise SystemExit(
                f"{sdist.name} must contain exactly one {relative_name}; "
                f"found {len(matches)}"
            )
        stream = archive.extractfile(matches[0])
        if stream is None:
            raise SystemExit(f"Could not read {relative_name} from {sdist.name}")
        return stream.read()


def _assert_windows_x64_native_payload(wheel: Path) -> None:
    """Require the selected Windows artifact to contain one AMD64 PE sidecar."""

    if os.name != "nt":
        return
    if platform.machine().casefold() not in {"amd64", "x86_64"}:
        raise SystemExit(
            f"P6_050 requires a Windows x64 runner, got {platform.machine()}"
        )
    if not wheel.name.endswith("-win_amd64.whl"):
        raise SystemExit(f"Monkey candidate is not a Windows x64 wheel: {wheel.name}")
    member = "kicad_monkey/_native/kicad-monkey-native.exe"
    with zipfile.ZipFile(wheel) as archive:
        names = archive.namelist()
        if names.count(member) != 1:
            raise SystemExit(f"Monkey wheel must contain exactly one {member}")
        binary = archive.read(member)
    if len(binary) < 0x40 or binary[:2] != b"MZ":
        raise SystemExit("Monkey native sidecar is not a PE executable")
    pe_offset = int.from_bytes(binary[0x3C:0x40], "little")
    if (
        pe_offset > len(binary) - 6
        or binary[pe_offset : pe_offset + 4] != b"PE\0\0"
        or int.from_bytes(binary[pe_offset + 4 : pe_offset + 6], "little") != 0x8664
    ):
        raise SystemExit("Monkey native sidecar is not an AMD64 PE executable")


def _venv_python(venv_dir: Path) -> Path:
    scripts = "Scripts" if os.name == "nt" else "bin"
    executable = "python.exe" if os.name == "nt" else "python"
    return venv_dir / scripts / executable


def _console_script(venv_dir: Path, command: str) -> Path:
    scripts = "Scripts" if os.name == "nt" else "bin"
    suffix = ".exe" if os.name == "nt" else ""
    return venv_dir / scripts / f"{command}{suffix}"


def _run(command: list[str], *, cwd: Path, env: dict[str, str]) -> None:
    completed = _run_captured(command, cwd=cwd, env=env)
    if completed.returncode != 0:
        raise SystemExit(
            f"Command failed ({completed.returncode}): {' '.join(command)}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )


def _run_captured(
    command: list[str],
    *,
    cwd: Path,
    env: dict[str, str],
) -> subprocess.CompletedProcess[str]:
    """Run one installed command and retain its exact public process result."""
    return subprocess.run(
        command,
        cwd=cwd,
        env=env,
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=180,
    )


def _expect_exit(
    command: list[str],
    expected: int,
    *,
    cwd: Path,
    env: dict[str, str],
    output_text: str,
    output_channel: str,
) -> subprocess.CompletedProcess[str]:
    """Require one controlled CLI exit on its exact public output channel."""
    completed = _run_captured(command, cwd=cwd, env=env)
    if completed.returncode != expected:
        raise SystemExit(
            f"Command returned {completed.returncode}, expected {expected}: "
            f"{' '.join(command)}\nstdout:\n{completed.stdout}\n"
            f"stderr:\n{completed.stderr}"
        )
    if output_channel not in {"stdout", "stderr"}:
        raise AssertionError(f"unsupported output channel: {output_channel}")
    selected = completed.stdout if output_channel == "stdout" else completed.stderr
    other = completed.stderr if output_channel == "stdout" else completed.stdout
    if output_text not in selected:
        raise SystemExit(
            f"Command {output_channel} omitted {output_text!r}: {' '.join(command)}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    if other:
        raise SystemExit(
            f"Command unexpectedly wrote to the other channel: {' '.join(command)}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    if "Traceback (most recent call last)" in selected:
        raise SystemExit(f"Command leaked a traceback: {' '.join(command)}")
    return completed


def _artifact_tree_bytes(root: Path) -> dict[str, bytes]:
    """Return one deterministic relative-file snapshot of an artifact tree."""
    if not root.is_dir():
        raise SystemExit(f"Design-review output directory is missing: {root}")
    return {
        path.relative_to(root).as_posix(): path.read_bytes()
        for path in sorted(root.rglob("*"))
        if path.is_file()
    }


def _require_no_output(root: Path) -> None:
    """Require a failed command to leave a previously absent destination absent."""
    if root.exists():
        raise SystemExit(f"Failed CLI invocation left an output path at {root}")


def _installed_cli_entrypoints(
    venv_dir: Path,
    python: Path,
) -> tuple[tuple[str, list[str], str], ...]:
    """Pair every public launcher with one established design-command alias."""
    return (
        (
            "kicad-cruncher design",
            [str(_console_script(venv_dir, "kicad-cruncher"))],
            "design",
        ),
        ("kcr design-review", [str(_console_script(venv_dir, "kcr"))], "design-review"),
        (
            "python -m kicad_cruncher dr",
            [str(python), "-I", "-m", "kicad_cruncher"],
            "dr",
        ),
    )


def _assert_installed_native_resolver(
    python: Path,
    *,
    temp_dir: Path,
    env: dict[str, str],
) -> None:
    """Require default native resolution to select this installed Monkey wheel."""

    if os.name != "nt":
        return
    completed = _run_captured(
        [
            str(python),
            "-I",
            "-c",
            (
                "import json, pathlib, platform, kicad_monkey; "
                "from kicad_monkey import resolve_kicad_native_executable; "
                "from kicad_cruncher.kicad_cruncher_native_design import "
                "use_native_design_facts_provider; "
                "from kicad_cruncher.kicad_cruncher_native_physical import "
                "use_native_physical_provider; "
                "print(json.dumps({'package': str(pathlib.Path(kicad_monkey.__file__)"
                ".resolve().parent), 'native': str(resolve_kicad_native_executable()), "
                "'machine': platform.machine(), "
                "'design_default': use_native_design_facts_provider(), "
                "'physical_default': use_native_physical_provider()}))"
            ),
        ],
        cwd=temp_dir,
        env=env,
    )
    if completed.returncode != 0 or completed.stderr:
        raise SystemExit(
            "Installed Monkey native resolver failed or wrote stderr:\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    payload = json.loads(completed.stdout)
    package_dir = Path(payload["package"]).resolve()
    native = Path(payload["native"]).resolve()
    if str(payload["machine"]).casefold() not in {"amd64", "x86_64"}:
        raise SystemExit(f"Installed Python did not report Windows x64: {payload}")
    if payload["design_default"] is not True or payload["physical_default"] is not True:
        raise SystemExit(
            f"Installed Windows native providers are not hard-selected: {payload}"
        )
    if "site-packages" not in package_dir.as_posix() or native.parent != (
        package_dir / "_native"
    ):
        raise SystemExit(
            f"Installed native resolver escaped its wheel payload: package={package_dir}, "
            f"native={native}"
        )


def _validate_design_review_output(
    output_dir: Path,
    *,
    require_native: bool,
) -> None:
    """Consume one installed graph-linked bundle and verify native provenance."""
    manifest = json.loads(
        (output_dir / "design_review_manifest.json").read_text(encoding="utf-8")
    )
    design = json.loads(
        (output_dir / manifest["design_json"]).read_text(encoding="utf-8")
    )
    graph_record = manifest["compiled_schematic_graph"]
    graph = json.loads((output_dir / graph_record["file"]).read_text(encoding="utf-8"))
    if graph != design["compiled_schematic_graph"]:
        raise SystemExit(
            "Installed design review standalone graph differs from Design JSON"
        )
    if require_native:
        facts = manifest.get("design_facts")
        if not isinstance(facts, dict) or facts.get("backend") != "kicad-monkey-native":
            raise SystemExit(
                "Installed Windows design review did not use native design facts"
            )
        if facts.get("resource_profile") != "design-facts-bounded-a1":
            raise SystemExit(
                "Installed native design facts use the wrong resource profile"
            )
        netlist = (output_dir / manifest["netlist_kicad_sexpr"]).read_bytes()
        if facts.get("kicad_netlist_bytes") != len(netlist):
            raise SystemExit(
                "Installed native netlist byte provenance differs from its file"
            )
        if facts.get("kicad_netlist_sha256") != hashlib.sha256(netlist).hexdigest():
            raise SystemExit(
                "Installed native netlist hash provenance differs from its file"
            )

    svg_record = manifest["schematic_svgs"][0]
    svg_text = (output_dir / svg_record["file"]).read_text(encoding="utf-8")
    root = ET.fromstring(svg_text)
    metadata = next(
        element
        for element in root.iter()
        if element.tag.rsplit("}", 1)[-1] == "metadata"
        and element.attrib.get("id") == "schematic-enrichment-a0"
    )
    view = json.loads("".join(metadata.itertext()))["compiled_schematic_graph_view"]
    if view["page_occurrence_ref"] != svg_record["page_occurrence_ref"]:
        raise SystemExit(
            "Installed schematic SVG page occurrence differs from manifest"
        )
    if not view["element_to_graphical_artifact_link_refs"]:
        raise SystemExit("Installed schematic SVG graph view has no drawing linkage")
    svg_ids = {
        element.attrib["id"] for element in root.iter() if element.attrib.get("id")
    }
    if not set(view["element_to_graphical_artifact_link_refs"]).issubset(svg_ids):
        raise SystemExit("Installed schematic SVG graph selector is missing")


def _validate_installed_design_review_cli(
    venv_dir: Path,
    python: Path,
    *,
    temp_dir: Path,
    env: dict[str, str],
) -> None:
    """Require all installed CLI launchers and design aliases to be equivalent."""
    schematic_path = temp_dir / "smoke.kicad_sch"
    schematic_path.write_text(_INSTALLED_SMOKE_SCHEMATIC, encoding="utf-8")
    baseline: dict[str, bytes] | None = None

    for ordinal, (label, prefix, alias) in enumerate(
        _installed_cli_entrypoints(venv_dir, python)
    ):
        version = _run_captured([*prefix, "--version"], cwd=temp_dir, env=env)
        if (
            version.returncode != 0
            or "kicad-cruncher " not in version.stdout
            or version.stderr
        ):
            raise SystemExit(
                f"Installed {label} version surface failed:\n"
                f"stdout:\n{version.stdout}\nstderr:\n{version.stderr}"
            )
        help_result = _run_captured([*prefix, alias, "--help"], cwd=temp_dir, env=env)
        if (
            help_result.returncode != 0
            or "design review bundle" not in help_result.stdout
            or help_result.stderr
        ):
            raise SystemExit(
                f"Installed {label} help surface failed:\n"
                f"stdout:\n{help_result.stdout}\nstderr:\n{help_result.stderr}"
            )

        output_dir = temp_dir / f"review-{ordinal}"
        _expect_exit(
            [*prefix, alias, str(schematic_path), "-o", str(output_dir)],
            0,
            cwd=temp_dir,
            env=env,
            output_text="Design review: starting bundle",
            output_channel="stdout",
        )
        _validate_design_review_output(output_dir, require_native=os.name == "nt")
        artifacts = _artifact_tree_bytes(output_dir)
        if baseline is None:
            baseline = artifacts
        elif artifacts != baseline:
            differing = sorted(set(artifacts) ^ set(baseline))
            changed = sorted(
                path
                for path in set(artifacts) & set(baseline)
                if artifacts[path] != baseline[path]
            )
            raise SystemExit(
                f"Installed {label} design artifact tree differs from the first launcher; "
                f"missing/extra={differing}, changed={changed}"
            )

    _validate_installed_design_cli_failures(
        venv_dir,
        python,
        schematic_path=schematic_path,
        temp_dir=temp_dir,
        env=env,
    )


def _validate_installed_design_cli_failures(
    venv_dir: Path,
    python: Path,
    *,
    schematic_path: Path,
    temp_dir: Path,
    env: dict[str, str],
) -> None:
    """Exercise stable public exit codes and transactional failure behavior."""
    entrypoints = _installed_cli_entrypoints(venv_dir, python)
    for ordinal, (label, prefix, alias) in enumerate(entrypoints):
        syntax_output = temp_dir / f"syntax-output-{ordinal}"
        _expect_exit(
            [
                *prefix,
                alias,
                str(schematic_path),
                "--not-a-real-option",
                "-o",
                str(syntax_output),
            ],
            2,
            cwd=temp_dir,
            env=env,
            output_text="unrecognized arguments",
            output_channel="stderr",
        )
        _require_no_output(syntax_output)

    _, prefix, alias = entrypoints[0]
    missing_output = temp_dir / "missing-input-output"
    _expect_exit(
        [
            *prefix,
            alias,
            str(temp_dir / "missing.kicad_sch"),
            "-o",
            str(missing_output),
        ],
        1,
        cwd=temp_dir,
        env=env,
        output_text="File not found",
        output_channel="stdout",
    )
    _require_no_output(missing_output)

    unsupported = temp_dir / "unsupported.txt"
    unsupported.write_text("not a KiCad document\n", encoding="utf-8")
    unsupported_output = temp_dir / "unsupported-input-output"
    _expect_exit(
        [*prefix, alias, str(unsupported), "-o", str(unsupported_output)],
        1,
        cwd=temp_dir,
        env=env,
        output_text="Unsupported file type",
        output_channel="stdout",
    )
    _require_no_output(unsupported_output)

    if os.name != "nt":
        return
    missing_native_env = dict(env)
    missing_native_env["KICAD_MONKEY_NATIVE"] = str(temp_dir / "missing-native.exe")
    for ordinal, (_label, current_prefix, current_alias) in enumerate(entrypoints):
        new_output = temp_dir / f"missing-native-output-{ordinal}"
        _expect_exit(
            [
                *current_prefix,
                current_alias,
                str(schematic_path),
                "-o",
                str(new_output),
            ],
            1,
            cwd=temp_dir,
            env=missing_native_env,
            output_text="Design review generation failed",
            output_channel="stdout",
        )
        _require_no_output(new_output)

    existing_output = temp_dir / "existing-design-output"
    existing_output.mkdir()
    sentinel = existing_output / "keep.txt"
    sentinel.write_bytes(b"previous-design-review\n")
    before = _artifact_tree_bytes(existing_output)
    _expect_exit(
        [*prefix, alias, str(schematic_path), "-o", str(existing_output)],
        1,
        cwd=temp_dir,
        env=missing_native_env,
        output_text="Design review generation failed",
        output_channel="stdout",
    )
    if _artifact_tree_bytes(existing_output) != before:
        raise SystemExit(
            "Failed native design command changed the previous output tree"
        )


def _validate_installed_native_pcb_svg(
    executable: Path,
    *,
    temp_dir: Path,
    env: dict[str, str],
) -> None:
    """Exercise the Windows no-fallback physical provider from installed wheels."""
    if os.name != "nt":
        return
    pcb_path = temp_dir / "native physical smoke.kicad_pcb"
    output_dir = temp_dir / "pcb-svg"
    pcb_path.write_text(_INSTALLED_SMOKE_PCB, encoding="utf-8")
    _run(
        [
            str(executable),
            "pcb-svg",
            str(pcb_path),
            "--views",
            "assembly-top",
            "-o",
            str(output_dir),
        ],
        cwd=temp_dir,
        env=env,
    )
    manifest_path = output_dir / "native physical smoke__views.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest["schema"] != "pcb.svg.manifest.a0":
        raise SystemExit("Installed native PCB SVG manifest has the wrong schema")
    svg_paths = sorted(output_dir.rglob("*.svg"))
    if not svg_paths:
        raise SystemExit("Installed native PCB SVG command emitted no SVG")
    roots = [ET.fromstring(path.read_text(encoding="utf-8")) for path in svg_paths]
    if not any(
        root.attrib.get("data-enrichment-schema")
        == "kicad_monkey.pcb.svg.enrichment.a0"
        for root in roots
    ):
        raise SystemExit("Installed native PCB SVG lacks physical enrichment")

    missing_native_env = dict(env)
    missing_native_env["KICAD_MONKEY_NATIVE"] = str(temp_dir / "missing-native.exe")
    missing_output = temp_dir / "pcb-svg-missing-native"
    _expect_exit(
        [
            str(executable),
            "pcb-svg",
            str(pcb_path),
            "--views",
            "assembly-top",
            "-o",
            str(missing_output),
        ],
        1,
        cwd=temp_dir,
        env=missing_native_env,
        output_text="PCB SVG generation failed",
        output_channel="stdout",
    )
    _require_no_output(missing_output)

    existing_output = temp_dir / "pcb-svg-existing"
    existing_output.mkdir()
    (existing_output / "keep.txt").write_bytes(b"previous-pcb-svg\n")
    before = _artifact_tree_bytes(existing_output)
    _expect_exit(
        [
            str(executable),
            "pcb-svg",
            str(pcb_path),
            "--views",
            "assembly-top",
            "-o",
            str(existing_output),
        ],
        1,
        cwd=temp_dir,
        env=missing_native_env,
        output_text="PCB SVG generation failed",
        output_channel="stdout",
    )
    if _artifact_tree_bytes(existing_output) != before:
        raise SystemExit(
            "Failed native pcb-svg command changed the previous output tree"
        )


def validate_artifacts(
    monkey_wheel: Path,
    cruncher_wheel: Path,
    monkey_sdist: Path,
    cruncher_sdist: Path,
) -> None:
    monkey_wheel = monkey_wheel.resolve()
    cruncher_wheel = cruncher_wheel.resolve()
    monkey_names, monkey_requirements = _metadata(monkey_wheel)
    cruncher_names, cruncher_requirements = _metadata(cruncher_wheel)
    monkey_sdist_names = _sdist_names(monkey_sdist.resolve())
    cruncher_sdist_names = _sdist_names(cruncher_sdist.resolve())
    _assert_windows_x64_native_payload(monkey_wheel)

    if any(name.startswith("kicad_cruncher/") for name in monkey_names):
        raise SystemExit("Monkey wheel unexpectedly contains Cruncher source")
    if any(name.startswith("kicad_monkey/") for name in cruncher_names):
        raise SystemExit("Cruncher wheel unexpectedly contains Monkey source")
    if any("/packages/kicad_cruncher/" in name for name in monkey_sdist_names):
        raise SystemExit("Monkey sdist unexpectedly contains Cruncher source")
    if any("/src/py/kicad_monkey/" in name for name in cruncher_sdist_names):
        raise SystemExit("Cruncher sdist unexpectedly contains Monkey source")
    for package_name, names in (
        ("Monkey", monkey_sdist_names),
        ("Cruncher", cruncher_sdist_names),
    ):
        if any("/docs/plans/" in name or "/docs/research/" in name for name in names):
            raise SystemExit(
                f"{package_name} sdist contains working-only documentation"
            )
        if any(
            "/tests/.tmp/" in name
            or "/dist/" in name
            or name.endswith((".whl", ".tar.gz"))
            for name in names
        ):
            raise SystemExit(
                f"{package_name} sdist contains transient or nested build artifacts"
            )

    packaged_manifest = _unique_sdist_payload(monkey_sdist, "Cargo.toml")
    expected_manifest = (
        REPOSITORY_ROOT / "packaging" / "Cargo.sdist.toml"
    ).read_bytes()
    if packaged_manifest != expected_manifest:
        raise SystemExit("Monkey sdist Cargo.toml is not the standalone workspace manifest")
    if any(
        requirement.startswith("kicad-cruncher") for requirement in monkey_requirements
    ):
        raise SystemExit("Monkey wheel unexpectedly depends on Cruncher")

    monkey_dependency = next(
        (
            requirement
            for requirement in cruncher_requirements
            if requirement.startswith("kicad-monkey")
        ),
        "",
    )
    if not monkey_dependency:
        raise SystemExit("Cruncher wheel is missing its public kicad-monkey dependency")
    if ">=2026.9.2" not in monkey_dependency.replace(" ", ""):
        raise SystemExit(
            "Cruncher wheel does not retain the governed kicad-monkey>=2026.9.2 "
            f"floor: {monkey_dependency}"
        )
    forbidden = (" @ ", "file:", "workspace", "\\", "../")
    if any(token in monkey_dependency for token in forbidden):
        raise SystemExit(
            f"Cruncher wheel leaks a non-public dependency: {monkey_dependency}"
        )

    with tempfile.TemporaryDirectory(prefix="kicad_toolchain_install_test_") as temp:
        temp_dir = Path(temp).resolve()
        venv_dir = temp_dir / "venv"
        subprocess.run(
            [sys.executable, "-m", "venv", str(venv_dir)],
            cwd=temp_dir,
            check=True,
        )
        python = _venv_python(venv_dir)
        env = os.environ.copy()
        env.pop("PYTHONPATH", None)
        env.pop("PYTHONHOME", None)
        for variable in (
            "KICAD_MONKEY_NATIVE",
            "KICAD_CRUNCHER_NATIVE_PHYSICAL",
            "KICAD_CRUNCHER_NATIVE_DESIGN_FACTS",
        ):
            env.pop(variable, None)
        scripts = venv_dir / ("Scripts" if os.name == "nt" else "bin")
        env["PATH"] = str(scripts) + os.pathsep + env.get("PATH", "")

        _run(
            [
                str(python),
                "-m",
                "pip",
                "install",
                "--disable-pip-version-check",
                "--no-cache-dir",
                str(monkey_wheel),
                str(cruncher_wheel),
            ],
            cwd=temp_dir,
            env=env,
        )
        _assert_installed_native_resolver(python, temp_dir=temp_dir, env=env)
        _run(
            [
                str(python),
                "-I",
                "-c",
                (
                    "import pathlib, kicad_monkey, kicad_cruncher; "
                    "assert 'site-packages' in pathlib.Path(kicad_monkey.__file__).as_posix(); "
                    "assert 'site-packages' in pathlib.Path(kicad_cruncher.__file__).as_posix()"
                ),
            ],
            cwd=temp_dir,
            env=env,
        )
        _run(
            [str(_console_script(venv_dir, "kicad-cruncher")), "--version"],
            cwd=temp_dir,
            env=env,
        )
        _run(
            [str(_console_script(venv_dir, "kcr")), "--version"], cwd=temp_dir, env=env
        )
        _run(
            [str(python), "-I", "-m", "kicad_cruncher", "version"],
            cwd=temp_dir,
            env=env,
        )
        _validate_installed_design_review_cli(
            venv_dir,
            python,
            temp_dir=temp_dir,
            env=env,
        )
        _validate_installed_native_pcb_svg(
            _console_script(venv_dir, "kcr"),
            temp_dir=temp_dir,
            env=env,
        )

    sys.stdout.write(
        "Toolchain artifact test passed: "
        f"{monkey_wheel.name} + {cruncher_wheel.name}; "
        f"{monkey_sdist.name} + {cruncher_sdist.name}\n"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--monkey-wheel", type=Path)
    parser.add_argument("--cruncher-wheel", type=Path)
    parser.add_argument("--monkey-sdist", type=Path)
    parser.add_argument("--cruncher-sdist", type=Path)
    args = parser.parse_args()
    monkey_wheel = args.monkey_wheel or _latest_wheel(
        REPOSITORY_ROOT / "dist", "kicad_monkey"
    )
    cruncher_wheel = args.cruncher_wheel or _latest_wheel(
        REPOSITORY_ROOT / "packages" / "kicad_cruncher" / "dist",
        "kicad_cruncher",
    )
    monkey_sdist = args.monkey_sdist or _latest_sdist(
        REPOSITORY_ROOT / "dist", "kicad_monkey"
    )
    cruncher_sdist = args.cruncher_sdist or _latest_sdist(
        REPOSITORY_ROOT / "packages" / "kicad_cruncher" / "dist",
        "kicad_cruncher",
    )
    validate_artifacts(monkey_wheel, cruncher_wheel, monkey_sdist, cruncher_sdist)


if __name__ == "__main__":
    main()
