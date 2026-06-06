"""Local KiCad Cruncher daemon application."""

from __future__ import annotations

from importlib.metadata import version as distribution_version
from pathlib import Path

from kicad_cruncher._version import __version__
from kicad_cruncher.kicad_cruncher_pcb_clean import (
    apply_pcb_clean,
    build_pcb_clean_mutation_request,
    plan_pcb_clean,
)

DAEMON_API_SCHEMA = "kicad_cruncher.daemon.health.v0"
DAEMON_COMMANDS_SCHEMA = "kicad_cruncher.daemon.commands.v0"
DAEMON_KICAD_SESSION_SCHEMA = "kicad_cruncher.daemon.kicad_session.v0"
DAEMON_PCB_LAYER_CLEANUP_REQUEST_SCHEMA = "kicad_cruncher.daemon.pcb.layer_cleanup.request.v0"
DAEMON_PCB_LAYER_CLEANUP_RESPONSE_SCHEMA = "kicad_cruncher.daemon.pcb.layer_cleanup.response.v0"
DEFAULT_DAEMON_HOST = "127.0.0.1"
DEFAULT_DAEMON_PORT = 8765


def daemon_health_payload() -> dict[str, object]:
    """Return the daemon health payload shared by CLI and HTTP routes."""
    return {
        "schema": DAEMON_API_SCHEMA,
        "ok": True,
        "service": "kicad-cruncher",
        "version": __version__,
        "controlled_dependencies": {
            "kicad-monkey": _safe_distribution_version("kicad-monkey"),
            "wn-geometer": _safe_distribution_version("wn-geometer"),
        },
    }


def daemon_command_inventory_payload() -> dict[str, object]:
    """Return the local daemon command inventory."""
    return {
        "schema": DAEMON_COMMANDS_SCHEMA,
        "service": "kicad-cruncher",
        "version": __version__,
        "commands": [
            {
                "id": "pcb.clean",
                "name": "PCB Clean",
                "status": "available",
                "adapters": ["cli:file", "daemon:file", "daemon:kicad-ipc-plan"],
                "endpoint": "/api/v1/pcb/layer-cleanup",
                "config_schema": "kicad_cruncher.pcb.clean.config.v0",
                "description": (
                    "Plan or apply safe documentation-layer cleanup for KiCad PCB files. "
                    "KiCad IPC plugins request mutation operations and apply them under "
                    "KiCad's commit/undo model."
                ),
            },
            {
                "id": "pcb.hlr",
                "name": "PCB HLR",
                "status": "planned",
                "adapters": [],
                "endpoint": None,
            },
            {
                "id": "schematic.clean",
                "name": "Schematic Clean",
                "status": "planned",
                "adapters": [],
                "endpoint": None,
            },
        ],
    }


def daemon_kicad_session_payload() -> dict[str, object]:
    """Return daemon-visible KiCad session state."""
    return {
        "schema": DAEMON_KICAD_SESSION_SCHEMA,
        "connected": False,
        "reason": "no KiCad IPC plugin session has registered with this daemon",
    }


def daemon_pcb_layer_cleanup(payload: dict[str, object]) -> dict[str, object]:
    """Plan or apply PCB layer cleanup through the daemon contract."""
    mode = str(payload.get("mode", "kicad-ipc") or "kicad-ipc").strip()
    board_path = _payload_path(payload, "board_path")
    config_path = _payload_path(payload, "config_path")
    apply = bool(payload.get("apply", False))

    if mode in {"file", "direct-file"}:
        result = (
            apply_pcb_clean(board_path=board_path, config_path=config_path)
            if apply
            else plan_pcb_clean(board_path=board_path, config_path=config_path, dry_run=True)
        )
        return _daemon_pcb_layer_cleanup_response(
            mode=mode,
            applied=apply and result.get("status") == "applied",
            result=result,
        )

    if mode in {"kicad-ipc", "ipc", "ipc-plan"}:
        result = build_pcb_clean_mutation_request(
            board_path=board_path,
            config_path=config_path,
        )
        return _daemon_pcb_layer_cleanup_response(
            mode="kicad-ipc",
            applied=False,
            result=result,
            message="plugin must apply returned operations through KiCad IPC commit/undo",
        )

    return _daemon_pcb_layer_cleanup_response(
        mode=mode,
        applied=False,
        result={
            "status": "unsupported_mode",
            "supported_modes": ["file", "kicad-ipc"],
        },
        ok=False,
    )


