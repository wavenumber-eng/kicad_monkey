"""Native source-bundle and hierarchy parity over compiled-graph projects."""

from __future__ import annotations

import json
import shutil
import subprocess
from decimal import Decimal, InvalidOperation, ROUND_HALF_EVEN
from pathlib import Path
from typing import Any, cast

from kicad_monkey import KiCadDesign
from kicad_monkey.kicad_bus_connectivity import build_bus_subgraphs
from kicad_monkey.kicad_netlist_compiler import (
    _resolve_instance_reference,
    _resolve_instance_unit,
    compile_sheet_subgraphs,
    name_net,
)
from kicad_monkey.kicad_netlist_design import (
    compile_design_subgraphs,
    merge_design_nets,
)
from kicad_monkey.kicad_netlist_model import KiCadDriverKind
from kicad_monkey.kicad_schematic_connectivity import (
    ConnectivityGraph,
    iter_symbol_pins,
    snap_mm_to_iu,
)
from kicad_monkey.kicad_schematic import KiCadSchematic
from kicad_monkey.kicad_schematic_occurrence import walk_schematic_occurrences
from kicad_monkey.testing.corpus import (
    get_kicad_corpus_case,
    resolve_kicad_manifest_path,
)

PACKAGE_ROOT = Path(__file__).resolve().parents[2]
REFERENCE_CASES = (
    "real_world/yoshi_mainboard",
    "real_world/taillight",
    "real_world/speedy_processing_module",
    "real_world/jumperless_v5r7",
)
COORDINATE_VECTORS = PACKAGE_ROOT / "tests" / "parity" / "schematic_coordinate_iu_vectors.json"
I64_MIN = -(2**63)
I64_MAX = 2**63 - 1


def _point(x_mm: float, y_mm: float) -> list[int]:
    return list(snap_mm_to_iu(x_mm, y_mm))


def _first_difference(left: object, right: object, path: str = "root") -> str | None:
    if type(left) is not type(right):
        return f"{path}: type {type(left).__name__} != {type(right).__name__}"
    if isinstance(left, dict) and isinstance(right, dict):
        if left.keys() != right.keys():
            return f"{path}: keys {list(left)} != {list(right)}"
        for key in left:
            difference = _first_difference(left[key], right[key], f"{path}.{key}")
            if difference is not None:
                return difference
        return None
    if isinstance(left, list) and isinstance(right, list):
        if len(left) != len(right):
            return f"{path}: length {len(left)} != {len(right)}"
        for index, (left_item, right_item) in enumerate(zip(left, right, strict=True)):
            difference = _first_difference(left_item, right_item, f"{path}[{index}]")
            if difference is not None:
                return difference
        return None
    if left != right:
        return f"{path}: {left!r} != {right!r}"
    return None


def _wire_stats(occurrences: list[dict[str, Any]]) -> list[tuple[int, int, int, int]]:
    stats = []
    for occurrence in occurrences:
        subgraphs = occurrence["subgraphs"]
        assert isinstance(subgraphs, list)
        stats.append(
            (
                len(subgraphs),
                sum(len(subgraph["coords"]) for subgraph in subgraphs),
                sum(len(subgraph["pins"]) for subgraph in subgraphs),
                sum(len(subgraph["labels"]) for subgraph in subgraphs),
            )
        )
    return stats


def _partition_difference(
    left: list[dict[str, Any]], right: list[dict[str, Any]]
) -> str:
    right_component_by_point = {
        tuple(point): component_index
        for component_index, subgraph in enumerate(right)
        for point in subgraph["coords"]
    }
    for component_index, subgraph in enumerate(left):
        right_components = {
            right_component_by_point.get(tuple(point)) for point in subgraph["coords"]
        }
        if len(right_components) > 1:
            right_indexes = sorted(index for index in right_components if index is not None)
            return (
                f"left component {component_index} joins right components "
                f"{right_indexes} at {subgraph['coords'][:12]}; "
                f"left chosen={subgraph['chosen_name']!r}, "
                f"labels={[label['text'] for label in subgraph['labels']]}, "
                f"pins={[(pin['reference'], pin['pin_number']) for pin in subgraph['pins']]}; "
                f"right summaries={[(right[index]['coords'], right[index]['chosen_name'], [label['text'] for label in right[index]['labels']], [(pin['reference'], pin['pin_number']) for pin in right[index]['pins']]) for index in right_indexes]}"
            )
    return "no merged-component witness"


