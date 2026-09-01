//! Named, byte-preserving multi-file inputs for the schematic compiler.

use kicad_monkey_contracts::generated::source_bundle_manifest::{
    SourceBundleManifestA0, SourceBundleSource, SourceKind,
};
use kicad_monkey_contracts::validate_source_bundle_manifest_contract;
use serde::Deserialize;
use serde::de::IgnoredAny;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;

/// Configurable ceilings for one owned source bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceBundleLimits {
    pub max_sources: usize,
    pub max_source_bytes: usize,
    pub max_total_bytes: usize,
    pub max_path_bytes: usize,
}

impl Default for SourceBundleLimits {
    fn default() -> Self {
        Self {
            max_sources: 1_000_000,
            max_source_bytes: 512 * 1024 * 1024,
            max_total_bytes: 4_usize.saturating_mul(1024 * 1024 * 1024),
            max_path_bytes: 32 * 1024,
        }
    }
}

/// One exact caller-supplied byte buffer after manifest validation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceFile {
    path: String,
    kind: SourceKind,
    bytes: Vec<u8>,
}

impl SourceFile {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn kind(&self) -> SourceKind {
        self.kind
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn text(&self) -> Result<&str, SourceBundleError> {
        std::str::from_utf8(&self.bytes).map_err(|error| {
            SourceBundleError::new(
                SourceBundleErrorKind::Utf8,
                Some(&self.path),
                format!("source is not UTF-8 at byte {}", error.valid_up_to()),
            )
        })
    }
}

/// Validated source inventory with exact source bytes owned once.
#[derive(Clone, Debug)]
pub struct SourceBundle {
    root_schematic_path: String,
    project_path: Option<String>,
    sources: BTreeMap<String, SourceFile>,
    total_bytes: usize,
    max_path_bytes: usize,
}

impl SourceBundle {
    /// Pair a generated manifest with separate byte slots and validate the boundary.
    pub fn from_manifest(
        manifest: SourceBundleManifestA0,
        mut buffers: Vec<Vec<u8>>,
        limits: SourceBundleLimits,
    ) -> Result<Self, SourceBundleError> {
        validate_source_bundle_manifest_contract(&manifest).map_err(|error| {
            SourceBundleError::new(SourceBundleErrorKind::Contract, None, error.to_string())
        })?;
        validate_bundle_cardinality(manifest.sources.len(), buffers.len(), limits)?;

        let root_schematic_path =
            normalize_bundle_path(&manifest.root_schematic_path, limits.max_path_bytes)?;
        let project_path = manifest
            .project_path
            .as_deref()
            .map(|path| normalize_bundle_path(path, limits.max_path_bytes))
            .transpose()?;
        let (sources, total_bytes) = assemble_sources(manifest.sources, &mut buffers, limits)?;

        require_source_kind(&sources, &root_schematic_path, SourceKind::Schematic)?;
        if let Some(path) = project_path.as_deref() {
            require_source_kind(&sources, path, SourceKind::Project)?;
        }
        for source in sources
            .values()
            .filter(|source| source.kind == SourceKind::Project)
        {
            validate_project_json(source)?;
        }

        Ok(Self {
            root_schematic_path,
            project_path,
            sources,
            total_bytes,
            max_path_bytes: limits.max_path_bytes,
        })
    }

    pub fn root_schematic_path(&self) -> &str {
        &self.root_schematic_path
    }

    pub fn project_path(&self) -> Option<&str> {
        self.project_path.as_deref()
    }

    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    pub fn sources(&self) -> impl ExactSizeIterator<Item = &SourceFile> {
        self.sources.values()
    }

    pub fn source(&self, path: &str) -> Result<Option<&SourceFile>, SourceBundleError> {
        let path = normalize_bundle_path(path, self.max_path_bytes)?;
        Ok(self.sources.get(&path))
    }

    pub fn root_schematic(&self) -> &SourceFile {
        &self.sources[&self.root_schematic_path]
    }

    pub fn project(&self) -> Option<&SourceFile> {
        self.project_path
            .as_ref()
            .and_then(|path| self.sources.get(path))
    }