def create_app() -> object:
    """Create the FastAPI app for the local daemon."""
    from fastapi import FastAPI
    from fastapi.responses import HTMLResponse

    app = FastAPI(title="KiCad Cruncher Daemon", version=__version__)

    @app.get("/health")
    def health() -> dict[str, object]:
        return daemon_health_payload()

    @app.get("/version")
    def version() -> dict[str, object]:
        return daemon_health_payload()

    @app.get("/api/v1/commands")
    def commands() -> dict[str, object]:
        return daemon_command_inventory_payload()

    @app.get("/api/v1/kicad/session")
    def kicad_session() -> dict[str, object]:
        return daemon_kicad_session_payload()

    @app.post("/api/v1/pcb/layer-cleanup")
    def pcb_layer_cleanup(payload: dict[str, object]) -> dict[str, object]:
        return daemon_pcb_layer_cleanup(payload)

    @app.get("/")
    def root() -> HTMLResponse:
        return HTMLResponse(_tool_center_html())

    return app


def _payload_path(payload: dict[str, object], key: str) -> Path | None:
    value = payload.get(key)
    if value is None:
        return None
    text = str(value).strip()
    return Path(text).expanduser() if text else None


def _daemon_pcb_layer_cleanup_response(
    *,
    mode: str,
    applied: bool,
    result: dict[str, object],
    ok: bool = True,
    message: str | None = None,
) -> dict[str, object]:
    payload: dict[str, object] = {
        "schema": DAEMON_PCB_LAYER_CLEANUP_RESPONSE_SCHEMA,
        "ok": ok,
        "mode": mode,
        "applied": applied,
        "result": result,
    }
    if message:
        payload["message"] = message
    return payload


def _safe_distribution_version(distribution_name: str) -> str | None:
    try:
        return distribution_version(distribution_name)
    except Exception:
        return None


def _tool_center_html() -> str:
    return """<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>KiCad Cruncher</title>
  <style>
    :root {
      --bg: #050505;
      --panel-bg: rgba(10, 10, 10, 0.9);
      --panel-border: rgba(255, 255, 255, 0.14);
      --text: rgb(231, 221, 221);
      --muted: rgb(150, 150, 150);
      --accent: rgb(180, 135, 0);
      --ok: rgb(100, 255, 100);
      --gap: 12px;
      --font: "Consolas", "Monaco", monospace;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      min-height: 100vh;
      background: var(--bg);
      color: var(--text);
      font-family: var(--font);
    }
    header {
      display: flex;
      gap: var(--gap);
      align-items: center;
      padding: 10px 14px;
      border-bottom: 1px solid var(--panel-border);
      background: var(--panel-bg);
    }
    h1 { margin: 0; font-size: 18px; }
    nav { display: flex; gap: 6px; }
    nav button {
      background: transparent;
      color: var(--text);
      border: 1px solid var(--panel-border);
      padding: 6px 10px;
    }
    nav button[aria-selected="true"] {
      border-color: var(--accent);
      color: var(--accent);
    }
    main { padding: 14px; }
    .panel {
      background: var(--panel-bg);
      border: 1px solid var(--panel-border);
      padding: 14px;
    }
    .status { color: var(--ok); margin-left: auto; }
    .muted { color: var(--muted); }
  </style>
</head>
<body>
  <header>
    <h1>KiCad Cruncher</h1>
    <nav aria-label="Tools">
      <button aria-selected="true">PCB Clean</button>
      <button disabled>Footprint HLR</button>
      <button disabled>Viewer</button>
      <button disabled>Config</button>
    </nav>
    <span class="status">daemon online</span>
  </header>
  <main>
    <section class="panel">
      <h2>PCB Clean</h2>
      <p class="muted">
        First implementation slice: shared CLI/config cleanup planner, daemon
        routing, and KiCad IPC plugin shim.
      </p>
    </section>
  </main>
</body>
</html>"""
