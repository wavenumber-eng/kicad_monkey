/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

export type NodeKind = "list" | "atom" | "quoted" | "integer" | "float";

/**
 * Deterministic generic-tree build request.
 */
export interface SExpressionBuildRequestA0 {
  type: "kicad_monkey.sexpr_build.request";
  version: "a0";
  root: Node;
  max_output_bytes: string;
  max_depth: number;
  max_nodes: number;
}
/**
 * Portable generic S-expression tree node used only by explicit build operations.
 */
export interface Node {
  kind: NodeKind;
  text?: string;
  integer?: string;
  float?: number;
  children?: Node[];
}
