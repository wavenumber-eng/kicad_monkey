use super::{
    SchematicPoint, carrier_form_spans, child_point, child_scalar, direct_scalars, limit_error,
    parse_symbol_instances, source_error,
};
use crate::schematic_bundle::SchematicBundleLimits;
use crate::sexpr_projection::FormSpan;
use crate::source_bundle::SourceBundleError;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use super::SchematicSymbolInstance;

#[derive(Clone, Debug, PartialEq)]
pub struct SchematicPlacedSymbol {
    pub lib_id: String,
    pub lib_name: String,
    pub at: SchematicPoint,
    pub angle_degrees: f64,
    pub mirror: Option<String>,
    pub unit: i64,
    pub convert: i64,
    pub exclude_from_sim: bool,
    pub in_bom: bool,
    pub on_board: bool,
    pub in_pos_files: bool,
    pub dnp: bool,
    pub fields_autoplaced: bool,
    pub uuid: String,
    pub properties: Vec<SchematicSymbolProperty>,
    pub pins: Vec<SchematicSymbolPin>,
    pub instances: Vec<SchematicSymbolInstance>,
    instance_by_project_path: HashMap<Arc<str>, HashMap<Arc<str>, InstanceIndexEntry>>,
    instance_by_path: HashMap<Arc<str>, InstanceIndexEntry>,
    suffix_index: InstanceSuffixIndex,
    first_instance_by_project: HashMap<Arc<str>, usize>,
}

impl SchematicPlacedSymbol {
    pub fn instance_for_project(
        &self,
        project: &str,
        path: &str,
    ) -> Result<Option<&SchematicSymbolInstance>, SchematicSymbolInstanceLookupError> {
        let entry = self
            .instance_by_project_path
            .get(project)
            .and_then(|paths| paths.get(normalized_path(path)));
        self.resolve_instance_entry(entry)
    }

    pub fn unique_instance_for_path(
        &self,
        path: &str,
    ) -> Result<Option<&SchematicSymbolInstance>, SchematicSymbolInstanceLookupError> {
        self.resolve_instance_entry(self.instance_by_path.get(normalized_path(path)))
    }

    pub(crate) fn compatible_instance_for_project(
        &self,
        project: &str,
        path: &str,
    ) -> Result<Option<&SchematicSymbolInstance>, SchematicSymbolInstanceLookupError> {
        let (ordered, prefixes) = self
            .suffix_index
            .by_project
            .get(project)
            .and_then(|ordered| {
                self.suffix_index
                    .prefixes_by_project
                    .get(project)
                    .map(|prefixes| (ordered, prefixes))
            })
            .unwrap_or((&self.suffix_index.all, &self.suffix_index.prefixes_all));
        let entry = self.suffix_index.query(path, ordered, prefixes);
        self.resolve_instance_entry(entry.as_ref())
    }

    pub(crate) fn first_instance_for_project(
        &self,
        project: &str,
    ) -> Option<&SchematicSymbolInstance> {
        self.first_instance_by_project
            .get(project)
            .map(|index| &self.instances[*index])
            .or_else(|| self.instances.first())
    }

    fn resolve_instance_entry(
        &self,
        entry: Option<&InstanceIndexEntry>,
    ) -> Result<Option<&SchematicSymbolInstance>, SchematicSymbolInstanceLookupError> {
        let Some(entry) = entry else {
            return Ok(None);
        };
        if entry.matches > 1 {
            return Err(SchematicSymbolInstanceLookupError {
                matches: entry.matches,
            });
        }
        Ok(Some(&self.instances[entry.index]))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SchematicSymbolInstanceLookupError {
    pub matches: usize,
}

impl fmt::Display for SchematicSymbolInstanceLookupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "symbol instance lookup is ambiguous across {} records",
            self.matches
        )
    }
}

impl std::error::Error for SchematicSymbolInstanceLookupError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InstanceIndexEntry {
    index: usize,
    matches: usize,
}

