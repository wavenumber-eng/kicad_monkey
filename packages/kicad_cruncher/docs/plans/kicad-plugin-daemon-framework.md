# KiCad Plugin Daemon Framework Plan

Status: active
Last updated: 2026-06-05
Owner: kicad_cruncher
Branch: plugin-daemon-framework-plan

## Goal

Build a public KiCad Cruncher plugin and daemon framework for a family of
Wavenumber-used, open-source KiCad tools.

The first production operations are:

1. Move plugin architecture, plans, code, installers, package data, and durable
   ownership out of `appz` and into `kicad_cruncher`.
2. Installable KiCad IPC action plugins.
3. A persistent local daemon that plugins can call for command execution,
   shared state, browser UI, and future KiCad Cruncher workflows.
4. A first clean PCB layer cleanup action that starts with existing board user
   layers and generated graphics.
5. A later schematic cleanup action for parameter coalescing, cruft deletion,
   and future font/size/look-and-feel normalization.
6. A later PCB HLR action that reuses existing `pcb-svg` and `wn-geometer`
   projection behavior.

The framework should support future utility applications such as SVG config
builders, library QA reports, placement/fabrication helpers, and selection
driven batch-edit tools.

Every plugin GUI tool should have a matching command-line tool. The command-line
tool owns the durable behavior contract and accepts a JSONC config file. The
daemon and plugin UI must share that command logic instead of reimplementing it.

## Current State

`appz/kicad_plugins/footprint_hlr` is the working prototype. It proves that a
KiCad IPC action plugin can:

- install into discovered user KiCad plugin folders;
- enable/report KiCad IPC API preferences;
- appear as a PCB-editor action and toolbar button;
- connect to the active PCB editor board;
- inspect selected footprints;
- open a localhost browser dialog;
- render footprint previews;
- clean selected footprint-local target-layer geometry; and
- stamp hidden `ALX_HLR_META` metadata through undoable KiCad commits.

The appz prototype does not yet have a persistent daemon, Rack coverage, or a
wired HLR projection backend. Its browser server is one-shot and plugin-local.

`kicad_cruncher` currently owns the public higher-level KiCad workflow package.
It already publishes through PyPI, has Rack signoff, and has public commands for
design review, manufacturing outputs, PCB SVG, and PCB layer STEP. This makes it
the right public home for plugin and daemon workflows.

`kicad_monkey` remains the low-level parser/model/rendering package. It should
not own plugin lifecycle, daemon UX, Wavenumber workflow commands, or browser
application orchestration.

## Ownership Decision

Public plugin work belongs in `kicad_cruncher`.

- `kicad_monkey` owns reusable KiCad source parsing, source-model helpers, SVG
  rendering primitives, and close-to-format file behavior.
- `wn-geometer` owns geometric projection, model bounds, and STEP/HLR kernels.
- `kicad_cruncher` owns public CLI workflows, plugin install commands, daemon
  endpoints, browser UI served by the daemon, KiCad IPC action shims, and Rack
  release signoff.
- `appz` consumes the public package and may provide Wavenumber workspace setup
  wrappers, but should stop owning plugin implementation code after migration.

The current appz plugin docs still say appz owns plugin packages. That wording
is correct for the prototype only and should be retired as the public
implementation lands.

The target end state is no plugin architecture, plugin plans, plugin code,
plugin installers, or plugin package artifacts in `appz`. Appz may keep only
thin setup wrappers that call public `kicad-cruncher` commands.

## Public Surface

Add the following public commands incrementally:

- `kicad-cruncher daemon`
- `kicad-cruncher plugin list-targets`
- `kicad-cruncher plugin install`
- `kicad-cruncher plugin status`
- `kicad-cruncher plugin uninstall`
- `kicad-cruncher pcb clean`
- `kicad-cruncher schematic clean`

The plugin commands should be discoverable through normal CLI help and listed in
the command manifest when they become public release behavior.

The first plugin package should be a thin KiCad IPC action that delegates to the
daemon. Avoid embedding durable command behavior in KiCad plugin entrypoints.

Each tool command should support:

- `--config <path>` to read JSONC config;
- `--write-config <path>` to emit the documented default config;
- `--dry-run` to produce a plan/report without changing KiCad files or live
  editor state;
- `--apply` or an equivalent explicit mutation switch for file/editor changes;
  and
- deterministic JSON report output suitable for tests and automation.

The GUI must call the same planning/apply functions used by these CLI commands.

## Logic Ownership Rule

Core behavior must be pure or adapter-light Python wherever possible:

- config parsing and validation;
- cleanup planning;
- target selection;
- safety/protection rules;
- report generation; and
- mutation request construction.

CLI commands, daemon endpoints, and KiCad IPC plugin actions are adapters around
that shared logic. The daemon is a router, state holder, and viewer server; it
must not become a second implementation of cleanup, HLR, or config behavior.

## Daemon Architecture

The daemon should be a local FastAPI service, following the successful
`lib_cruncher` pattern while keeping the KiCad plugin threat model tighter.

