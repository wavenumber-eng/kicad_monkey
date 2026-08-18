//! Bounded, one-request-per-process native transport.

#![forbid(unsafe_code)]

use cap_std::ambient_authority;
use cap_std::fs::Dir;
use kicad_monkey_contracts::decode_native_design_facts_request_a0;
use kicad_monkey_contracts::decode_native_svg_render_request_a0;
use kicad_monkey_contracts::generated::native_design_facts_request::{
    NativeDesignFactsLimits, NativeDesignFactsRequestA0, NativeFileSlot, NativeNetlistMetadata,
};
use kicad_monkey_contracts::generated::native_design_facts_result::NativeDesignFactsResultA0;
use kicad_monkey_contracts::generated::native_error::NativeErrorA0;
pub use kicad_monkey_contracts::generated::native_error::NativeErrorKind;
use kicad_monkey_contracts::generated::native_handshake::NativeHandshakeA0;
use kicad_monkey_contracts::generated::native_handshake_a1::{
    NativeHandshakeA1, NativeHandshakeA1OperationsItem,
};
use kicad_monkey_contracts::generated::native_svg_render_result::{
    CanonicalUint64Decimal as SvgCanonicalUint64Decimal, NativeSvgRenderResultA0,
    NativeSvgRenderResultA0SourceKind,
};
use kicad_monkey_core::{
    CompiledSchematicGraphLimits, KiCadNetlistLimits, ProjectDocument, ProjectLimits,
    SchematicBundleIndex, SchematicBundleLimits, SchematicBusConnectivityLimits,
    SchematicBusExpansionLimits, SchematicDesignNetLimits, SchematicOccurrenceConnectivityLimits,
    SourceBundle, SourceBundleLimits, build_compiled_schematic_graph, build_kicad_netlist,
    emit_kicad_netlist, validate_compiled_schematic_graph,
};
use kicad_monkey_svg::render_svg;
use serde::Serialize;
use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fmt;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

pub const PROTOCOL_VERSION: &str = "a0";
pub const ENGINE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const MAX_REQUEST_BYTES: usize = 1024 * 1024;
pub const MAX_SVG_REQUEST_BYTES: usize = 256 * 1024 * 1024;
pub const MAX_SVG_REQUEST_NODES: usize = 8 * 1024 * 1024;

const REQUEST_TYPE: &str = "kicad_monkey.native.design_facts.request";
const RESULT_TYPE: &str = "kicad_monkey.native.design_facts.result";
const ERROR_TYPE: &str = "kicad_monkey.native.error";
const HANDSHAKE_TYPE: &str = "kicad_monkey.native.handshake";
const SVG_RESULT_TYPE: &str = "kicad_monkey.native.svg.result";
const SVG_PROFILE: &str = "plotter-base-a0";
const MAX_SOURCES: usize = 4096;
const MAX_SOURCE_BYTES: usize = 64 * 1024 * 1024;
const MAX_TOTAL_SOURCE_BYTES: usize = 256 * 1024 * 1024;
const MAX_PATH_BYTES: usize = 4096;
const MAX_OUTPUT_BYTES: usize = 256 * 1024 * 1024;
/// Native application ceiling, deliberately below multi-million core defaults.
const MAX_FAMILY_ITEMS: usize = 250_000;
const MAX_METADATA_BYTES: usize = 64 * 1024;
const MAX_ERROR_BYTES: usize = 64 * 1024;

/// Backward-compatible public name for the promoted generated request DTO.
pub type DesignFactsRequestA0 = NativeDesignFactsRequestA0;

/// Backward-compatible public name for the promoted generated handshake DTO.
pub type HandshakeA0 = NativeHandshakeA0;
pub type HandshakeA1 = NativeHandshakeA1;

#[derive(Debug)]
pub struct NativeError {
    pub kind: NativeErrorKind,
    pub message: String,
}

