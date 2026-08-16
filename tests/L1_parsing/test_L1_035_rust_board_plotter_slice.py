"""Rack ownership for the first Rust board-to-plotter-IR slice."""

from __future__ import annotations

import contextlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

from jsonschema import Draft202012Validator
import msgspec
import pytest

from kicad_monkey.contracts.generated import (
    decode_board_plot_document_a0,
    decode_board_plot_request_a0,
)
from kicad_monkey.kicad_pcb import KiCadPcb
from kicad_monkey.kicad_pcb_to_ir import pcb_to_ir
from kicad_monkey.kicad_project import KiCadProject


PACKAGE_ROOT = Path(__file__).resolve().parents[2]
VECTOR_PATH = PACKAGE_ROOT / "tests" / "parity" / "board_plotter_a0_vectors.json"
SLICE_SCHEMA_PATH = (
    PACKAGE_ROOT / "contracts" / "generated" / "schema" / "BoardPlotDocument.json"
)
ESTABLISHED_SCHEMA_PATH = (
    PACKAGE_ROOT / "docs" / "contracts" / "kicad_plotter_ir_a0.schema.json"
)


def _run(command: list[str]) -> None:
    completed = subprocess.run(
        command,
        cwd=PACKAGE_ROOT,
        capture_output=True,
        text=True,
        timeout=240,
        check=False,
    )
    assert completed.returncode == 0, (
        f"Command failed: {' '.join(command)}\n"
        f"stdout:\n{completed.stdout}\n"
        f"stderr:\n{completed.stderr}"
    )


@contextlib.contextmanager
def _without_shapely():
    """Pin the governed no-Shapely synthetic-knockout baseline.

    The Rust producer ports the established serializer's behavior when
    Shapely is unavailable: text-box knockout leaves the text operation
    unchanged. Blocking the import keeps the oracle on that baseline even
    in environments where Shapely is installed.
    """

    saved = {
        name: sys.modules.pop(name)
        for name in list(sys.modules)
        if name == "shapely" or name.startswith("shapely.")
    }
    sys.modules["shapely"] = None
    try:
        yield
    finally:
        del sys.modules["shapely"]
        sys.modules.update(saved)


