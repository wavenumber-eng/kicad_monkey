// PCB SVG config authority. Generate downstream contracts; edit this source.
import "@typespec/json-schema";
using TypeSpec.JsonSchema;
namespace KiCadCruncher;

alias SyntheticLayer = "BOARD_SUBSTRATE" | "BOARD_OUTLINE" | "BOARD_CUTOUTS" | "DRILLS" | "SLOTS" | "SOLDERMASK_FILM_TOP" | "SOLDERMASK_FILM_BOTTOM" | "ILLUSTRATION_TOP" | "ILLUSTRATION_BOTTOM" | "ASSEMBLY_HLR_TOP" | "ASSEMBLY_HLR_BOTTOM" | "ASSEMBLY_DESIGNATORS_TOP" | "ASSEMBLY_DESIGNATORS_BOTTOM" | "PIN1_TOP" | "PIN1_BOTTOM" | "ASSEMBLY_HLR_TOP_OUTLINE" | "ASSEMBLY_HLR_BOTTOM_OUTLINE" | "ASSEMBLY_HLR_TOP_DETAIL" | "ASSEMBLY_HLR_BOTTOM_DETAIL" | "ASSEMBLY_BOUNDS_TOP_MODEL" | "ASSEMBLY_BOUNDS_BOTTOM_MODEL" | "ASSEMBLY_BOUNDS_TOP_PADS" | "ASSEMBLY_BOUNDS_BOTTOM_PADS";

alias ProjectionMode = "detail" | "outline" | "bounding_box" | "model_bounds" | "pad_bounds" | "none";

model ComponentOverride {
  "side"?: "top" | "bottom";
  "projection"?: ProjectionMode;
  "assembly_hlr"?: ComponentOverrideAssemblyHlr;
  "assembly_designators"?: ComponentOverrideAssemblyDesignators;
  "pin1_enabled"?: boolean;
  "pin1_pad"?: string;
  "cathode_pad"?: string;
  "diode"?: boolean;
  "diode_line_art"?: boolean;
  "show_designator"?: boolean;
}

model ComponentOverrideAssemblyHlr {
  ...Record<unknown>;
}

model ComponentOverrideAssemblyDesignators {
  ...Record<unknown>;
}

model StyleTable {
  ...Record<StyleTableExtension>;
  "illustration"?: StyleTableIllustration;
  "board_substrate"?: StyleTableBoardSubstrate;
  "soldermask_film"?: StyleTableSoldermaskFilm;
  "board_outline"?: StyleTableBoardOutline;
  "board_cutouts"?: StyleTableBoardCutouts;
  "drills"?: HoleStyle;
  "slots"?: HoleStyle;
  "pin1_marker"?: StyleTablePin1Marker;
  "assembly_designators"?: StyleTableAssemblyDesignators;
  "assembly_hlr"?: StyleTableAssemblyHlr;
}

model StyleTableIllustration {
  ...Record<unknown>;
  "enabled"?: boolean;
  @minValue(0)
  @maxValue(1)
  "opacity"?: float64;
  @minValueExclusive(0)
  "outline_width_mm"?: float64;
  @minValueExclusive(0)
  "detail_width_mm"?: float64;
}

model StyleTableBoardSubstrate {
  ...Record<unknown>;
  "enabled"?: boolean;
  "color"?: string;
  @minValue(0)
  @maxValue(1)
  "opacity"?: float64;
}

model StyleTableSoldermaskFilm {
  ...Record<unknown>;
  "enabled"?: boolean;
  "color"?: string;
  @minValue(0)
  @maxValue(1)
  "opacity"?: float64;
}

model StyleTableExtension {
  ...Record<unknown>;
}