def _reference_decimal_iu(value: str) -> int | None:
    try:
        decimal = Decimal(value)
    except InvalidOperation:
        return None
    if not decimal.is_finite():
        return None
    rounded = int((decimal * 10_000).to_integral_value(rounding=ROUND_HALF_EVEN))
    return rounded if I64_MIN <= rounded <= I64_MAX else None


def test_shared_coordinate_vectors_encode_exact_python_ties_even_policy() -> None:
    payload = json.loads(COORDINATE_VECTORS.read_text(encoding="utf-8"))
    assert payload["schema"] == "kicad_monkey.schematic_coordinate_iu_vectors.a0"
    for case in payload["cases"]:
        expected = case["expected_iu"]
        actual = _reference_decimal_iu(case["millimetres"])
        assert actual == (None if expected is None else int(expected)), case["name"]


def _polyline(value: object) -> dict[str, object]:
    return {
        "uuid": str(getattr(value, "uuid", "") or ""),
        "points": [_point(x, y) for x, y in getattr(value, "points", ())],
    }


def _marker(value: object) -> dict[str, object]:
    return {
        "uuid": str(getattr(value, "uuid", "") or ""),
        "at": _point(float(getattr(value, "at_x", 0.0)), float(getattr(value, "at_y", 0.0))),
    }


def _label(value: object, scope: str) -> dict[str, object]:
    shape = getattr(value, "shape", "")
    return {
        "scope": scope,
        "text": str(getattr(value, "text", "") or ""),
        "shape": str(getattr(shape, "value", shape) or ""),
        "uuid": str(getattr(value, "uuid", "") or ""),
        "at": _point(float(getattr(value, "at_x", 0.0)), float(getattr(value, "at_y", 0.0))),
    }


def _definition_summary(schematic: KiCadSchematic, bundle_root: Path) -> dict[str, object]:
    graph = ConnectivityGraph()
    for wire in getattr(schematic, "wires", ()):
        graph.add_wire(wire)
    for bus in getattr(schematic, "buses", ()):
        graph.add_bus(bus)
    for entry in getattr(schematic, "bus_entries", ()):
        graph.add_bus_entry(entry)
    graph.add_junctions(getattr(schematic, "junctions", ()))
    components = sorted(
        [sorted([list(point) for point in component]) for component in graph.components()]
    )
    source_path = Path(str(getattr(schematic, "source_path"))).resolve()
    bus_subgraphs = []
    for subgraph in build_bus_subgraphs(schematic):
        bus_subgraphs.append(
            {
                "coords": [list(point) for point in sorted(subgraph.coords)],
                "drivers": [
                    {
                        "text": driver.text,
                        "at": list(driver.coord),
                        "priority": int(driver.priority),
                        "kind": str(driver.kind),
                    }
                    for driver in subgraph.drivers
                ],
                "tap_wire_coords": [list(point) for point in subgraph.tap_wire_coords],
                "chosen_name": subgraph.chosen_name,
                "chosen_priority": int(subgraph.chosen_priority),
                "chosen_kind": str(subgraph.chosen_kind),
                "members": list(subgraph.members),
            }
        )
    bus_subgraphs.sort(key=lambda value: (value["chosen_name"], value["coords"]))
    return {
        "source_path": source_path.relative_to(bundle_root).as_posix(),
        "sheets": [
            {
                "uuid": str(getattr(sheet, "uuid", "") or ""),
                "pins": [
                    {
                        "name": str(getattr(pin, "name", "") or ""),
                        "shape": str(
                            getattr(getattr(pin, "shape", ""), "value", getattr(pin, "shape", ""))
                            or ""
                        ),
                        "uuid": str(getattr(pin, "uuid", "") or ""),
                        "at": _point(pin.at_x, pin.at_y),
                    }
                    for pin in getattr(sheet, "pins", ())
                ],
            }
            for sheet in getattr(schematic, "sheets", ())
        ],
        "symbols": [
            {
                "lib_id": symbol.lib_id,
                "lib_name": symbol.lib_name,
                "at": _point(symbol.at_x, symbol.at_y),
                "angle_degrees": symbol.at_angle,
                "mirror": symbol.mirror,
                "unit": symbol.unit,
                "convert": symbol.convert,
                "policy": [
                    symbol.exclude_from_sim,
                    symbol.in_bom,
                    symbol.on_board,
                    symbol.in_pos_files,
                    symbol.dnp,
                    symbol.fields_autoplaced,
                ],
                "uuid": symbol.uuid,
                "properties": [[prop.key, prop.value] for prop in symbol.properties],
                "pins": [
                    {"number": pin.number, "uuid": pin.uuid, "alternate": pin.alternate}
                    for pin in symbol.pins
                ],
                "instances": [
                    {
                        "project": instance.project,
                        "path": instance.path,
                        "reference": instance.reference,
                        "unit": instance.unit,
                        "variants": [
                            {
                                "name": variant.name,
                                "policy": [
                                    variant.dnp,
                                    variant.exclude_from_sim,
                                    variant.in_bom,
                                    variant.on_board,
                                    variant.in_pos_files,
                                ],
                                "fields": [list(field) for field in variant.fields],
                            }
                            for variant in instance.variants
                        ],
                    }
                    for instance in symbol.instances
                ],
            }
            for symbol in getattr(schematic, "symbols", ())
        ],
        "legacy_symbol_instances": [
            {
                "path": instance.path,
                "reference": instance.reference,
                "unit": instance.unit,
                "value": instance.value,
                "footprint": instance.footprint,
            }
            for instance in getattr(schematic, "symbol_instances", ())
        ],
        "wires": [_polyline(value) for value in getattr(schematic, "wires", ())],
        "buses": [_polyline(value) for value in getattr(schematic, "buses", ())],
        "bus_entries": [
            {
                "uuid": str(getattr(value, "uuid", "") or ""),
                "at": _point(value.at_x, value.at_y),
                "size": _point(value.size_x, value.size_y),
            }
            for value in getattr(schematic, "bus_entries", ())
        ],
        "bus_aliases": [
            {"name": alias.name, "members": list(alias.members)}
            for alias in getattr(schematic, "bus_aliases", ())
        ],
        "junctions": [_marker(value) for value in getattr(schematic, "junctions", ())],
        "no_connects": [_marker(value) for value in getattr(schematic, "no_connects", ())],
        "labels": [
            *[_label(value, "local") for value in getattr(schematic, "labels", ())],
            *[_label(value, "global") for value in getattr(schematic, "global_labels", ())],
            *[
                _label(value, "hierarchical")
                for value in getattr(schematic, "hierarchical_labels", ())
            ],
        ],
        "connectivity_components": components,
        "bus_subgraphs": bus_subgraphs,
    }


