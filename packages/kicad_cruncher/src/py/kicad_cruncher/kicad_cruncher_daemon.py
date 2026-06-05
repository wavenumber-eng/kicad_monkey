"""Local KiCad Cruncher daemon application."""

from __future__ import annotations

from importlib.metadata import version as distribution_version

from kicad_cruncher._version import __version__

DAEMON_API_SCHEMA = "kicad_cruncher.daemon.health.v0"
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

    @app.get("/")
    def root() -> HTMLResponse:
        return HTMLResponse(_tool_center_html())

    return app


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
