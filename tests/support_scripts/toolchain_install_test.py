"""Validate the two monorepo wheels together without source-tree leakage."""

from __future__ import annotations

import argparse
import email.parser
import json
import os
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


def _latest_wheel(dist_dir: Path, prefix: str) -> Path:
    wheels = sorted(dist_dir.glob(f"{prefix}-*.whl"), key=lambda path: path.stat().st_mtime)
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
        metadata_name = next(name for name in names if name.endswith(".dist-info/METADATA"))
        payload = archive.read(metadata_name).decode("utf-8")
    parsed = email.parser.Parser().parsestr(payload)
    return names, parsed.get_all("Requires-Dist", [])


def _sdist_names(sdist: Path) -> list[str]:
    with tarfile.open(sdist, "r:gz") as archive:
        return archive.getnames()


def _venv_python(venv_dir: Path) -> Path:
    scripts = "Scripts" if os.name == "nt" else "bin"
    executable = "python.exe" if os.name == "nt" else "python"
    return venv_dir / scripts / executable


def _console_script(venv_dir: Path, command: str) -> Path:
    scripts = "Scripts" if os.name == "nt" else "bin"
    suffix = ".exe" if os.name == "nt" else ""
    return venv_dir / scripts / f"{command}{suffix}"


def _run(command: list[str], *, cwd: Path, env: dict[str, str]) -> None:
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        check=False,
        capture_output=True,
        text=True,
    )
    if completed.returncode != 0:
        raise SystemExit(
            f"Command failed ({completed.returncode}): {' '.join(command)}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )


def _validate_installed_design_review(
    executable: Path,
    *,
    temp_dir: Path,
    env: dict[str, str],
) -> None:
    """Generate and consume a graph-linked bundle from installed wheels only."""
    schematic_path = temp_dir / "smoke.kicad_sch"
    output_dir = temp_dir / "review"
    schematic_path.write_text(_INSTALLED_SMOKE_SCHEMATIC, encoding="utf-8")
    _run(
        [str(executable), "dr", str(schematic_path), "-o", str(output_dir)],
        cwd=temp_dir,
        env=env,
    )

    manifest = json.loads(
        (output_dir / "design_review_manifest.json").read_text(encoding="utf-8")
    )
    design = json.loads((output_dir / manifest["design_json"]).read_text(encoding="utf-8"))
    graph_record = manifest["compiled_schematic_graph"]
    graph = json.loads((output_dir / graph_record["file"]).read_text(encoding="utf-8"))
    if graph != design["compiled_schematic_graph"]:
        raise SystemExit("Installed design review standalone graph differs from Design JSON")

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
        raise SystemExit("Installed schematic SVG page occurrence differs from manifest")
    if not view["element_to_graphical_artifact_link_refs"]:
        raise SystemExit("Installed schematic SVG graph view has no drawing linkage")
    svg_ids = {
        element.attrib["id"] for element in root.iter() if element.attrib.get("id")
    }
    if not set(view["element_to_graphical_artifact_link_refs"]).issubset(svg_ids):
        raise SystemExit("Installed schematic SVG graph selector is missing")


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
            raise SystemExit(f"{package_name} sdist contains working-only documentation")
    if any(requirement.startswith("kicad-cruncher") for requirement in monkey_requirements):
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
    forbidden = (" @ ", "file:", "workspace", "\\", "../")
    if any(token in monkey_dependency for token in forbidden):
        raise SystemExit(f"Cruncher wheel leaks a non-public dependency: {monkey_dependency}")

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
        _run([str(_console_script(venv_dir, "kicad-cruncher")), "--version"], cwd=temp_dir, env=env)
        _run([str(_console_script(venv_dir, "kcr")), "--version"], cwd=temp_dir, env=env)
        _run([str(python), "-I", "-m", "kicad_cruncher", "version"], cwd=temp_dir, env=env)
        _validate_installed_design_review(
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