def _request(
    case_id: str,
) -> tuple[
    dict[str, object],
    set[str],
    list[dict[str, object]],
    dict[str, dict[str, object]],
    list[dict[str, object]],
    list[dict[str, object]],
    list[dict[str, object]],
    list[dict[str, object]],
    dict[str, object],
]:
    case = get_kicad_corpus_case(case_id)
    assert case is not None
    project_path = resolve_kicad_manifest_path(case, "project_file")
    assert project_path is not None
    design = KiCadDesign.from_project_file(project_path)
    top = design.top_schematic
    assert top is not None and top.source_path is not None
    occurrences = list(walk_schematic_occurrences(top))
    schematic_paths = sorted(
        {
            str(Path(occurrence.schematic.source_path).resolve())
            for occurrence in occurrences
            if occurrence.schematic.source_path is not None
        }
    )
    bundle_root = project_path.parent.resolve()
    request = {
        "bundle_root": str(bundle_root),
        "project_path": str(project_path.resolve()),
        "root_schematic_path": str(Path(top.source_path).resolve()),
        "schematic_paths": schematic_paths,
    }
    expected_occurrences = []
    for occurrence in occurrences:
        occurrence_source = occurrence.schematic.source_path
        assert occurrence_source is not None
        expected_occurrences.append(
            {
                "source_path": Path(occurrence_source)
                .resolve()
                .relative_to(bundle_root)
                .as_posix(),
                "parent_index": occurrence.parent.index
                if occurrence.parent
                else None,
                "parent_sheet_index": (
                    next(
                        index
                        for index, sheet in enumerate(occurrence.parent.schematic.sheets)
                        if sheet is occurrence.sheet_symbol
                    )
                    if occurrence.parent is not None
                    else None
                ),
                "occurrence_address": occurrence.occurrence_address,
                "legacy_address": occurrence.sheet_path_uuids,
                "human_address": occurrence.sheet_path,
                "effective_in_bom": occurrence.effective_in_bom,
                "effective_on_board": occurrence.effective_on_board,
                "effective_dnp": occurrence.effective_dnp,
                "effective_exclude_from_sim": occurrence.effective_exclude_from_sim,
            }
        )
    expected_definitions = {
        Path(path).relative_to(bundle_root).as_posix() for path in map(Path, schematic_paths)
    }
    schematic_by_path = {
        str(Path(occurrence.schematic.source_path).resolve()): occurrence.schematic
        for occurrence in occurrences
        if occurrence.schematic.source_path is not None
    }
    expected_source_models: dict[str, dict[str, object]] = {}
    for schematic in schematic_by_path.values():
        summary = _definition_summary(schematic, bundle_root)
        source_key = summary["source_path"]
        assert isinstance(source_key, str)
        expected_source_models[source_key] = summary
    legacy_references: dict[str, str] = {}
    legacy_units: dict[str, int] = {}
    for schematic in schematic_by_path.values():
        for instance in getattr(schematic, "symbol_instances", ()):
            path = str(instance.path or "").rstrip("/")
            if path:
                legacy_references.setdefault(path, str(instance.reference or ""))
                legacy_units.setdefault(path, int(instance.unit or 1))
    expected_effective = []
    expected_terminals = []
    expected_wire_subgraphs = []
    expected_local_nets = []
    for occurrence in occurrences:
        symbols = []
        terminals = []
        for symbol_index, symbol in enumerate(getattr(occurrence.schematic, "symbols", ())):
            fields = {
                str(getattr(prop, "key", "")): str(getattr(prop, "value", ""))
                for prop in getattr(symbol, "properties", ())
            }
            reference = _resolve_instance_reference(
                symbol,
                occurrence.sheet_path_uuids,
                legacy_references,
                occurrence.occurrence_address,
            )
            unit = _resolve_instance_unit(
                symbol,
                occurrence.sheet_path_uuids,
                legacy_units,
                occurrence.occurrence_address,
            )
            symbols.append(
                {
                    "symbol_index": symbol_index,
                    "uuid": symbol.uuid,
                    "lib_id": symbol.lib_id,
                    "reference": reference,
                    "value": fields.get("Value", ""),
                    "unit": unit,
                    "convert": symbol.convert,
                    "policy": [
                        occurrence.effective_dnp or symbol.dnp,
                        occurrence.effective_exclude_from_sim or symbol.exclude_from_sim,
                        occurrence.effective_in_bom and symbol.in_bom,
                        occurrence.effective_on_board and symbol.on_board,
                        symbol.in_pos_files,
                    ],
                    "fields": fields,
                }
            )
            library_symbol = occurrence.schematic.get_lib_symbol_for_symbol(symbol)
            if library_symbol is not None:
                for number, world_x, world_y, pin in iter_symbol_pins(
                    symbol, library_symbol, unit_override=unit
                ):
                    terminals.append(
                        {
                            "symbol_index": symbol_index,
                            "symbol_uuid": symbol.uuid,
                            "reference": reference,
                            "pin_number": number,
                            "pin_name": pin.name,
                            "electrical_type": pin.electrical_type.value,
                            "graphic_style": pin.graphic_style.value,
                            "hidden": pin.hide,
                            "library_at": _point(pin.at_x, pin.at_y),
                            "at": _point(world_x, world_y),
                        }
                    )
        expected_effective.append(
            {"occurrence_index": occurrence.index, "symbols": symbols}
        )
        expected_terminals.append(
            {"occurrence_index": occurrence.index, "terminals": terminals}
        )
        subgraphs = compile_sheet_subgraphs(
            occurrence.schematic,
            sheet_path=occurrence.sheet_path_uuids,
            legacy_lookup=legacy_references,
            canonical_path=occurrence.occurrence_address,
            legacy_unit_lookup=legacy_units,
        )
        expected_wire_subgraphs.append(
            {
                "occurrence_index": occurrence.index,
                "subgraphs": [
                    {
                        "coords": [list(point) for point in sorted(subgraph.coords)],
                        "pins": [
                            {
                                "symbol_index": next(
                                    index
                                    for index, symbol in enumerate(occurrence.schematic.symbols)
                                    if str(getattr(symbol, "uuid", "") or "")
                                    == pin.svg_uuid
                                ),
                                "reference": pin.designator,
                                "pin_number": pin.pin_number,
                                "pin_name": pin.pin_name,
                                "electrical_type": pin.pin_type,
                                "at": list(pin.coord),
                                "priority": int(pin.priority),
                                "kind": (
                                    "global_power_pin"
                                    if int(pin.priority) == 6
                                    else "local_power_pin"
                                    if int(pin.priority) == 5
                                    else "pin"
                                ),
                                "power_value": pin.power_value,
                                "has_multiple": pin.has_multiple,
                                "designator_with_unit": pin.designator_with_unit,
                                "parent_pin_count": pin.parent_pin_count,
                                "is_power": pin.is_power,
                                "is_implicit_hidden_power": pin.is_implicit_hidden_power,
                                "source_pin_uuid": pin.source_uuid,
                                "pin_svg_id": pin.pin_svg_uuid,
                            }
                            for pin in subgraph.pin_drivers
                        ],
                        "labels": [
                            {
                                "text": label.text,
                                "at": list(label.coord),
                                "priority": int(label.priority),
                                "kind": str(label.kind),
                                "shape": label.shape,
                                "source_uuid": label.source_uuid,
                            }
                            for label in subgraph.label_drivers
                        ],
                        "chosen_name": subgraph.chosen_name,
                        "chosen_priority": int(subgraph.chosen_priority),
                        "chosen_kind": str(subgraph.chosen_kind),
                        "no_connect": subgraph.no_connect,
                    }
                    for subgraph in subgraphs
                ],
            }
        )
        nets: list[dict[str, object]] = []
        sheet_pin_names: dict[str, int] = {}
        symbol_index_by_uuid = {
            str(getattr(symbol, "uuid", "") or ""): symbol_index
            for symbol_index, symbol in enumerate(occurrence.schematic.symbols)
        }
        code = 1
        for subgraph in subgraphs:
            if not subgraph.pin_drivers and not subgraph.label_drivers:
                continue
            net_name, auto_named = name_net(
                subgraph, sheet_path=occurrence.sheet_path_uuids
            )
            if str(subgraph.chosen_kind) == "sheet_pin":
                duplicate = sheet_pin_names.get(net_name, 0)
                sheet_pin_names[net_name] = duplicate + 1
                if duplicate:
                    net_name = f"{net_name}_{duplicate}"
            nets.append(
                {
                    "name": net_name,
                    "code": code,
                    "driver_priority": int(subgraph.chosen_priority),
                    "driver_kind": str(subgraph.chosen_kind),
                    "auto_named": auto_named,
                    "terminals": [
                        {
                            "symbol_index": symbol_index_by_uuid[pin.svg_uuid],
                            "designator": pin.designator,
                            "pin": pin.pin_number,
                            "pin_name": pin.pin_name,
                            "pin_type": pin.pin_type,
                            "sheet_path": occurrence.sheet_path_uuids,
                            "source_pin_id": pin.source_uuid,
                            "svg_id": pin.pin_svg_uuid or pin.svg_uuid,
                        }
                        for pin in sorted(
                            subgraph.pin_drivers,
                            key=lambda value: (value.designator, value.pin_number),
                        )
                        if pin.designator
                    ],
                }
            )
            code += 1
        expected_local_nets.append(
            {"occurrence_index": occurrence.index, "nets": nets}
        )
    expected_scalar_design = _scalar_design_summary(top)
    return (
        request,
        expected_definitions,
        expected_occurrences,
        expected_source_models,
        expected_effective,
        expected_terminals,
        expected_wire_subgraphs,
        expected_local_nets,
        expected_scalar_design,
    )


