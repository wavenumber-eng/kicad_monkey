/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Resource-bounded board plotter operation. Source bytes are out of band.
 */
export interface BoardPlotRequestA0 {
  type: "kicad_monkey.board_plot.request";
  version: "a0";
  source_path?: string;
  document_id?: string;
  max_source_bytes: string;
  max_output_bytes: string;
  max_depth: number;
  max_graphics: number;
  max_operations: number;
  max_points: number;
  max_text_bytes: string;
  max_parse_nodes: number;
  max_input_points: number;
  max_input_polygons: number;
  max_cache_polygons: number;
  max_cache_contours: number;
  net_class_assignments?: BoardNetClassAssignment[];
  text_variables?: BoardTextVariable[];
}
/**
 * One exact net-name to ordered net-class assignment mirrored from the
 * project sidecar's `net_settings.netclass_assignments` entries.
 */
export interface BoardNetClassAssignment {
  net_name: string;
  classes: string[];
}
/**
 * One project-sidecar text variable. The producer case-expands names to
 * original/lower/upper aliases and overlays board `(property ...)` values,
 * matching the established `board_text_variables` merge.
 */
export interface BoardTextVariable {
  name: string;
  value: string;
}