def test_shared_board_vectors_match_python_generated_types_and_both_schemas() -> None:
    payload = json.loads(VECTOR_PATH.read_text(encoding="utf-8"))
    assert payload["schema"] == "kicad_monkey.board_plotter_parity.a0"

    slice_schema = json.loads(SLICE_SCHEMA_PATH.read_text(encoding="utf-8"))
    established_schema = json.loads(ESTABLISHED_SCHEMA_PATH.read_text(encoding="utf-8"))
    Draft202012Validator.check_schema(slice_schema)
    Draft202012Validator.check_schema(established_schema)
    safe_integer = slice_schema["$defs"]["JavaScriptSafeInteger"]
    assert safe_integer["minimum"] == -9_007_199_254_740_991
    assert safe_integer["maximum"] == 9_007_199_254_740_991

    for vector in payload["vectors"]:
        pcb = KiCadPcb.from_string(vector["source"])
        project_raw: dict = {}
        assignments = vector.get("net_class_assignments")
        if assignments is not None:
            project_raw["net_settings"] = {"netclass_assignments": assignments}
        variables = vector.get("text_variables")
        if variables is not None:
            project_raw["text_variables"] = variables
        if project_raw:
            pcb.project = KiCadProject.from_json_dict(project_raw)
        with _without_shapely():
            actual = pcb_to_ir(
                pcb,
                source_path=vector["source_path"],
                document_id=vector["document_id"],
            ).to_dict()
        assert actual == vector["expected"], vector["id"]
        Draft202012Validator(slice_schema).validate(actual)
        Draft202012Validator(established_schema).validate(actual)

        # The established Python model retains integral coordinates as floats
        # internally; the canonical interchange vector normalizes them to the
        # TypeSpec-owned integer representation used by Rust and WASM.
        encoded = json.dumps(vector["expected"]).encode("utf-8")
        generated = decode_board_plot_document_a0(encoded)
        assert json.loads(msgspec.json.encode(generated)) == vector["expected"]

        unsafe = json.loads(json.dumps(actual))
        unsafe["version"] = 9_007_199_254_740_992
        assert list(Draft202012Validator(slice_schema).iter_errors(unsafe))

    mixed = next(
        vector["expected"]
        for vector in payload["vectors"]
        if vector["id"] == "board-metadata-and-category-ordered-graphics"
    )

    unknown_record_kind = json.loads(json.dumps(mixed))
    unknown_record_kind["records"][0]["kind"] = "gr_text"
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(unknown_record_kind).encode("utf-8"))

    missing_layer = json.loads(json.dumps(mixed))
    del missing_layer["records"][0]["layer"]
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(missing_layer).encode("utf-8"))

    unknown_operation = json.loads(json.dumps(mixed))
    unknown_operation["records"][0]["operations"][0]["kind"] = "Unknown"
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(unknown_operation).encode("utf-8"))

    malformed_point = json.loads(json.dumps(mixed))
    malformed_point["records"][4]["operations"][0]["points"][0] = [0]
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(malformed_point).encode("utf-8"))

    tracks = next(
        vector["expected"]
        for vector in payload["vectors"]
        if vector["id"] == "tracks-follow-graphics-with-net-extras"
    )

    # Vector record layout: gr_line, three segments, two arcs, one via.
    missing_lock = json.loads(json.dumps(tracks))
    del missing_lock["records"][1]["locked"]
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(missing_lock).encode("utf-8"))

    non_boolean_lock = json.loads(json.dumps(tracks))
    non_boolean_lock["records"][1]["locked"] = "yes"
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(non_boolean_lock).encode("utf-8"))

    locked_arc = json.loads(json.dumps(tracks))
    locked_arc["records"][4]["locked"] = True
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(locked_arc).encode("utf-8"))

    unknown_via_role = json.loads(json.dumps(tracks))
    unknown_via_role["records"][6]["operations"][0]["role"] = "Unknown"
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(unknown_via_role).encode("utf-8"))

    unknown_via_type = json.loads(json.dumps(tracks))
    unknown_via_type["records"][6]["via_type"] = "half"
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(unknown_via_type).encode("utf-8"))

    missing_via_type = json.loads(json.dumps(tracks))
    del missing_via_type["records"][6]["via_type"]
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(missing_via_type).encode("utf-8"))

    zones = next(
        vector["expected"]
        for vector in payload["vectors"]
        if vector["id"] == "zones-bundle-fill-rings-and-annotations"
    )

    # Vector record layout: zone-single (two rings), zone-multi,
    # zone-keepout (no rings), zone-empty-name.
    missing_fill_layers = json.loads(json.dumps(zones))
    del missing_fill_layers["records"][0]["fill_layers"]
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(missing_fill_layers).encode("utf-8"))

    non_boolean_island = json.loads(json.dumps(zones))
    non_boolean_island["records"][0]["fill_island"][0] = "no"
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(non_boolean_island).encode("utf-8"))

    unknown_zone_kind = json.loads(json.dumps(zones))
    unknown_zone_kind["records"][0]["kind"] = "zone_outline"
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(unknown_zone_kind).encode("utf-8"))

    net_class_extras = next(
        vector["expected"]
        for vector in payload["vectors"]
        if vector["id"] == "net-class-extras-follow-project-assignments"
    )

    non_array_classes = json.loads(json.dumps(net_class_extras))
    non_array_classes["records"][1]["net_classes"] = "Power"
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(non_array_classes).encode("utf-8"))

    texts = next(
        vector["expected"]
        for vector in payload["vectors"]
        if vector["id"] == "board-text-follows-python-serializer"
    )

    # Vector record layout: plain, styled, empty, vars, cache, stale-cache,
    # knockout, face-cache.
    missing_hide = json.loads(json.dumps(texts))
    del missing_hide["records"][0]["hide"]
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(missing_hide).encode("utf-8"))

    unknown_alignment = json.loads(json.dumps(texts))
    unknown_alignment["records"][0]["operations"][0]["h_align"] = "GR_TEXT_H_ALIGN_JUSTIFY"
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(unknown_alignment).encode("utf-8"))

    missing_cache_schema = json.loads(json.dumps(texts))
    del missing_cache_schema["records"][4]["operations"][0]["render_cache"]["schema"]
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(missing_cache_schema).encode("utf-8"))

    malformed_cache_point = json.loads(json.dumps(texts))
    cache = malformed_cache_point["records"][4]["operations"][0]["render_cache"]
    cache["polygons"][0]["contours"][0][0] = [0]
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(malformed_cache_point).encode("utf-8"))

    text_boxes = next(
        vector["expected"]
        for vector in payload["vectors"]
        if vector["id"] == "text-boxes-bundle-border-and-alignment"
    )

    missing_border = json.loads(json.dumps(text_boxes))
    del missing_border["records"][0]["border"]
    with pytest.raises(msgspec.ValidationError):
        decode_board_plot_document_a0(json.dumps(missing_border).encode("utf-8"))


def test_rust_core_and_host_adapter_consume_the_shared_board_vector() -> None:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for the Rust plotter-IR gate"
    _run(
        [
            cargo,
            "test",
            "--locked",
            "--package",
            "kicad-monkey-core",
            "--test",
            "board_plotter_slice",
        ]
    )
    _run([cargo, "test", "--locked", "--package", "kicad-monkey-wasm"])


def test_board_plot_request_requires_explicit_graphic_and_point_budgets() -> None:
    request = {
        "type": "kicad_monkey.board_plot.request",
        "version": "a0",
        "max_source_bytes": "65536",
        "max_output_bytes": "65536",
        "max_depth": 32,
        "max_graphics": 1000,
        "max_operations": 1000,
        "max_points": 10000,
    }
    decoded = decode_board_plot_request_a0(json.dumps(request).encode())
    assert decoded.max_graphics == 1000
    assert decoded.max_points == 10000
    for budget in ("max_graphics", "max_points"):
        trimmed = dict(request)
        del trimmed[budget]
        with pytest.raises(msgspec.ValidationError):
            decode_board_plot_request_a0(json.dumps(trimmed).encode())
