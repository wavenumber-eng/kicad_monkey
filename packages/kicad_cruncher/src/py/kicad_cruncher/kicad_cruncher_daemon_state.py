"""Local daemon discovery state for KiCad Cruncher."""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

from kicad_cruncher._version import __version__

DAEMON_STATE_SCHEMA = "kicad_cruncher.daemon.state.v0"
DAEMON_STATE_ENV = "KICAD_CRUNCHER_DAEMON_STATE"


def daemon_state_path() -> Path:
    """Return the local daemon state file path."""
    override = os.environ.get(DAEMON_STATE_ENV)
    if override:
        return Path(override).expanduser()

    if os.name == "nt":
        root = Path(os.environ.get("LOCALAPPDATA") or Path.home() / "AppData" / "Local")
    elif sys.platform == "darwin":
        root = Path.home() / "Library" / "Application Support"
    else:
        root = Path(os.environ.get("XDG_STATE_HOME") or Path.home() / ".local" / "state")
    return root / "kicad-cruncher" / "daemon.json"


def daemon_state_payload(*, host: str, port: int) -> dict[str, object]:
    """Return the JSON state payload for one running daemon endpoint."""
    return {
        "schema": DAEMON_STATE_SCHEMA,
        "service": "kicad-cruncher",
        "version": __version__,
        "host": host,
        "port": port,
        "url": f"http://{host}:{port}",
        "pid": os.getpid(),
    }


def write_daemon_state(*, host: str, port: int, path: Path | None = None) -> Path:
    """Write daemon discovery state and return its path."""
    state_path = path or daemon_state_path()
    state_path.parent.mkdir(parents=True, exist_ok=True)
    state_path.write_text(json.dumps(daemon_state_payload(host=host, port=port), indent=2) + "\n")
    return state_path
