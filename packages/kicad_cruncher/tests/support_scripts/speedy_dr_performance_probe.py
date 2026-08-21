"""Benchmark Python and pure-Rust design review on reviewed Speedy bytes.

This is a research/signoff helper, not a microbenchmark. Run it on the same
host, commit, dependency lock, archive, release profile, and power policy when
comparing results. Build time, fixture extraction, validation, and cleanup are
outside the measured command interval.
"""

from __future__ import annotations

import argparse
import copy
import hashlib
import importlib.metadata
import importlib.util
import json
import math
import os
import platform
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
import xml.etree.ElementTree as ET
import zipfile
from collections import Counter
from pathlib import Path, PurePosixPath
from typing import Any

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
WORKSPACE = PACKAGE_ROOT.parents[1]
SPEEDY_PREFIX = PurePosixPath("kicad/projects/speedy_processing_module/input")
SPEEDY_PROJECT = "11-10084__speedy_processing_module__B.kicad_pro"
JsonObject = dict[str, Any]
_NUMBER = re.compile(r"[-+]?\d*\.?\d+(?:[eE][-+]?\d+)?")
_DRAWABLE = {"path", "polygon", "polyline", "line", "rect", "circle", "ellipse"}
_RUST_PROFILE_PREFIX = "KICAD_CRUNCHER_PERFORMANCE_PROFILE="
_RUST_PROFILE_SCHEMA = "kicad_cruncher.design_review.performance_profile.a3"
_RUST_PROFILE_STAGES = (
    "resolve_and_validate_output",
    "create_staging_directory",
    "load_design_sources",
    "build_structured_design_facts",
    "write_structured_artifacts",
    "build_schematic_plot_documents",
    "render_schematic_base_svgs",
    "enrich_schematic_review_svgs",
    "write_schematic_svgs",
    "build_board_plot_document",
    "build_pcb_review_svgs",
    "write_pcb_svgs",
    "build_and_write_bundle_metadata",
    "publish_staged_tree",
    "cleanup_transaction",
)
_RUST_PROFILE_DETAILS = (
    ("load_design_sources", "resolve_design_paths"),
    ("load_design_sources", "read_project_source"),
    ("load_design_sources", "read_schematic_sources"),
    ("load_design_sources", "parse_schematic_documents"),
    ("load_design_sources", "extract_schematic_definitions"),
    ("load_design_sources", "discover_schematic_hierarchy"),
    ("load_design_sources", "insert_schematic_source_carriers"),
    ("load_design_sources", "assemble_and_hash_source_bundle"),
    ("load_design_sources", "read_pcb_source"),
    ("build_structured_design_facts", "parse_schematic_index_definitions"),
    ("build_structured_design_facts", "realize_schematic_occurrences"),
    ("build_structured_design_facts", "assemble_schematic_indexes"),
    ("build_structured_design_facts", "parse_project_document"),
    ("build_structured_design_facts", "build_compiled_schematic_graph"),
    ("build_structured_design_facts", "build_kicad_netlist"),
    ("build_structured_design_facts", "validate_compiled_schematic_graph"),
    ("build_structured_design_facts", "emit_kicad_netlist"),
    ("build_structured_design_facts", "build_kicad_netlist_json"),
    ("build_structured_design_facts", "parse_pcb_view"),
    ("build_structured_design_facts", "design_json_binding_and_preflight"),
    ("build_structured_design_facts", "design_json_netlist_json"),
    ("build_structured_design_facts", "design_json_project_variants_options"),
    ("build_structured_design_facts", "design_json_sheets"),
    ("build_structured_design_facts", "design_json_components"),
    (
        "build_structured_design_facts",
        "design_json_schematic_hierarchy_and_nets",
    ),
    ("build_structured_design_facts", "design_json_compiled_graph_value"),
    ("build_structured_design_facts", "design_json_pnp"),
    ("build_structured_design_facts", "design_json_classes_and_indexes"),
    (
        "build_structured_design_facts",
        "design_json_output_limit_serialization",
    ),
    ("build_structured_design_facts", "enumerate_schematic_instances"),
    ("build_schematic_plot_documents", "extract_embedded_sidecars"),
    ("build_schematic_plot_documents", "load_project_plot_sidecars"),
    ("build_schematic_plot_documents", "scan_requested_font_faces"),
    ("build_schematic_plot_documents", "index_and_select_plot_fonts"),
    ("build_schematic_plot_documents", "build_plot_font_resources"),
    ("build_schematic_plot_documents", "plot_ir_validate_source_parse"),
    ("build_schematic_plot_documents", "plot_ir_select_and_collect_inputs"),
    ("build_schematic_plot_documents", "plot_ir_worksheet_header"),
    ("build_schematic_plot_documents", "plot_ir_connectivity"),
    ("build_schematic_plot_documents", "plot_ir_text_resource_setup"),
    ("build_schematic_plot_documents", "plot_ir_annotations"),
    ("build_schematic_plot_documents", "plot_ir_graphics_and_rule_areas"),
    ("build_schematic_plot_documents", "plot_ir_images"),
    ("build_schematic_plot_documents", "plot_ir_tables"),
    ("build_schematic_plot_documents", "plot_ir_symbols"),
    ("build_schematic_plot_documents", "plot_ir_sheets"),
    ("build_schematic_plot_documents", "budget_plot_contracts"),
    ("build_schematic_plot_documents", "project_plot_contract_json"),
    ("build_schematic_plot_documents", "serialize_plot_contract_aggregate"),
    ("render_schematic_base_svgs", "project_native_svg_requests"),
    ("render_schematic_base_svgs", "render_native_base_svg"),
    ("render_schematic_base_svgs", "bind_base_svg_identity"),
    ("enrich_schematic_review_svgs", "validate_graph_and_design_binding"),
    ("enrich_schematic_review_svgs", "build_graph_page_index"),
    ("enrich_schematic_review_svgs", "build_view_index_authority"),
    ("enrich_schematic_review_svgs", "validate_document_alignment"),
    ("enrich_schematic_review_svgs", "build_graph_page_views"),
    ("enrich_schematic_review_svgs", "project_record_attributes"),
    ("enrich_schematic_review_svgs", "index_and_validate_svg_selectors"),
    ("enrich_schematic_review_svgs", "build_schematic_view_indexes"),
    ("enrich_schematic_review_svgs", "compose_review_svg_root"),
    ("enrich_schematic_review_svgs", "serialize_review_svg_metadata"),
    ("enrich_schematic_review_svgs", "transform_review_svg_body"),
    ("enrich_schematic_review_svgs", "finish_review_svg_output"),
)


