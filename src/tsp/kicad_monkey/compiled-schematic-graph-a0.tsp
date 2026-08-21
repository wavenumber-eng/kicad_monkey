import "@typespec/json-schema";

using TypeSpec.JsonSchema;

namespace Wavenumber.KicadMonkey.Contracts.CompiledSchematicGraphA0;

/** Registered producer provenance retained for importer replay and diagnostics. */
model SourceIdentity {
  `sch.source_key.source_uuid`?: string;
  `sch.source_key.source_path`?: string;
  `sch.source_key.source_record`?: string;
  `sch.source_key.source_subobject`?: string;
  `sch.source_key.compiled_net`?: string;
  `sch.source_key.artifact_element`?: string;
}

enum TerminalRole {
  component_pin,
  sheet_entry,
  port,
  power_port,
}

enum ResolutionDiagnostic {
  logical_pin_unresolved,
  component_occurrence_unresolved,
  hierarchy_terminal_binding_unresolved,
  design_net_unresolved,
}

enum GraphicalTargetType {
  `sch.component_occurrence`,
  `sch.hierarchy_occurrence`,
  `sch.terminal_occurrence`,
  `sch.local_net_occurrence`,
  `sch.page_occurrence`,
}

model UnitDefinition {
  type: "sch.unit_definition";
  id: string;
  display_name: string;
  page_definition_refs: string[];
  source_identity: SourceIdentity;
}

model PageDefinition {
  type: "sch.page_definition";
  id: string;
  display_name: string;
  unit_definition_ref: string;
  source_identity: SourceIdentity;
}

model UnitOccurrence {
  type: "sch.unit_occurrence";
  id: string;
  display_name: string;
  unit_definition_ref: string;
  page_occurrence_refs: string[];
  parent_hierarchy_occurrence_ref?: string;
  source_identity: SourceIdentity;
}

model PageOccurrence {
  type: "sch.page_occurrence";
  id: string;
  display_name: string;
  page_definition_ref: string;
  unit_occurrence_ref: string;
  address_key?: string;
  sheet_number?: string;
  instance_order: uint32;
  source_identity: SourceIdentity;
}

model HierarchyOccurrence {
  type: "sch.hierarchy_occurrence";
  id: string;
  parent_unit_occurrence_ref: string;
  parent_page_occurrence_ref: string;
  child_unit_occurrence_ref: string;
  source_identity: SourceIdentity;
}

model ComponentOccurrence {
  type: "sch.component_occurrence";
  id: string;
  page_occurrence_ref: string;
  design_component_ref?: string;
  source_designator: string;
  physical_designator: string;
  display_designator: string;
  @minValue(1)
  unit: uint32;
  body_style: uint32;
  source_identity: SourceIdentity;
}

model LocalNetOccurrence {
  type: "sch.local_net_occurrence";
  id: string;
  page_occurrence_ref: string;
  display_name: string;
  design_net_ref?: string;
  qualified_name?: string;
  aliases: string[];
  source_identity: SourceIdentity;
}

model TerminalOccurrence {
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

model HierarchyTerminalBinding {
  type: "sch.hierarchy_terminal_binding";
  id: string;
  hierarchy_occurrence_ref: string;
  parent_terminal_occurrence_ref: string;
  child_terminal_occurrence_ref: string;
  design_net_ref?: string;
  source_identity: SourceIdentity;
}

model GraphicalArtifactLink {
  type: "sch.graphical_artifact_link";
  id: string;
  page_occurrence_ref: string;
  target_type: GraphicalTargetType;
  target_ref: string;
  artifact_key: "sch.dwg_scene";
  element_id: string;
  source_identity: SourceIdentity;
}

/** Complete variant-neutral schematic occurrence and connectivity graph. */
@jsonSchema
@id("urn:wavenumber:schema:kicad_monkey.compiled_schematic_graph:a0")
@extension("title", "Compiled schematic graph a0")
model CompiledSchematicGraph {
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
