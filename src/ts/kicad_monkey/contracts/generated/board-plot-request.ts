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
}
