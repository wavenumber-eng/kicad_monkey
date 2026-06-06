"""Public workflow tests for daemon/plugin PCB cleanup routing."""

from __future__ import annotations

import importlib.util
import json
import shutil
from pathlib import Path
from types import ModuleType
from typing import Protocol, cast

from kicad_cruncher.kicad_cruncher_cmd_daemon import run_daemon
from kicad_cruncher.kicad_cruncher_daemon import (
    create_app,
    daemon_command_inventory_payload,
    daemon_pcb_layer_cleanup,
)
from kicad_cruncher.kicad_cruncher_daemon_state import DAEMON_STATE_SCHEMA, write_daemon_state
from kicad_cruncher.kicad_cruncher_plugin_installer import (
    DEFAULT_PLUGIN_NAME,
    _copy_filter,
    install_plugin,
    plugin_identifier,
    plugin_package_root,
)
from kicad_monkey import KiCadPcb

_PROJECT_ROOT = Path(__file__).resolve().parents[2]
_CORPUS_ROOT = _PROJECT_ROOT / "tests" / "corpus" / "kicad"
_HLR_TEST_PCB = _CORPUS_ROOT / "projects" / "hlr_test" / "hlr_test.kicad_pcb"
_PLUGIN_MAIN_PATH = (
    _PROJECT_ROOT
    / "src"
    / "py"
    / "kicad_cruncher"
    / "kicad_plugins"
    / "kicad-cruncher-tools"
    / "main.py"
)


class _PluginMainModule(Protocol):
    def main(self) -> int: ...

    def _resolve_daemon_url(self) -> str: ...


class _UrllibModule(Protocol):
    request: object


class _WebbrowserModule(Protocol):
    def open(self, url: str) -> bool: ...


class _PluginRuntimeModule(_PluginMainModule, Protocol):
    urllib: _UrllibModule
    webbrowser: _WebbrowserModule


def _json_object(value: object) -> dict[str, object]:
    assert isinstance(value, dict)
    return value


def _json_list(value: object) -> list[object]:
    assert isinstance(value, list)
    return value


def _load_plugin_main_module() -> ModuleType:
    spec = importlib.util.spec_from_file_location("kicad_cruncher_plugin_main", _PLUGIN_MAIN_PATH)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return cast(ModuleType, module)


def _load_plugin_main() -> _PluginMainModule:
    return cast(_PluginMainModule, _load_plugin_main_module())


class _RecordingServerRunner:
    def __init__(self) -> None:
        self.calls: list[dict[str, object]] = []

    def __call__(
        self,
        app: str,
        *,
        factory: bool,
        host: str,
        port: int,
        reload: bool,
    ) -> None:
        self.calls.append(
            {
                "app": app,
                "factory": factory,
                "host": host,
                "port": port,
                "reload": reload,
            }
        )


class _FakeHttpResponse:
    def __init__(self, *, status: int = 200, body: str = "{}") -> None:
        self.status = status
        self._body = body.encode("utf-8")

    def __enter__(self) -> _FakeHttpResponse:
        return self

    def __exit__(self, _exc_type: object, _exc: object, _tb: object) -> None:
        return None

    def read(self) -> bytes:
        return self._body


def test_daemon_command_inventory_exposes_pcb_clean() -> None:
    """Verify the daemon advertises the shared PCB clean command."""
    app = create_app()
    paths = {str(getattr(route, "path", "")) for route in getattr(app, "routes", [])}
    payload = daemon_command_inventory_payload()
    commands = _json_list(payload["commands"])
    pcb_clean = next(
        _json_object(item)
        for item in commands
        if _json_object(item)["id"] == "pcb.clean"
    )

    assert "/api/v1/commands" in paths
    assert "/api/v1/pcb/layer-cleanup" in paths
    assert payload["schema"] == "kicad_cruncher.daemon.commands.v0"
    assert pcb_clean["endpoint"] == "/api/v1/pcb/layer-cleanup"
    assert "daemon:kicad-ipc-plan" in _json_list(pcb_clean["adapters"])


def test_daemon_startup_writes_state_and_invokes_runner(tmp_path: Path, monkeypatch) -> None:
    """Verify daemon startup writes discovery state before running the server."""
    state_path = tmp_path / "daemon.json"
    runner = _RecordingServerRunner()
    monkeypatch.setenv("KICAD_CRUNCHER_DAEMON_STATE", str(state_path))

    exit_code = run_daemon(
        host="127.0.0.1",
        port=9021,
        reload=True,
        allow_remote_host=False,
        server_runner=runner,
    )

    payload = json.loads(state_path.read_text(encoding="utf-8"))
    assert exit_code == 0
    assert payload["schema"] == DAEMON_STATE_SCHEMA
    assert payload["url"] == "http://127.0.0.1:9021"
    assert runner.calls == [
        {
            "app": "kicad_cruncher.kicad_cruncher_daemon:create_app",
            "factory": True,
            "host": "127.0.0.1",
            "port": 9021,
            "reload": True,
        }
    ]


