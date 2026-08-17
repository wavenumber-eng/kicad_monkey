/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Resource-bounded footprint plotter operation. Source bytes are out of band.
 */
export interface FootprintPlotRequestA0 {
  type: "kicad_monkey.footprint_plot.request";
  version: "a0";
  source_path?: string;
  document_id?: string;
  max_source_bytes: string;
  max_output_bytes: string;
  max_depth: number;
  max_metadata_forms: number;
  /**
   * Optional Phase 5 ceilings; older a0 requests receive bounded defaults.
   */
  max_text_carriers?: number;
  max_text_bytes?: string;
  max_operations: number;
  max_points: number;
}