Defaults:

- bind to `127.0.0.1`;
- choose a stable default port or write the selected dynamic port to a config
  file;
- write a local daemon state file that installed plugins can use for endpoint
  discovery, with an explicit environment override for custom/test installs;
- expose no remote host binding unless explicitly requested;
- keep one process alive for all KiCad Cruncher plugin actions; and
- serve no-build HTML, CSS, and JavaScript for local utility UI.

Initial endpoints:

- `GET /health`
- `GET /version`
- `GET /api/v1/commands`
- `GET /api/v1/kicad/session`
- `POST /api/v1/pcb/layer-cleanup`
- `POST /api/v1/pcb/hlr/preview`
- `POST /api/v1/pcb/hlr/apply`

Future UI routes:

- `/ui/layer-cleanup`
- `/ui/footprint-hlr`
- `/ui/svg-config`

The daemon should own browser UI state and command routing. KiCad IPC plugins
should health-check the daemon, start it when configured to do so, or show a
clear message telling the user which command to run.

The daemon UI should be a KiCad Cruncher tool center rather than a one-tool
dialog. Start with a Lib Cruncher-style shell:

- header with service/version/status;
- tabbed tool navigation;
- left-side or top-level command/config controls;
- main preview/report panel;
- toast notifications;
- modal support for confirmation and detail views; and
- a shared log/status drawer for daemon and KiCad IPC activity.

Initial tabs should be:

- PCB Clean;
- Footprint HLR;
- Viewer / Inspection; and
- Config / Settings.

Only PCB Clean needs to be functional in the first rough implementation. The
other tabs may be disabled placeholders that establish the navigation model.

## Install Flow

Primary public install:

```powershell
uv tool install kicad-cruncher
kicad-cruncher plugin install --enable-api
kicad-cruncher daemon
```

POSIX equivalent:

```bash
uv tool install kicad-cruncher
kicad-cruncher plugin install --enable-api
kicad-cruncher daemon
```

Thin convenience wrappers may exist for Wavenumber workstations, but the shared
logic should live in Python command modules so Windows and POSIX behavior stays
identical.

The plugin installer should preserve the useful appz prototype behavior:

- discover KiCad user plugin folders;
- accept `--plugins-dir` and `--kicad-version`;
- support dry-run mode;
- optionally create a default plugin folder;
- enable/report KiCad IPC API preferences;
- report missing Python interpreter paths; and
- exclude build outputs, caches, package archives, and tests from installed
  plugin copies.

## Naming And Metadata

The appz prototype uses `ALX_HLR_META` as the hidden footprint metadata field
and `wavenumber.kicad_footprint_hlr.metadata.v1` as the metadata schema.

Before public release, decide whether plugin metadata should be:

- Wavenumber-specific, because these plugins are branded Wavenumber tools; or
- generic KiCad Cruncher namespaced, because the packages are open-source public
  utilities.

Do this before users install the first public plugin, because metadata fields
and plugin identifiers become persistent board/library content.

## Test And Signoff Requirements

Add Rack coverage before making plugin behavior release-facing:

- plugin manifest schema validation;
- plugin package-data inclusion/exclusion checks;
- installer dry-run tests using temporary KiCad config and document roots;
- installer copy-filter tests proving `dist`, `__pycache__`, virtualenvs, and
  generated archives are excluded;
- daemon startup and `/health` tests;
- daemon API tests with FastAPI test clients;
- pure layer-cleanup algorithm tests with known input/output fixtures;
- mocked KiCad IPC apply-adapter tests for commit, push, drop, and skipped
  operations;
- mocked KiCad IPC plugin-shim tests for daemon available/unavailable cases;
- CLI command manifest, help, design-doc, and module-ownership tests; and
- optional live KiCad IPC validation outside default CI.

The first release-facing daemon/plugin slice should pass:

```bash
uv run --extra test rack run --all
uv run python -m build
uv run twine check dist/*
```

## Web UI Rules

Daemon-served UI should follow the no-build web standard:

- HTML, CSS, and JavaScript should be directly inspectable and runnable.
- Start from the Lib Cruncher visual vocabulary: dark industrial theme, compact
  tabbed header, panels, tables, status pills, modals, toasts, and CSS custom
  properties in a single application stylesheet.
- JavaScript modules that contain algorithms or data transformations need tests
  with known inputs and outputs.
- DOM/browser behavior should get agent-time browser inspection during feature
  development, with core behavior covered by non-visual tests.
- Use JSDoc and `// @ts-check` for no-build JavaScript unless the UI complexity
  justifies a TypeScript migration.
- CSS should use custom properties for colors, spacing, typography, and shared
  layout constants instead of scattered hard-coded values.
- Shared UI primitives such as modals, toasts, panels, and command surfaces
  should be implemented once, with Web Components considered for reusable
  no-framework components.

Use `lib_cruncher/src/py/lib_cruncher/static/index.html` and
`lib_cruncher/src/py/lib_cruncher/static/style.css` as reference material for
the first styling pass. Do not blindly copy unrelated parts-table behavior; copy
the shell, component vocabulary, CSS variable discipline, and compact
tool-oriented layout.

