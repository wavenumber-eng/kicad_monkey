"""Public workflow tests for plugin-side KiCad IPC apply behavior."""

from __future__ import annotations

import importlib.util
from pathlib import Path
from types import ModuleType

_PROJECT_ROOT = Path(__file__).resolve().parents[2]
_IPC_APPLY_PATH = (
    _PROJECT_ROOT
    / "src"
    / "py"
    / "kicad_cruncher"
    / "kicad_plugins"
    / "kicad-cruncher-tools"
    / "ipc_apply.py"
)


class _Layer:
    def __init__(self, ordinal: int, canonical_name: str) -> None:
        self.ordinal = ordinal
        self.canonical_name = canonical_name

    def __int__(self) -> int:
        return self.ordinal


class _Id:
    def __init__(self, value: str) -> None:
        self.value = value


class _Graphic:
    def __init__(self, identifier: str, layer: str) -> None:
        self.id = _Id(identifier)
        self.layer = layer


class _Field:
    def __init__(self, identifier: str, *, visible: bool = True) -> None:
        self.id = _Id(identifier)
        self.name = "Value"
        self.layer = "F.Fab"
        self.visible = visible


class _Definition:
    def __init__(self, items: list[object]) -> None:
        self.items = items


class _Footprint:
    def __init__(self) -> None:
        self.id = _Id("fp-1")
        self.reference = "U1"
        self.definition = _Definition([_Graphic("line-1", "F.Fab"), _Graphic("pad-1", "F.Cu")])
        self._value_field = _Field("value-1")

    @property
    def value_field(self) -> _Field:
        return self._value_field

    @value_field.setter
    def value_field(self, field: _Field) -> None:
        self._value_field = field


class _Board:
    def __init__(self, *, fail_update: bool = False) -> None:
        self.layers = [_Layer(32, "Dwgs.User")]
        self.footprints = [_Footprint()]
        self.layer_names: dict[str, str | None] = {"Dwgs.User": "User Drawings"}
        self.commits_pushed: list[tuple[object, str]] = []
        self.commits_dropped: list[object] = []
        self.updated_items: list[object] = []
        self.fail_update = fail_update

    def begin_commit(self) -> object:
        return object()

    def get_enabled_layers(self) -> list[_Layer]:
        return self.layers

    def get_footprints(self) -> list[_Footprint]:
        return self.footprints

    def set_layer_user_name(self, layer: _Layer, user_name: str | None) -> None:
        self.layer_names[layer.canonical_name] = user_name

    def update_items(self, items: list[object]) -> None:
        if self.fail_update:
            raise RuntimeError("update failed")
        self.updated_items.extend(items)

    def push_commit(self, commit: object, label: str) -> None:
        self.commits_pushed.append((commit, label))

    def drop_commit(self, commit: object) -> None:
        self.commits_dropped.append(commit)


def _load_ipc_apply() -> ModuleType:
    spec = importlib.util.spec_from_file_location(
        "kicad_cruncher_plugin_ipc_apply",
        _IPC_APPLY_PATH,
    )
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _item_id_value(item: object) -> str | None:
    value = getattr(getattr(item, "id", None), "value", None)
    return str(value) if value is not None else None


def _mutation_request() -> dict[str, object]:
    return {
        "schema": "kicad_cruncher.pcb.clean.mutation_request.v0",
        "commit_label": "KiCad Cruncher PCB clean",
        "operations": [
            {
                "op": "reset_layer_user_name",
                "layer_ordinal": 32,
                "canonical_name": "Dwgs.User",
            },
            {
                "op": "remove_footprint_item",
                "footprint_uuid": "fp-1",
                "item_uuid": "line-1",
                "item_type": "fp_line",
                "layer": "F.Fab",
            },
            {
                "op": "hide_footprint_value_field",
                "footprint_uuid": "fp-1",
                "item_uuid": "value-1",
                "field_name": "Value",
                "layer": "F.Fab",
            },
        ],
    }


def test_ipc_apply_mutates_fake_board_under_one_commit() -> None:
    """Verify plugin-side IPC apply maps operations onto board objects."""
    ipc_apply = _load_ipc_apply()
    board = _Board()

    result = ipc_apply.apply_pcb_clean_mutation_request(board, _mutation_request())

    footprint = board.footprints[0]
    item_ids = [
        item_id
        for item in footprint.definition.items
        if (item_id := _item_id_value(item)) is not None
    ]
    assert result["status"] == "applied"
    assert result["applied"] == {
        "hide_footprint_value_field": 1,
        "remove_footprint_item": 1,
        "reset_layer_user_name": 1,
    }
    assert board.layer_names["Dwgs.User"] is None
    assert item_ids == ["pad-1"]
    assert footprint.value_field.visible is False
    assert board.commits_pushed[0][1] == "KiCad Cruncher PCB clean"
    assert board.commits_dropped == []
    assert board.updated_items == [footprint]


def test_ipc_apply_drops_commit_when_no_changes() -> None:
    """Verify no-op mutation requests do not push empty commits."""
    ipc_apply = _load_ipc_apply()
    board = _Board()

    result = ipc_apply.apply_pcb_clean_mutation_request(
        board,
        {
            "schema": "kicad_cruncher.pcb.clean.mutation_request.v0",
            "operations": [{"op": "remove_footprint_item", "footprint_uuid": "missing"}],
        },
    )

    assert result["status"] == "no_changes"
    assert result["skipped"] == {"footprint_not_found": 1}
    assert board.commits_pushed == []
    assert len(board.commits_dropped) == 1


def test_ipc_apply_drops_commit_on_update_failure() -> None:
    """Verify exceptions during KiCad update drop the active commit."""
    ipc_apply = _load_ipc_apply()
    board = _Board(fail_update=True)

    try:
        ipc_apply.apply_pcb_clean_mutation_request(board, _mutation_request())
    except RuntimeError as exc:
        assert str(exc) == "update failed"
    else:
        raise AssertionError("expected update failure")

    assert board.commits_pushed == []
    assert len(board.commits_dropped) == 1
