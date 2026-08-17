/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Selection of the drawing-sheet byte sidecar supplied out of band.
 */
export type SchematicWorksheetMode = "default" | "provided";
/**
 * Finite nonnegative project text-offset ratio used by schematic plotting.
 */
export type SchematicTextOffsetRatio = number;
/**
 * Effective schematic plot width after KiCad's minimum-pen clamp.
 */
export type SchematicDefaultLineWidthNm = number;

/**
 * Resource-bounded schematic plot operation. Source bytes are out of band.
 */
export interface SchematicPlotRequestA0 {
  type: "kicad_monkey.schematic_plot.request";
  version: "a0";
  source_path?: string;
  document_id?: string;
  sheet_index: number;
  sheet_count: number;
  sheet_path: string;
  sheet_name: string;
  worksheet_mode: SchematicWorksheetMode;
  text_variables?: SchematicTextVariable[];
  text_offset_ratio: SchematicTextOffsetRatio;
  default_line_width_nm: SchematicDefaultLineWidthNm;
  max_source_bytes: string;
  max_worksheet_bytes: string;
  max_output_bytes: string;
  max_depth: number;
  max_parse_nodes: number;
  max_selected_forms: number;
  max_records: number;
  max_operations: number;
  max_points: number;
  max_input_points: number;
  max_text_bytes: string;
  max_metadata_bytes: string;
  max_wires: number;
  max_buses: number;
  max_bus_entries: number;
  max_junctions: number;
  max_no_connects: number;
  max_labels: number;
  max_global_labels: number;
  max_hierarchical_labels: number;
  max_netclass_flags: number;
  max_netclass_flag_properties: number;
  max_texts: number;
  max_text_boxes: number;
  max_text_box_lines: number;
  max_text_variables: number;
  max_text_variable_bytes: string;
  max_worksheet_items: number;
  max_worksheet_repeats: number;
  max_worksheet_point_sets: number;
  max_worksheet_points: number;
  max_worksheet_bitmap_data_parts: number;
  max_worksheet_bitmap_encoded_bytes: string;
  max_worksheet_bitmap_decoded_bytes: string;
  max_worksheet_bitmap_width_px: number;
  max_worksheet_bitmap_height_px: number;
  max_worksheet_bitmap_pixels: string;
  max_worksheet_bitmap_decode_work: string;
}
/**
 * One exact-case project text variable supplied by the caller.
 */
export interface SchematicTextVariable {
  name: string;
  value: string;
}
