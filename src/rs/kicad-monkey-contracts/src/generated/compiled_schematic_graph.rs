/// Error types.
pub mod error {
    /// Error from a `TryFrom` or `FromStr` implementation.
    pub struct ConversionError(::std::borrow::Cow<'static, str>);
    impl ::std::error::Error for ConversionError {}
    impl ::std::fmt::Display for ConversionError {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Display::fmt(&self.0, f)
        }
    }
    impl ::std::fmt::Debug for ConversionError {
        fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> Result<(), ::std::fmt::Error> {
            ::std::fmt::Debug::fmt(&self.0, f)
        }
    }
    impl From<&'static str> for ConversionError {
        fn from(value: &'static str) -> Self {
            Self(value.into())
        }
    }
    impl From<String> for ConversionError {
        fn from(value: String) -> Self {
            Self(value.into())
        }
    }
}
///Complete variant-neutral schematic occurrence and connectivity graph.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.compiled_schematic_graph:a0",
///  "title": "Compiled schematic graph a0",
///  "description": "Complete variant-neutral schematic occurrence and connectivity graph.",
///  "type": "object",
///  "required": [
///    "component_occurrences",
///    "graphical_artifact_links",
///    "hierarchy_occurrences",
///    "hierarchy_terminal_bindings",
///    "identity_namespace",
///    "local_net_occurrences",
///    "page_definitions",
///    "page_occurrences",
///    "schema",
///    "terminal_occurrences",
///    "type",
///    "unit_definitions",
///    "unit_occurrences"
///  ],
///  "properties": {
///    "component_occurrences": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/ComponentOccurrence"
///      }
///    },
///    "graphical_artifact_links": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/GraphicalArtifactLink"
///      }
///    },
///    "hierarchy_occurrences": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/HierarchyOccurrence"
///      }
///    },
///    "hierarchy_terminal_bindings": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/HierarchyTerminalBinding"
///      }
///    },
///    "identity_namespace": {
///      "type": "string",
///      "const": "sch.compiled_schematic_graph.a0"
///    },
///    "local_net_occurrences": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/LocalNetOccurrence"
///      }
///    },
///    "page_definitions": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/PageDefinition"
///      }
///    },
///    "page_occurrences": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/PageOccurrence"
///      }
///    },
///    "schema": {
///      "type": "string",
///      "const": "kicad_monkey.compiled_schematic_graph.a0"
///    },
///    "terminal_occurrences": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/TerminalOccurrence"
///      }
///    },
///    "type": {
///      "type": "string",
///      "const": "sch.compiled_schematic_graph"
///    },
///    "unit_definitions": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/UnitDefinition"
///      }
///    },
///    "unit_occurrences": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/UnitOccurrence"
///      }
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct CompiledSchematicGraphA0 {
    pub component_occurrences: ::std::vec::Vec<ComponentOccurrence>,
    pub graphical_artifact_links: ::std::vec::Vec<GraphicalArtifactLink>,
    pub hierarchy_occurrences: ::std::vec::Vec<HierarchyOccurrence>,
    pub hierarchy_terminal_bindings: ::std::vec::Vec<HierarchyTerminalBinding>,
    pub identity_namespace: ::std::string::String,
    pub local_net_occurrences: ::std::vec::Vec<LocalNetOccurrence>,
    pub page_definitions: ::std::vec::Vec<PageDefinition>,
    pub page_occurrences: ::std::vec::Vec<PageOccurrence>,
    pub schema: ::std::string::String,
    pub terminal_occurrences: ::std::vec::Vec<TerminalOccurrence>,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub unit_definitions: ::std::vec::Vec<UnitDefinition>,
    pub unit_occurrences: ::std::vec::Vec<UnitOccurrence>,
}
///`ComponentOccurrence`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "body_style",
///    "display_designator",
///    "id",
///    "page_occurrence_ref",
///    "physical_designator",
///    "source_designator",
///    "source_identity",
///    "type",
///    "unit"
///  ],
///  "properties": {
///    "body_style": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "design_component_ref": {
///      "type": "string"
///    },
///    "display_designator": {
///      "type": "string"
///    },
///    "id": {
///      "type": "string"
///    },
///    "page_occurrence_ref": {
///      "type": "string"
///    },
///    "physical_designator": {
///      "type": "string"
///    },
///    "source_designator": {
///      "type": "string"
///    },
///    "source_identity": {
///      "$ref": "#/$defs/SourceIdentity"
///    },
///    "type": {
///      "type": "string",
///      "const": "sch.component_occurrence"
///    },
///    "unit": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 1.0
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct ComponentOccurrence {
    pub body_style: u32,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub design_component_ref: ::std::option::Option<::std::string::String>,
    pub display_designator: ::std::string::String,
    pub id: ::std::string::String,
    pub page_occurrence_ref: ::std::string::String,
    pub physical_designator: ::std::string::String,
    pub source_designator: ::std::string::String,
    pub source_identity: SourceIdentity,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub unit: ::std::num::NonZeroU64,
}
///`GraphicalArtifactLink`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "artifact_key",
///    "element_id",
///    "id",
///    "page_occurrence_ref",
///    "source_identity",
///    "target_ref",
///    "target_type",
///    "type"
///  ],
///  "properties": {
///    "artifact_key": {
///      "type": "string",
///      "const": "sch.dwg_scene"
///    },
///    "element_id": {
///      "type": "string"
///    },
///    "id": {
///      "type": "string"
///    },
///    "page_occurrence_ref": {
///      "type": "string"
///    },
///    "source_identity": {
///      "$ref": "#/$defs/SourceIdentity"
///    },
///    "target_ref": {
///      "type": "string"
///    },
///    "target_type": {
///      "$ref": "#/$defs/GraphicalTargetType"
///    },
///    "type": {
///      "type": "string",
///      "const": "sch.graphical_artifact_link"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct GraphicalArtifactLink {
    pub artifact_key: ::std::string::String,
    pub element_id: ::std::string::String,
    pub id: ::std::string::String,
    pub page_occurrence_ref: ::std::string::String,
    pub source_identity: SourceIdentity,
    pub target_ref: ::std::string::String,
    pub target_type: GraphicalTargetType,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
