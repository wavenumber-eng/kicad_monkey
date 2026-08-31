# SVG Contract

KiCad Monkey has two PCB SVG output profiles:

- `enriched`: source-aware SVG for inspection and downstream applications
- `oracle`: metadata-free SVG shaped for KiCad CLI oracle comparison

Do not use `enriched` SVG as the strict KiCad CLI parity artifact. Oracle tests
that compare against `kicad-cli pcb export svg` must request
`profile="oracle"`.

## PCB SVG

PCB enriched SVG uses millimeter user coordinates. SVG ids are render-artifact
lookup keys. Downstream tools should prefer documented `data-*` attributes and
the embedded metadata payload for semantic identity.

When PCB metadata is enabled, the root SVG carries:

- `data-stage`
- `data-group-mode`
- `data-enrichment-schema`
- `data-view-kind`
- `data-profile`
- `data-source`
- `data-included-layers`

PCB primitive groups carry `data-primitive` values including:

- `track`
- `arc`
- `via`
- `via-hole`
- `zone`
- `footprint`
- `pad`
- `pad-hole`
- `graphic`
- `text`
- `dimension`

Layer metadata uses KiCad layer names directly:

- `data-layer-name` for one layer
- `data-layer-names` for multiple layers
- `data-layer-role` / `data-layer-roles` for normalized roles

The embedded PCB metadata also records imported/user layer aliases. Use
`layers.layers[].display_name` or `layers.layer_name_to_display_name` for UI
labels, and `layers.layer_name_to_user_name` when the original KiCad user alias
must be distinguished from the canonical layer name.

Electrical and component relationships are emitted when known:

- `data-net-index`, `data-net-id`, `data-net`
- `data-net-class`, `data-net-classes`
- `data-component`, `data-component-uid`, `data-component-uuid`
- `data-footprint`
- `data-pad-designator`, `data-pad-number`
- `data-pad-type`, `data-pad-shape`

Footprint child metadata is emitted for enriched SVG only:

- `data-ref="property"` with `data-footprint-text-role` of `designator`,
  `value`, or `property`
- `data-ref="fp_text"` / `data-ref="fp_text_box"` with
  `data-footprint-text-role="user"` when applicable
- `data-ref="fp_line"`, `fp_arc`, `fp_circle`, `fp_rect`, or `fp_poly` with
  `data-primitive="footprint-graphic"`
- `data-footprint-primitive` and `data-footprint-graphic-kind` identify the
  source footprint item class

Drill geometry uses:

- `data-primitive="pad-hole"` or `data-primitive="via-hole"`
- `data-hole-owner`
- `data-hole-kind`: `round` or `slot`
- `data-hole-plating`: `plated`, `non_plated`, or `unknown`
- `data-hole-render`
- `data-hole-diameter-mm` for round holes
- `data-hole-width-mm` / `data-hole-height-mm` for slot holes

Non-plated through-hole pad drill records are board cutouts. In enriched PCB
SVG, their `pad-hole` groups remain visible in layer-filtered views for every
board layer, including inner copper layers, while `data-layer-names` preserves
the source pad layer declaration.

Plated pad-hole operations instead list the complete enabled copper stack,
because their physical drill scope is through-board even when the pad's copper
membership is only `F&B.Cu` or has removed internal annuli. NPTH hole roles
remain unlayered/all-layer cutouts and retain the authored layer declaration.

For plated through-hole pads and vias, copper-flash scope and drill scope are
separate. `via_aperture` and pad flash operations carry the layers resolved
from KiCad's unused-layer policy, geometric local copper connectivity, and
`zone_layer_connections`. Pad membership is exact; unlike via endpoint pairs,
pad layer lists do not imply an intervening span, and `F&B.Cu` expands only to
the two external copper layers. Pad/via drill operations keep the authored
physical span. Consumers must not treat the two endpoint names on a resolved
`via_aperture` operation as a span; only `via_drill` endpoint layers expand
across the intervening copper stack. When no annular land is flashed, a via
record legitimately contains its drill without a `via_aperture` operation.

Via metadata uses:

- `data-via-type`: `through`, `blind`, `buried`, or `micro`
- `data-via-drill-mm`
- `data-via-size-mm`
- `data-ipc4761-*` attributes for KiCad via fabrication settings when present,
  including tenting, covering, plugging, capping, and filling

## PCB Enrichment Metadata

PCB enriched SVG embeds document-level JSON metadata as:

```xml
<metadata id="pcb-enrichment-a0" data-schema="kicad_monkey.pcb.svg.enrichment.a0">
  ...
</metadata>
```

The schema file is `pcb_svg_enrichment_a0.schema.json`.

The payload records:

- source PCB path
- project-level text variables
- board bounding box, auxiliary origin, thickness, and stackup
- emitted view information
- layer maps, user aliases, display names, and normalized layer roles
- net, netclass, and component lookup tables
- component placement summaries

## Schematic SVG

Schematic SVG uses source-owned ids as the DOM lookup surface. Enriched
schematic SVG embeds document-level JSON metadata as:

```xml
<metadata id="schematic-enrichment-a0" data-schema="kicad_monkey.schematic.svg.enrichment.a0">
  ...
</metadata>
```

The schema file is `schematic_svg_enrichment_a0.schema.json`. The payload
records the rendered sheet view and embeds the `kicad_monkey.design.a0`
KiCad design JSON payload under
`design`. That design payload carries components, nets, graphical SVG ids, and
lookup indexes:

- `components[].svg_id` points to the component SVG group id
- `nets[].graphical` groups related schematic SVG ids by record type
- `nets[].graphical.pins[]` maps designator/pin pairs to SVG ids
- `view_indexes.svg_to_net` maps rendered electrical SVG group ids for the
  current sheet view directly to `{uid, name}` net summaries
- `view_indexes.svg_to_nets` preserves multiple candidates if a current-view
  SVG id cannot be reduced to one net summary
- `view_indexes.net_to_svg` and `view_indexes.net_uid_to_svg` support
  current-view net highlighting without walking the whole design payload
- `indexes.svg_to_net` maps globally unambiguous electrical SVG group ids back
  to net names
- `indexes.svg_to_nets` maps every electrical SVG group id, including pin
  groups, to all candidate net names; repeated hierarchical sheets can make a
  source-owned SVG id non-unique
- `indexes.sheet_svg_to_nets` maps KiCad sheet instance path plus electrical
  SVG group id to candidate net names for the rendered sheet instance; use the
  SVG metadata `view.sheet_instance_path` first and `view.sheet_path` as a
  fallback
- `indexes.net_to_graphics` maps each net name to its rendered electrical SVG
  group ids
- `nets[].endpoints[]` provides semantic trace endpoints

Consumers should use `view_indexes` first for interactions with one rendered
schematic SVG. The design-wide `indexes` payload is for cross-sheet reasoning,
global search, and diagnostics. KiCad-facing net names remain in `net.name` and
in the view net summaries as `name`; the sheet instance path disambiguates
shared source UUIDs from repeated hierarchical sheets.

In `profile="enriched"` schematic output, records are wrapped in source-owned
`<g>` elements. These groups carry `data-ref` for the KiCad record kind and
`data-primitive` for the normalized review object:

- placed component symbols use `data-primitive="symbol"`
- power symbols use `data-primitive="power-symbol"`
- hierarchical sheet symbols use `data-primitive="sheet-symbol"`
- hierarchical labels use `data-primitive="port"`
- sheet pins use nested `data-ref="sheet_pin"` groups with
  `data-primitive="sheet-entry"`
- placed symbol pins use nested `data-ref="symbol_pin"` groups with
  `data-primitive="pin"`

Real-world visual-review outputs name repeated hierarchical sheet instances by
the sheet instance name, not the shared schematic file stem. For example,
multiple `TPS62A02_BUCK.kicad_sch` instances render as
`TPS62A02_BUCK_1V0`, `TPS62A02_BUCK_1V8`, etc.

`profile="oracle"` suppresses these metadata hooks for KiCad CLI parity.
Schematic colors are controlled through semantic role colors on
`KiCadSvgRenderOptions.schematic_role_colors` or the copy helper
`KiCadSvgRenderOptions.with_schematic_role_colors(...)`. This is the canonical
schematic theming path, including black-and-white output:
`KiCadSvgRenderOptions.black_and_white_native()` installs the
`SCHEMATIC_SVG_BLACK_AND_WHITE_ROLE_COLORS` role theme instead of using a
separate schematic monochrome switch. Valid roles are the KiCad schematic theme
keys exposed by `SCHEMATIC_SVG_COLOR_ROLES`, including `wire`, `bus`,
`junction`, `component_outline`, `component_body`, `pin`, `pin_name`,
`pin_number`, `reference`, `value`, `fields`, `label_local`, `label_global`,
`label_hier`, `sheet`, `sheet_background`, `sheet_label`, `worksheet`, and
`background`. The special `foreground` role is a fallback for explicit custom
schematic colors that do not match a KiCad theme source color; black-and-white
themes set it to black so custom-colored symbol graphics still become
monochrome. Role aliases such as `symbol_fill`, `symbol_outline`,
`global_label`, `drawing_sheet`, and `default_foreground` are normalized to the
canonical role names when options are built.
`schematic_svg_options_from_preferences(...)` loads KiCad preferences into role
colors and `font_face_override`. Raw
`color_overrides` remains available as a source-color escape hatch; application
themes should use semantic roles.

Downstream tools should not infer schematic connectivity from rendered text or
group nesting alone.

### Compiled graph page view

When a caller supplies the compiled schematic graph and concrete schematic
instance, the a0 enrichment payload additively includes
`compiled_schematic_graph_view` with schema
`kicad_monkey.schematic.svg.compiled_graph_view.a0`. The view contains:

- the graph schema and identity namespace;
- a graph artifact path relative to the SVG file;
- linkage contract
  `kicad_monkey.schematic.svg.compiled_graph_linkage.a0`;
- canonical `page_occurrence_ref` and artifact key `sch.dwg_scene`;
- page-scoped graphical-artifact-link refs;
- `element_id -> graphical_artifact_link_ref[]` and
  `target_ref -> element_id[]` indexes;
- the graph-owned target type for each indexed target ref.

The root SVG mirrors the graph/view schema, page occurrence, artifact key, and
linkage contract as compact discovery attributes. The authoritative drawing
join is `page_occurrence_ref + artifact_key + element_id`. Displayed names,
designators, net names, text, DOM order, and geometry are not join keys.
Repeated sheet occurrences may share an `element_id`; the page occurrence
keeps their graph links and semantic targets distinct.

`validate_schematic_svg_compiled_graph_view(...)` fails when a page is unknown,
the projected view differs from the graph, or a projected selector is missing
or duplicated in the rendered SVG. Existing enrichment payloads without this
optional additive view remain valid a0 documents.
