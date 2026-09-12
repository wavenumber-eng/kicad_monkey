/* Generated from Cruncher TypeSpec. Do not edit. */

export type RotationDirection =
  | "cw"
  | "clockwise"
  | "right"
  | Positive90
  | Ninety
  | "ccw"
  | "counterclockwise"
  | CounterClockwiseHyphenated
  | "left"
  | Negative90;
export type Positive90 = "+90";
export type Ninety = "90";
export type CounterClockwiseHyphenated = "counter-clockwise";
export type Negative90 = "-90";

export interface PcbSvgConfig {
  schema: "kicad_cruncher.pcb_svg.config.a0";
  global: PcbSvgConfigGlobal;
  assembly?: PcbSvgConfigAssembly;
  dnp?: PcbSvgConfigDnp;
  diodes?: PcbSvgConfigDiodes;
  pin1?: PcbSvgConfigPin1;
  components?: PcbSvgConfigComponents;
  layer_outputs: PcbSvgConfigLayerOutputs;
  views: PcbSvgConfigViewsItem[];
  [k: string]: unknown;
}
export interface PcbSvgConfigGlobal {
  pcbdoc?: string | null;
  canvas?: PcbSvgConfigGlobalCanvas;
  include_metadata?: boolean;
  show_empty_layers?: boolean;
  clip_to_outline?: boolean;
  clip_holes_from_copper?: boolean;
  mirror_bottom_view?: boolean;
  svg_scale?: number;
  svg_size_unit?: string;
  clean_output?: boolean;
  styles?: StyleTable;
  [k: string]: unknown;
}
export interface PcbSvgConfigGlobalCanvas {
  bounds?: "board_outline" | "all_geometry";
  margin_mm?: number;
  [k: string]: unknown;
}
export interface StyleTable {
  board_outline?: StyleTableBoardOutline;
  board_cutouts?: StyleTableBoardCutouts;
  drills?: HoleStyle;
  slots?: HoleStyle;
  pin1_marker?: StyleTablePin1Marker;
  assembly_designators?: StyleTableAssemblyDesignators;
  assembly_hlr?: StyleTableAssemblyHlr;
  [k: string]: unknown;
}
export interface StyleTableBoardOutline {
  enabled?: boolean;
  color?: string;
  line_width_mm?: number;
  max_arc_segment_mm?: number;
  max_curve_segment_mm?: number;
  max_circle_segment_mm?: number;
  min_arc_segments?: number;
  min_curve_segments?: number;
  min_circle_segments?: number;
  max_arc_segments?: number;
  max_curve_segments?: number;
  max_circle_segments?: number;
  [k: string]: unknown;
}
export interface StyleTableBoardCutouts {
  enabled?: boolean;
  color?: string;
  hatch?: boolean;
  hatch_spacing_mm?: number;
  hatch_angle_deg?: number;
  hatch_line_width_mm?: number;
  outline_style?: "solid" | "dashed";
  outline_dash_mm?: number;
  outline_width_mm?: number;
  [k: string]: unknown;
}
export interface HoleStyle {
  enabled?: boolean;
  plated_color?: string;
  non_plated_color?: string;
  opacity?: number;
  [k: string]: unknown;
}
export interface StyleTablePin1Marker {
  enabled?: boolean;
  color?: string;
  dot_diameter_mm?: number;
  pad_diameter_ratio?: number;
  min_dot_diameter_mm?: number;
  max_dot_diameter_mm?: number;
  [k: string]: unknown;
}
export interface StyleTableAssemblyDesignators {
  enabled?: boolean;
  color?: string;
  font_family?: string;
  font_weight?: string | number;
  box_fill_ratio?: number;
  min_font_size_mm?: number;
  max_font_size_mm?: number;
  rotation_aspect_threshold?: number;
  rotation_direction?: RotationDirection;
  selector_overrides?: StyleTableAssemblyDesignatorsSelectorOverrides;
  opacity?: number;
  [k: string]: unknown;
}
export interface StyleTableAssemblyDesignatorsSelectorOverrides {
  [k: string]: unknown;
}
export interface StyleTableAssemblyHlr {
  enabled?: boolean;
  color?: string;
  line_width_mm?: number;
  opacity?: number;
  [k: string]: unknown;
}
export interface PcbSvgConfigAssembly {
  default_projection?: "detail" | "outline" | "bounding_box" | "model_bounds" | "pad_bounds" | "none";
  dnp_projection?: "detail" | "outline" | "bounding_box" | "model_bounds" | "pad_bounds" | "none";
  designator_color?: string;
  dnp_designator_color?: string;
  [k: string]: unknown;
}
export interface PcbSvgConfigDnp {
  color?: string;
  hatch?: boolean;
  hatch_spacing_mm?: number;
  hatch_angle_deg?: number;
  hatch_line_width_mm?: number;
  [k: string]: unknown;
}
export interface PcbSvgConfigDiodes {
  enabled?: boolean;
  line_art?: boolean;
  marker_color?: string;
  numeric_cathode_pad?: string;
  cathode_pad_names?: string[];
  designator_prefixes?: string[];
  parameter_terms?: string[];
  [k: string]: unknown;
}
export interface PcbSvgConfigPin1 {
  exclude_designators?: string | string[];
  exclude_designator_prefixes?: string[];
  exclude_single_pin?: boolean;
  [k: string]: unknown;
}
export interface PcbSvgConfigComponents {
  [k: string]: unknown;
}
export interface PcbSvgConfigLayerOutputs {
  enabled?: boolean;
  layers?: "auto" | string[];
  include_special_layers?: (
    | "BOARD_OUTLINE"
    | "BOARD_CUTOUTS"
    | "DRILLS"
    | "SLOTS"
    | "ASSEMBLY_HLR_TOP"
    | "ASSEMBLY_HLR_BOTTOM"
    | "ASSEMBLY_DESIGNATORS_TOP"
    | "ASSEMBLY_DESIGNATORS_BOTTOM"
    | "PIN1_TOP"
    | "PIN1_BOTTOM"
    | "ASSEMBLY_HLR_TOP_OUTLINE"
    | "ASSEMBLY_HLR_BOTTOM_OUTLINE"
    | "ASSEMBLY_HLR_TOP_DETAIL"
    | "ASSEMBLY_HLR_BOTTOM_DETAIL"
    | "ASSEMBLY_BOUNDS_TOP_MODEL"
    | "ASSEMBLY_BOUNDS_BOTTOM_MODEL"
    | "ASSEMBLY_BOUNDS_TOP_PADS"
    | "ASSEMBLY_BOUNDS_BOTTOM_PADS"
  )[];
  /**
   * Include raw Edge.Cuts geometry as context in each non-Edge.Cuts physical layer output.
   */
  add_edge_cuts_to_physical_layers?: boolean;
  /**
   * Include computed round drill overlays as context in each non-Edge.Cuts physical layer output.
   */
  add_drills_to_physical_layers?: boolean;
  /**
   * Include computed slot overlays as context in each non-Edge.Cuts physical layer output.
   */
  add_slots_to_physical_layers?: boolean;
  /**
   * Write standalone __virtual__ layer SVG files selected by include_special_layers and synthetic layer tokens.
   */
  write_virtual_layers?: boolean;
  output_dir?: string;
  [k: string]: unknown;
}
export interface PcbSvgConfigViewsItem {
  name: string;
  enabled?: boolean;
  group_id?: string;
  output_svg?: string;
  layers: string[];
  mirror?: boolean;
  assembly_hlr_mode?: "outline" | "detail" | "bounding_box" | "model_bounds" | "pad_bounds" | "none";
  styles?: StyleTable;
  pin1?: Pin1;
  description?: string;
  [k: string]: unknown;
}
export interface Pin1 {
  exclude_designators?: string | string[];
  exclude_designator_prefixes?: string[];
  exclude_single_pin?: boolean;
  [k: string]: unknown;
}