model StyleTableBoardOutline {
  ...Record<unknown>;
  "enabled"?: boolean;
  "color"?: string;
  @minValueExclusive(0)
  "line_width_mm"?: float64;
  @minValueExclusive(0)
  "max_arc_segment_mm"?: float64;
  @minValueExclusive(0)
  "max_curve_segment_mm"?: float64;
  @minValueExclusive(0)
  "max_circle_segment_mm"?: float64;
  @minValue(1)
  "min_arc_segments"?: integer;
  @minValue(1)
  "min_curve_segments"?: integer;
  @minValue(8)
  "min_circle_segments"?: integer;
  @minValue(8)
  "max_arc_segments"?: integer;
  @minValue(8)
  "max_curve_segments"?: integer;
  @minValue(8)
  "max_circle_segments"?: integer;
}

model StyleTableBoardCutouts {
  ...Record<unknown>;
  "enabled"?: boolean;
  "color"?: string;
  "hatch"?: boolean;
  @minValueExclusive(0)
  "hatch_spacing_mm"?: float64;
  "hatch_angle_deg"?: float64;
  @minValueExclusive(0)
  "hatch_line_width_mm"?: float64;
  "hatch_color"?: string;
  @minValue(0)
  @maxValue(1)
  "hatch_opacity"?: float64;
  "outline_style"?: "solid" | "dashed";
  @minValueExclusive(0)
  "outline_dash_mm"?: float64;
  @minValueExclusive(0)
  "outline_width_mm"?: float64;
  @minValue(0)
  @maxValue(1)
  "outline_opacity"?: float64;
}

model StyleTablePin1Marker {
  ...Record<unknown>;
  "enabled"?: boolean;
  "color"?: string;
  @minValueExclusive(0)
  "dot_diameter_mm"?: float64;
  @minValue(0)
  "pad_diameter_ratio"?: float64;
  @minValue(0)
  "min_dot_diameter_mm"?: float64;
  @minValue(0)
  "max_dot_diameter_mm"?: float64;
}

model StyleTableAssemblyDesignators {
  ...Record<unknown>;
  "enabled"?: boolean;
  "color"?: string;
  "font_family"?: string;
  "font_weight"?: string | float64;
  @minValueExclusive(0)
  @maxValue(1)
  "box_fill_ratio"?: float64;
  @minValue(0)
  "min_font_size_mm"?: float64;
  @minValue(0)
  "max_font_size_mm"?: float64;
  @minValueExclusive(0)
  "rotation_aspect_threshold"?: float64;
  "rotation_direction"?: RotationDirection;
  "selector_overrides"?: StyleTableAssemblyDesignatorsSelectorOverrides;
  @minValue(0)
  @maxValue(1)
  "opacity"?: float64;
}

model StyleTableAssemblyDesignatorsSelectorOverrides {
  ...Record<StyleTableAssemblyDesignatorsSelectorOverridesExtension>;
}

model StyleTableAssemblyDesignatorsSelectorOverridesExtension {
  ...Record<unknown>;
}

model StyleTableAssemblyHlr {
  ...Record<unknown>;
  "enabled"?: boolean;
  "color"?: string;
  @minValueExclusive(0)
  "line_width_mm"?: float64;
  @minValue(0)
  @maxValue(1)
  "opacity"?: float64;
}

model HoleStyle {
  ...Record<unknown>;
  "enabled"?: boolean;
  "plated_color"?: string;
  "non_plated_color"?: string;
  "outline"?: boolean;
  @minValueExclusive(0)
  "outline_width_mm"?: float64;
  "respect_tenting"?: boolean;
  @minValue(0)
  @maxValue(1)
  "opacity"?: float64;
}

model Pin1 {
  "exclude_designators"?: string | (string)[];
  "exclude_designator_prefixes"?: (string)[];
  "exclude_single_pin"?: boolean;
}

@jsonSchema
@extension("title", "PcbSvgConfig")
@id("pcb_svg_config.a0.schema.json")
model PcbSvgConfig {
  "schema": "kicad_cruncher.pcb_svg.config.a0";
  "global": PcbSvgConfigGlobal;
  "assembly"?: PcbSvgConfigAssembly;
  "dnp"?: PcbSvgConfigDnp;
  "diodes"?: PcbSvgConfigDiodes;
  "pin1"?: PcbSvgConfigPin1;
  "components"?: PcbSvgConfigComponents;
  "layer_outputs": PcbSvgConfigLayerOutputs;
  "views": (PcbSvgConfigViewsItem)[];
}

