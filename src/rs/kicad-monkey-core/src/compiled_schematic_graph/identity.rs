use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt;
use unicode_casefold::UnicodeCaseFold;

const NAMESPACE: &str = "sch.compiled_schematic_graph.a0";
const IDENTITY_EPOCH_MS: u64 = 1_786_060_800_000;

pub type IdentityMapping = BTreeMap<String, Value>;

/// Build the portable project namespace used by compiled-graph identities.
pub fn compiled_schematic_graph_design_scope(
    source_cad: &str,
    project_filename_or_name: &str,
) -> Result<IdentityMapping, CompiledGraphIdentityError> {
    let project_file = project_filename_or_name.trim();
    if project_file.is_empty() {
        return Err(CompiledGraphIdentityError::new(
            "missing_project",
            "compiled schematic identity requires a source project filename or name",
        ));
    }
    Ok(BTreeMap::from([
        (
            "project_file".to_owned(),
            Value::String(case_fold(&project_file.replace('\\', "/"))),
        ),
        (
            "source_cad".to_owned(),
            Value::String(case_fold(source_cad.trim_or("unknown"))),
        ),
    ]))
}

/// Deterministic UUIDv7 allocator matching the accepted Python address contract.
#[derive(Debug)]
pub struct CompiledGraphIdentityAllocator {
    design_scope: IdentityMapping,
    address_by_id: HashMap<String, String>,
    allocated_addresses: HashSet<String>,
}

impl CompiledGraphIdentityAllocator {
    pub fn new(design_scope: &IdentityMapping) -> Result<Self, CompiledGraphIdentityError> {
        let design_scope = normalize_mapping(design_scope)?;
        if design_scope.is_empty() {
            return Err(CompiledGraphIdentityError::new(
                "missing_design_scope",
                "schematic identity design_scope must not be empty",
            ));
        }
        Ok(Self {
            design_scope,
            address_by_id: HashMap::new(),
            allocated_addresses: HashSet::new(),
        })
    }

    pub fn allocate_source(
        &mut self,
        object_type: &str,
        source_identity: &IdentityMapping,
        owner_refs: &[String],
    ) -> Result<String, CompiledGraphIdentityError> {
        let source = stable_source_selector(object_type, source_identity)?;
        if source.is_empty() {
            return Err(CompiledGraphIdentityError::new(
                "missing_source_identity",
                format!("{object_type} requires governed source identity for stable allocation"),
            ));
        }
        let identity = BTreeMap::from([
            (
                "owner_refs".to_owned(),
                Value::Array(
                    owner_refs
                        .iter()
                        .filter_map(|value| nonempty_string(value))
                        .map(Value::String)
                        .collect(),
                ),
            ),
            (
                "source_identity".to_owned(),
                Value::Object(source.into_iter().collect()),
            ),
        ]);
        self.allocate(object_type, &identity)
    }

    pub fn allocate_derived(
        &mut self,
        object_type: &str,
        identity: &IdentityMapping,
    ) -> Result<String, CompiledGraphIdentityError> {
        let identity = normalize_mapping(identity)?;
        if identity.is_empty() {
            return Err(CompiledGraphIdentityError::new(
                "missing_derived_identity",
                format!("{object_type} derived identity must not be empty"),
            ));
        }
        self.allocate(object_type, &identity)
    }

