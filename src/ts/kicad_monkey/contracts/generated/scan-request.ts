/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

/**
 * Metadata envelope for a byte-buffer structural scan operation.
 */
export interface SExpressionScanRequestA0 {
  type: "kicad_monkey.sexpr_scan.request";
  version: "a0";
  selector: Selector;
  max_source_bytes: string;
  max_depth: number;
  max_selected_forms: number;
}
/**
 * Generic structural selector; source bytes are always supplied out of band.
 */
export interface Selector {
  heads?: string[];
  paths?: string[][];
  min_depth?: number;
  max_depth?: number;
  prune_heads?: string[];
}