def _scalar_design_summary(top: KiCadSchematic) -> dict[str, object]:
    compiled = compile_design_subgraphs(top)
    compiled_index_by_identity = {
        id(compiled_sheet): index for index, compiled_sheet in enumerate(compiled)
    }
    bindings: list[dict[str, object]] = []
    for child_index, child in enumerate(compiled):
        parent = child.parent
        sheet = child.parent_sheet
        if parent is None or sheet is None:
            continue
        parent_index = compiled_index_by_identity[id(parent)]
        hierarchical_by_name: dict[str, tuple[int, str]] = {}
        for subgraph_index, subgraph in enumerate(child.subgraphs):
            for label in subgraph.label_drivers:
                if label.kind == KiCadDriverKind.HIER_LABEL:
                    hierarchical_by_name.setdefault(
                        label.text, (subgraph_index, label.source_uuid)
                    )
        for pin in sheet.pins:
            parent_subgraph = parent.coord_to_sg.get(
                snap_mm_to_iu(pin.at_x, pin.at_y)
            )
            child_match = hierarchical_by_name.get(pin.name)
            child_subgraph = child_match[0] if child_match is not None else None
            bindings.append(
                {
                    "parent_occurrence_index": parent_index + 1,
                    "child_occurrence_index": child_index + 1,
                    "sheet_pin_name": pin.name,
                    "sheet_pin_uuid": pin.uuid,
                    "hierarchical_label_uuid": (
                        child_match[1] if child_match is not None else None
                    ),
                    "parent_subgraph_index": parent_subgraph,
                    "child_subgraph_index": child_subgraph,
                    "resolved": parent_subgraph is not None
                    and child_subgraph is not None,
                }
            )
    for compiled_sheet in compiled:
        compiled_sheet.bus_subgraphs = []
        compiled_sheet.bus_member_wire_sg = []
        compiled_sheet.bus_aliases_design = {}
    nets = merge_design_nets(compiled)
    members_by_code: dict[int, list[list[int]]] = {}
    for occurrence_index, compiled_sheet in enumerate(compiled, start=1):
        for subgraph_index, code in compiled_sheet.subgraph_net_codes.items():
            members_by_code.setdefault(code, []).append(
                [occurrence_index, subgraph_index]
            )
    return {
        "nets": [
            {
                "name": net.name,
                "code": net.code,
                "driver_priority": int(net.driver_priority),
                "driver_kind": str(net.driver_kind),
                "auto_named": net.auto_named,
                "members": members_by_code.get(net.code, []),
                "terminals": [
                    {
                        "designator": terminal.designator,
                        "pin": terminal.pin,
                        "pin_name": terminal.pin_name,
                        "pin_type": terminal.pin_type,
                        "sheet_path": terminal.sheet_path,
                        "source_pin_id": terminal.source_pin_id,
                        "svg_id": terminal.svg_id,
                    }
                    for terminal in net.terminals
                ],
            }
            for net in nets
        ],
        "hierarchy_bindings": bindings,
    }