    pub(crate) fn bundle_identity_sha256(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        hash_identity_field(&mut hasher, self.root_schematic_path.as_bytes());
        match &self.project_path {
            Some(path) => {
                hasher.update([1]);
                hash_identity_field(&mut hasher, path.as_bytes());
            }
            None => hasher.update([0]),
        }
        hash_identity_field(&mut hasher, &self.sources.len().to_le_bytes());
        for source in self.sources.values() {
            hash_identity_field(&mut hasher, source.path().as_bytes());
            hash_identity_field(&mut hasher, source_kind_identity(source.kind()));
            hash_identity_field(&mut hasher, source.bytes());
        }
        hasher.finalize().into()
    }

    pub(crate) fn resolve_schematic(
        &self,
        parent_path: &str,
        reference: &str,
        max_path_bytes: usize,
    ) -> Result<&SourceFile, SourceBundleError> {
        let resolved = resolve_relative_path(parent_path, reference, max_path_bytes)?;
        let source = self.sources.get(&resolved).ok_or_else(|| {
            SourceBundleError::new(
                SourceBundleErrorKind::MissingSource,
                Some(&resolved),
                "referenced schematic is absent from the source bundle",
            )
        })?;
        if source.kind != SourceKind::Schematic {
            return Err(SourceBundleError::new(
                SourceBundleErrorKind::Kind,
                Some(&resolved),
                "referenced sheet source is not classified as schematic",
            ));
        }
        Ok(source)
    }
}

fn source_kind_identity(kind: SourceKind) -> &'static [u8] {
    match kind {
        SourceKind::Project => b"project",
        SourceKind::Schematic => b"schematic",
        SourceKind::SymbolLibrary => b"symbol_library",
        SourceKind::SymbolTable => b"symbol_table",
        SourceKind::Worksheet => b"worksheet",
        SourceKind::Other => b"other",
    }
}

fn hash_identity_field(hasher: &mut Sha256, value: &[u8]) {
    hasher.update((value.len() as u128).to_le_bytes());
    hasher.update(value);
}

fn validate_bundle_cardinality(
    descriptor_count: usize,
    buffer_count: usize,
    limits: SourceBundleLimits,
) -> Result<(), SourceBundleError> {
    if descriptor_count == 0 || descriptor_count > limits.max_sources {
        return Err(limit_error(
            "source count is outside the configured bundle limit",
        ));
    }
    if buffer_count != descriptor_count {
        return Err(SourceBundleError::new(
            SourceBundleErrorKind::Slot,
            None,
            "the byte-slot count must exactly equal the source descriptor count",
        ));
    }
    Ok(())
}

fn assemble_sources(
    descriptors: Vec<SourceBundleSource>,
    buffers: &mut [Vec<u8>],
    limits: SourceBundleLimits,
) -> Result<(BTreeMap<String, SourceFile>, usize), SourceBundleError> {
    let mut used_slots = vec![false; buffers.len()];
    let mut sources = BTreeMap::new();
    let mut total_bytes = 0_usize;
    for descriptor in descriptors {
        let (path, source, byte_count) =
            consume_descriptor(descriptor, buffers, &mut used_slots, limits)?;
        total_bytes = checked_bundle_total(total_bytes, byte_count, limits.max_total_bytes)?;
        if sources.insert(path.clone(), source).is_some() {
            return Err(SourceBundleError::new(
                SourceBundleErrorKind::Path,
                Some(&path),
                "normalized source paths must be unique",
            ));
        }
    }
    require_all_slots(&used_slots)?;
    Ok((sources, total_bytes))
}