def _run_checked(
    command: list[str],
    *,
    cwd: Path,
    env: dict[str, str] | None = None,
    timeout: float = 900,
) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=env,
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=timeout,
        check=False,
    )
    if completed.returncode != 0:
        raise AssertionError(
            f"Command failed ({completed.returncode}): {' '.join(command)}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    return completed


def _sha256_bytes(payload: bytes) -> str:
    return hashlib.sha256(payload).hexdigest()


def _sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _tree_sha256(root: Path) -> str:
    digest = hashlib.sha256()
    for path in sorted(item for item in root.rglob("*") if item.is_file()):
        relative = path.relative_to(root).as_posix().encode("utf-8")
        digest.update(len(relative).to_bytes(8, "big"))
        digest.update(relative)
        digest.update(path.stat().st_size.to_bytes(8, "big"))
        with path.open("rb") as stream:
            for chunk in iter(lambda: stream.read(1024 * 1024), b""):
                digest.update(chunk)
    return digest.hexdigest()


def _canonical_json_sha256(path: Path) -> str:
    payload = json.loads(path.read_text(encoding="utf-8"))
    canonical = json.dumps(
        payload,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    return _sha256_bytes(canonical)


def _reviewed_archive(workspace: Path) -> Path:
    configured = os.environ.get("KM_CORPUS", "").strip()
    archive = (
        (Path(configured) if configured else workspace / "tests/corpus/kicad.zip")
        .expanduser()
        .resolve()
    )
    if archive.suffix.lower() != ".zip" or not archive.is_file():
        raise AssertionError(f"KM_CORPUS must name the reviewed kicad.zip: {archive}")
    return archive


def _extract_speedy(archive_path: Path, destination: Path) -> Path:
    selected = 0
    resolved_destination = destination.resolve()
    with zipfile.ZipFile(archive_path) as archive:
        for info in archive.infolist():
            member = PurePosixPath(info.filename)
            if not member.is_relative_to(SPEEDY_PREFIX):
                continue
            relative = member.relative_to(SPEEDY_PREFIX)
            if relative == PurePosixPath(".") or info.is_dir():
                continue
            if any(
                part in ("", ".", "..") or "\\" in part or ":" in part for part in relative.parts
            ):
                raise AssertionError(f"unsafe reviewed corpus member: {info.filename}")
            target = destination.joinpath(*relative.parts).resolve()
            if not target.is_relative_to(resolved_destination):
                raise AssertionError(f"reviewed corpus member escaped: {info.filename}")
            target.parent.mkdir(parents=True, exist_ok=True)
            with archive.open(info) as source, target.open("wb") as output:
                shutil.copyfileobj(source, output)
            selected += 1
    project = destination / SPEEDY_PROJECT
    if selected == 0 or not project.is_file():
        raise AssertionError(
            f"reviewed corpus omits Speedy fixture {SPEEDY_PREFIX}/{SPEEDY_PROJECT}"
        )
    return project


def _release_binaries(workspace: Path) -> tuple[Path, Path]:
    native = workspace / "target/release/kicad-monkey-native.exe"
    cruncher = workspace / "target/release/kicad-cruncher.exe"
    if os.name != "nt":
        native = native.with_suffix("")
        cruncher = cruncher.with_suffix("")
    missing = [str(path) for path in (native, cruncher) if not path.is_file()]
    if missing:
        raise AssertionError(
            "release binaries are missing; run with --build-release or build them: "
            + ", ".join(missing)
        )
    return native.resolve(), cruncher.resolve()


def _build_release(workspace: Path) -> None:
    _run_checked(
        [
            "cargo",
            "build",
            "--release",
            "--locked",
            "-p",
            "kicad-monkey-native",
            "-p",
            "kicad-cruncher-cli",
            "--bins",
        ],
        cwd=workspace,
        timeout=1800,
    )


def _python_provenance(workspace: Path) -> JsonObject:
    spec = importlib.util.find_spec("kicad_cruncher")
    if spec is None or spec.origin is None:
        raise AssertionError("kicad_cruncher is not importable in the probe environment")
    module = Path(spec.origin).resolve()
    expected_root = (workspace / "packages/kicad_cruncher/src/py").resolve()
    if not module.is_relative_to(expected_root):
        raise AssertionError(f"Python benchmark resolved outside the workspace package: {module}")
    return {
        "module": str(module),
        "module_sha256": _sha256_file(module),
        "version": importlib.metadata.version("kicad-cruncher"),
        "executable": sys.executable,
    }


def _cargo_package_version(workspace: Path, package_name: str) -> str:
    metadata = json.loads(
        _run_checked(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"],
            cwd=workspace,
            timeout=120,
        ).stdout
    )
    return next(
        str(package["version"])
        for package in metadata["packages"]
        if package["name"] == package_name
    )


def _manifest_without_source_snapshot(manifest: JsonObject) -> JsonObject:
    normalized = copy.deepcopy(manifest)
    design_facts = normalized.get("design_facts")
    if isinstance(design_facts, dict):
        design_facts.pop("source_snapshot_sha256", None)
    return normalized


def _bundle_signature(output: Path) -> JsonObject:
    manifest_path = output / "design_review_manifest.json"
    if not manifest_path.is_file():
        raise AssertionError(f"bundle omitted {manifest_path}")
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("schema") != "kicad_cruncher.design_review_manifest.a0":
        raise AssertionError("bundle emitted an unexpected manifest schema")
    design_path = output / str(manifest["design_json"])
    graph_path = output / str(manifest["compiled_schematic_graph"]["file"])
    netlist_json_path = output / str(manifest["netlist_json"])
    netlist_path = output / str(manifest["netlist_kicad_sexpr"])
    readme_path = output / str(manifest["readme"])
    design_payload = json.loads(design_path.read_text(encoding="utf-8"))
    netlist_payload = json.loads(netlist_json_path.read_text(encoding="utf-8"))
    files = sorted(
        path.relative_to(output).as_posix() for path in output.rglob("*") if path.is_file()
    )
    normalized_manifest = _manifest_without_source_snapshot(manifest)
    normalized_manifest_bytes = json.dumps(
        normalized_manifest,
        ensure_ascii=False,
        sort_keys=True,
        separators=(",", ":"),
    ).encode("utf-8")
    readme_lf = readme_path.read_text(encoding="utf-8").replace("\r\n", "\n")
    return {
        "file_count": len(files),
        "files": files,
        "total_bytes": sum((output / relative).stat().st_size for relative in files),
        "manifest_without_source_snapshot_sha256": _sha256_bytes(normalized_manifest_bytes),
        "source_snapshot_sha256": manifest.get("design_facts", {}).get("source_snapshot_sha256"),
        "design_json_semantic_sha256": _canonical_json_sha256(design_path),
        "compiled_graph_semantic_sha256": _canonical_json_sha256(graph_path),
        "netlist_json_semantic_sha256": _canonical_json_sha256(netlist_json_path),
        "netlist_sexpr_sha256": _sha256_file(netlist_path),
        "readme_lf_sha256": _sha256_bytes(readme_lf.encode("utf-8")),
        "components": len(design_payload.get("components", [])),
        "nets": len(netlist_payload.get("nets", [])),
        "schematic_svgs": len(manifest.get("schematic_svgs", [])),
        "pcb_svgs": len(manifest.get("pcb_svgs", [])),
    }


def _parity_signature(signature: JsonObject) -> JsonObject:
    ignored = {"source_snapshot_sha256", "total_bytes"}
    return {key: value for key, value in signature.items() if key not in ignored}


def _local_name(tag: str) -> str:
    return tag.rsplit("}", 1)[-1]


def _svg_geometry(
    element: ET.Element,
    *,
    coordinate_scale: float = 1.0,
) -> tuple[int, tuple[float, ...]]:
    points: list[tuple[float, float]] = []
    for child in element.iter():
        tag = _local_name(child.tag)
        if tag not in _DRAWABLE:
            continue
        attrs = child.attrib
        if tag in {"polygon", "polyline"}:
            numbers = [float(value) for value in _NUMBER.findall(attrs.get("points", ""))]
            points.extend(zip(numbers[::2], numbers[1::2], strict=False))
        elif tag == "path":
            numbers = [float(value) for value in _NUMBER.findall(attrs.get("d", ""))]
            points.extend(zip(numbers[::2], numbers[1::2], strict=False))
        elif tag == "line":
            points.extend(
                [
                    (float(attrs["x1"]), float(attrs["y1"])),
                    (float(attrs["x2"]), float(attrs["y2"])),
                ]
            )
        elif tag == "rect":
            x, y = float(attrs.get("x", 0)), float(attrs.get("y", 0))
            width, height = float(attrs.get("width", 0)), float(attrs.get("height", 0))
            points.extend([(x, y), (x + width, y + height)])
        else:
            cx, cy = float(attrs.get("cx", 0)), float(attrs.get("cy", 0))
            rx = float(attrs.get("r", attrs.get("rx", 0)))
            ry = float(attrs.get("r", attrs.get("ry", 0)))
            points.extend([(cx - rx, cy - ry), (cx + rx, cy + ry)])
    if not points:
        return 0, ()
    normalized = {
        (round(x / coordinate_scale, 6), round(y / coordinate_scale, 6)) for x, y in points
    }
    xs, ys = zip(*normalized, strict=True)
    envelope = tuple(round(value, 6) for value in (min(xs), min(ys), max(xs), max(ys)))
    if not all(math.isfinite(value) for value in envelope):
        raise AssertionError("SVG geometry contains a non-finite coordinate")
    return len(normalized), envelope


def _svg_colors(element: ET.Element) -> set[str]:
    colors: set[str] = set()
    for child in element.iter():
        for name, value in child.attrib.items():
            if name not in {"fill", "stroke"} or value == "none":
                continue
            normalized = value.upper()
            if re.fullmatch(r"#[0-9A-F]{8}", normalized):
                normalized = normalized[:7]
            colors.add(normalized)
    return colors


def _assert_close_sequence(
    actual: tuple[float, ...] | list[float],
    expected: tuple[float, ...] | list[float],
    *,
    absolute: float,
    context: str,
) -> None:
    if len(actual) != len(expected) or any(
        not math.isclose(left, right, abs_tol=absolute)
        for left, right in zip(actual, expected, strict=True)
    ):
        raise AssertionError(f"{context}: {actual!r} != {expected!r}")


def _unique_svg_ids(root: ET.Element, *, context: str) -> dict[str, ET.Element]:
    identifiers = [element.attrib["id"] for element in root.iter() if element.attrib.get("id")]
    duplicates = [name for name, count in Counter(identifiers).items() if count != 1]
    if duplicates:
        raise AssertionError(f"{context} contains duplicate SVG IDs: {duplicates[:5]}")
    return {element.attrib["id"]: element for element in root.iter() if element.attrib.get("id")}


def _metadata_payload(root: ET.Element, *, context: str) -> JsonObject:
    metadata = [element for element in root.iter() if _local_name(element.tag) == "metadata"]
    if len(metadata) != 1:
        raise AssertionError(f"{context} must contain exactly one metadata payload")
    payload = json.loads(metadata[0].text or "null")
    if not isinstance(payload, dict):
        raise AssertionError(f"{context} metadata must be a JSON object")
    return payload


def _schematic_coordinate_scale(root: ET.Element) -> float:
    width_mm = float(root.attrib["width"].removesuffix("mm"))
    view_box = [float(value) for value in root.attrib["viewBox"].split()]
    if len(view_box) != 4 or width_mm <= 0:
        raise AssertionError("schematic SVG has an invalid viewport")
    return view_box[2] / width_mm


def _normalized_schematic_attrs(element: ET.Element) -> dict[str, str]:
    attrs = dict(element.attrib)
    attrs.pop("data-uuid", None)
    attrs.pop("data-object-id", None)
    attrs.setdefault("data-source-kind", "schematic")
    attrs.setdefault("data-element-key", attrs["id"])
    attrs.setdefault("data-primitive", attrs["data-ref"])
    return attrs


def _assert_schematic_svg_parity(python_path: Path, rust_path: Path) -> int:
    python_root = ET.parse(python_path).getroot()
    rust_root = ET.parse(rust_path).getroot()
    python_attrs, rust_attrs = dict(python_root.attrib), dict(rust_root.attrib)
    python_attrs.pop("viewBox")
    rust_attrs.pop("viewBox")
    if python_attrs != rust_attrs:
        raise AssertionError(f"schematic SVG root attribute drift: {python_path.name}")
    python_scale = _schematic_coordinate_scale(python_root)
    rust_scale = _schematic_coordinate_scale(rust_root)
    python_view = [float(value) / python_scale for value in python_root.attrib["viewBox"].split()]
    rust_view = [float(value) / rust_scale for value in rust_root.attrib["viewBox"].split()]
    _assert_close_sequence(
        rust_view,
        python_view,
        absolute=0.001,
        context=f"schematic SVG viewport {python_path.name}",
    )
    if _metadata_payload(python_root, context=str(python_path)) != _metadata_payload(
        rust_root, context=str(rust_path)
    ):
        raise AssertionError(f"schematic SVG metadata drift: {python_path.name}")
    python_by_id = _unique_svg_ids(python_root, context=str(python_path))
    rust_by_id = _unique_svg_ids(rust_root, context=str(rust_path))
    python_records = {
        name: element
        for name, element in python_by_id.items()
        if element.attrib.get("data-ref") and not name.endswith(":background")
    }
    rust_records = {
        name: element for name, element in rust_by_id.items() if element.attrib.get("data-ref")
    }
    if python_records.keys() != rust_records.keys():
        raise AssertionError(
            f"schematic SVG record identity drift {python_path.name}: "
            f"{python_records.keys() ^ rust_records.keys()}"
        )
    for element_id, python_element in python_records.items():
        rust_element = rust_records[element_id]
        if _normalized_schematic_attrs(python_element) != _normalized_schematic_attrs(rust_element):
            raise AssertionError(
                f"schematic SVG record attribute drift {python_path.name}: {element_id}"
            )
        if python_element.attrib.get("data-ref") == "sheet_header":
            if (
                _svg_geometry(python_element, coordinate_scale=python_scale)[0] == 0
                or _svg_geometry(rust_element, coordinate_scale=rust_scale)[0] == 0
            ):
                raise AssertionError(
                    f"schematic SVG sheet header is empty {python_path.name}: {element_id}"
                )
            continue
        python_colors = _svg_colors(python_element)
        rust_colors = _svg_colors(rust_element)
        if not python_colors <= {"#000000", "#FFFFFF"} or not rust_colors <= {
            "#000000",
            "#FFFFFF",
        }:
            raise AssertionError(
                f"schematic SVG record is not black-and-white {python_path.name}: {element_id}"
            )
        python_geometry = _svg_geometry(python_element, coordinate_scale=python_scale)
        rust_geometry = _svg_geometry(rust_element, coordinate_scale=rust_scale)
        if not math.isclose(rust_geometry[0], python_geometry[0], rel_tol=0.01, abs_tol=2):
            raise AssertionError(
                f"schematic SVG geometry-count drift {python_path.name}: {element_id}"
            )
        _assert_close_sequence(
            rust_geometry[1],
            python_geometry[1],
            absolute=0.001,
            context=f"schematic SVG geometry {python_path.name}: {element_id}",
        )
    return len(python_records)


def _assert_pcb_svg_parity(python_path: Path, rust_path: Path) -> int:
    python_root = ET.parse(python_path).getroot()
    rust_root = ET.parse(rust_path).getroot()
    python_attrs, rust_attrs = dict(python_root.attrib), dict(rust_root.attrib)
    python_view = [float(value) for value in python_attrs.pop("viewBox").split()]
    rust_view = [float(value) for value in rust_attrs.pop("viewBox").split()]
    python_width = float(python_attrs.pop("width").removesuffix("mm"))
    rust_width = float(rust_attrs.pop("width").removesuffix("mm"))
    if python_attrs != rust_attrs:
        raise AssertionError(f"PCB SVG root attribute drift: {python_path.name}")
    _assert_close_sequence(
        rust_view, python_view, absolute=0.001, context=f"PCB viewport {python_path.name}"
    )
    if not math.isclose(rust_width, python_width, abs_tol=0.001):
        raise AssertionError(f"PCB SVG width drift: {python_path.name}")
    python_metadata = _metadata_payload(python_root, context=str(python_path))
    rust_metadata = _metadata_payload(rust_root, context=str(rust_path))
    python_bbox = python_metadata["board"].pop("bbox_mm")
    rust_bbox = rust_metadata["board"].pop("bbox_mm")
    _assert_close_sequence(
        rust_bbox, python_bbox, absolute=0.001, context=f"PCB bounds {python_path.name}"
    )
    if python_metadata != rust_metadata:
        raise AssertionError(f"PCB SVG metadata drift: {python_path.name}")
    python_by_id = _unique_svg_ids(python_root, context=str(python_path))
    rust_by_id = _unique_svg_ids(rust_root, context=str(rust_path))
    if python_by_id.keys() != rust_by_id.keys():
        raise AssertionError(f"PCB SVG identity drift: {python_path.name}")
    for element_id, python_element in python_by_id.items():
        rust_element = rust_by_id[element_id]
        if python_element.attrib != rust_element.attrib:
            raise AssertionError(f"PCB SVG attribute drift {python_path.name}: {element_id}")
        if _svg_colors(python_element) != _svg_colors(rust_element):
            raise AssertionError(f"PCB SVG color drift {python_path.name}: {element_id}")
        python_geometry = _svg_geometry(python_element)
        rust_geometry = _svg_geometry(rust_element)
        if not math.isclose(rust_geometry[0], python_geometry[0], rel_tol=0.01, abs_tol=2):
            raise AssertionError(f"PCB SVG geometry-count drift {python_path.name}: {element_id}")
        _assert_close_sequence(
            rust_geometry[1],
            python_geometry[1],
            absolute=0.001,
            context=f"PCB SVG geometry {python_path.name}: {element_id}",
        )
    return len(python_by_id)


def _assert_svg_parity(python_output: Path, rust_output: Path) -> JsonObject:
    python_files = sorted(
        path.relative_to(python_output) for path in python_output.rglob("*.svg") if path.is_file()
    )
    rust_files = sorted(
        path.relative_to(rust_output) for path in rust_output.rglob("*.svg") if path.is_file()
    )
    if python_files != rust_files:
        raise AssertionError("Python/Rust SVG artifact paths differ")
    schematic_records = 0
    pcb_records = 0
    for relative in python_files:
        if relative.parts[0] == "schematics":
            schematic_records += _assert_schematic_svg_parity(
                python_output / relative, rust_output / relative
            )
        elif relative.parts[:2] == ("pcb", "copper_layers"):
            pcb_records += _assert_pcb_svg_parity(python_output / relative, rust_output / relative)
        else:
            raise AssertionError(f"unexpected SVG artifact path: {relative.as_posix()}")
    return {
        "files": len(python_files),
        "schematic_files": sum(relative.parts[0] == "schematics" for relative in python_files),
        "pcb_files": sum(
            relative.parts[:2] == ("pcb", "copper_layers") for relative in python_files
        ),
        "schematic_record_ids": schematic_records,
        "pcb_ids": pcb_records,
        "contract": "L3_011 semantic SVG identity/metadata/attributes/theme/geometry",
    }


def _performance_profile(
    stderr_lines: list[str],
    *,
    expected: bool,
    signature: JsonObject,
) -> JsonObject | None:
    profile_lines = [
        line.removeprefix(_RUST_PROFILE_PREFIX)
        for line in stderr_lines
        if line.startswith(_RUST_PROFILE_PREFIX)
    ]
    if len(profile_lines) != int(expected):
        raise AssertionError(
            f"expected {int(expected)} Rust performance profile, got {len(profile_lines)}"
        )
    if not profile_lines:
        return None
    profile = json.loads(profile_lines[0])
    if not isinstance(profile, dict):
        raise AssertionError("Rust performance profile must be a JSON object")
    expected_keys = {
        "schema",
        "total_elapsed_ns",
        "accounted_elapsed_ns",
        "unattributed_elapsed_ns",
        "artifact_count",
        "artifact_bytes",
        "stages",
        "details",
    }
    if profile.keys() != expected_keys:
        raise AssertionError("Rust performance profile fields do not match the contract")
    if profile["schema"] != _RUST_PROFILE_SCHEMA:
        raise AssertionError("Rust performance profile schema is not recognized")
    integer_fields = (
        "total_elapsed_ns",
        "accounted_elapsed_ns",
        "unattributed_elapsed_ns",
        "artifact_count",
        "artifact_bytes",
    )
    if any(not isinstance(profile[name], int) or profile[name] < 0 for name in integer_fields):
        raise AssertionError("Rust performance profile counters must be unsigned integers")
    if profile["total_elapsed_ns"] == 0 or profile["accounted_elapsed_ns"] == 0:
        raise AssertionError("Rust performance profile timing must be nonzero")
    stages = profile["stages"]
    if not isinstance(stages, list) or any(
        not isinstance(stage, dict)
        or stage.keys() != {"name", "elapsed_ns"}
        or not isinstance(stage["name"], str)
        or not isinstance(stage["elapsed_ns"], int)
        or stage["elapsed_ns"] < 0
        for stage in stages
    ):
        raise AssertionError("Rust performance profile stages are malformed")
    stage_names = [stage["name"] for stage in stages]
    if tuple(stage_names) != _RUST_PROFILE_STAGES or len(set(stage_names)) != len(stage_names):
        raise AssertionError("Rust performance profile stage inventory is incomplete")
    stage_total = sum(stage["elapsed_ns"] for stage in stages)
    if stage_total != profile["accounted_elapsed_ns"]:
        raise AssertionError("Rust performance profile accounted time does not match its stages")
    if (
        profile["accounted_elapsed_ns"] + profile["unattributed_elapsed_ns"]
        != profile["total_elapsed_ns"]
    ):
        raise AssertionError("Rust performance profile total time arithmetic is invalid")
    details = profile["details"]
    if not isinstance(details, list) or any(
        not isinstance(detail, dict)
        or detail.keys() != {"parent", "name", "elapsed_ns"}
        or not isinstance(detail["parent"], str)
        or not isinstance(detail["name"], str)
        or not isinstance(detail["elapsed_ns"], int)
        or detail["elapsed_ns"] < 0
        for detail in details
    ):
        raise AssertionError("Rust performance profile details are malformed")
    detail_inventory = [(detail["parent"], detail["name"]) for detail in details]
    if tuple(detail_inventory) != _RUST_PROFILE_DETAILS or len(set(detail_inventory)) != len(
        detail_inventory
    ):
        raise AssertionError("Rust performance profile detail inventory is incomplete")
    stage_times = {stage["name"]: stage["elapsed_ns"] for stage in stages}
    for parent in {detail["parent"] for detail in details}:
        detail_total = sum(detail["elapsed_ns"] for detail in details if detail["parent"] == parent)
        if detail_total > stage_times[parent]:
            raise AssertionError("Rust performance profile details exceed their parent stage")
    if profile["artifact_count"] != signature["file_count"]:
        raise AssertionError("Rust performance profile artifact count does not match the bundle")
    if profile["artifact_bytes"] != signature["total_bytes"]:
        raise AssertionError("Rust performance profile artifact bytes do not match the bundle")
    return profile


def _run_round(
    *,
    implementation: str,
    command: list[str],
    cwd: Path,
    env: dict[str, str],
    output: Path,
    expect_profile: bool,
) -> JsonObject:
    if output.exists():
        shutil.rmtree(output)
    started = time.perf_counter()
    completed = _run_checked(command, cwd=cwd, env=env)
    elapsed = time.perf_counter() - started
    signature = _bundle_signature(output)
    stderr_lines = completed.stderr.strip().splitlines()
    performance_profile = _performance_profile(
        stderr_lines, expected=expect_profile, signature=signature
    )
    result: JsonObject = {
        "implementation": implementation,
        "seconds": elapsed,
        "stdout_tail": completed.stdout.strip().splitlines()[-1:],
        "stderr_tail": [line for line in stderr_lines if not line.startswith(_RUST_PROFILE_PREFIX)][
            -1:
        ],
        "signature": signature,
    }
    if performance_profile is not None:
        result["performance_profile"] = performance_profile
    return result


def _summary(rounds: list[JsonObject]) -> JsonObject:
    seconds = [float(result["seconds"]) for result in rounds]
    return {
        "rounds": len(seconds),
        "min_seconds": min(seconds),
        "median_seconds": statistics.median(seconds),
        "max_seconds": max(seconds),
    }


def _assert_parity(rounds: list[JsonObject]) -> None:
    expected = _parity_signature(rounds[0]["signature"])
    for result in rounds[1:]:
        actual = _parity_signature(result["signature"])
        if actual != expected:
            raise AssertionError(
                "Python/Rust bundle signature drift:\n"
                + json.dumps({"expected": expected, "actual": actual}, indent=2)
            )


def _git_sha(workspace: Path) -> str:
    return _run_checked(["git", "rev-parse", "HEAD"], cwd=workspace, timeout=30).stdout.strip()


def _git_status(workspace: Path) -> list[str]:
    return _run_checked(
        ["git", "status", "--porcelain"], cwd=workspace, timeout=30
    ).stdout.splitlines()


def run_probe(args: argparse.Namespace) -> JsonObject:
    workspace = args.workspace.resolve()
    build_release = not args.skip_release_build
    if build_release:
        _build_release(workspace)
    native, cruncher = _release_binaries(workspace)
    archive = _reviewed_archive(workspace)
    python_provenance = _python_provenance(workspace)
    cargo_version = _cargo_package_version(workspace, "kicad-cruncher-cli")
    rust_version = _run_checked([str(cruncher), "--version"], cwd=workspace).stdout.strip()
    if python_provenance["version"] != cargo_version:
        raise AssertionError(
            "Python/Rust kicad-cruncher versions differ: "
            f"{python_provenance['version']} != {cargo_version}"
        )
    if cargo_version not in rust_version:
        raise AssertionError(
            f"Rust executable version does not contain Cargo version {cargo_version}: "
            f"{rust_version}"
        )
    with tempfile.TemporaryDirectory(prefix="kicad_speedy_dr_performance_") as temp:
        temp_root = Path(temp)
        source = temp_root / "source"
        project = _extract_speedy(archive, source)
        source_tree_before = _tree_sha256(source)
        runtime = temp_root / "runtime"
        runtime.mkdir()
        python_output = runtime / "python-review"
        rust_output = runtime / "rust-review"
        python_env = os.environ.copy()
        python_env["KICAD_MONKEY_NATIVE"] = str(native)
        rust_env = os.environ.copy()
        if args.rust_profile:
            rust_env["KICAD_CRUNCHER_PERFORMANCE_PROFILE"] = "1"
        commands = {
            "python": [
                sys.executable,
                "-m",
                "kicad_cruncher",
                "dr",
                str(project),
                "--output",
                str(python_output),
            ],
            "rust": [
                str(cruncher),
                "dr",
                str(project),
                "--output",
                str(rust_output),
            ],
        }
        environments = {"python": python_env, "rust": rust_env}
        outputs = {"python": python_output, "rust": rust_output}
        measured: list[JsonObject] = []
        for round_index in range(args.rounds):
            order = ("python", "rust") if round_index % 2 == 0 else ("rust", "python")
            for implementation in order:
                measured.append(
                    _run_round(
                        implementation=implementation,
                        command=commands[implementation],
                        cwd=runtime,
                        env=environments[implementation],
                        output=outputs[implementation],
                        expect_profile=args.rust_profile and implementation == "rust",
                    )
                )
            svg_parity = _assert_svg_parity(python_output, rust_output)
            for result in measured[-2:]:
                result["svg_parity"] = svg_parity
            shutil.rmtree(python_output)
            shutil.rmtree(rust_output)
        _assert_parity(measured)
        source_tree_after = _tree_sha256(source)
        if source_tree_after != source_tree_before:
            raise AssertionError("Python/Rust design review mutated the Speedy source tree")
    python_rounds = [result for result in measured if result["implementation"] == "python"]
    rust_rounds = [result for result in measured if result["implementation"] == "rust"]
    python_summary = _summary(python_rounds)
    rust_summary = _summary(rust_rounds)
    python_median = float(python_summary["median_seconds"])
    rust_median = float(rust_summary["median_seconds"])
    return {
        "schema": "kicad_cruncher.speedy_dr_performance_probe.a0",
        "git_sha": _git_sha(workspace),
        "git_status_porcelain": _git_status(workspace),
        "platform": platform.platform(),
        "python": sys.version,
        "release_build_performed": build_release,
        "rust_profile_enabled": args.rust_profile,
        "probe": {
            "path": str(Path(__file__).resolve()),
            "sha256": _sha256_file(Path(__file__).resolve()),
        },
        "locks": {name: _sha256_file(workspace / name) for name in ("Cargo.lock", "uv.lock")},
        "python_package": python_provenance,
        "cargo_package_version": cargo_version,
        "rust_version_output": rust_version,
        "archive": {"path": str(archive), "sha256": _sha256_file(archive)},
        "speedy_source_tree": {
            "before_sha256": source_tree_before,
            "after_sha256": source_tree_after,
            "mutated": False,
        },
        "binaries": {
            "kicad_monkey_native": {
                "path": str(native),
                "sha256": _sha256_file(native),
            },
            "kicad_cruncher": {
                "path": str(cruncher),
                "sha256": _sha256_file(cruncher),
            },
        },
        "rounds": measured,
        "summary": {
            "python": python_summary,
            "rust": rust_summary,
            "rust_speedup": python_median / rust_median,
            "ten_x_target_seconds": python_median / 10,
            "target_gap_ratio": rust_median / (python_median / 10),
        },
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--workspace", type=Path, default=WORKSPACE)
    parser.add_argument("--rounds", type=int, default=3)
    parser.add_argument(
        "--skip-release-build",
        action="store_true",
        help="reuse existing release binaries (report is non-authoritative)",
    )
    parser.add_argument(
        "--rust-profile",
        action="store_true",
        help="capture native whole-pipeline stage timings from each Rust round",
    )
    parser.add_argument("--output", type=Path)
    args = parser.parse_args(argv)
    if args.rounds < 1:
        raise SystemExit("--rounds must be >= 1")
    result = run_probe(args)
    text = json.dumps(result, indent=2) + "\n"
    if args.output is None:
        sys.stdout.write(text)
    else:
        output = args.output.resolve()
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_text(text, encoding="utf-8")
        sys.stdout.write(f"Wrote {output}\n")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