def test_daemon_startup_rejects_remote_host_before_runner(tmp_path: Path, monkeypatch) -> None:
    """Verify daemon startup refuses remote hosts unless explicitly allowed."""
    state_path = tmp_path / "daemon.json"
    runner = _RecordingServerRunner()
    monkeypatch.setenv("KICAD_CRUNCHER_DAEMON_STATE", str(state_path))

    exit_code = run_daemon(
        host="0.0.0.0",
        port=9021,
        reload=False,
        allow_remote_host=False,
        server_runner=runner,
    )

    assert exit_code == 2
    assert runner.calls == []
    assert not state_path.exists()


def test_plugin_main_reports_unreachable_daemon(monkeypatch) -> None:
    """Verify plugin entrypoint fails clearly when the daemon is unavailable."""
    module = _load_plugin_main_module()
    plugin_main = cast(_PluginRuntimeModule, module)

    def fail_urlopen(_request: object, timeout: object = None) -> _FakeHttpResponse:
        raise OSError(f"daemon unavailable after timeout {timeout}")

    monkeypatch.setattr(module, "_resolve_daemon_url", lambda: "http://127.0.0.1:9999")
    monkeypatch.setattr(plugin_main.urllib.request, "urlopen", fail_urlopen)

    assert plugin_main.main() == 1


def test_plugin_main_posts_cleanup_request_and_opens_daemon(monkeypatch) -> None:
    """Verify plugin entrypoint health-checks, posts cleanup plan, and opens UI."""
    module = _load_plugin_main_module()
    plugin_main = cast(_PluginRuntimeModule, module)
    posted_payloads: list[dict[str, object]] = []
    opened_urls: list[str] = []

    def fake_urlopen(request: object, timeout: object = None) -> _FakeHttpResponse:
        url = str(getattr(request, "full_url", request))
        if url.endswith("/health"):
            return _FakeHttpResponse(status=200)

        data = getattr(request, "data", b"{}")
        if not isinstance(data, bytes):
            data = b"{}"
        posted_payloads.append(json.loads(data.decode("utf-8")))
        return _FakeHttpResponse(
            status=200,
            body=json.dumps({"ok": True, "result": {"schema": "mutation"}}),
        )

    def fake_open(url: str) -> bool:
        opened_urls.append(url)
        return True

    monkeypatch.setattr(module, "_resolve_daemon_url", lambda: "http://127.0.0.1:9999")
    monkeypatch.setattr(
        module,
        "_discover_kicad_session",
        lambda: (None, {"connected": False, "source": "test"}),
    )
    monkeypatch.setattr(plugin_main.urllib.request, "urlopen", fake_urlopen)
    monkeypatch.setattr(plugin_main.webbrowser, "open", fake_open)

    assert plugin_main.main() == 0
    assert opened_urls == ["http://127.0.0.1:9999"]
    assert posted_payloads == [
        {
            "schema": "kicad_cruncher.daemon.pcb.layer_cleanup.request.v0",
            "mode": "kicad-ipc",
            "apply": False,
            "session": {"connected": False, "source": "test"},
        }
    ]


def test_plugin_install_copies_apply_adapter(tmp_path: Path) -> None:
    """Verify installed plugin package includes the IPC apply adapter."""
    plugins_dir = tmp_path / "plugins"

    results = install_plugin(plugins_dir=plugins_dir)

    source_dir = plugin_package_root(DEFAULT_PLUGIN_NAME)
    target_dir = plugins_dir / plugin_identifier(source_dir)
    assert len(results) == 1
    assert results[0].target_dir == target_dir
    assert (target_dir / "plugin.json").is_file()
    assert (target_dir / "main.py").is_file()
    assert (target_dir / "ipc_apply.py").is_file()
    assert not list(target_dir.rglob("__pycache__"))


def test_plugin_copy_filter_excludes_generated_artifacts() -> None:
    """Verify plugin install copy filter excludes caches, tests, and package outputs."""
    ignored = _copy_filter(
        "unused",
        [
            "plugin.json",
            "main.py",
            "ipc_apply.py",
            "__pycache__",
            ".venv",
            "dist",
            "tests",
            "plugin.pyc",
        ],
    )

    assert ignored == {"__pycache__", ".venv", "dist", "tests", "plugin.pyc"}