fn consume_descriptor(
    descriptor: SourceBundleSource,
    buffers: &mut [Vec<u8>],
    used_slots: &mut [bool],
    limits: SourceBundleLimits,
) -> Result<(String, SourceFile, usize), SourceBundleError> {
    let path = normalize_bundle_path(&descriptor.path, limits.max_path_bytes)?;
    validate_kind_suffix(&descriptor, &path)?;
    let slot = descriptor.slot.0 as usize;
    if slot >= buffers.len() || used_slots[slot] {
        return Err(SourceBundleError::new(
            SourceBundleErrorKind::Slot,
            Some(&path),
            "source slots must be unique and in range",
        ));
    }
    let declared_bytes = descriptor.source_bytes.parse::<usize>().map_err(|_| {
        SourceBundleError::new(
            SourceBundleErrorKind::Contract,
            Some(&path),
            "source_bytes must be a platform-sized decimal string",
        )
    })?;
    let actual_bytes = buffers[slot].len();
    if declared_bytes != actual_bytes {
        return Err(SourceBundleError::new(
            SourceBundleErrorKind::Contract,
            Some(&path),
            "source_bytes does not match its byte slot",
        ));
    }
    if actual_bytes > limits.max_source_bytes {
        return Err(limit_error_for(&path, "source exceeds max_source_bytes"));
    }
    used_slots[slot] = true;
    let source = SourceFile {
        path: path.clone(),
        kind: descriptor.kind,
        bytes: std::mem::take(&mut buffers[slot]),
    };
    Ok((path, source, actual_bytes))
}

fn checked_bundle_total(
    current: usize,
    additional: usize,
    maximum: usize,
) -> Result<usize, SourceBundleError> {
    let total = current
        .checked_add(additional)
        .ok_or_else(|| limit_error("source bundle byte total overflowed the platform size"))?;
    if total > maximum {
        Err(limit_error("source bundle exceeds max_total_bytes"))
    } else {
        Ok(total)
    }
}

fn require_all_slots(used_slots: &[bool]) -> Result<(), SourceBundleError> {
    if used_slots.iter().all(|used| *used) {
        Ok(())
    } else {
        Err(SourceBundleError::new(
            SourceBundleErrorKind::Slot,
            None,
            "every byte slot must have exactly one descriptor",
        ))
    }
}

fn validate_kind_suffix(
    descriptor: &SourceBundleSource,
    path: &str,
) -> Result<(), SourceBundleError> {
    let valid = match descriptor.kind {
        SourceKind::Project => path_ends_with_ascii_case(path, ".kicad_pro"),
        SourceKind::Schematic => path_ends_with_ascii_case(path, ".kicad_sch"),
        SourceKind::SymbolLibrary => path_ends_with_ascii_case(path, ".kicad_sym"),
        SourceKind::SymbolTable => {
            path_ends_with_ascii_case(path, "sym-lib-table")
                || path_ends_with_ascii_case(path, "fp-lib-table")
        }
        SourceKind::Worksheet => path_ends_with_ascii_case(path, ".kicad_wks"),
        SourceKind::Other => true,
    };
    if valid {
        Ok(())
    } else {
        Err(SourceBundleError::new(
            SourceBundleErrorKind::Kind,
            Some(path),
            "source kind does not match its KiCad filename",
        ))
    }
}

fn path_ends_with_ascii_case(path: &str, suffix: &str) -> bool {
    path.get(path.len().saturating_sub(suffix.len())..)
        .is_some_and(|value| value.eq_ignore_ascii_case(suffix))
}

fn validate_project_json(source: &SourceFile) -> Result<(), SourceBundleError> {
    let text = source.text()?;
    if text.trim_start().as_bytes().first() != Some(&b'{') {
        return Err(SourceBundleError::new(
            SourceBundleErrorKind::Project,
            Some(source.path()),
            "KiCad project JSON root must be an object",
        ));
    }
    let mut deserializer = serde_json::Deserializer::from_str(text);
    IgnoredAny::deserialize(&mut deserializer).map_err(|error| {
        SourceBundleError::new(
            SourceBundleErrorKind::Project,
            Some(source.path()),
            format!("invalid KiCad project JSON: {error}"),
        )
    })?;
    deserializer.end().map_err(|error| {
        SourceBundleError::new(
            SourceBundleErrorKind::Project,
            Some(source.path()),
            format!("invalid trailing KiCad project JSON: {error}"),
        )
    })
}

fn require_source_kind(
    sources: &BTreeMap<String, SourceFile>,
    path: &str,
    kind: SourceKind,
) -> Result<(), SourceBundleError> {
    let source = sources.get(path).ok_or_else(|| {
        SourceBundleError::new(
            SourceBundleErrorKind::MissingSource,
            Some(path),
            "manifest entry path is absent from the source inventory",
        )
    })?;
    if source.kind == kind {
        Ok(())
    } else {
        Err(SourceBundleError::new(
            SourceBundleErrorKind::Kind,
            Some(path),
            "manifest entry path has the wrong source kind",
        ))
    }
}