model PcbSvgConfigGlobal {
  ...Record<unknown>;
  "pcbdoc"?: string | null;
  "canvas"?: PcbSvgConfigGlobalCanvas;
  "include_metadata"?: boolean;
  "show_empty_layers"?: boolean;
  "clip_to_outline"?: boolean;
  "clip_holes_from_copper"?: boolean;
  "mirror_bottom_view"?: boolean;
  @minValueExclusive(0)
  "svg_scale"?: float64;
  "svg_size_unit"?: string;
  "clean_output"?: boolean;
  "styles"?: StyleTable;
}

model PcbSvgConfigGlobalCanvas {
  "bounds"?: "board_outline" | "all_geometry";
  @minValue(0)
  "margin_mm"?: float64;
}

model PcbSvgConfigAssembly {
  "default_projection"?: ProjectionMode;
  "dnp_projection"?: ProjectionMode;
  "designator_color"?: string;
  "dnp_designator_color"?: string;
}

model PcbSvgConfigDnp {
  "color"?: string;
  "hatch"?: boolean;
  @minValueExclusive(0)
  "hatch_spacing_mm"?: float64;
  "hatch_angle_deg"?: float64;
  @minValueExclusive(0)
  "hatch_line_width_mm"?: float64;
}

model PcbSvgConfigDiodes {
  "enabled"?: boolean;
  "line_art"?: boolean;
  "marker_color"?: string;
  "numeric_cathode_pad"?: string;
  "cathode_pad_names"?: (string)[];
  "designator_prefixes"?: (string)[];
  "parameter_terms"?: (string)[];
}

model PcbSvgConfigPin1 {
  "exclude_designators"?: string | (string)[];
  "exclude_designator_prefixes"?: (string)[];
  "exclude_single_pin"?: boolean;
}

model PcbSvgConfigComponents {
  ...Record<ComponentOverride>;
}

model PcbSvgConfigLayerOutputs {
  "enabled"?: boolean;
  "layers"?: "auto" | (string)[];
  "include_special_layers"?: (SyntheticLayer)[];
  @doc("Include raw Edge.Cuts geometry as context in each non-Edge.Cuts physical layer output.")
  "add_edge_cuts_to_physical_layers"?: boolean;
  @doc("Include computed round drill overlays as context in each non-Edge.Cuts physical layer output.")
  "add_drills_to_physical_layers"?: boolean;
  @doc("Include computed slot overlays as context in each non-Edge.Cuts physical layer output.")
  "add_slots_to_physical_layers"?: boolean;
  @doc("Write standalone __virtual__ layer SVG files selected by include_special_layers and synthetic layer tokens.")
  "write_virtual_layers"?: boolean;
  "output_dir"?: string;
}

model PcbSvgConfigViewsItem {
  @minLength(1)
  "name": string;
  "enabled"?: boolean;
  "group_id"?: string;
  "output_svg"?: string;
  "layers": (string)[];
  "mirror"?: boolean;
  "assembly_hlr_mode"?: "outline" | "detail" | "bounding_box" | "model_bounds" | "pad_bounds" | "none";
  "styles"?: StyleTable;
  "pin1"?: Pin1;
  "description"?: string;
}

union RotationDirection {
  cw: "cw",
  clockwise: "clockwise",
  right: "right",
  positive90: Positive90,
  ninety: Ninety,
  ccw: "ccw",
  counterclockwise: "counterclockwise",
  counterClockwiseHyphenated: CounterClockwiseHyphenated,
  left: "left",
  negative90: Negative90,
}
@extension("const", "+90") scalar Positive90 extends string;
@extension("const", "90") scalar Ninety extends string;
@extension("const", "-90") scalar Negative90 extends string;
@extension("const", "counter-clockwise") scalar CounterClockwiseHyphenated extends string;