impl NativeError {
    fn new(kind: NativeErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    #[must_use]
    pub fn new_for_cli(message: impl Into<String>) -> Self {
        Self::new(NativeErrorKind::Request, message)
    }
}

impl fmt::Display for NativeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for NativeError {}

#[must_use]
pub fn handshake() -> HandshakeA0 {
    HandshakeA0 {
        type_: HANDSHAKE_TYPE.to_owned(),
        version: PROTOCOL_VERSION.to_owned(),
        engine_version: ENGINE_VERSION.to_owned(),
        operations: ["design-facts".to_owned()],
    }
}

#[must_use]
pub fn handshake_a1() -> HandshakeA1 {
    HandshakeA1 {
        type_: HANDSHAKE_TYPE.to_owned(),
        version: "a1".to_owned(),
        engine_version: ENGINE_VERSION.to_owned(),
        operations: [
            NativeHandshakeA1OperationsItem::DesignFacts,
            NativeHandshakeA1OperationsItem::RenderSvg,
        ],
    }
}

pub fn execute_request_reader(mut reader: impl Read) -> Result<Vec<u8>, NativeError> {
    let read_limit = MAX_REQUEST_BYTES
        .checked_add(1)
        .ok_or_else(|| resource_error("request byte limit overflowed"))?;
    let mut request = Vec::new();
    reader
        .by_ref()
        .take(read_limit as u64)
        .read_to_end(&mut request)
        .map_err(|error| io_error("could not read request", error))?;
    execute_request_bytes(&request)
}

pub fn execute_svg_request_reader(mut reader: impl Read) -> Result<Vec<u8>, NativeError> {
    let read_limit = MAX_SVG_REQUEST_BYTES
        .checked_add(1)
        .ok_or_else(|| resource_error("SVG request byte limit overflowed"))?;
    let mut request = Vec::new();
    reader
        .by_ref()
        .take(read_limit as u64)
        .read_to_end(&mut request)
        .map_err(|error| io_error("could not read SVG request", error))?;
    execute_svg_request_bytes(&request)
}

pub fn execute_svg_request_bytes(request_bytes: &[u8]) -> Result<Vec<u8>, NativeError> {
    if request_bytes.len() > MAX_SVG_REQUEST_BYTES {
        return Err(resource_error(
            "SVG request exceeds the fixed 256 MiB limit",
        ));
    }
    preflight_svg_request_structure(request_bytes, MAX_SVG_REQUEST_NODES)?;
    let request = decode_native_svg_render_request_a0(request_bytes)
        .map_err(|error| request_error(format!("invalid native SVG request: {error}")))?;
    let artifact = render_svg(&request)
        .map_err(|error| NativeError::new(NativeErrorKind::Core, error.to_string()))?;
    let svg_bytes = artifact.svg.len();
    let svg_sha256 = hex_digest(Sha256::digest(artifact.svg.as_bytes()).as_slice());
    let source_kind = match artifact.source_kind {
        "MOD" => NativeSvgRenderResultA0SourceKind::Mod,
        "SYM" => NativeSvgRenderResultA0SourceKind::Sym,
        "PCB" => NativeSvgRenderResultA0SourceKind::Pcb,
        "SCH" => NativeSvgRenderResultA0SourceKind::Sch,
        _ => {
            return Err(NativeError::new(
                NativeErrorKind::Core,
                "SVG renderer returned an unsupported source kind",
            ));
        }
    };
    serialize_bounded(
        &NativeSvgRenderResultA0 {
            document_id: artifact.document_id,
            engine_version: ENGINE_VERSION.to_owned(),
            profile: SVG_PROFILE.to_owned(),
            source_kind,
            svg_bytes: SvgCanonicalUint64Decimal(svg_bytes.to_string()),
            svg_sha256,
            svg_utf8: artifact.svg,
            type_: SVG_RESULT_TYPE.to_owned(),
            version: PROTOCOL_VERSION.to_owned(),
        },
        artifact.max_result_bytes,
    )
}

fn preflight_svg_request_structure(
    request_bytes: &[u8],
    maximum_nodes: usize,
) -> Result<(), NativeError> {
    let mut counter = JsonNodeCounter {
        nodes: 0,
        maximum: maximum_nodes,
        exceeded: false,
    };
    let mut deserializer = serde_json::Deserializer::from_slice(request_bytes);
    let parsed = JsonNodeSeed {
        counter: &mut counter,
    }
    .deserialize(&mut deserializer)
    .and_then(|()| deserializer.end());
    if counter.exceeded {
        return Err(resource_error(
            "SVG request exceeds the fixed structural node limit",
        ));
    }
    parsed.map_err(|error| request_error(format!("invalid native SVG request: {error}")))
}

struct JsonNodeCounter {
    nodes: usize,
    maximum: usize,
    exceeded: bool,
}

impl JsonNodeCounter {
    fn charge<E: serde::de::Error>(&mut self) -> Result<(), E> {
        self.nodes = self.nodes.checked_add(1).ok_or_else(|| {
            self.exceeded = true;
            E::custom("JSON structural node count overflowed")
        })?;
        if self.nodes > self.maximum {
            self.exceeded = true;
            return Err(E::custom("JSON structural node limit exceeded"));
        }
        Ok(())
    }
}

struct JsonNodeSeed<'a> {
    counter: &'a mut JsonNodeCounter,
}

impl<'de> DeserializeSeed<'de> for JsonNodeSeed<'_> {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        self.counter.charge()?;
        deserializer.deserialize_any(JsonNodeVisitor {
            counter: self.counter,
        })
    }
}

struct JsonNodeVisitor<'a> {
    counter: &'a mut JsonNodeCounter,
}

