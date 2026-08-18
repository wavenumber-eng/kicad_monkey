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
///Strict result for the native compiled-graph and version-E netlist operation.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.native.design_facts.result:a0",
///  "title": "Native design facts result a0",
///  "description": "Strict result for the native compiled-graph and version-E netlist operation.",
///  "type": "object",
///  "required": [
///    "compiled_schematic_graph",
///    "engine_version",
///    "kicad_netlist",
///    "kicad_netlist_version",
///    "type",
///    "version"
///  ],
///  "properties": {
///    "compiled_schematic_graph": {
///      "$ref": "#/$defs/NativeCompiledSchematicGraphProjection"
///    },
///    "engine_version": {
///      "type": "string"
///    },
///    "kicad_netlist": {
///      "type": "string"
///    },
///    "kicad_netlist_version": {
///      "type": "string",
///      "const": "E"
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.native.design_facts.result"
///    },
///    "version": {
///      "type": "string",
///      "const": "a0"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NativeDesignFactsResultA0 {
    pub compiled_schematic_graph:
        crate::generated::compiled_schematic_graph::CompiledSchematicGraphA0,
    pub engine_version: ::std::string::String,
    pub kicad_netlist: ::std::string::String,
    pub kicad_netlist_version: ::std::string::String,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
}