    fn allocate(
        &mut self,
        object_type: &str,
        identity: &IdentityMapping,
    ) -> Result<String, CompiledGraphIdentityError> {
        let object_type = object_type.trim();
        if !object_type.starts_with("sch.") {
            return Err(CompiledGraphIdentityError::new(
                "invalid_object_type",
                "schematic occurrence identity object_type must use the sch. namespace",
            ));
        }
        let address = canonical_json(&Value::Object(
            BTreeMap::from([
                (
                    "design_scope".to_owned(),
                    Value::Object(self.design_scope.clone().into_iter().collect()),
                ),
                (
                    "identity".to_owned(),
                    Value::Object(identity.clone().into_iter().collect()),
                ),
                ("namespace".to_owned(), Value::String(NAMESPACE.to_owned())),
                (
                    "object_type".to_owned(),
                    Value::String(object_type.to_owned()),
                ),
            ])
            .into_iter()
            .collect(),
        ));
        if !self.allocated_addresses.insert(address.clone()) {
            return Err(CompiledGraphIdentityError::new(
                "duplicate_address",
                format!("duplicate stable schematic identity address for {object_type}"),
            ));
        }
        let object_id = deterministic_uuid_v7(&address);
        if self
            .address_by_id
            .get(&object_id)
            .is_some_and(|previous| previous != &address)
        {
            return Err(CompiledGraphIdentityError::new(
                "identity_collision",
                format!("stable schematic identity collision for {object_type}: {object_id}"),
            ));
        }
        self.address_by_id.insert(object_id.clone(), address);
        Ok(object_id)
    }
}

fn stable_source_selector(
    object_type: &str,
    source_identity: &IdentityMapping,
) -> Result<IdentityMapping, CompiledGraphIdentityError> {
    let source = normalize_mapping(source_identity)?;
    let get = |key: &str| source.get(key).filter(|value| truthy(value));
    if object_type == "sch.unit_occurrence" {
        return Ok(single_selector("sch.source_key.source_path", get));
    }
    if matches!(object_type, "sch.unit_definition" | "sch.page_definition") {
        return Ok(["sch.source_key.source_path", "sch.source_key.source_uuid"]
            .into_iter()
            .filter_map(|key| get(key).cloned().map(|value| (key.to_owned(), value)))
            .collect());
    }
    if object_type == "sch.terminal_occurrence" {
        let Some(source_uuid) = get("sch.source_key.source_uuid") else {
            return Ok(BTreeMap::new());
        };
        let mut selector =
            BTreeMap::from([("sch.source_key.source_uuid".to_owned(), source_uuid.clone())]);
        if let Some(value) = get("sch.source_key.source_subobject") {
            selector.insert("sch.source_key.source_subobject".to_owned(), value.clone());
        }
        return Ok(selector);
    }
    if object_type == "sch.local_net_occurrence" {
        return Ok(BTreeMap::new());
    }
    for key in [
        "sch.source_key.source_uuid",
        "sch.source_key.compiled_net",
        "sch.source_key.source_path",
        "sch.source_key.artifact_element",
        "sch.source_key.source_record",
    ] {
        let selector = single_selector(key, get);
        if !selector.is_empty() {
            return Ok(selector);
        }
    }
    Ok(BTreeMap::new())
}

fn single_selector<'a>(key: &str, get: impl Fn(&str) -> Option<&'a Value>) -> IdentityMapping {
    get(key)
        .cloned()
        .map(|value| BTreeMap::from([(key.to_owned(), value)]))
        .unwrap_or_default()
}

fn normalize_mapping(
    mapping: &IdentityMapping,
) -> Result<IdentityMapping, CompiledGraphIdentityError> {
    let mut normalized = BTreeMap::new();
    for (key, value) in mapping {
        let key = key.trim();
        if key.is_empty() || value.is_null() {
            continue;
        }
        let value = match value {
            Value::Object(values) => {
                let values = normalize_mapping(&values.clone().into_iter().collect())?;
                if values.is_empty() {
                    continue;
                }
                Value::Object(values.into_iter().collect())
            }
            Value::Array(values) => Value::Array(
                values
                    .iter()
                    .filter_map(sequence_scalar)
                    .map(Value::String)
                    .collect(),
            ),
            Value::String(value) => {
                let Some(value) = nonempty_string(value) else {
                    continue;
                };
                Value::String(value)
            }
            Value::Bool(_) | Value::Number(_) => value.clone(),
            Value::Null => continue,
        };
        normalized.insert(key.to_owned(), value);
    }
    Ok(normalized)
}