pub(crate) fn resolve_relative_path(
    parent_path: &str,
    reference: &str,
    max_path_bytes: usize,
) -> Result<String, SourceBundleError> {
    let parent = normalize_bundle_path(parent_path, max_path_bytes)?;
    let directory = parent
        .rsplit_once('/')
        .map_or("", |(directory, _)| directory);
    let joined = if directory.is_empty() {
        reference.to_owned()
    } else {
        format!("{directory}/{reference}")
    };
    normalize_bundle_path(&joined, max_path_bytes)
}

pub(crate) fn normalize_bundle_path(
    value: &str,
    max_path_bytes: usize,
) -> Result<String, SourceBundleError> {
    if value.is_empty() {
        return Err(invalid_path_text(value));
    }
    if value.len() > max_path_bytes {
        return Err(invalid_path_text(value));
    }
    if value.contains('\0') {
        return Err(invalid_path_text(value));
    }
    let portable = value.replace('\\', "/");
    if portable.starts_with('/') {
        return Err(non_relative_path(value));
    }
    if portable
        .split('/')
        .next()
        .is_some_and(|part| part.contains(':'))
    {
        return Err(non_relative_path(value));
    }
    normalize_path_parts(&portable, value)
}

fn normalize_path_parts(portable: &str, original: &str) -> Result<String, SourceBundleError> {
    let mut parts = Vec::new();
    for part in portable.split('/') {
        match part {
            "" | "." => {}
            ".." => pop_path_part(&mut parts, original)?,
            _ => parts.push(part),
        }
    }
    if parts.is_empty() {
        return Err(SourceBundleError::new(
            SourceBundleErrorKind::Path,
            Some(original),
            "source path resolves to the bundle root",
        ));
    }
    Ok(parts.join("/"))
}

fn pop_path_part(parts: &mut Vec<&str>, original: &str) -> Result<(), SourceBundleError> {
    if parts.pop().is_none() {
        Err(SourceBundleError::new(
            SourceBundleErrorKind::Path,
            Some(original),
            "source path escapes the bundle root",
        ))
    } else {
        Ok(())
    }
}

fn invalid_path_text(value: &str) -> SourceBundleError {
    SourceBundleError::new(
        SourceBundleErrorKind::Path,
        Some(value),
        "source path is empty, contains NUL, or exceeds max_path_bytes",
    )
}

fn non_relative_path(value: &str) -> SourceBundleError {
    SourceBundleError::new(
        SourceBundleErrorKind::Path,
        Some(value),
        "source paths must be relative portable paths",
    )
}

/// Stable category for source-boundary diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceBundleErrorKind {
    Contract,
    ResourceLimit,
    Path,
    Slot,
    MissingSource,
    Kind,
    Utf8,
    Project,
    Schematic,
    HierarchyCycle,
}

/// Source-bundle or hierarchy preparation failure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SourceBundleError {
    pub kind: SourceBundleErrorKind,
    pub source_path: Option<String>,
    pub message: String,
}

impl SourceBundleError {
    pub(crate) fn new(
        kind: SourceBundleErrorKind,
        source_path: Option<&str>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            source_path: source_path.map(str::to_owned),
            message: message.into(),
        }
    }
}

impl fmt::Display for SourceBundleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(path) = &self.source_path {
            write!(formatter, "{:?} in {path}: {}", self.kind, self.message)
        } else {
            write!(formatter, "{:?}: {}", self.kind, self.message)
        }
    }
}

impl std::error::Error for SourceBundleError {}

fn limit_error(message: impl Into<String>) -> SourceBundleError {
    SourceBundleError::new(SourceBundleErrorKind::ResourceLimit, None, message)
}

fn limit_error_for(path: &str, message: impl Into<String>) -> SourceBundleError {
    SourceBundleError::new(SourceBundleErrorKind::ResourceLimit, Some(path), message)
}
