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
/**Selectable products. Every operation belongs to exactly one product. The
board profile has no product of its own: it is the add operation of every
dielectric and solder-mask slice, with cutouts subtracted there.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Selectable products. Every operation belongs to exactly one product. The\nboard profile has no product of its own: it is the add operation of every\ndielectric and solder-mask slice, with cutouts subtracted there.",
///  "type": "string",
///  "enum": [
///    "copper",
///    "holes",
///    "dielectric",
///    "solder_mask",
///    "paste",
///    "silkscreen",
///    "components"
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
pub enum MaterialProduct {
    #[serde(rename = "copper")]
    Copper,
    #[serde(rename = "holes")]
    Holes,
    #[serde(rename = "dielectric")]
    Dielectric,
    #[serde(rename = "solder_mask")]
    SolderMask,
    #[serde(rename = "paste")]
    Paste,
    #[serde(rename = "silkscreen")]
    Silkscreen,
    #[serde(rename = "components")]
    Components,
}
impl ::std::fmt::Display for MaterialProduct {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        match *self {
            Self::Copper => f.write_str("copper"),
            Self::Holes => f.write_str("holes"),
            Self::Dielectric => f.write_str("dielectric"),
            Self::SolderMask => f.write_str("solder_mask"),
            Self::Paste => f.write_str("paste"),
            Self::Silkscreen => f.write_str("silkscreen"),
            Self::Components => f.write_str("components"),
        }
    }
}
impl ::std::str::FromStr for MaterialProduct {
    type Err = self::error::ConversionError;
    fn from_str(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        match value {
            "copper" => Ok(Self::Copper),
            "holes" => Ok(Self::Holes),
            "dielectric" => Ok(Self::Dielectric),
            "solder_mask" => Ok(Self::SolderMask),
            "paste" => Ok(Self::Paste),
            "silkscreen" => Ok(Self::Silkscreen),
            "components" => Ok(Self::Components),
            _ => Err("invalid value".into()),
        }
    }
}
impl ::std::convert::TryFrom<&str> for MaterialProduct {
    type Error = self::error::ConversionError;
    fn try_from(value: &str) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<&::std::string::String> for MaterialProduct {
    type Error = self::error::ConversionError;
    fn try_from(
        value: &::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<::std::string::String> for MaterialProduct {
    type Error = self::error::ConversionError;
    fn try_from(
        value: ::std::string::String,
    ) -> ::std::result::Result<Self, self::error::ConversionError> {
        value.parse()
    }
}
///Resource-bounded materialization request. Source bytes are out of band.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "$id": "urn:wavenumber:schema:kicad_monkey.materialization.request:a0",
///  "title": "Materialization request a0",
///  "description": "Resource-bounded materialization request. Source bytes are out of band.",
///  "type": "object",
///  "required": [
///    "max_operations",
///    "max_points",
///    "max_source_bytes",
///    "products",
///    "tile_size_nm",
///    "type",
///    "version"
///  ],
///  "properties": {
///    "max_operations": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_points": {
///      "type": "integer",
///      "maximum": 4294967295.0,
///      "minimum": 0.0
///    },
///    "max_source_bytes": {
///      "description": "Bounds the board and project files together.",
///      "type": "string"
///    },
///    "net_keys": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "products": {
///      "type": "array",
///      "items": {
///        "$ref": "#/$defs/MaterialProduct"
///      }
///    },
///    "region": {
///      "$ref": "#/$defs/NmBounds"
///    },
///    "slice_keys": {
///      "type": "array",
///      "items": {
///        "type": "string"
///      }
///    },
///    "tile_size_nm": {
///      "$ref": "#/$defs/NonNegativeNm"
///    },
///    "type": {
///      "type": "string",
///      "const": "kicad_monkey.materialization.request"
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
pub struct MaterializationRequestA0 {
    pub max_operations: u32,
    pub max_points: u32,
    ///Bounds the board and project files together.
    pub max_source_bytes: ::std::string::String,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub net_keys: ::std::vec::Vec<::std::string::String>,
    pub products: ::std::vec::Vec<MaterialProduct>,
    #[serde(
        default,
        deserialize_with = "crate::deserialize_present_nonnull",
        skip_serializing_if = "::std::option::Option::is_none"
    )]
    pub region: ::std::option::Option<NmBounds>,
    #[serde(default, skip_serializing_if = "::std::vec::Vec::is_empty")]
    pub slice_keys: ::std::vec::Vec<::std::string::String>,
    pub tile_size_nm: NonNegativeNm,
    #[serde(rename = "type")]
    pub type_: ::std::string::String,
    pub version: ::std::string::String,
}
/**Board-frame length in integer nanometres. The range matches KiCad's own
32-bit internal-unit limit, so the packed encoding stores it losslessly.*/
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Board-frame length in integer nanometres. The range matches KiCad's own\n32-bit internal-unit limit, so the packed encoding stores it losslessly.",
///  "type": "integer",
///  "maximum": 2147483647.0,
///  "minimum": -2147483648.0
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct Nm(pub i32);
impl ::std::ops::Deref for Nm {
    type Target = i32;
    fn deref(&self) -> &i32 {
        &self.0
    }
}
impl ::std::convert::From<Nm> for i32 {
    fn from(value: Nm) -> Self {
        value.0
    }
}
impl ::std::convert::From<i32> for Nm {
    fn from(value: i32) -> Self {
        Self(value)
    }
}
impl ::std::str::FromStr for Nm {
    type Err = <i32 as ::std::str::FromStr>::Err;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.parse()?))
    }
}
impl ::std::convert::TryFrom<&str> for Nm {
    type Error = <i32 as ::std::str::FromStr>::Err;
    fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<String> for Nm {
    type Error = <i32 as ::std::str::FromStr>::Err;
    fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
impl ::std::fmt::Display for Nm {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
///Axis-aligned bounds in the board frame, inclusive.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Axis-aligned bounds in the board frame, inclusive.",
///  "type": "object",
///  "required": [
///    "max_x_nm",
///    "max_y_nm",
///    "min_x_nm",
///    "min_y_nm"
///  ],
///  "properties": {
///    "max_x_nm": {
///      "$ref": "#/$defs/Nm"
///    },
///    "max_y_nm": {
///      "$ref": "#/$defs/Nm"
///    },
///    "min_x_nm": {
///      "$ref": "#/$defs/Nm"
///    },
///    "min_y_nm": {
///      "$ref": "#/$defs/Nm"
///    }
///  },
///  "additionalProperties": false
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct NmBounds {
    pub max_x_nm: Nm,
    pub max_y_nm: Nm,
    pub min_x_nm: Nm,
    pub min_y_nm: Nm,
}
///Non-negative length in integer nanometres.
///
/// <details><summary>JSON schema</summary>
///
/// ```json
///{
///  "description": "Non-negative length in integer nanometres.",
///  "type": "integer",
///  "maximum": 2147483647.0,
///  "minimum": 0.0
///}
/// ```
/// </details>
#[derive(::serde::Deserialize, ::serde::Serialize, Clone, Debug)]
#[serde(transparent)]
pub struct NonNegativeNm(pub i64);
impl ::std::ops::Deref for NonNegativeNm {
    type Target = i64;
    fn deref(&self) -> &i64 {
        &self.0
    }
}
impl ::std::convert::From<NonNegativeNm> for i64 {
    fn from(value: NonNegativeNm) -> Self {
        value.0
    }
}
impl ::std::convert::From<i64> for NonNegativeNm {
    fn from(value: i64) -> Self {
        Self(value)
    }
}
impl ::std::str::FromStr for NonNegativeNm {
    type Err = <i64 as ::std::str::FromStr>::Err;
    fn from_str(value: &str) -> ::std::result::Result<Self, Self::Err> {
        Ok(Self(value.parse()?))
    }
}
impl ::std::convert::TryFrom<&str> for NonNegativeNm {
    type Error = <i64 as ::std::str::FromStr>::Err;
    fn try_from(value: &str) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
impl ::std::convert::TryFrom<String> for NonNegativeNm {
    type Error = <i64 as ::std::str::FromStr>::Err;
    fn try_from(value: String) -> ::std::result::Result<Self, Self::Error> {
        value.parse()
    }
}
impl ::std::fmt::Display for NonNegativeNm {
    fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        self.0.fmt(f)
    }
}