def test_native_source_bundle_matches_python_hierarchy_inventory() -> None:
    cargo = shutil.which("cargo")
    assert cargo is not None, "cargo is required for native source-bundle validation"
    requests_and_counts = [_request(case_id) for case_id in REFERENCE_CASES]
    completed = subprocess.run(
        [
            cargo,
            "run",
            "--locked",
            "--quiet",
            "--package",
            "kicad-monkey-core",
            "--example",
            "schematic_bundle_gate",
        ],
        cwd=PACKAGE_ROOT,
        input="".join(
            f"{json.dumps(request, separators=(',', ':'))}\n"
            for request, _definitions, _occurrences, _source_models, _effective, _terminals, _wire_subgraphs, _local_nets, _scalar_design in requests_and_counts
        ),
        capture_output=True,
        text=True,
        encoding="utf-8",
        timeout=300,
        check=False,
    )
    assert completed.returncode == 0, completed.stderr
    results = [json.loads(line) for line in completed.stdout.splitlines()]
    assert len(results) == len(REFERENCE_CASES)
    for result, (
        _request_payload,
        definitions,
        occurrences,
        source_models,
        effective,
        terminals,
        wire_subgraphs,
        local_nets,
        scalar_design,
    ) in zip(
        results, requests_and_counts, strict=True
    ):
        assert set(result["definition_paths"]) == definitions
        assert result["occurrences"] == occurrences
        assert result["effective_symbols"] == effective
        assert result["symbol_terminals"] == terminals
        difference = _first_difference(result["wire_subgraphs"], wire_subgraphs)
        assert difference is None, (
            f"{difference}; Rust stats={_wire_stats(result['wire_subgraphs'])}; "
            f"Python stats={_wire_stats(wire_subgraphs)}; "
            f"partition={_partition_difference(result['wire_subgraphs'][0]['subgraphs'], cast(list[dict[str, Any]], wire_subgraphs[0]['subgraphs']))}"
        )
        assert result["local_nets"] == local_nets
        assert result["scalar_design"] == scalar_design, _first_difference(
            result["scalar_design"], scalar_design
        )
        assert {
            definition["source_path"]: definition for definition in result["definitions"]
        } == source_models
        assert result["total_bytes"] > 0