fn sequence_scalar(value: &Value) -> Option<String> {
    let text = match value {
        Value::Null | Value::Bool(false) => return None,
        Value::Bool(true) => "True".to_owned(),
        Value::Number(value) if value.as_f64() == Some(0.0) => return None,
        Value::Number(value) => value.to_string(),
        Value::String(value) => value.clone(),
        Value::Array(_) | Value::Object(_) => return None,
    };
    nonempty_string(&text)
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null | Value::Bool(false) => false,
        Value::Bool(true) => true,
        Value::Number(value) => value.as_f64() != Some(0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
    }
}

fn canonical_json(value: &Value) -> String {
    let mut output = String::new();
    write_canonical_json(value, &mut output);
    output
}

fn write_canonical_json(value: &Value, output: &mut String) {
    match value {
        Value::Null => output.push_str("null"),
        Value::Bool(value) => output.push_str(if *value { "true" } else { "false" }),
        Value::Number(value) => output.push_str(&value.to_string()),
        Value::String(value) => write_json_string(value, output),
        Value::Array(values) => write_json_array(values, output),
        Value::Object(values) => write_json_object(values, output),
    }
}

fn write_json_array(values: &[Value], output: &mut String) {
    output.push('[');
    for (index, value) in values.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write_canonical_json(value, output);
    }
    output.push(']');
}

fn write_json_object(values: &serde_json::Map<String, Value>, output: &mut String) {
    output.push('{');
    for (index, (key, value)) in values.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        write_json_string(key, output);
        output.push(':');
        write_canonical_json(value, output);
    }
    output.push('}');
}

fn write_json_string(value: &str, output: &mut String) {
    output.push('"');
    for character in value.chars() {
        write_json_character(character, output);
    }
    output.push('"');
}

fn write_json_character(character: char, output: &mut String) {
    match character {
        '"' => output.push_str("\\\""),
        '\\' => output.push_str("\\\\"),
        '\u{0008}' => output.push_str("\\b"),
        '\u{000c}' => output.push_str("\\f"),
        '\n' => output.push_str("\\n"),
        '\r' => output.push_str("\\r"),
        '\t' => output.push_str("\\t"),
        character if character.is_ascii_control() => {
            output.push_str(&format!("\\u{:04x}", character as u32));
        }
        character if character.is_ascii() => output.push(character),
        character => write_unicode_escape(character, output),
    }
}

fn write_unicode_escape(character: char, output: &mut String) {
    let code = character as u32;
    if code <= 0xffff {
        output.push_str(&format!("\\u{code:04x}"));
        return;
    }
    let adjusted = code - 0x1_0000;
    let high = 0xd800 + (adjusted >> 10);
    let low = 0xdc00 + (adjusted & 0x3ff);
    output.push_str(&format!("\\u{high:04x}\\u{low:04x}"));
}

fn deterministic_uuid_v7(address: &str) -> String {
    let digest = Sha256::digest(address.as_bytes());
    let mut bytes = [0_u8; 16];
    bytes[..6].copy_from_slice(&IDENTITY_EPOCH_MS.to_be_bytes()[2..]);
    bytes[6] = 0x70 | (digest[0] & 0x0f);
    bytes[7] = digest[1];
    bytes[8] = 0x80 | (digest[2] & 0x3f);
    bytes[9..].copy_from_slice(&digest[3..10]);
    format_uuid(bytes)
}

fn format_uuid(bytes: [u8; 16]) -> String {
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0],
        bytes[1],
        bytes[2],
        bytes[3],
        bytes[4],
        bytes[5],
        bytes[6],
        bytes[7],
        bytes[8],
        bytes[9],
        bytes[10],
        bytes[11],
        bytes[12],
        bytes[13],
        bytes[14],
        bytes[15],
    )
}

fn case_fold(value: &str) -> String {
    value.case_fold().collect()
}

fn nonempty_string(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

trait TrimOr {
    fn trim_or<'a>(&'a self, fallback: &'a str) -> &'a str;
}

impl TrimOr for str {
    fn trim_or<'a>(&'a self, fallback: &'a str) -> &'a str {
        let value = self.trim();
        if value.is_empty() { fallback } else { value }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompiledGraphIdentityError {
    pub code: &'static str,
    pub message: String,
}

impl CompiledGraphIdentityError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for CompiledGraphIdentityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for CompiledGraphIdentityError {}
