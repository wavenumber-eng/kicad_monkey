/** Generated from KiCad Monkey TypeSpec JSON Schema. Do not edit. */

export type TerminalRole = "component_pin" | "sheet_entry" | "port" | "power_port";
export type ResolutionDiagnostic =
  | "logical_pin_unresolved"
  | "component_occurrence_unresolved"
  | "hierarchy_terminal_binding_unresolved"
  | "design_net_unresolved";
export type GraphicalTargetType =
  | "sch.component_occurrence"
  | "sch.hierarchy_occurrence"
  | "sch.terminal_occurrence"
  | "sch.local_net_occurrence"
  | "sch.page_occurrence";
/**
 * Canonical decimal wire encoding for an unsigned 64-bit byte count.
 */
export type CanonicalUint64Decimal = string;

/**
 * Strict bounded a1 result with source identity and netlist byte integrity.
 */
export interface NativeDesignFactsResultA1 {
  type: "kicad_monkey.native.design_facts.result";
  version: "a1";
  engine_version: string;
  resource_profile: "design-facts-bounded-a1";
  source_snapshot_sha256: string;
  compiled_schematic_graph: CompiledSchematicGraphA0;
  kicad_netlist_version: "E";
  kicad_netlist: string;
  kicad_netlist_bytes: CanonicalUint64Decimal;
  kicad_netlist_sha256: string;
}
/**
 * Complete variant-neutral schematic occurrence and connectivity graph.
 */
export interface CompiledSchematicGraphA0 {
  schema: "kicad_monkey.compiled_schematic_graph.a0";
  type: "sch.compiled_schematic_graph";
  identity_namespace: "sch.compiled_schematic_graph.a0";
  unit_definitions: UnitDefinition[];
  page_definitions: PageDefinition[];
  unit_occurrences: UnitOccurrence[];
  page_occurrences: PageOccurrence[];
  hierarchy_occurrences: HierarchyOccurrence[];
  component_occurrences: ComponentOccurrence[];
  local_net_occurrences: LocalNetOccurrence[];
  terminal_occurrences: TerminalOccurrence[];
  hierarchy_terminal_bindings: HierarchyTerminalBinding[];
  graphical_artifact_links: GraphicalArtifactLink[];
}
export interface UnitDefinition {
  type: "sch.unit_definition";
  id: string;
  display_name: string;
  page_definition_refs: string[];
  source_identity: SourceIdentity;
}
/**
 * Registered producer provenance retained for importer replay and diagnostics.
 */
export interface SourceIdentity {
  "sch.source_key.source_uuid"?: string;
  "sch.source_key.source_path"?: string;
  "sch.source_key.source_record"?: string;
  "sch.source_key.source_subobject"?: string;
  "sch.source_key.compiled_net"?: string;
  "sch.source_key.artifact_element"?: string;
}
export interface PageDefinition {
  type: "sch.page_definition";
  id: string;
  display_name: string;
  unit_definition_ref: string;
  source_identity: SourceIdentity;
}
export interface UnitOccurrence {
  type: "sch.unit_occurrence";
  id: string;
  display_name: string;
  unit_definition_ref: string;
  page_occurrence_refs: string[];
  parent_hierarchy_occurrence_ref?: string;
  source_identity: SourceIdentity;
}
export interface PageOccurrence {
  type: "sch.page_occurrence";
  id: string;
  display_name: string;
  page_definition_ref: string;
  unit_occurrence_ref: string;
  address_key?: string;
  sheet_number?: string;
  instance_order: number;
  source_identity: SourceIdentity;
}
export interface HierarchyOccurrence {
  type: "sch.hierarchy_occurrence";
  id: string;
  parent_unit_occurrence_ref: string;
  parent_page_occurrence_ref: string;
  child_unit_occurrence_ref: string;
  source_identity: SourceIdentity;
}
export interface ComponentOccurrence {
  type: "sch.component_occurrence";
  id: string;
  page_occurrence_ref: string;
  design_component_ref?: string;
  source_designator: string;
  physical_designator: string;
  display_designator: string;
  unit: number;
  body_style: number;
  source_identity: SourceIdentity;
}
export interface LocalNetOccurrence {
  type: "sch.local_net_occurrence";
  id: string;
  page_occurrence_ref: string;
  display_name: string;
  design_net_ref?: string;
  qualified_name?: string;
  aliases: string[];
  source_identity: SourceIdentity;
}
export interface TerminalOccurrence {
  type: "sch.terminal_occurrence";
  id: string;
  page_occurrence_ref: string;
  role: TerminalRole;
  local_net_occurrence_ref?: string;
  design_net_ref?: string;
  component_occurrence_ref?: string;
  design_component_pin_ref?: string;
  name: string;
  pin_designator: string;
  resolution_diagnostics?: ResolutionDiagnostic[];
  source_identity: SourceIdentity;
}
export interface HierarchyTerminalBinding {
  type: "sch.hierarchy_terminal_binding";
  id: string;
  hierarchy_occurrence_ref: string;
  parent_terminal_occurrence_ref: string;
  child_terminal_occurrence_ref: string;
  design_net_ref?: string;
  source_identity: SourceIdentity;
}
export interface GraphicalArtifactLink {
  type: "sch.graphical_artifact_link";
  id: string;
  page_occurrence_ref: string;
  target_type: GraphicalTargetType;
  target_ref: string;
  artifact_key: "sch.dwg_scene";
  element_id: string;
  source_identity: SourceIdentity;
}