## Milestones

### 0. Branch And Prerequisites

- [x] Create local branch `plugin-daemon-framework-plan`.
- [x] Sync appz to `origin/dev`.
- [x] Update `kicad_cruncher` to depend on `wn-geometer==2026.6.4`.
- [x] Refresh `uv.lock`.
- [x] Run L99 signoff after the dependency update.

### 1. Durable Design

- [x] Add an ADR for plugin and daemon ownership.
- [x] Add daemon architecture/design docs.
- [x] Add plugin install contract docs.
- [x] Add the CLI/config/GUI parity rule to durable design docs.
- [x] Add command manifest entries only when commands become public behavior.
- [ ] Decide metadata namespace and plugin identifier policy.

### 2. Installer Port

- [ ] Port appz `kicad_plugins/shared/install.py` into `kicad_cruncher`.
- [ ] Convert appz-specific wording to public KiCad Cruncher wording.
- [ ] Add `plugin list-targets`, `plugin install`, and `plugin status`.
- [ ] Add installer dry-run and copy-filter tests.
- [ ] Keep appz setup as a wrapper that delegates to public CLI commands.

### 3. Daemon Skeleton

- [x] Add `kicad-cruncher daemon`.
- [x] Add `/health`, `/version`, and command inventory endpoints.
- [x] Add daemon startup tests.
- [x] Add local-only host defaults and explicit remote-host opt-in.
- [x] Add config/port discovery for plugin shims.
- [x] Add the initial Lib Cruncher-style tabbed tool-center shell with PCB
      Clean as the first active tab.

### 4. First Plugin Shim

- [x] Package the first KiCad IPC action as `kicad_cruncher` package data.
- [x] Keep the action shim small: discover board/session, call daemon, report
      failure clearly.
- [x] Add mocked KiCad IPC apply-adapter tests for commit/undo behavior.
- [x] Add mocked shim tests for daemon available/unavailable cases.
- [ ] Preserve the useful appz installer diagnostics.

### 5. PCB Layer Cleanup

- [x] Define the `pcb.clean.config` JSONC contract.
- [x] Add `kicad-cruncher pcb clean --write-config`.
- [x] Add `kicad-cruncher pcb clean --config <path> --dry-run`.
- [x] Add explicit apply behavior for the CLI file path.
- [x] Extract pure cleanup planning from the appz HLR prototype.
- [x] Add known-input/known-output tests for cleanup selection and protection
      rules.
- [x] Add daemon endpoint and optional browser UI.
- [x] Wire the KiCad plugin action through the daemon.
- [x] Document file-path target-layer safety rules.
- [x] Document plugin undo/commit behavior.
- [x] Add the plugin-side adapter that applies daemon mutation requests through
      KiCad IPC commit, update, push, and drop operations.
- [ ] Start with existing PCB user/generated layers before expanding into HLR
      generation or broader board edits.

### 6. Schematic Cleanup

- [ ] Define the future `schematic.clean.config.a0` JSONC contract.
- [ ] Plan parameter coalescing, generated-field/cruft deletion, and safe
      schematic formatting normalization.
- [ ] Keep this behind PCB Clean until the daemon and CLI/config pattern is
      proven.

### 7. PCB HLR

- [ ] Reuse existing `pcb-svg` pose and Geometer projection logic.
- [ ] Add footprint-local preview and apply flows through the daemon.
- [ ] Preserve selected-footprint defaults and target-layer preview behavior.
- [ ] Add fixture-backed tests that validate request shape and metadata without
      needing a live KiCad editor.

### 8. Retire Appz Prototype

- [ ] Update appz setup to call `kicad-cruncher plugin install`.
- [ ] Remove appz-owned plugin implementation code.
- [ ] Remove appz-owned plugin installer code.
- [ ] Remove appz-owned plugin planning docs.
- [ ] Keep only Wavenumber workspace wrapper behavior in appz.

## Risks

- KiCad IPC plugin loading and toolbar refresh behavior differs across KiCad 9,
  KiCad 10.0.0, and later KiCad 10 builds.
- A plugin that auto-starts a daemon needs careful local-only defaults and clear
  failure UX.
- Metadata names and plugin identifiers are hard to change after public use.
- The first daemon UI can become application-specific if shared panel, modal,
  toast, and command patterns are not factored early.
- Live KiCad IPC validation is valuable but should not be required for default
  CI unless a stable runner exists.

## Completion Criteria

The framework is ready for public release when:

- install, daemon, and first plugin behavior are documented in durable design
  docs and ADRs;
- Rack signoff checks plugin docs, command manifests, installer tests, daemon
  tests, and package-data hygiene;
- appz delegates to public `kicad-cruncher` commands;
- a fresh public install can install the plugin, start the daemon, and execute a
  cleanup operation from KiCad; and
- the release package contains no prototype build outputs, generated archives,
  private workspace paths, or appz-only assumptions.