def test_plugin_discovers_daemon_url_from_state_file(tmp_path: Path, monkeypatch) -> None:
    """Verify plugin daemon discovery uses the daemon state file."""
    state_path = tmp_path / "daemon.json"
    monkeypatch.setenv("KICAD_CRUNCHER_DAEMON_STATE", str(state_path))
    monkeypatch.delenv("KICAD_CRUNCHER_DAEMON_URL", raising=False)
    write_daemon_state(host="127.0.0.1", port=9012)

    assert _load_plugin_main()._resolve_daemon_url() == "http://127.0.0.1:9012"


def test_plugin_daemon_url_env_overrides_state_file(tmp_path: Path, monkeypatch) -> None:
    """Verify explicit daemon URL env remains the highest-priority override."""
    state_path = tmp_path / "daemon.json"
    monkeypatch.setenv("KICAD_CRUNCHER_DAEMON_STATE", str(state_path))
    monkeypatch.setenv("KICAD_CRUNCHER_DAEMON_URL", "http://127.0.0.1:9123/")
    write_daemon_state(host="127.0.0.1", port=9012)

    assert _load_plugin_main()._resolve_daemon_url() == "http://127.0.0.1:9123"


def test_plugin_ignores_remote_daemon_state_url(tmp_path: Path, monkeypatch) -> None:
    """Verify plugin state discovery does not trust remote daemon URLs."""
    state_path = tmp_path / "daemon.json"
    monkeypatch.setenv("KICAD_CRUNCHER_DAEMON_STATE", str(state_path))
    monkeypatch.delenv("KICAD_CRUNCHER_DAEMON_URL", raising=False)
    write_daemon_state(host="0.0.0.0", port=9012)

    assert _load_plugin_main()._resolve_daemon_url() == "http://127.0.0.1:8765"


def test_daemon_pcb_clean_kicad_ipc_mode_returns_mutation_request() -> None:
    """Verify plugin-mode requests get IPC operations instead of file mutation."""
    payload = daemon_pcb_layer_cleanup(
        {
            "schema": "kicad_cruncher.daemon.pcb.layer_cleanup.request.v0",
            "mode": "kicad-ipc",
            "board_path": str(_HLR_TEST_PCB),
        }
    )

    result = _json_object(payload["result"])
    operation_counts = _json_object(result["operation_counts"])
    operations = _json_list(result["operations"])
    first_graphic = _json_object(
        next(item for item in operations if _json_object(item)["op"] == "remove_footprint_item")
    )

    assert payload["schema"] == "kicad_cruncher.daemon.pcb.layer_cleanup.response.v0"
    assert payload["mode"] == "kicad-ipc"
    assert payload["applied"] is False
    assert result["schema"] == "kicad_cruncher.pcb.clean.mutation_request.v0"
    assert result["operation_target"] == "kicad-ipc"
    assert result["plugin_apply_required"] is True
    assert operation_counts == {
        "remove_footprint_item": 310,
        "reset_layer_user_name": 6,
    }
    assert first_graphic["collection"] == "fp_lines"
    assert first_graphic["layer"] == "F.Fab"
    assert first_graphic["footprint_reference"] == "U1"


def test_daemon_pcb_clean_file_mode_apply_mutates_copy(tmp_path: Path) -> None:
    """Verify daemon file mode delegates to the same safe direct-file apply."""
    board_copy = tmp_path / "hlr_test.kicad_pcb"
    shutil.copy2(_HLR_TEST_PCB, board_copy)
    pcb = KiCadPcb(board_copy)
    value_property = pcb.footprints[0].get_property_object("Value")
    assert value_property is not None
    value_property.hide = False
    pcb.save(board_copy)

    payload = daemon_pcb_layer_cleanup(
        {
            "schema": "kicad_cruncher.daemon.pcb.layer_cleanup.request.v0",
            "mode": "file",
            "apply": True,
            "board_path": str(board_copy),
        }
    )

    result = _json_object(payload["result"])
    mutation_report = _json_object(result["mutation_report"])
    removed = _json_object(mutation_report["footprint_graphics_removed"])
    after = KiCadPcb(board_copy)
    after_value = after.footprints[0].get_property_object("Value")

    assert payload["applied"] is True
    assert result["status"] == "applied"
    assert removed["total"] == 310
    assert after_value is not None
    assert after_value.hide is True