///`GraphicalTargetType`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "enum": [
///    "sch.component_occurrence",
///    "sch.hierarchy_occurrence",
///    "sch.terminal_occurrence",
///    "sch.local_net_occurrence",
///    "sch.page_occurrence"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum GraphicalTargetType {
    #[serde(rename = "sch.component_occurrence")]
    SchComponentOccurrence,
    #[serde(rename = "sch.hierarchy_occurrence")]
    SchHierarchyOccurrence,
    #[serde(rename = "sch.terminal_occurrence")]
    SchTerminalOccurrence,
    #[serde(rename = "sch.local_net_occurrence")]
    SchLocalNetOccurrence,
    #[serde(rename = "sch.page_occurrence")]
    SchPageOccurrence,
}
impl ::std::fmt::Display for GraphicalTargetType {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::SchComponentOccurrence => f.write_str("sch.component_occurrence"),
            Self::SchHierarchyOccurrence => f.write_str("sch.hierarchy_occurrence"),
            Self::SchTerminalOccurrence => f.write_str("sch.terminal_occurrence"),
            Self::SchLocalNetOccurrence => f.write_str("sch.local_net_occurrence"),
            Self::SchPageOccurrence => f.write_str("sch.page_occurrence"),
        }
    }
}
impl ::std::str::FromStr for GraphicalTargetType {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "sch.component_occurrence" => Ok(Self::SchComponentOccurrence),
            "sch.hierarchy_occurrence" => Ok(Self::SchHierarchyOccurrence),
            "sch.terminal_occurrence" => Ok(Self::SchTerminalOccurrence),
            "sch.local_net_occurrence" => Ok(Self::SchLocalNetOccurrence),
            "sch.page_occurrence" => Ok(Self::SchPageOccurrence),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for GraphicalTargetType {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for GraphicalTargetType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for GraphicalTargetType {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`HierarchyOccurrence`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "child_unit_occurrence_ref",
///    "id",
///    "parent_page_occurrence_ref",
///    "parent_unit_occurrence_ref",
///    "source_identity",
///    "type"
///  ],
///  "properties": {
///    "child_unit_occurrence_ref": {
///      "type": "string"
///    },
///    "id": {
///      "type": "string"
///    },
///    "parent_page_occurrence_ref": {
///      "type": "string"
///    },
///    "parent_unit_occurrence_ref": {
///      "type": "string"
///    },
///    "source_identity": {
///      "$ref": "#/$defs/SourceIdentity"
///    },
///    "type": {
///      "type": "string",
///      "const": "sch.hierarchy_occurrence"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct HierarchyOccurrence {
    pub child_unit_occurrence_ref: ::std::string::String,
    pub id: ::std::string::String,
    pub parent_page_occurrence_ref: ::std::string::String,
    pub parent_unit_occurrence_ref: ::std::string::String,
    pub source_identity: SourceIdentity,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
///`HierarchyTerminalBinding`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "child_terminal_occurrence_ref",
///    "hierarchy_occurrence_ref",
///    "id",
///    "parent_terminal_occurrence_ref",
///    "source_identity",
///    "type"
///  ],
///  "properties": {
///    "child_terminal_occurrence_ref": {
///      "type": "string"
///    },
///    "design_net_ref": {
///      "type": "string"
///    },
///    "hierarchy_occurrence_ref": {
///      "type": "string"
///    },
///    "id": {
///      "type": "string"
///    },
///    "parent_terminal_occurrence_ref": {
///      "type": "string"
///    },
///    "source_identity": {
///      "$ref": "#/$defs/SourceIdentity"
///    },
///    "type": {
///      "type": "string",
///      "const": "sch.hierarchy_terminal_binding"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct HierarchyTerminalBinding {
    pub child_terminal_occurrence_ref: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub design_net_ref: ::std::option::Option<::std::string::String>,
    pub hierarchy_occurrence_ref: ::std::string::String,
    pub id: ::std::string::String,
    pub parent_terminal_occurrence_ref: ::std::string::String,
    pub source_identity: SourceIdentity,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
///`LocalNetOccurrence`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "aliases",
///    "display_name",
///    "id",
///    "page_occurrence_ref",
///    "source_identity",
///    "type"
///  ],
///  "properties": {
///    "aliases": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "design_net_ref": {
///      "type": "string"
///    },
///    "display_name": {
///      "type": "string"
///    },
///    "id": {
///      "type": "string"
///    },
///    "page_occurrence_ref": {
///      "type": "string"
///    },
///    "qualified_name": {
///      "type": "string"
///    },
///    "source_identity": {
///      "$ref": "#/$defs/SourceIdentity"
///    },
///    "type": {
///      "type": "string",
///      "const": "sch.local_net_occurrence"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct LocalNetOccurrence {
    pub aliases: ::std::vec::Vec<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub design_net_ref: ::std::option::Option<::std::string::String>,
    pub display_name: ::std::string::String,
    pub id: ::std::string::String,
    pub page_occurrence_ref: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub qualified_name: ::std::option::Option<::std::string::String>,
    pub source_identity: SourceIdentity,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
///`PageDefinition`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "display_name",
///    "id",
///    "source_identity",
///    "type",
///    "unit_definition_ref"
///  ],
///  "properties": {
///    "display_name": {
///      "type": "string"
///    },
///    "id": {
///      "type": "string"
///    },
///    "source_identity": {
///      "$ref": "#/$defs/SourceIdentity"
///    },
///    "type": {
///      "type": "string",
///      "const": "sch.page_definition"
///    },
///    "unit_definition_ref": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PageDefinition {
    pub display_name: ::std::string::String,
    pub id: ::std::string::String,
    pub source_identity: SourceIdentity,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub unit_definition_ref: ::std::string::String,
}
///`PageOccurrence`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "display_name",
///    "id",
///    "instance_order",
///    "page_definition_ref",
///    "source_identity",
///    "type",
///    "unit_occurrence_ref"
///  ],
///  "properties": {
///    "address_key": {
///      "type": "string"
///    },
///    "display_name": {
///      "type": "string"
///    },
///    "id": {
///      "type": "string"
///    },
///    "instance_order": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "page_definition_ref": {
///      "type": "string"
///    },
///    "sheet_number": {
///      "type": "string"
///    },
///    "source_identity": {
///      "$ref": "#/$defs/SourceIdentity"
///    },
///    "type": {
///      "type": "string",
///      "const": "sch.page_occurrence"
///    },
///    "unit_occurrence_ref": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct PageOccurrence {
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub address_key: ::std::option::Option<::std::string::String>,
    pub display_name: ::std::string::String,
    pub id: ::std::string::String,
    pub instance_order: u32,
    pub page_definition_ref: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub sheet_number: ::std::option::Option<::std::string::String>,
    pub source_identity: SourceIdentity,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub unit_occurrence_ref: ::std::string::String,
}
///`ResolutionDiagnostic`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "enum": [
///    "logical_pin_unresolved",
///    "component_occurrence_unresolved",
///    "hierarchy_terminal_binding_unresolved",
///    "design_net_unresolved"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum ResolutionDiagnostic {
    #[serde(rename = "logical_pin_unresolved")]
    LogicalPinUnresolved,
    #[serde(rename = "component_occurrence_unresolved")]
    ComponentOccurrenceUnresolved,
    #[serde(rename = "hierarchy_terminal_binding_unresolved")]
    HierarchyTerminalBindingUnresolved,
    #[serde(rename = "design_net_unresolved")]
    DesignNetUnresolved,
}
impl ::std::fmt::Display for ResolutionDiagnostic {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::LogicalPinUnresolved => f.write_str("logical_pin_unresolved"),
            Self::ComponentOccurrenceUnresolved => f.write_str("component_occurrence_unresolved"),
            Self::HierarchyTerminalBindingUnresolved => {
                f.write_str("hierarchy_terminal_binding_unresolved")
            }
            Self::DesignNetUnresolved => f.write_str("design_net_unresolved"),
        }
    }
}
impl ::std::str::FromStr for ResolutionDiagnostic {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "logical_pin_unresolved" => Ok(Self::LogicalPinUnresolved),
            "component_occurrence_unresolved" => Ok(Self::ComponentOccurrenceUnresolved),
            "hierarchy_terminal_binding_unresolved" => Ok(Self::HierarchyTerminalBindingUnresolved),
            "design_net_unresolved" => Ok(Self::DesignNetUnresolved),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for ResolutionDiagnostic {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for ResolutionDiagnostic {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for ResolutionDiagnostic {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Registered producer provenance retained for importer replay and diagnostics.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Registered producer provenance retained for importer replay and diagnostics.",
///  "type": "object",
///  "properties": {
///    "sch.source_key.artifact_element": {
///      "type": "string"
///    },
///    "sch.source_key.compiled_net": {
///      "type": "string"
///    },
///    "sch.source_key.source_path": {
///      "type": "string"
///    },
///    "sch.source_key.source_record": {
///      "type": "string"
///    },
///    "sch.source_key.source_subobject": {
///      "type": "string"
///    },
///    "sch.source_key.source_uuid": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct SourceIdentity {
    #[serde(
        rename = "sch.source_key.artifact_element",
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub sch_source_key_artifact_element: ::std::option::Option<::std::string::String>,
    #[serde(
        rename = "sch.source_key.compiled_net",
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub sch_source_key_compiled_net: ::std::option::Option<::std::string::String>,
    #[serde(
        rename = "sch.source_key.source_path",
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub sch_source_key_source_path: ::std::option::Option<::std::string::String>,
    #[serde(
        rename = "sch.source_key.source_record",
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub sch_source_key_source_record: ::std::option::Option<::std::string::String>,
    #[serde(
        rename = "sch.source_key.source_subobject",
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub sch_source_key_source_subobject: ::std::option::Option<::std::string::String>,
    #[serde(
        rename = "sch.source_key.source_uuid",
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub sch_source_key_source_uuid: ::std::option::Option<::std::string::String>,
}
impl ::std::default::Default for SourceIdentity {
    fn default() -> Self {
        Self {
            sch_source_key_artifact_element: Default::default(),
            sch_source_key_compiled_net: Default::default(),
            sch_source_key_source_path: Default::default(),
            sch_source_key_source_record: Default::default(),
            sch_source_key_source_subobject: Default::default(),
            sch_source_key_source_uuid: Default::default(),
        }
    }
}
///`TerminalOccurrence`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "id",
///    "name",
///    "page_occurrence_ref",
///    "pin_designator",
///    "role",
///    "source_identity",
///    "type"
///  ],
///  "properties": {
///    "component_occurrence_ref": {
///      "type": "string"
///    },
///    "design_component_pin_ref": {
///      "type": "string"
///    },
///    "design_net_ref": {
///      "type": "string"
///    },
///    "id": {
///      "type": "string"
///    },
///    "local_net_occurrence_ref": {
///      "type": "string"
///    },
///    "name": {
///      "type": "string"
///    },
///    "page_occurrence_ref": {
///      "type": "string"
///    },
///    "pin_designator": {
///      "type": "string"
///    },
///    "resolution_diagnostics": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/ResolutionDiagnostic"
///      }
///    },
///    "role": {
///      "$ref": "#/$defs/TerminalRole"
///    },
///    "source_identity": {
///      "$ref": "#/$defs/SourceIdentity"
///    },
///    "type": {
///      "type": "string",
///      "const": "sch.terminal_occurrence"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct TerminalOccurrence {
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub component_occurrence_ref: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub design_component_pin_ref: ::std::option::Option<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub design_net_ref: ::std::option::Option<::std::string::String>,
    pub id: ::std::string::String,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub local_net_occurrence_ref: ::std::option::Option<::std::string::String>,
    pub name: ::std::string::String,
    pub page_occurrence_ref: ::std::string::String,
    pub pin_designator: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub resolution_diagnostics: ::std::vec::Vec<ResolutionDiagnostic>,
    pub role: TerminalRole,
    pub source_identity: SourceIdentity,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
///`TerminalRole`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "string",
///  "enum": [
///    "component_pin",
///    "sheet_entry",
///    "port",
///    "power_port"
///  ]
///}
/// ```
/// </details>
#[derive(
    ::serde::Deserialize,
    ::serde::Serialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum TerminalRole {
    #[serde(rename = "component_pin")]
    ComponentPin,
    #[serde(rename = "sheet_entry")]
    SheetEntry,
    #[serde(rename = "port")]
    Port,
    #[serde(rename = "power_port")]
    PowerPort,
}
impl ::std::fmt::Display for TerminalRole {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::ComponentPin => f.write_str("component_pin"),
            Self::SheetEntry => f.write_str("sheet_entry"),
            Self::Port => f.write_str("port"),
            Self::PowerPort => f.write_str("power_port"),
        }
    }
}
impl ::std::str::FromStr for TerminalRole {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "component_pin" => Ok(Self::ComponentPin),
            "sheet_entry" => Ok(Self::SheetEntry),
            "port" => Ok(Self::Port),
            "power_port" => Ok(Self::PowerPort),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for TerminalRole {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for TerminalRole {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for TerminalRole {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///`UnitDefinition`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "display_name",
///    "id",
///    "page_definition_refs",
///    "source_identity",
///    "type"
///  ],
///  "properties": {
///    "display_name": {
///      "type": "string"
///    },
///    "id": {
///      "type": "string"
///    },
///    "page_definition_refs": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "source_identity": {
///      "$ref": "#/$defs/SourceIdentity"
///    },
///    "type": {
///      "type": "string",
///      "const": "sch.unit_definition"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct UnitDefinition {
    pub display_name: ::std::string::String,
    pub id: ::std::string::String,
    pub page_definition_refs: ::std::vec::Vec<::std::string::String>,
    pub source_identity: SourceIdentity,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
}
///`UnitOccurrence`
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "type": "object",
///  "required": [
///    "display_name",
///    "id",
///    "page_occurrence_refs",
///    "source_identity",
///    "type",
///    "unit_definition_ref"
///  ],
///  "properties": {
///    "display_name": {
///      "type": "string"
///    },
///    "id": {
///      "type": "string"
///    },
///    "page_occurrence_refs": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "parent_hierarchy_occurrence_ref": {
///      "type": "string"
///    },
///    "source_identity": {
///      "$ref": "#/$defs/SourceIdentity"
///    },
///    "type": {
///      "type": "string",
///      "const": "sch.unit_occurrence"
///    },
///    "unit_definition_ref": {
///      "type": "string"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct UnitOccurrence {
    pub display_name: ::std::string::String,
    pub id: ::std::string::String,
    pub page_occurrence_refs: ::std::vec::Vec<::std::string::String>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub parent_hierarchy_occurrence_ref: ::std::option::Option<::std::string::String>,
    pub source_identity: SourceIdentity,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub unit_definition_ref: ::std::string::String,
}