impl<'de> Visitor<'de> for JsonNodeVisitor<'_> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a bounded JSON value")
    }

    fn visit_bool<E>(self, _value: bool) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_i64<E>(self, _value: i64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_u64<E>(self, _value: u64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_f64<E>(self, _value: f64) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_str<E>(self, _value: &str) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_borrowed_str<E>(self, _value: &'de str) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_none<E>(self) -> Result<Self::Value, E> {
        Ok(())
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence
            .next_element_seed(JsonNodeSeed {
                counter: self.counter,
            })?
            .is_some()
        {}
        Ok(())
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        while map
            .next_key_seed(JsonKeySeed {
                counter: self.counter,
            })?
            .is_some()
        {
            map.next_value_seed(JsonNodeSeed {
                counter: self.counter,
            })?;
        }
        Ok(())
    }
}

struct JsonKeySeed<'a> {
    counter: &'a mut JsonNodeCounter,
}

impl<'de> DeserializeSeed<'de> for JsonKeySeed<'_> {
    type Value = IgnoredAny;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        self.counter.charge()?;
        deserializer.deserialize_identifier(IgnoredAny)
    }
}

fn hex_digest(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

pub fn execute_request_bytes(request_bytes: &[u8]) -> Result<Vec<u8>, NativeError> {
    if request_bytes.len() > MAX_REQUEST_BYTES {
        return Err(resource_error("request exceeds the fixed 1 MiB limit"));
    }
    let request = decode_native_design_facts_request_a0(request_bytes)
        .map_err(|error| request_error(format!("invalid native request: {error}")))?;
    execute_request(request)
}

#[must_use]
pub fn serialize_error(error: &NativeError) -> Vec<u8> {
    let encoded = serde_json::to_vec(&NativeErrorA0 {
        type_: ERROR_TYPE.to_owned(),
        version: PROTOCOL_VERSION.to_owned(),
        kind: error.kind,
        message: error.message.clone(),
    })
    .unwrap_or_default();
    if encoded.len() <= MAX_ERROR_BYTES && !encoded.is_empty() {
        return encoded;
    }
    serde_json::to_vec(&NativeErrorA0 {
        type_: ERROR_TYPE.to_owned(),
        version: PROTOCOL_VERSION.to_owned(),
        kind: error.kind,
        message: "native error detail exceeded the fixed diagnostic limit".to_owned(),
    })
    .unwrap_or_else(|_| b"{\"type\":\"kicad_monkey.native.error\",\"version\":\"a0\",\"kind\":\"core\",\"message\":\"native error\"}".to_vec())
}

fn execute_request(request: DesignFactsRequestA0) -> Result<Vec<u8>, NativeError> {
    validate_identity(&request)?;
    let limits = AppLimits::from_wire(request.limits)?;
    validate_metadata(&request.netlist, limits.max_path_bytes)?;
    let root = canonical_bundle_root(&request.bundle_root, limits.max_path_bytes)?;
    let buffers = load_slots(&root, request.file_slots, limits)?;
    let source_defaults = SourceBundleLimits::default();
    let bundle = SourceBundle::from_manifest(
        request.manifest,
        buffers,
        SourceBundleLimits {
            max_sources: source_defaults.max_sources.min(limits.max_sources),
            max_source_bytes: source_defaults
                .max_source_bytes
                .min(limits.max_source_bytes),
            max_total_bytes: source_defaults
                .max_total_bytes
                .min(limits.max_total_source_bytes),
            max_path_bytes: source_defaults.max_path_bytes.min(limits.max_path_bytes),
        },
    )
    .map_err(core_error)?;
    let index =
        SchematicBundleIndex::build(&bundle, schematic_limits(limits)).map_err(core_error)?;
    let project = bundle
        .project()
        .map(|source| ProjectDocument::from_reader(source.bytes(), project_limits(limits)))
        .transpose()
        .map_err(|error| NativeError::new(NativeErrorKind::Core, error.to_string()))?;
    let graph = build_compiled_schematic_graph(&index, graph_limits(limits)).map_err(core_error)?;
    validate_compiled_schematic_graph(&graph)
        .map_err(|error| NativeError::new(NativeErrorKind::Core, error.to_string()))?;
    let netlist_limits = netlist_limits(limits);
    let netlist = build_kicad_netlist(
        &index,
        project.as_ref().map(ProjectDocument::view),
        netlist_limits,
    )
    .map_err(core_error)?;
    let netlist_text = emit_kicad_netlist(
        &netlist,
        &request.netlist.source_path,
        &request.netlist.date,
        &request.netlist.tool,
        limits.max_output_bytes,
    )
    .map_err(core_error)?;
    serialize_bounded(
        &NativeDesignFactsResultA0 {
            type_: RESULT_TYPE.to_owned(),
            version: PROTOCOL_VERSION.to_owned(),
            engine_version: ENGINE_VERSION.to_owned(),
            compiled_schematic_graph: graph,
            kicad_netlist_version: "E".to_owned(),
            kicad_netlist: netlist_text,
        },
        limits.max_output_bytes,
    )
}

#[derive(Clone, Copy)]
struct AppLimits {
    max_sources: usize,
    max_source_bytes: usize,
    max_total_source_bytes: usize,
    max_path_bytes: usize,
    max_output_bytes: usize,
}

impl AppLimits {
    fn from_wire(wire: NativeDesignFactsLimits) -> Result<Self, NativeError> {
        Ok(Self {
            max_sources: bounded_u32(wire.max_sources, MAX_SOURCES, "max_sources")?,
            max_source_bytes: decimal_usize(
                &wire.max_source_bytes,
                MAX_SOURCE_BYTES,
                "max_source_bytes",
            )?,
            max_total_source_bytes: decimal_usize(
                &wire.max_total_source_bytes,
                MAX_TOTAL_SOURCE_BYTES,
                "max_total_source_bytes",
            )?,
            max_path_bytes: bounded_u32(wire.max_path_bytes, MAX_PATH_BYTES, "max_path_bytes")?,
            max_output_bytes: decimal_usize(
                &wire.max_output_bytes,
                MAX_OUTPUT_BYTES,
                "max_output_bytes",
            )?,
        })
    }
}

fn validate_identity(request: &NativeDesignFactsRequestA0) -> Result<(), NativeError> {
    if request.type_ == REQUEST_TYPE && request.version == PROTOCOL_VERSION {
        Ok(())
    } else {
        Err(request_error(
            "unsupported request identity or protocol version",
        ))
    }
}

fn validate_metadata(
    metadata: &NativeNetlistMetadata,
    max_path_bytes: usize,
) -> Result<(), NativeError> {
    if metadata.source_path.len() > max_path_bytes {
        return Err(resource_error("netlist source_path exceeds max_path_bytes"));
    }
    let bytes = metadata
        .date
        .len()
        .checked_add(metadata.tool.len())
        .ok_or_else(|| resource_error("netlist metadata byte count overflowed"))?;
    if bytes > MAX_METADATA_BYTES {
        return Err(resource_error(
            "netlist date/tool exceed the fixed metadata limit",
        ));
    }
    Ok(())
}

fn canonical_bundle_root(raw: &str, max_path_bytes: usize) -> Result<PathBuf, NativeError> {
    if raw.is_empty() || raw.len() > max_path_bytes {
        return Err(path_error("bundle_root is empty or exceeds max_path_bytes"));
    }
    let root = Path::new(raw);
    if !root.is_absolute() {
        return Err(path_error("bundle_root must be absolute"));
    }
    let canonical = std::fs::canonicalize(root)
        .map_err(|error| io_error("could not resolve bundle_root", error))?;
    if !canonical.is_dir() {
        return Err(path_error("bundle_root must resolve to a directory"));
    }
    Ok(canonical)
}

fn load_slots(
    root: &Path,
    slots: Vec<NativeFileSlot>,
    limits: AppLimits,
) -> Result<Vec<Vec<u8>>, NativeError> {
    if slots.len() > limits.max_sources {
        return Err(resource_error("file_slots exceeds max_sources"));
    }
    let mut ordered: Vec<Option<Vec<u8>>> = (0..slots.len()).map(|_| None).collect();
    let mut seen = HashSet::with_capacity(slots.len());
    let mut total = 0_usize;
    let directory = Dir::open_ambient_dir(root, ambient_authority())
        .map_err(|error| io_error("could not open bundle_root capability", error))?;
    for slot in slots {
        let index = usize::try_from(slot.slot)
            .map_err(|_| request_error("file slot does not fit usize"))?;
        if index >= ordered.len() || !seen.insert(index) {
            return Err(request_error(
                "file slots must be unique and contiguous from zero",
            ));
        }
        let relative = checked_relative_path(&slot.path, limits.max_path_bytes)?;
        let bytes = read_bounded_contained_file(&directory, &relative, limits.max_source_bytes)?;
        total = total
            .checked_add(bytes.len())
            .filter(|value| *value <= limits.max_total_source_bytes)
            .ok_or_else(|| resource_error("source slots exceed max_total_source_bytes"))?;
        ordered[index] = Some(bytes);
    }
    ordered
        .into_iter()
        .map(|slot| slot.ok_or_else(|| request_error("file slots are incomplete")))
        .collect()
}

fn checked_relative_path(raw: &str, max_path_bytes: usize) -> Result<PathBuf, NativeError> {
    if raw.is_empty()
        || raw.len() > max_path_bytes
        || raw.contains('\\')
        || raw.contains(':')
        || raw.starts_with('/')
    {
        return Err(path_error(
            "source slot path is not a portable relative path",
        ));
    }
    let path = Path::new(raw);
    if path
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(path_error(
            "source slot path contains a non-normal component",
        ));
    }
    Ok(path.to_owned())
}

