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
      --surface: #101214;
      --surface-2: #171a1d;
      --surface-3: #20252a;
      --panel-bg: rgba(16, 18, 20, 0.96);
      --panel-border: rgba(255, 255, 255, 0.14);
      --text: rgb(231, 221, 221);
      --muted: rgb(150, 150, 150);
      --accent: rgb(180, 135, 0);
      --accent-2: rgb(74, 144, 226);
      --ok: rgb(100, 255, 100);
      --warn: rgb(255, 189, 89);
      --danger: rgb(255, 106, 106);
      --gap: 12px;
      --font: "Consolas", "Monaco", monospace;
      --radius: 6px;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      min-height: 100vh;
      background: var(--bg);
      color: var(--text);
      font-family: var(--font);
      letter-spacing: 0;
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
    h2 { margin: 0; font-size: 16px; }
    h3 { margin: 0 0 8px; font-size: 13px; color: var(--muted); }
    nav { display: flex; gap: 6px; }
    nav button {
      background: transparent;
      color: var(--text);
      border: 1px solid var(--panel-border);
      padding: 6px 10px;
      min-height: 32px;
    }
    nav button[aria-selected="true"] {
      border-color: var(--accent);
      color: var(--accent);
    }
    button {
      border: 1px solid var(--panel-border);
      border-radius: var(--radius);
      color: var(--text);
      background: var(--surface-3);
      min-height: 34px;
      padding: 7px 10px;
      font: inherit;
      cursor: pointer;
    }
    button.primary {
      border-color: var(--accent);
      color: #111;
      background: var(--accent);
    }
    button.danger {
      border-color: var(--danger);
      color: var(--danger);
      background: transparent;
    }
    button:disabled {
      cursor: not-allowed;
      color: var(--muted);
      background: transparent;
      border-color: rgba(255, 255, 255, 0.08);
    }
    input,
    select {
      width: 100%;
      min-height: 34px;
      border: 1px solid var(--panel-border);
      border-radius: var(--radius);
      background: #070808;
      color: var(--text);
      font: inherit;
      padding: 7px 9px;
    }
    label {
      display: grid;
      gap: 5px;
      color: var(--muted);
      font-size: 12px;
    }
    main {
      display: grid;
      grid-template-columns: minmax(320px, 430px) minmax(0, 1fr);
      gap: var(--gap);
      padding: 14px;
    }
    .panel {
      background: var(--panel-bg);
      border: 1px solid var(--panel-border);
      border-radius: var(--radius);
      padding: 14px;
    }
    .status { color: var(--ok); margin-left: auto; }
    .muted { color: var(--muted); }
    .stack { display: grid; gap: var(--gap); }
    .row { display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }
    .toolbar { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
    .toolbar button:last-child { grid-column: 1 / -1; }
    .badge {
      display: inline-flex;
      align-items: center;
      border: 1px solid var(--panel-border);
      border-radius: 999px;
      min-height: 24px;
      padding: 2px 8px;
      color: var(--muted);
      background: rgba(255, 255, 255, 0.04);
      white-space: nowrap;
    }
    .badge.ok { color: var(--ok); border-color: rgba(100, 255, 100, 0.35); }
    .badge.warn { color: var(--warn); border-color: rgba(255, 189, 89, 0.35); }
    .summary-grid {
      display: grid;
      grid-template-columns: repeat(4, minmax(130px, 1fr));
      gap: var(--gap);
    }
    .metric {
      background: var(--surface);
      border: 1px solid var(--panel-border);
      border-radius: var(--radius);
      padding: 11px;
      min-height: 74px;
    }
    .metric strong {
      display: block;
      font-size: 24px;
      line-height: 1.1;
      color: var(--accent-2);
    }
    .metric span {
      display: block;
      margin-top: 5px;
      color: var(--muted);
      font-size: 12px;
    }
    .table {
      width: 100%;
      border-collapse: collapse;
      font-size: 12px;
    }
    .table th,
    .table td {
      border-bottom: 1px solid rgba(255, 255, 255, 0.08);
      padding: 7px 6px;
      text-align: left;
      vertical-align: top;
    }
    .table th {
      color: var(--muted);
      font-weight: 400;
    }
    pre {
      margin: 0;
      max-height: 42vh;
      overflow: auto;
      padding: 12px;
      border: 1px solid var(--panel-border);
      border-radius: var(--radius);
      background: #050607;
      color: var(--text);
      font-size: 12px;
      white-space: pre-wrap;
      overflow-wrap: anywhere;
    }
    dialog {
      width: min(520px, calc(100vw - 28px));
      border: 1px solid var(--panel-border);
      border-radius: var(--radius);
      background: var(--surface-2);
      color: var(--text);
      padding: 0;
    }
    dialog::backdrop { background: rgba(0, 0, 0, 0.68); }
    .dialog-body { display: grid; gap: var(--gap); padding: 14px; }
    @media (max-width: 920px) {
      main { grid-template-columns: 1fr; }
      .summary-grid { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    }
    @media (max-width: 520px) {
      header { align-items: flex-start; flex-direction: column; }
      .status { margin-left: 0; }
      .summary-grid { grid-template-columns: 1fr; }
      .toolbar { grid-template-columns: 1fr; }
    }
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
    <span class="status" id="daemon-status">daemon online</span>
  </header>
  <main>
    <section class="panel stack">
      <div class="row">
        <h2>PCB Clean</h2>
        <span class="badge" id="mode-badge">file mode</span>
      </div>
      <label>
        Board
        <input id="board-path" type="text" autocomplete="off" spellcheck="false"
               placeholder="C:\\work\\board\\board.kicad_pcb">
      </label>
      <label>
        Config
        <input id="config-path" type="text" autocomplete="off" spellcheck="false"
               placeholder="pcb.clean.config">
      </label>
      <label>
        Mode
        <select id="cleanup-mode">
          <option value="file">File</option>
          <option value="kicad-ipc">KiCad IPC Plan</option>
        </select>
      </label>
      <div class="toolbar">
        <button class="primary" id="plan-button" type="button">Plan</button>
        <button id="ipc-button" type="button">IPC Plan</button>
        <button class="danger" id="apply-button" type="button">Apply To File</button>
      </div>
      <div class="row">
        <span class="badge" id="result-status">idle</span>
        <span class="badge" id="result-board">no board</span>
      </div>
    </section>
    <section class="stack">
      <div class="panel stack">
        <div class="row">
          <h2>Plan Summary</h2>
          <span class="badge" id="result-mode">not run</span>
        </div>
        <div class="summary-grid">
          <div class="metric">
            <strong id="metric-layer-resets">0</strong><span>Layer Names</span>
          </div>
          <div class="metric">
            <strong id="metric-footprint-graphics">0</strong><span>Footprint Graphics</span>
          </div>
          <div class="metric">
            <strong id="metric-board-graphics">0</strong><span>Board Graphics</span>
          </div>
          <div class="metric"><strong id="metric-values">0</strong><span>Value Fields</span></div>
        </div>
        <table class="table" aria-label="Cleanup layer totals">
          <thead><tr><th>Layer</th><th>Items</th></tr></thead>
          <tbody id="layer-table"><tr><td class="muted" colspan="2">No plan loaded</td></tr></tbody>
        </table>
      </div>
      <div class="panel stack">
        <div class="row">
          <h2>Response</h2>
          <span class="badge" id="response-schema">none</span>
        </div>
        <pre id="raw-json">{}</pre>
      </div>
    </section>
  </main>
  <dialog id="apply-dialog">
    <form method="dialog" class="dialog-body">
      <h2>Apply PCB Clean</h2>
      <p class="muted" id="apply-dialog-board"></p>
      <div class="row">
        <button value="cancel" type="submit">Cancel</button>
        <button class="danger" id="confirm-apply-button" value="apply" type="submit">Apply</button>
      </div>
    </form>
  </dialog>
  <script>
    // @ts-check
    const endpoint = "/api/v1/pcb/layer-cleanup";
    const $ = (id) => document.getElementById(id);
    const boardInput = $("board-path");
    const configInput = $("config-path");
    const modeSelect = $("cleanup-mode");
    const planButton = $("plan-button");
    const ipcButton = $("ipc-button");
    const applyButton = $("apply-button");
    const applyDialog = $("apply-dialog");
    const confirmApplyButton = $("confirm-apply-button");

    function textInputValue(element) {
      return element instanceof HTMLInputElement ? element.value.trim() : "";
    }

    function selectedMode() {
      return modeSelect instanceof HTMLSelectElement ? modeSelect.value : "file";
    }

    function setText(id, value) {
      const element = $(id);
      if (element) {
        element.textContent = value;
      }
    }

    function setBusy(isBusy) {
      for (const button of [planButton, ipcButton, applyButton]) {
        if (button instanceof HTMLButtonElement) {
          button.disabled = isBusy;
        }
      }
      setText("daemon-status", isBusy ? "daemon busy" : "daemon online");
    }

    function payload(mode, apply) {
      const body = {
        schema: "kicad_cruncher.daemon.pcb.layer_cleanup.request.v0",
        mode,
        apply
      };
      const boardPath = textInputValue(boardInput);
      const configPath = textInputValue(configInput);
      if (boardPath) {
        body.board_path = boardPath;
      }
      if (configPath) {
        body.config_path = configPath;
      }
      return body;
    }

    async function postCleanup(mode, apply) {
      setBusy(true);
      try {
        const response = await fetch(endpoint, {
          method: "POST",
          headers: {"Content-Type": "application/json"},
          body: JSON.stringify(payload(mode, apply))
        });
        const decoded = await response.json();
        render(decoded);
      } catch (error) {
        render({
          ok: false,
          mode,
          applied: false,
          result: {status: "request_failed", error: String(error)}
        });
      } finally {
        setBusy(false);
      }
    }

    function render(payloadValue) {
      const result = objectValue(payloadValue.result);
      const report = objectValue(result.board_report);
      const mutation = objectValue(result.mutation_report);
      const source = Object.keys(mutation).length ? mutation : report;
      const inventory = objectValue(report.inventory);
      const operationCounts = objectValue(result.operation_counts);

      setText("raw-json", JSON.stringify(payloadValue, null, 2));
      setText("response-schema", String(payloadValue.schema || result.schema || "none"));
      setText("result-mode", String(payloadValue.mode || "unknown"));
      setText("result-status", String(result.status || "unknown"));
      setText("result-board", boardLabel(result, report));
      setText("mode-badge", selectedMode() === "file" ? "file mode" : "ipc plan");

      const layerResets = numberValue(
        result.operation_counts ? operationCounts.reset_layer_user_name :
        mutation.layer_user_names_reset ?? arrayValue(report.layer_user_name_resets).length
      );
      const footprintGraphics = tallyTotal(
        source.footprint_graphics_removed ?? report.footprint_graphics,
        operationCounts.remove_footprint_item
      );
      const boardGraphics = tallyTotal(
        source.board_graphics_removed ?? report.board_graphics,
        operationCounts.remove_board_item
      );
      const values = tallyTotal(
        source.value_fields_hidden ?? report.value_fields,
        operationCounts.hide_footprint_value_field
      );

      setText("metric-layer-resets", String(layerResets));
      setText("metric-footprint-graphics", String(footprintGraphics));
      setText("metric-board-graphics", String(boardGraphics));
      setText("metric-values", String(values));
      renderLayerTable(source, report);
    }

    function renderLayerTable(source, report) {
      const rows = new Map();
      collectLayerRows(
        rows,
        objectValue(source.footprint_graphics_removed ?? report.footprint_graphics)
      );
      collectLayerRows(rows, objectValue(source.board_graphics_removed ?? report.board_graphics));
      collectLayerRows(rows, objectValue(source.generated_items_removed ?? report.generated_items));
      collectLayerRows(rows, objectValue(source.value_fields_hidden ?? report.value_fields));
      const body = $("layer-table");
      if (!body) {
        return;
      }
      body.textContent = "";
      if (rows.size === 0) {
        const row = document.createElement("tr");
        const cell = document.createElement("td");
        cell.colSpan = 2;
        cell.className = "muted";
        cell.textContent = "No layer items";
        row.append(cell);
        body.append(row);
        return;
      }
      for (const [layer, total] of [...rows.entries()].sort()) {
        const row = document.createElement("tr");
        const name = document.createElement("td");
        const count = document.createElement("td");
        name.textContent = layer;
        count.textContent = String(total);
        row.append(name, count);
        body.append(row);
      }
    }

    function collectLayerRows(rows, tally) {
      const byLayer = objectValue(tally.by_layer);
      for (const [layer, value] of Object.entries(byLayer)) {
        rows.set(layer, (rows.get(layer) || 0) + numberValue(value));
      }
    }

    function boardLabel(result, report) {
      const board = result.board || report.resolved_board || report.input;
      return board ? String(board) : "no board";
    }

    function tallyTotal(tally, fallback) {
      const value = objectValue(tally).total;
      return numberValue(value ?? fallback);
    }

    function objectValue(value) {
      return value && typeof value === "object" && !Array.isArray(value) ? value : {};
    }

    function arrayValue(value) {
      return Array.isArray(value) ? value : [];
    }

    function numberValue(value) {
      return typeof value === "number" && Number.isFinite(value) ? value : 0;
    }

    if (planButton instanceof HTMLButtonElement) {
      planButton.addEventListener("click", () => postCleanup(selectedMode(), false));
    }
    if (ipcButton instanceof HTMLButtonElement) {
      ipcButton.addEventListener("click", () => postCleanup("kicad-ipc", false));
    }
    if (applyButton instanceof HTMLButtonElement) {
      applyButton.addEventListener("click", () => {
        const boardPath = textInputValue(boardInput);
        setText("apply-dialog-board", boardPath || "No board path supplied.");
        if (applyDialog instanceof HTMLDialogElement) {
          applyDialog.showModal();
        }
      });
    }
    if (confirmApplyButton instanceof HTMLButtonElement) {
      confirmApplyButton.addEventListener("click", () => {
        window.setTimeout(() => postCleanup("file", true), 0);
      });
    }
    if (modeSelect instanceof HTMLSelectElement) {
      modeSelect.addEventListener("change", () => {
        setText("mode-badge", selectedMode() === "file" ? "file mode" : "ipc plan");
      });
    }
  </script>
</body>
</html>"""