impl InstanceIndexEntry {
    fn merge(self, other: Self) -> Self {
        Self {
            index: self.index.min(other.index),
            matches: self.matches.saturating_add(other.matches),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct InstanceSuffixIndex {
    reversed_bytes: Vec<u8>,
    reversed_ranges: Vec<InstancePathRange>,
    by_project: HashMap<Arc<str>, Vec<usize>>,
    all: Vec<usize>,
    prefixes_by_project: HashMap<Arc<str>, HashMap<PathFingerprint, FingerprintBucket>>,
    prefixes_all: HashMap<PathFingerprint, FingerprintBucket>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct InstancePathRange {
    start: usize,
    end: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct PathFingerprint {
    first: u64,
    second: u64,
    length: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FingerprintBucket {
    One(InstanceIndexEntry),
    Collision(Vec<InstanceIndexEntry>),
}

#[derive(Debug, Default)]
struct SchematicSymbolInstanceIndexes {
    by_project_path: HashMap<Arc<str>, HashMap<Arc<str>, InstanceIndexEntry>>,
    by_path: HashMap<Arc<str>, InstanceIndexEntry>,
    suffix_index: InstanceSuffixIndex,
    first_by_project: HashMap<Arc<str>, usize>,
}

impl InstanceSuffixIndex {
    fn insert(&mut self, path: &str, project: &Arc<str>, instance_index: usize) {
        let path = normalized_path(path);
        let start = self.reversed_bytes.len();
        self.reversed_bytes.extend(path.bytes().rev());
        self.reversed_ranges.push(InstancePathRange {
            start,
            end: self.reversed_bytes.len(),
        });
        let project_paths = self.by_project.entry(Arc::clone(project)).or_default();
        if path.is_empty() {
            return;
        }
        project_paths.push(instance_index);
        self.all.push(instance_index);
    }

    fn finish(
        &mut self,
        by_project_path: &HashMap<Arc<str>, HashMap<Arc<str>, InstanceIndexEntry>>,
        by_path: &HashMap<Arc<str>, InstanceIndexEntry>,
    ) {
        let bytes = &self.reversed_bytes;
        let ranges = &self.reversed_ranges;
        let compare = |left: &usize, right: &usize| {
            reversed_path(bytes, ranges, *left)
                .cmp(reversed_path(bytes, ranges, *right))
                .then(left.cmp(right))
        };
        self.all.sort_unstable_by(compare);
        for ordered in self.by_project.values_mut() {
            ordered.sort_unstable_by(compare);
        }
        self.prefixes_all = fingerprint_index(by_path, bytes, ranges);
        for (project, paths) in by_project_path {
            self.prefixes_by_project
                .insert(Arc::clone(project), fingerprint_index(paths, bytes, ranges));
        }
    }

    fn query(
        &self,
        path: &str,
        ordered: &[usize],
        prefixes: &HashMap<PathFingerprint, FingerprintBucket>,
    ) -> Option<InstanceIndexEntry> {
        let path = normalized_path(path);
        if path.is_empty() {
            return None;
        }
        let reversed = path.bytes().rev().collect::<Vec<_>>();
        let mut result = None;
        let mut fingerprint = PathFingerprint::default();
        for (offset, byte) in reversed.iter().copied().enumerate() {
            fingerprint.push(byte);
            if offset + 1 < reversed.len()
                && let Some(bucket) = prefixes.get(&fingerprint)
                && let Some(entry) = bucket.matching(&reversed[..=offset], self)
            {
                merge_index_entry(&mut result, entry);
            }
        }
        let start =
            ordered.partition_point(|index| self.reversed_path(*index) < reversed.as_slice());
        let end = prefix_successor(&reversed).map_or(ordered.len(), |upper| {
            ordered.partition_point(|index| self.reversed_path(*index) < upper.as_slice())
        });
        if end > start {
            merge_index_entry(
                &mut result,
                InstanceIndexEntry {
                    index: ordered[start],
                    matches: end - start,
                },
            );
        }
        result
    }

    fn reversed_path(&self, instance_index: usize) -> &[u8] {
        reversed_path(&self.reversed_bytes, &self.reversed_ranges, instance_index)
    }
}

fn prefix_successor(prefix: &[u8]) -> Option<Vec<u8>> {
    let mut successor = prefix.to_vec();
    while successor.last() == Some(&u8::MAX) {
        successor.pop();
    }
    let last = successor.last_mut()?;
    *last += 1;
    Some(successor)
}

impl Default for PathFingerprint {
    fn default() -> Self {
        Self {
            first: 0xcbf2_9ce4_8422_2325,
            second: 0x8422_2325_cbf2_9ce4,
            length: 0,
        }
    }
}

impl PathFingerprint {
    fn push(&mut self, byte: u8) {
        self.first = self
            .first
            .wrapping_mul(0x0000_0100_0000_01b3)
            .wrapping_add(u64::from(byte) + 1);
        self.second = self
            .second
            .wrapping_mul(0x9e37_79b1_85eb_ca87)
            .wrapping_add(u64::from(byte) + 0x100);
        self.length += 1;
    }

    fn for_reversed_path(path: &[u8]) -> Self {
        let mut fingerprint = Self::default();
        for byte in path {
            fingerprint.push(*byte);
        }
        fingerprint
    }
}

impl FingerprintBucket {
    fn matching(
        &self,
        path: &[u8],
        suffix_index: &InstanceSuffixIndex,
    ) -> Option<InstanceIndexEntry> {
        match self {
            Self::One(entry) => (suffix_index.reversed_path(entry.index) == path).then_some(*entry),
            Self::Collision(entries) => entries
                .iter()
                .find(|entry| suffix_index.reversed_path(entry.index) == path)
                .copied(),
        }
    }
}

fn fingerprint_index(
    paths: &HashMap<Arc<str>, InstanceIndexEntry>,
    reversed_bytes: &[u8],
    reversed_ranges: &[InstancePathRange],
) -> HashMap<PathFingerprint, FingerprintBucket> {
    let mut index = HashMap::with_capacity(paths.len());
    for entry in paths
        .values()
        .filter(|entry| !reversed_path(reversed_bytes, reversed_ranges, entry.index).is_empty())
    {
        let path = reversed_path(reversed_bytes, reversed_ranges, entry.index);
        let fingerprint = PathFingerprint::for_reversed_path(path);
        index
            .entry(fingerprint)
            .and_modify(|bucket| match bucket {
                FingerprintBucket::One(first) => {
                    *bucket = FingerprintBucket::Collision(vec![*first, *entry]);
                }
                FingerprintBucket::Collision(entries) => entries.push(*entry),
            })
            .or_insert(FingerprintBucket::One(*entry));
    }
    index
}

fn reversed_path<'a>(
    bytes: &'a [u8],
    ranges: &[InstancePathRange],
    instance_index: usize,
) -> &'a [u8] {
    let range = ranges[instance_index];
    &bytes[range.start..range.end]
}

fn merge_index_entry(target: &mut Option<InstanceIndexEntry>, entry: InstanceIndexEntry) {
    *target = Some(target.map_or(entry, |current| current.merge(entry)));
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicSymbolProperty {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicSymbolPin {
    pub number: String,
    pub uuid: String,
    pub alternate: Option<String>,
}

pub(crate) fn parse_placed_symbols(
    source: &str,
    source_path: &str,
    spans: &[FormSpan],
    limits: SchematicBundleLimits,
) -> Result<Vec<SchematicPlacedSymbol>, SourceBundleError> {
    let mut symbols = Vec::new();
    let mut retained_instance_index_bytes = 0_usize;
    for span in spans
        .iter()
        .filter(|span| span.depth == 1 && span.head.as_deref() == Some("symbol"))
    {
        if symbols.len() >= limits.max_symbols_per_source {
            return Err(limit_error(
                source_path,
                "placed symbol count exceeds its limit",
            ));
        }
        let text = span
            .text(source)
            .map_err(|error| source_error(source_path, error.to_string()))?;
        symbols.push(parse_symbol(
            text,
            source_path,
            limits,
            &mut retained_instance_index_bytes,
        )?);
    }
    Ok(symbols)
}

fn parse_symbol(
    source: &str,
    source_path: &str,
    limits: SchematicBundleLimits,
    retained_instance_index_bytes: &mut usize,
) -> Result<SchematicPlacedSymbol, SourceBundleError> {
    let selected_limit = limits
        .max_symbol_properties_per_symbol
        .saturating_mul(2)
        .saturating_add(limits.max_symbol_pins_per_symbol.saturating_mul(3))
        .saturating_add(20);
    let spans = carrier_form_spans(
        source,
        &[
            "symbol",
            "lib_id",
            "lib_name",
            "at",
            "mirror",
            "unit",
            "convert",
            "exclude_from_sim",
            "in_bom",
            "on_board",
            "in_pos_files",
            "dnp",
            "fields_autoplaced",
            "uuid",
            "property",
            "pin",
            "instances",
        ],
        source_path,
        limits,
        selected_limit,
    )?;
    let at_span = child(&spans, "at");
    let at = at_span.map_or(Ok(SchematicPoint { x_iu: 0, y_iu: 0 }), |span| {
        child_point_from_span(source, span, source_path, limits)
    })?;
    let angle_degrees = at_span.map_or(Ok(0.0), |span| {
        let values = direct_scalars(source, span, 3, source_path, limits)?;
        values
            .get(2)
            .map_or(Ok(0.0), |value| finite_f64(value, source_path))
    })?;
    let instances =
        parse_symbol_instances(source, child(&spans, "instances"), source_path, limits)?;
    let indexes = index_symbol_instances(
        &instances,
        limits.max_symbol_instance_index_bytes_per_source,
        retained_instance_index_bytes,
        source_path,
    )?;
    Ok(SchematicPlacedSymbol {
        lib_id: scalar(source, &spans, "lib_id", source_path, limits)?.unwrap_or_default(),
        lib_name: scalar(source, &spans, "lib_name", source_path, limits)?.unwrap_or_default(),
        at,
        angle_degrees,
        mirror: scalar(source, &spans, "mirror", source_path, limits)?,
        unit: integer(source, &spans, "unit", 1, source_path, limits)?,
        convert: integer(source, &spans, "convert", 1, source_path, limits)?,
        exclude_from_sim: boolean(
            source,
            &spans,
            "exclude_from_sim",
            false,
            source_path,
            limits,
        )?,
        in_bom: boolean(source, &spans, "in_bom", true, source_path, limits)?,
        on_board: boolean(source, &spans, "on_board", true, source_path, limits)?,
        in_pos_files: boolean(source, &spans, "in_pos_files", true, source_path, limits)?,
        dnp: boolean(source, &spans, "dnp", false, source_path, limits)?,
        fields_autoplaced: maybe_absent_boolean(
            source,
            &spans,
            "fields_autoplaced",
            source_path,
            limits,
        )?,
        uuid: scalar(source, &spans, "uuid", source_path, limits)?.unwrap_or_default(),
        properties: parse_properties(source, &spans, source_path, limits)?,
        pins: parse_pins(source, &spans, source_path, limits)?,
        instances,
        instance_by_project_path: indexes.by_project_path,
        instance_by_path: indexes.by_path,
        suffix_index: indexes.suffix_index,
        first_instance_by_project: indexes.first_by_project,
    })
}

fn index_symbol_instances(
    instances: &[SchematicSymbolInstance],
    max_index_bytes: usize,
    retained_bytes: &mut usize,
    source_path: &str,
) -> Result<SchematicSymbolInstanceIndexes, SourceBundleError> {
    let mut indexes = SchematicSymbolInstanceIndexes {
        by_path: HashMap::with_capacity(instances.len()),
        ..SchematicSymbolInstanceIndexes::default()
    };
    for (index, instance) in instances.iter().enumerate() {
        let path = normalized_path(&instance.path);
        let required = instance_index_bytes(path).ok_or_else(|| {
            limit_error(source_path, "symbol instance index byte count overflows")
        })?;
        let candidate_bytes = retained_bytes.checked_add(required).ok_or_else(|| {
            limit_error(source_path, "symbol instance index byte count overflows")
        })?;
        if candidate_bytes > max_index_bytes {
            return Err(limit_error(
                source_path,
                "symbol instance index bytes exceed their limit",
            ));
        }
        *retained_bytes = candidate_bytes;
        let path_key = indexes
            .by_path
            .get_key_value(path)
            .map_or_else(|| Arc::<str>::from(path), |(key, _)| Arc::clone(key));
        update_instance_index(
            indexes
                .by_project_path
                .entry(Arc::clone(&instance.project))
                .or_default(),
            Arc::clone(&path_key),
            index,
        );
        update_instance_index(&mut indexes.by_path, path_key, index);
        indexes.suffix_index.insert(path, &instance.project, index);
        indexes
            .first_by_project
            .entry(Arc::clone(&instance.project))
            .or_insert(index);
    }
    indexes
        .suffix_index
        .finish(&indexes.by_project_path, &indexes.by_path);
    Ok(indexes)
}

fn update_instance_index(
    index: &mut HashMap<Arc<str>, InstanceIndexEntry>,
    path: Arc<str>,
    instance_index: usize,
) {
    index
        .entry(path)
        .and_modify(|entry| entry.matches = entry.matches.saturating_add(1))
        .or_insert(InstanceIndexEntry {
            index: instance_index,
            matches: 1,
        });
}

fn instance_index_bytes(path: &str) -> Option<usize> {
    let range_bytes = std::mem::size_of::<InstancePathRange>();
    if path.is_empty() {
        return Some(range_bytes);
    }
    path.len()
        .checked_mul(2)
        .and_then(|bytes| bytes.checked_add(range_bytes))
        .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<usize>()))
        .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<PathFingerprint>()))
        .and_then(|bytes| bytes.checked_add(2 * std::mem::size_of::<InstanceIndexEntry>()))
}

fn normalized_path(path: &str) -> &str {
    path.trim_end_matches('/')
}

fn parse_properties(
    source: &str,
    spans: &[FormSpan],
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<Vec<SchematicSymbolProperty>, SourceBundleError> {
    let mut properties = Vec::new();
    for span in direct_children(spans, "property") {
        if properties.len() >= limits.max_symbol_properties_per_symbol {
            return Err(limit_error(
                source_path,
                "symbol property count exceeds its limit",
            ));
        }
        let values = direct_scalars(source, span, 2, source_path, limits)?;
        properties.push(SchematicSymbolProperty {
            key: values.first().cloned().unwrap_or_default(),
            value: values.get(1).cloned().unwrap_or_default(),
        });
    }
    Ok(properties)
}

fn parse_pins(
    source: &str,
    spans: &[FormSpan],
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<Vec<SchematicSymbolPin>, SourceBundleError> {
    let mut pins = Vec::new();
    for span in direct_children(spans, "pin") {
        if pins.len() >= limits.max_symbol_pins_per_symbol {
            return Err(limit_error(
                source_path,
                "symbol pin count exceeds its limit",
            ));
        }
        let text = span
            .text(source)
            .map_err(|error| source_error(source_path, error.to_string()))?;
        let nested =
            carrier_form_spans(text, &["pin", "uuid", "alternate"], source_path, limits, 4)?;
        let root = nested
            .iter()
            .find(|selected| selected.depth == 0)
            .ok_or_else(|| source_error(source_path, "symbol pin root is missing"))?;
        let number = direct_scalars(text, root, 1, source_path, limits)?
            .into_iter()
            .next()
            .unwrap_or_default();
        pins.push(SchematicSymbolPin {
            number,
            uuid: child_scalar(text, &nested, "uuid", source_path, limits)?.unwrap_or_default(),
            alternate: child_scalar(text, &nested, "alternate", source_path, limits)?,
        });
    }
    Ok(pins)
}

fn child_point_from_span(
    source: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<SchematicPoint, SourceBundleError> {
    let local = [span.clone()];
    child_point(
        source,
        &local,
        span.head.as_deref().unwrap_or("at"),
        SchematicPoint { x_iu: 0, y_iu: 0 },
        source_path,
        limits,
    )
}

fn direct_children<'a>(spans: &'a [FormSpan], head: &'a str) -> impl Iterator<Item = &'a FormSpan> {
    spans
        .iter()
        .filter(move |span| span.depth == 1 && span.head.as_deref() == Some(head))
}

fn child<'a>(spans: &'a [FormSpan], head: &str) -> Option<&'a FormSpan> {
    spans
        .iter()
        .find(|span| span.depth == 1 && span.head.as_deref() == Some(head))
}

fn scalar(
    source: &str,
    spans: &[FormSpan],
    head: &str,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<Option<String>, SourceBundleError> {
    child_scalar(source, spans, head, source_path, limits)
}

fn integer(
    source: &str,
    spans: &[FormSpan],
    head: &str,
    default: i64,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<i64, SourceBundleError> {
    scalar(source, spans, head, source_path, limits)?.map_or(Ok(default), |value| {
        value
            .parse::<i64>()
            .map_err(|_| source_error(source_path, format!("invalid {head} integer")))
    })
}

fn boolean(
    source: &str,
    spans: &[FormSpan],
    head: &str,
    default: bool,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<bool, SourceBundleError> {
    Ok(scalar(source, spans, head, source_path, limits)?.map_or(default, |value| value == "yes"))
}

fn maybe_absent_boolean(
    source: &str,
    spans: &[FormSpan],
    head: &str,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<bool, SourceBundleError> {
    let Some(span) = child(spans, head) else {
        return Ok(false);
    };
    Ok(direct_scalars(source, span, 1, source_path, limits)?
        .first()
        .is_none_or(|value| value == "yes"))
}

fn finite_f64(value: &str, source_path: &str) -> Result<f64, SourceBundleError> {
    let value = value
        .parse::<f64>()
        .map_err(|_| source_error(source_path, "invalid symbol angle"))?;
    if value.is_finite() {
        Ok(value)
    } else {
        Err(source_error(source_path, "symbol angle must be finite"))
    }
}