fn read_bounded_contained_file(
    directory: &Dir,
    path: &Path,
    maximum: usize,
) -> Result<Vec<u8>, NativeError> {
    let read_limit = maximum
        .checked_add(1)
        .ok_or_else(|| resource_error("source byte limit overflowed"))?;
    let mut file = directory
        .open(path)
        .map_err(|error| io_error("could not open contained source slot", error))?;
    let metadata = file
        .metadata()
        .map_err(|error| io_error("could not inspect opened source slot", error))?;
    if !metadata.is_file() {
        return Err(path_error("source slot is not a regular file"));
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(read_limit as u64)
        .read_to_end(&mut bytes)
        .map_err(|error| io_error("could not read source slot", error))?;
    if bytes.len() > maximum {
        return Err(resource_error("source slot exceeds max_source_bytes"));
    }
    Ok(bytes)
}

fn schematic_limits(limits: AppLimits) -> SchematicBundleLimits {
    let default = SchematicBundleLimits::default();
    SchematicBundleLimits {
        max_source_bytes: default.max_source_bytes.min(limits.max_source_bytes),
        max_depth: default.max_depth.min(256),
        max_selected_forms_per_source: family(default.max_selected_forms_per_source),
        max_sheets_per_source: family(default.max_sheets_per_source),
        max_sheet_properties: family(default.max_sheet_properties),
        max_sheet_pins_per_sheet: family(default.max_sheet_pins_per_sheet),
        max_title_block_children_per_source: family(default.max_title_block_children_per_source),
        max_title_block_comments_per_source: family(default.max_title_block_comments_per_source),
        max_symbols_per_source: family(default.max_symbols_per_source),
        max_symbol_properties_per_symbol: family(default.max_symbol_properties_per_symbol),
        max_symbol_pins_per_symbol: family(default.max_symbol_pins_per_symbol),
        max_library_symbols_per_source: family(default.max_library_symbols_per_source),
        max_library_properties_per_symbol: family(default.max_library_properties_per_symbol),
        max_library_subsymbols_per_source: family(default.max_library_subsymbols_per_source),
        max_library_pins_per_source: family(default.max_library_pins_per_source),
        max_jumper_groups_per_source: family(default.max_jumper_groups_per_source),
        max_jumper_members_per_source: family(default.max_jumper_members_per_source),
        max_jumper_member_bytes_per_source: bytes(
            default.max_jumper_member_bytes_per_source,
            limits.max_total_source_bytes,
        ),
        max_library_lookup_key_bytes_per_source: bytes(
            default.max_library_lookup_key_bytes_per_source,
            limits.max_total_source_bytes,
        ),
        max_symbol_terminals_per_occurrence: family(default.max_symbol_terminals_per_occurrence),
        max_symbol_terminal_retained_bytes_per_occurrence: bytes(
            default.max_symbol_terminal_retained_bytes_per_occurrence,
            limits.max_total_source_bytes,
        ),
        max_symbol_instance_projects_per_symbol: family(
            default.max_symbol_instance_projects_per_symbol,
        ),
        max_symbol_instances_per_symbol: family(default.max_symbol_instances_per_symbol),
        max_symbol_instance_index_bytes_per_source: bytes(
            default.max_symbol_instance_index_bytes_per_source,
            limits.max_total_source_bytes,
        ),
        max_symbol_variants_per_instance: family(default.max_symbol_variants_per_instance),
        max_symbol_variant_fields_per_variant: family(
            default.max_symbol_variant_fields_per_variant,
        ),
        max_legacy_symbol_instances_per_source: family(
            default.max_legacy_symbol_instances_per_source,
        ),
        max_decoded_string_bytes: bytes(
            default.max_decoded_string_bytes,
            limits.max_total_source_bytes,
        ),
        max_connectivity_objects_per_source: family(default.max_connectivity_objects_per_source),
        max_wires_per_source: family(default.max_wires_per_source),
        max_buses_per_source: family(default.max_buses_per_source),
        max_bus_entries_per_source: family(default.max_bus_entries_per_source),
        max_bus_aliases_per_source: family(default.max_bus_aliases_per_source),
        max_bus_alias_members_per_source: family(default.max_bus_alias_members_per_source),
        max_junctions_per_source: family(default.max_junctions_per_source),
        max_no_connects_per_source: family(default.max_no_connects_per_source),
        max_labels_per_source: family(default.max_labels_per_source),
        max_points_per_connectivity_object: family(default.max_points_per_connectivity_object),
        max_connectivity_points_per_source: family(default.max_connectivity_points_per_source),
        max_occurrences: family(default.max_occurrences),
        max_path_bytes: default.max_path_bytes.min(limits.max_path_bytes),
    }
}

fn graph_limits(limits: AppLimits) -> CompiledSchematicGraphLimits {
    let default = CompiledSchematicGraphLimits::default();
    CompiledSchematicGraphLimits {
        design: design_limits(limits),
        max_unit_definitions: family(default.max_unit_definitions),
        max_page_definitions: family(default.max_page_definitions),
        max_unit_occurrences: family(default.max_unit_occurrences),
        max_page_occurrences: family(default.max_page_occurrences),
        max_hierarchy_occurrences: family(default.max_hierarchy_occurrences),
        max_component_occurrences: family(default.max_component_occurrences),
        max_local_net_occurrences: family(default.max_local_net_occurrences),
        max_terminal_occurrences: family(default.max_terminal_occurrences),
        max_hierarchy_terminal_bindings: family(default.max_hierarchy_terminal_bindings),
        max_graphical_artifact_links: family(default.max_graphical_artifact_links),
        max_retained_string_bytes: bytes(
            default.max_retained_string_bytes,
            limits.max_output_bytes,
        ),
    }
}

fn netlist_limits(limits: AppLimits) -> KiCadNetlistLimits {
    let default = KiCadNetlistLimits::default();
    KiCadNetlistLimits {
        design: design_limits(limits),
        max_nets: family(default.max_nets),
        max_terminals: family(default.max_terminals),
        max_components: family(default.max_components),
        max_component_candidates: family(default.max_component_candidates),
        max_component_fields: family(default.max_component_fields),
        max_component_units: family(default.max_component_units),
        max_component_unit_pins: family(default.max_component_unit_pins),
        max_expanded_string_bytes: bytes(
            default.max_expanded_string_bytes,
            limits.max_total_source_bytes,
        ),
        max_variable_expansion_work_bytes: bytes(
            default.max_variable_expansion_work_bytes,
            limits.max_total_source_bytes,
        ),
        max_libparts: family(default.max_libparts),
        max_libpart_pins: family(default.max_libpart_pins),
        max_sheets: family(default.max_sheets),
        max_wildcard_match_work: family(default.max_wildcard_match_work),
        max_retained_string_bytes: bytes(
            default.max_retained_string_bytes,
            limits.max_output_bytes,
        ),
        max_output_bytes: default.max_output_bytes.min(limits.max_output_bytes),
    }
}

fn design_limits(limits: AppLimits) -> SchematicDesignNetLimits {
    let default = SchematicDesignNetLimits::default();
    SchematicDesignNetLimits {
        connectivity: connectivity_limits(default.connectivity, limits),
        max_subgraphs: family(default.max_subgraphs),
        max_indexed_coords: family(default.max_indexed_coords),
        max_union_work: family(default.max_union_work),
        max_merge_keys: family(default.max_merge_keys),
        max_design_bus_aliases: family(default.max_design_bus_aliases),
        max_bus_subgraphs: family(default.max_bus_subgraphs),
        max_bus_members: family(default.max_bus_members),
        max_bus_indexed_coords: family(default.max_bus_indexed_coords),
        max_bus_mapping_work_bytes: bytes(
            default.max_bus_mapping_work_bytes,
            limits.max_total_source_bytes,
        ),
        max_bus_member_union_work: family(default.max_bus_member_union_work),
        max_bus_overrides: family(default.max_bus_overrides),
        max_bus_override_refs: family(default.max_bus_override_refs),
        max_bus_override_string_bytes: bytes(
            default.max_bus_override_string_bytes,
            limits.max_total_source_bytes,
        ),
        max_sheet_pin_targets: family(default.max_sheet_pin_targets),
        max_target_index_bytes: bytes(
            default.max_target_index_bytes,
            limits.max_total_source_bytes,
        ),
        max_hierarchy_bindings: family(default.max_hierarchy_bindings),
        max_drivers_per_net: family(default.max_drivers_per_net),
        max_nets: family(default.max_nets),
        max_net_members: family(default.max_net_members),
        max_terminals: family(default.max_terminals),
        max_name_bytes: bytes(default.max_name_bytes, limits.max_total_source_bytes),
        max_retained_string_bytes: bytes(
            default.max_retained_string_bytes,
            limits.max_output_bytes,
        ),
        max_work_string_bytes: bytes(default.max_work_string_bytes, limits.max_total_source_bytes),
        max_merged_driver_bytes: bytes(
            default.max_merged_driver_bytes,
            limits.max_total_source_bytes,
        ),
    }
}

fn bus_expansion_limits(
    default: SchematicBusExpansionLimits,
    limits: AppLimits,
) -> SchematicBusExpansionLimits {
    SchematicBusExpansionLimits {
        max_input_bytes: bytes(default.max_input_bytes, limits.max_source_bytes),
        max_group_members: family(default.max_group_members),
        max_expanded_members: family(default.max_expanded_members),
        max_parsed_member_bytes: bytes(
            default.max_parsed_member_bytes,
            limits.max_total_source_bytes,
        ),
        max_expansion_work_items: family(default.max_expansion_work_items),
        max_expansion_work_bytes: bytes(
            default.max_expansion_work_bytes,
            limits.max_total_source_bytes,
        ),
        max_expanded_output_bytes: bytes(
            default.max_expanded_output_bytes,
            limits.max_total_source_bytes,
        ),
        max_nesting_depth: default.max_nesting_depth.min(256),
    }
}

fn bus_connectivity_limits(
    default: SchematicBusConnectivityLimits,
    limits: AppLimits,
) -> SchematicBusConnectivityLimits {
    SchematicBusConnectivityLimits {
        max_segments: family(default.max_segments),
        max_segment_index_nodes: family(default.max_segment_index_nodes),
        max_segment_query_work: family(default.max_segment_query_work),
        max_subgraphs: family(default.max_subgraphs),
        max_drivers: family(default.max_drivers),
        max_taps: family(default.max_taps),
        max_aliases: family(default.max_aliases),
        max_graph_points: family(default.max_graph_points),
        max_retained_points: family(default.max_retained_points),
        max_retained_string_bytes: bytes(
            default.max_retained_string_bytes,
            limits.max_output_bytes,
        ),
        max_expanded_members: family(default.max_expanded_members),
        max_expanded_member_bytes: bytes(
            default.max_expanded_member_bytes,
            limits.max_total_source_bytes,
        ),
        expansion: bus_expansion_limits(default.expansion, limits),
    }
}

fn connectivity_limits(
    default: SchematicOccurrenceConnectivityLimits,
    limits: AppLimits,
) -> SchematicOccurrenceConnectivityLimits {
    SchematicOccurrenceConnectivityLimits {
        bus: bus_connectivity_limits(default.bus, limits),
        max_entry_segments: family(default.max_entry_segments),
        max_entry_index_nodes: family(default.max_entry_index_nodes),
        max_attachment_query_work: family(default.max_attachment_query_work),
        max_graph_points: family(default.max_graph_points),
        max_pin_drivers: family(default.max_pin_drivers),
        max_label_drivers: family(default.max_label_drivers),
        max_subgraphs: family(default.max_subgraphs),
        max_retained_points: family(default.max_retained_points),
        max_retained_string_bytes: bytes(
            default.max_retained_string_bytes,
            limits.max_output_bytes,
        ),
        max_expanded_pins: family(default.max_expanded_pins),
        max_expanded_pin_bytes: bytes(
            default.max_expanded_pin_bytes,
            limits.max_total_source_bytes,
        ),
        max_jumper_union_work: family(default.max_jumper_union_work),
    }
}

fn project_limits(limits: AppLimits) -> ProjectLimits {
    let default = ProjectLimits::default();
    ProjectLimits {
        max_source_bytes: default.max_source_bytes.min(limits.max_source_bytes),
        max_output_bytes: default.max_output_bytes.min(limits.max_source_bytes),
        max_json_nodes: family(default.max_json_nodes),
        max_json_depth: default.max_json_depth.min(256),
        max_text_variables: family(default.max_text_variables),
        max_variants: family(default.max_variants),
        max_net_classes: family(default.max_net_classes),
        max_netclass_assignments: family(default.max_netclass_assignments),
        max_netclass_assignment_references: family(default.max_netclass_assignment_references),
        max_netclass_patterns: family(default.max_netclass_patterns),
        max_net_colors: family(default.max_net_colors),
        max_diff_pair_dimensions: family(default.max_diff_pair_dimensions),
        max_typed_string_bytes: bytes(
            default.max_typed_string_bytes,
            limits.max_total_source_bytes,
        ),
    }
}

fn family(default: usize) -> usize {
    default.min(MAX_FAMILY_ITEMS)
}

fn bytes(default: usize, requested: usize) -> usize {
    default.min(requested)
}

fn serialize_bounded(value: &impl Serialize, maximum: usize) -> Result<Vec<u8>, NativeError> {
    let mut writer = BoundedWriter::new(maximum);
    serde_json::to_writer(&mut writer, value).map_err(|error| {
        if writer.exceeded {
            resource_error("result exceeds max_output_bytes")
        } else {
            NativeError::new(
                NativeErrorKind::Core,
                format!("could not serialize result: {error}"),
            )
        }
    })?;
    Ok(writer.bytes)
}

struct BoundedWriter {
    bytes: Vec<u8>,
    maximum: usize,
    exceeded: bool,
}

impl BoundedWriter {
    fn new(maximum: usize) -> Self {
        Self {
            bytes: Vec::new(),
            maximum,
            exceeded: false,
        }
    }
}

impl Write for BoundedWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let Some(total) = self.bytes.len().checked_add(buffer.len()) else {
            self.exceeded = true;
            return Err(std::io::Error::other("bounded output overflowed"));
        };
        if total > self.maximum {
            self.exceeded = true;
            return Err(std::io::Error::other("bounded output exceeded"));
        }
        self.bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn bounded_u32(value: u32, hard_maximum: usize, field: &str) -> Result<usize, NativeError> {
    let value =
        usize::try_from(value).map_err(|_| request_error(format!("{field} does not fit usize")))?;
    if value > hard_maximum {
        Err(resource_error(format!(
            "{field} exceeds its fixed application ceiling"
        )))
    } else {
        Ok(value)
    }
}

fn decimal_usize(raw: &str, hard_maximum: usize, field: &str) -> Result<usize, NativeError> {
    if raw.is_empty()
        || (raw.len() > 1 && raw.starts_with('0'))
        || !raw.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(request_error(format!(
            "{field} is not canonical unsigned decimal"
        )));
    }
    let value = raw
        .parse::<u64>()
        .map_err(|_| request_error(format!("{field} exceeds uint64")))?;
    let value = usize::try_from(value)
        .map_err(|_| resource_error(format!("{field} does not fit this platform")))?;
    if value > hard_maximum {
        Err(resource_error(format!(
            "{field} exceeds its fixed application ceiling"
        )))
    } else {
        Ok(value)
    }
}

