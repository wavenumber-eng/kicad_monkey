# Contracts

This directory stores public machine-readable command manifests and future
schema contracts.

Current contracts:

- `command_manifest.v0.json` lists public CLI commands.
- `bom_pnp_config.v1.schema.json` defines the shared BOM/PnP/JLC config
  contract used by `bom`, `pnp`, and `jlc`.
- `interface_design_manifest.v0.json` lists major interfaces that require
  durable design documentation.
- `pcb_clean_config.v0.schema.json` defines the first PCB cleanup config
  contract used by `pcb clean`.
- `pcb_layer_step_config.v2.schema.json` defines the v2 `pcb-layer-step`
  fixture-alignment STEP config contract used by `pcb-layer-step`.
- `pcb_svg_config.a0.schema.json` defines the A0 `pcb.svg.config` view and
  layer-output config contract used by `pcb-svg`.