fn request_error(message: impl Into<String>) -> NativeError {
    NativeError::new(NativeErrorKind::Request, message)
}

fn path_error(message: impl Into<String>) -> NativeError {
    NativeError::new(NativeErrorKind::Path, message)
}

fn resource_error(message: impl Into<String>) -> NativeError {
    NativeError::new(NativeErrorKind::ResourceLimit, message)
}

fn io_error(context: &str, error: std::io::Error) -> NativeError {
    NativeError::new(NativeErrorKind::Io, format!("{context}: {error}"))
}

fn core_error(error: kicad_monkey_core::SourceBundleError) -> NativeError {
    use kicad_monkey_core::SourceBundleErrorKind;
    let kind = if error.kind == SourceBundleErrorKind::ResourceLimit {
        NativeErrorKind::ResourceLimit
    } else {
        NativeErrorKind::Core
    };
    NativeError::new(kind, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        AppLimits, design_limits, preflight_svg_request_structure, project_limits, schematic_limits,
    };
    use kicad_monkey_core::{ProjectLimits, SchematicBundleLimits, SchematicDesignNetLimits};

    #[test]
    fn application_limits_never_raise_core_defaults() {
        let limits = AppLimits {
            max_sources: 4096,
            max_source_bytes: 64 * 1024 * 1024,
            max_total_source_bytes: 256 * 1024 * 1024,
            max_path_bytes: 4096,
            max_output_bytes: 256 * 1024 * 1024,
        };
        let schematic = schematic_limits(limits);
        let schematic_default = SchematicBundleLimits::default();
        assert!(
            schematic.max_title_block_children_per_source
                <= schematic_default.max_title_block_children_per_source
        );
        assert!(
            schematic.max_title_block_comments_per_source
                <= schematic_default.max_title_block_comments_per_source
        );
        assert!(
            schematic.max_library_lookup_key_bytes_per_source
                <= schematic_default.max_library_lookup_key_bytes_per_source
        );
        assert!(schematic.max_decoded_string_bytes <= schematic_default.max_decoded_string_bytes);

        let design = design_limits(limits);
        let design_default = SchematicDesignNetLimits::default();
        assert!(
            design.connectivity.bus.expansion.max_input_bytes
                <= design_default.connectivity.bus.expansion.max_input_bytes
        );
        assert!(
            design.connectivity.bus.expansion.max_expansion_work_bytes
                <= design_default
                    .connectivity
                    .bus
                    .expansion
                    .max_expansion_work_bytes
        );

        let project = project_limits(limits);
        let project_default = ProjectLimits::default();
        assert!(project.max_typed_string_bytes <= project_default.max_typed_string_bytes);
        assert!(project.max_json_nodes <= project_default.max_json_nodes);
    }

    #[test]
    fn svg_request_structural_node_limit_is_inclusive() {
        // Arrays and objects, their members, and object keys are all nodes,
        // matching the public Python client's iterative accounting.
        preflight_svg_request_structure(br#"{"a":[null,true]}"#, 5)
            .expect("exact five-node request");
        let error = preflight_svg_request_structure(br#"{"a":[null,true,false]}"#, 5)
            .expect_err("one node over the structural ceiling");
        assert!(error.message.contains("structural node limit"));
    }
}
