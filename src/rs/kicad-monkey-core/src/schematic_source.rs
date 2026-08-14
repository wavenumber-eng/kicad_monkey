//! Typed schematic connectivity carriers and deterministic point connectivity.

use crate::schematic_bundle::SchematicBundleLimits;
use crate::sexpr::{Lexer, Token, TokenKind, decode_quoted};
use crate::sexpr_projection::{FormSpan, ProjectionLimits, Selector, scan_form_spans_with_limits};
use crate::source_bundle::{SourceBundleError, SourceBundleErrorKind};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// KiCad's schematic internal-unit grid: 100 nm, or 10,000 units per millimetre.
pub const SCHEMATIC_IU_PER_MM: i64 = 10_000;

/// Exact schematic-grid coordinate used by connectivity rather than floating point.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SchematicPoint {
    pub x_iu: i64,
    pub y_iu: i64,
}

impl SchematicPoint {
    pub fn x_mm(self) -> f64 {
        self.x_iu as f64 / SCHEMATIC_IU_PER_MM as f64
    }

    pub fn y_mm(self) -> f64 {
        self.y_iu as f64 / SCHEMATIC_IU_PER_MM as f64
    }
}

/// A source wire or bus polyline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicPolyline {
    pub uuid: String,
    pub points: Vec<SchematicPoint>,
}

/// The stored origin and diagonal size of a bus entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicBusEntry {
    pub uuid: String,
    pub at: SchematicPoint,
    pub size: SchematicPoint,
}

impl SchematicBusEntry {
    pub fn wire_end(&self) -> Option<SchematicPoint> {
        Some(SchematicPoint {
            x_iu: self.at.x_iu.checked_add(self.size.x_iu)?,
            y_iu: self.at.y_iu.checked_add(self.size.y_iu)?,
        })
    }
}

/// A junction marker at one exact schematic coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicJunction {
    pub uuid: String,
    pub at: SchematicPoint,
}

/// An intentional no-connect marker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicNoConnect {
    pub uuid: String,
    pub at: SchematicPoint,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SchematicLabelScope {
    Local,
    Global,
    Hierarchical,
}

/// Connectivity-relevant fields shared by local, global, and hierarchical labels.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchematicLabel {
    pub scope: SchematicLabelScope,
    pub text: String,
    pub shape: String,
    pub uuid: String,
    pub at: SchematicPoint,
}

/// Deterministic connected components over wire endpoints and registered carrier points.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SchematicConnectivity {
    points: Vec<SchematicPoint>,
    components: Vec<Vec<SchematicPoint>>,
    component_by_point: BTreeMap<SchematicPoint, usize>,
}

impl SchematicConnectivity {
    pub fn points(&self) -> impl ExactSizeIterator<Item = &SchematicPoint> {
        self.points.iter()
    }

    pub fn components(&self) -> impl ExactSizeIterator<Item = &[SchematicPoint]> {
        self.components.iter().map(Vec::as_slice)
    }

    pub fn component(&self, point: SchematicPoint) -> Option<&[SchematicPoint]> {
        self.component_by_point
            .get(&point)
            .map(|index| self.components[*index].as_slice())
    }

    pub fn connected(&self, left: SchematicPoint, right: SchematicPoint) -> bool {
        self.component_by_point
            .get(&left)
            .is_some_and(|left_index| self.component_by_point.get(&right) == Some(left_index))
    }
}

#[derive(Debug, Default)]
pub(crate) struct SchematicSourceCarriers {
    pub wires: Vec<SchematicPolyline>,
    pub buses: Vec<SchematicPolyline>,
    pub bus_entries: Vec<SchematicBusEntry>,
    pub junctions: Vec<SchematicJunction>,
    pub no_connects: Vec<SchematicNoConnect>,
    pub labels: Vec<SchematicLabel>,
    pub connectivity: SchematicConnectivity,
}

pub(crate) fn parse_source_carriers(
    source: &str,
    source_path: &str,
    spans: &[FormSpan],
    limits: SchematicBundleLimits,
) -> Result<SchematicSourceCarriers, SourceBundleError> {
    let carrier_count = spans
        .iter()
        .filter(|span| span.depth == 1 && is_carrier(span.head.as_deref()))
        .count();
    if carrier_count > limits.max_connectivity_objects_per_source {
        return Err(limit_error(
            source_path,
            "schematic connectivity object count exceeds its limit",
        ));
    }
    let mut carriers = SchematicSourceCarriers::default();
    let mut point_count = 0_usize;
    for span in spans
        .iter()
        .filter(|span| span.depth == 1 && is_carrier(span.head.as_deref()))
    {
        parse_carrier(
            &mut carriers,
            &mut point_count,
            source,
            span,
            source_path,
            limits,
        )?;
    }
    carriers.connectivity = build_connectivity(&carriers, source_path)?;
    Ok(carriers)
}

fn parse_carrier(
    carriers: &mut SchematicSourceCarriers,
    point_count: &mut usize,
    source: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<(), SourceBundleError> {
    let text = span
        .text(source)
        .map_err(|error| source_error(source_path, error.to_string()))?;
    match span.head.as_deref() {
        Some("wire") => {
            require_family_capacity(
                carriers.wires.len(),
                limits.max_wires_per_source,
                source_path,
                "wire",
            )?;
            let value = parse_polyline(
                text,
                "wire",
                source_path,
                limits,
                remaining_points(*point_count, limits),
            )?;
            add_points(point_count, value.points.len(), source_path, limits)?;
            carriers.wires.push(value);
        }
        Some("bus") => {
            require_family_capacity(
                carriers.buses.len(),
                limits.max_buses_per_source,
                source_path,
                "bus",
            )?;
            let value = parse_polyline(
                text,
                "bus",
                source_path,
                limits,
                remaining_points(*point_count, limits),
            )?;
            add_points(point_count, value.points.len(), source_path, limits)?;
            carriers.buses.push(value);
        }
        Some("bus_entry") => {
            require_family_capacity(
                carriers.bus_entries.len(),
                limits.max_bus_entries_per_source,
                source_path,
                "bus-entry",
            )?;
            add_points(point_count, 2, source_path, limits)?;
            carriers
                .bus_entries
                .push(parse_bus_entry(text, source_path, limits)?);
        }
        Some("junction") => {
            require_family_capacity(
                carriers.junctions.len(),
                limits.max_junctions_per_source,
                source_path,
                "junction",
            )?;
            add_points(point_count, 1, source_path, limits)?;
            carriers
                .junctions
                .push(parse_point_marker(text, "junction", source_path, limits)?);
        }
        Some("no_connect") => {
            require_family_capacity(
                carriers.no_connects.len(),
                limits.max_no_connects_per_source,
                source_path,
                "no-connect",
            )?;
            add_points(point_count, 1, source_path, limits)?;
            let marker = parse_point_marker(text, "no_connect", source_path, limits)?;
            carriers.no_connects.push(SchematicNoConnect {
                uuid: marker.uuid,
                at: marker.at,
            });
        }
        Some(head @ ("label" | "global_label" | "hierarchical_label")) => {
            require_family_capacity(
                carriers.labels.len(),
                limits.max_labels_per_source,
                source_path,
                "label",
            )?;
            add_points(point_count, 1, source_path, limits)?;
            carriers
                .labels
                .push(parse_label(text, head, source_path, limits)?);
        }
        _ => {}
    }
    Ok(())
}

fn require_family_capacity(
    current: usize,
    maximum: usize,
    source_path: &str,
    family: &str,
) -> Result<(), SourceBundleError> {
    if current >= maximum {
        Err(limit_error(
            source_path,
            format!("schematic {family} count exceeds its limit"),
        ))
    } else {
        Ok(())
    }
}

fn remaining_points(current: usize, limits: SchematicBundleLimits) -> usize {
    limits
        .max_connectivity_points_per_source
        .saturating_sub(current)
        .min(limits.max_points_per_connectivity_object)
}

fn is_carrier(head: Option<&str>) -> bool {
    matches!(
        head,
        Some(
            "wire"
                | "bus"
                | "bus_entry"
                | "junction"
                | "no_connect"
                | "label"
                | "global_label"
                | "hierarchical_label"
        )
    )
}

fn add_points(
    count: &mut usize,
    additional: usize,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<(), SourceBundleError> {
    *count = count
        .checked_add(additional)
        .ok_or_else(|| limit_error(source_path, "schematic connectivity point count overflowed"))?;
    if *count > limits.max_connectivity_points_per_source {
        return Err(limit_error(
            source_path,
            "schematic connectivity point count exceeds its limit",
        ));
    }
    Ok(())
}

fn parse_polyline(
    source: &str,
    root_head: &str,
    source_path: &str,
    limits: SchematicBundleLimits,
    maximum_points: usize,
) -> Result<SchematicPolyline, SourceBundleError> {
    let spans = carrier_form_spans(
        source,
        &[root_head, "xy", "uuid"],
        source_path,
        limits,
        maximum_points.saturating_add(3),
    )?;
    let mut points = Vec::new();
    for span in spans
        .iter()
        .filter(|span| span.depth == 2 && span.head.as_deref() == Some("xy"))
    {
        if points.len() >= maximum_points {
            return Err(limit_error(
                source_path,
                "schematic polyline point count exceeds its limit",
            ));
        }
        points.push(parse_point(source, span, source_path, limits)?);
    }
    Ok(SchematicPolyline {
        uuid: child_scalar(source, &spans, "uuid", source_path, limits)?.unwrap_or_default(),
        points,
    })
}

fn parse_bus_entry(
    source: &str,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<SchematicBusEntry, SourceBundleError> {
    let spans = carrier_form_spans(
        source,
        &["bus_entry", "at", "size", "uuid"],
        source_path,
        limits,
        8,
    )?;
    let at = child_point(
        source,
        &spans,
        "at",
        SchematicPoint { x_iu: 0, y_iu: 0 },
        source_path,
        limits,
    )?;
    let size = child_point(
        source,
        &spans,
        "size",
        SchematicPoint {
            x_iu: 25_400,
            y_iu: 25_400,
        },
        source_path,
        limits,
    )?;
    let value = SchematicBusEntry {
        uuid: child_scalar(source, &spans, "uuid", source_path, limits)?.unwrap_or_default(),
        at,
        size,
    };
    value.wire_end().ok_or_else(|| {
        source_error(
            source_path,
            "bus-entry endpoint overflows the schematic grid",
        )
    })?;
    Ok(value)
}

fn parse_point_marker(
    source: &str,
    root_head: &str,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<SchematicJunction, SourceBundleError> {
    let spans = carrier_form_spans(source, &[root_head, "at", "uuid"], source_path, limits, 8)?;
    Ok(SchematicJunction {
        uuid: child_scalar(source, &spans, "uuid", source_path, limits)?.unwrap_or_default(),
        at: child_point(
            source,
            &spans,
            "at",
            SchematicPoint { x_iu: 0, y_iu: 0 },
            source_path,
            limits,
        )?,
    })
}

fn parse_label(
    source: &str,
    root_head: &str,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<SchematicLabel, SourceBundleError> {
    let spans = carrier_form_spans(
        source,
        &[root_head, "at", "shape", "uuid"],
        source_path,
        limits,
        8,
    )?;
    let root = spans
        .iter()
        .find(|span| span.depth == 0 && span.head.as_deref() == Some(root_head))
        .ok_or_else(|| source_error(source_path, "schematic label root is missing"))?;
    let text = direct_scalars(source, root, 1, source_path, limits)?
        .into_iter()
        .next()
        .unwrap_or_default();
    let scope = match root_head {
        "label" => SchematicLabelScope::Local,
        "global_label" => SchematicLabelScope::Global,
        "hierarchical_label" => SchematicLabelScope::Hierarchical,
        _ => unreachable!("caller restricts label heads"),
    };
    Ok(SchematicLabel {
        scope,
        text,
        shape: child_scalar(source, &spans, "shape", source_path, limits)?.unwrap_or_else(|| {
            if scope == SchematicLabelScope::Local {
                String::new()
            } else {
                "input".to_owned()
            }
        }),
        uuid: child_scalar(source, &spans, "uuid", source_path, limits)?.unwrap_or_default(),
        at: child_point(
            source,
            &spans,
            "at",
            SchematicPoint { x_iu: 0, y_iu: 0 },
            source_path,
            limits,
        )?,
    })
}

fn carrier_form_spans(
    source: &str,
    heads: &[&str],
    source_path: &str,
    limits: SchematicBundleLimits,
    maximum_selected_forms: usize,
) -> Result<Vec<FormSpan>, SourceBundleError> {
    scan_form_spans_with_limits(
        source,
        &Selector {
            heads: Some(
                heads
                    .iter()
                    .map(|head| (*head).to_owned())
                    .collect::<BTreeSet<_>>(),
            ),
            min_depth: Some(0),
            max_depth: Some(2),
            ..Selector::default()
        },
        ProjectionLimits {
            max_source_bytes: limits.max_source_bytes,
            max_depth: limits.max_depth,
            max_selected_forms: maximum_selected_forms,
            ..ProjectionLimits::default()
        },
    )
    .map_err(|error| source_error(source_path, error.to_string()))
}

fn child_point(
    source: &str,
    spans: &[FormSpan],
    head: &str,
    default: SchematicPoint,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<SchematicPoint, SourceBundleError> {
    spans
        .iter()
        .find(|span| span.depth == 1 && span.head.as_deref() == Some(head))
        .map_or(Ok(default), |span| {
            parse_point(source, span, source_path, limits)
        })
}

fn child_scalar(
    source: &str,
    spans: &[FormSpan],
    head: &str,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<Option<String>, SourceBundleError> {
    spans
        .iter()
        .find(|span| span.depth == 1 && span.head.as_deref() == Some(head))
        .map(|span| direct_scalars(source, span, 1, source_path, limits))
        .transpose()
        .map(|values| values.and_then(|mut values| values.pop()))
}

fn parse_point(
    source: &str,
    span: &FormSpan,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<SchematicPoint, SourceBundleError> {
    // `(at X Y ANGLE)` carries a third direct scalar; connectivity consumes X/Y.
    let values = direct_scalars(source, span, 3, source_path, limits)?;
    if values.len() < 2 {
        return Err(source_error(
            source_path,
            "schematic point requires X and Y coordinates",
        ));
    }
    Ok(SchematicPoint {
        x_iu: parse_iu(&values[0], source_path)?,
        y_iu: parse_iu(&values[1], source_path)?,
    })
}

fn parse_iu(value: &str, source_path: &str) -> Result<i64, SourceBundleError> {
    let decimal = parse_decimal(value).ok_or_else(|| {
        source_error(
            source_path,
            format!("invalid schematic coordinate {value:?}"),
        )
    })?;
    scaled_decimal_to_i64(&decimal).ok_or_else(|| {
        source_error(
            source_path,
            "schematic coordinate is outside the internal-unit range",
        )
    })
}

struct DecimalParts {
    negative: bool,
    digits: Vec<u8>,
    scale_shift: i64,
}

fn parse_decimal(value: &str) -> Option<DecimalParts> {
    let (negative, unsigned) = match value.as_bytes().first() {
        Some(b'-') => (true, &value[1..]),
        Some(b'+') => (false, &value[1..]),
        _ => (false, value),
    };
    let exponent_at = unsigned.find(['e', 'E']);
    let (mantissa, exponent_text) = exponent_at.map_or((unsigned, None), |index| {
        (&unsigned[..index], Some(&unsigned[index + 1..]))
    });
    let exponent = exponent_text.map_or(Some(0), parse_exponent)?;
    let mut digits = Vec::with_capacity(mantissa.len());
    let mut decimal_seen = false;
    let mut fractional_digits = 0_usize;
    for byte in mantissa.bytes() {
        match byte {
            b'0'..=b'9' => {
                digits.push(byte);
                fractional_digits += usize::from(decimal_seen);
            }
            b'.' if !decimal_seen => decimal_seen = true,
            _ => return None,
        }
    }
    if digits.is_empty()
        || unsigned[exponent_at.unwrap_or(unsigned.len())..]
            .matches(['e', 'E'])
            .count()
            > 1
    {
        return None;
    }
    let first_nonzero = digits.iter().position(|digit| *digit != b'0');
    let digits = first_nonzero.map_or_else(Vec::new, |index| digits.split_off(index));
    let fraction = i64::try_from(fractional_digits).unwrap_or(i64::MAX);
    Some(DecimalParts {
        negative,
        digits,
        scale_shift: exponent.saturating_sub(fraction).saturating_add(4),
    })
}

fn parse_exponent(value: &str) -> Option<i64> {
    let (negative, digits) = match value.as_bytes().first() {
        Some(b'-') => (true, &value[1..]),
        Some(b'+') => (false, &value[1..]),
        _ => (false, value),
    };
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    let magnitude = digits.bytes().fold(0_i64, |current, byte| {
        current
            .saturating_mul(10)
            .saturating_add(i64::from(byte - b'0'))
    });
    Some(if negative { -magnitude } else { magnitude })
}

fn scaled_decimal_to_i64(decimal: &DecimalParts) -> Option<i64> {
    if decimal.digits.is_empty() {
        return Some(0);
    }
    let magnitude = if decimal.scale_shift >= 0 {
        scaled_integer_magnitude(&decimal.digits, decimal.scale_shift)?
    } else {
        rounded_fractional_magnitude(&decimal.digits, decimal.scale_shift)?
    };
    signed_magnitude(magnitude, decimal.negative)
}

fn scaled_integer_magnitude(digits: &[u8], shift: i64) -> Option<u64> {
    let shift = usize::try_from(shift).ok()?;
    if digits.len().checked_add(shift)? > 19 {
        return None;
    }
    let mut magnitude = parse_digit_magnitude(digits)?;
    for _ in 0..shift {
        magnitude = magnitude.checked_mul(10)?;
    }
    Some(magnitude)
}

fn rounded_fractional_magnitude(digits: &[u8], shift: i64) -> Option<u64> {
    let discarded = usize::try_from(shift.unsigned_abs()).ok()?;
    if discarded > digits.len() {
        return Some(0);
    }
    let retained = digits.len() - discarded;
    let mut magnitude = parse_digit_magnitude(&digits[..retained])?;
    if should_round_up(&digits[retained..], magnitude) {
        magnitude = magnitude.checked_add(1)?;
    }
    Some(magnitude)
}

fn parse_digit_magnitude(digits: &[u8]) -> Option<u64> {
    digits.iter().try_fold(0_u64, |current, digit| {
        current
            .checked_mul(10)?
            .checked_add(u64::from(*digit - b'0'))
    })
}

fn should_round_up(discarded: &[u8], retained: u64) -> bool {
    match discarded.first() {
        Some(b'6'..=b'9') => true,
        Some(b'5') => discarded[1..].iter().any(|digit| *digit != b'0') || retained % 2 == 1,
        _ => false,
    }
}

fn signed_magnitude(magnitude: u64, negative: bool) -> Option<i64> {
    if negative {
        if magnitude == i64::MAX as u64 + 1 {
            Some(i64::MIN)
        } else {
            i64::try_from(magnitude).ok().map(|value| -value)
        }
    } else {
        i64::try_from(magnitude).ok()
    }
}

fn direct_scalars(
    source: &str,
    span: &FormSpan,
    maximum: usize,
    source_path: &str,
    limits: SchematicBundleLimits,
) -> Result<Vec<String>, SourceBundleError> {
    let text = span
        .text(source)
        .map_err(|error| source_error(source_path, error.to_string()))?;
    let mut lexer = Lexer::new(text);
    require_header(&mut lexer, source_path)?;
    let mut depth = 1_usize;
    let mut values = Vec::new();
    for token in lexer {
        let token = token.map_err(|error| source_error(source_path, error.to_string()))?;
        match token.kind {
            TokenKind::Left => depth = depth.saturating_add(1),
            TokenKind::Right => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    break;
                }
            }
            _ if depth == 1 => {
                if values.len() >= maximum {
                    return Err(limit_error(
                        source_path,
                        "direct scalar count exceeds its limit",
                    ));
                }
                if token.lexeme.len() > limits.max_decoded_string_bytes.saturating_add(2) {
                    return Err(limit_error(source_path, "decoded string exceeds its limit"));
                }
                let decoded = decoded(token);
                if decoded.len() > limits.max_decoded_string_bytes {
                    return Err(limit_error(source_path, "decoded string exceeds its limit"));
                }
                values.push(decoded.into_owned());
            }
            _ => {}
        }
    }
    Ok(values)
}

fn require_header(lexer: &mut Lexer<'_>, source_path: &str) -> Result<(), SourceBundleError> {
    let left = next_token(lexer, source_path)?;
    let head = next_token(lexer, source_path)?;
    if left.kind == TokenKind::Left && head.kind == TokenKind::Atom {
        Ok(())
    } else {
        Err(source_error(
            source_path,
            "selected schematic form has an invalid header",
        ))
    }
}

fn next_token<'a>(
    lexer: &mut Lexer<'a>,
    source_path: &str,
) -> Result<Token<'a>, SourceBundleError> {
    lexer
        .next()
        .transpose()
        .map_err(|error| source_error(source_path, error.to_string()))?
        .ok_or_else(|| {
            source_error(
                source_path,
                "selected schematic form ended before its header",
            )
        })
}

fn decoded(token: Token<'_>) -> Cow<'_, str> {
    if token.kind == TokenKind::QuotedString {
        Cow::Owned(decode_quoted(token.lexeme))
    } else {
        Cow::Borrowed(token.lexeme)
    }
}

fn build_connectivity(
    carriers: &SchematicSourceCarriers,
    source_path: &str,
) -> Result<SchematicConnectivity, SourceBundleError> {
    let mut union = PointUnion::default();
    for wire in &carriers.wires {
        union.add_polyline(&wire.points);
    }
    for bus in &carriers.buses {
        for point in &bus.points {
            union.add(*point);
        }
    }
    for entry in &carriers.bus_entries {
        union.add(entry.at);
        union.add(entry.wire_end().ok_or_else(|| {
            source_error(
                source_path,
                "bus-entry endpoint overflows the schematic grid",
            )
        })?);
    }
    for junction in &carriers.junctions {
        union.add(junction.at);
    }
    Ok(union.finish())
}

#[derive(Default)]
struct PointUnion {
    index_by_point: HashMap<SchematicPoint, usize>,
    points: Vec<SchematicPoint>,
    parent: Vec<usize>,
    rank: Vec<u8>,
}

impl PointUnion {
    fn add(&mut self, point: SchematicPoint) -> usize {
        if let Some(index) = self.index_by_point.get(&point) {
            return *index;
        }
        let index = self.points.len();
        self.index_by_point.insert(point, index);
        self.points.push(point);
        self.parent.push(index);
        self.rank.push(0);
        index
    }

    fn add_polyline(&mut self, points: &[SchematicPoint]) {
        let mut previous = None;
        for point in points {
            let current = self.add(*point);
            if let Some(previous) = previous
                && previous != current
            {
                self.union(previous, current);
            }
            previous = Some(current);
        }
    }

    fn root(&mut self, index: usize) -> usize {
        let mut root = index;
        while self.parent[root] != root {
            root = self.parent[root];
        }
        let mut current = index;
        while self.parent[current] != root {
            let next = self.parent[current];
            self.parent[current] = root;
            current = next;
        }
        root
    }

    fn union(&mut self, left: usize, right: usize) {
        let mut left_root = self.root(left);
        let mut right_root = self.root(right);
        if left_root == right_root {
            return;
        }
        if self.rank[left_root] < self.rank[right_root] {
            std::mem::swap(&mut left_root, &mut right_root);
        }
        self.parent[right_root] = left_root;
        if self.rank[left_root] == self.rank[right_root] {
            self.rank[left_root] = self.rank[left_root].saturating_add(1);
        }
    }

    fn finish(mut self) -> SchematicConnectivity {
        let mut grouped = BTreeMap::<usize, Vec<SchematicPoint>>::new();
        for index in 0..self.points.len() {
            let root = self.root(index);
            grouped.entry(root).or_default().push(self.points[index]);
        }
        let mut components = grouped.into_values().collect::<Vec<_>>();
        for component in &mut components {
            component.sort_unstable();
        }
        components.sort_unstable();
        let mut component_by_point = BTreeMap::new();
        for (component_index, component) in components.iter().enumerate() {
            for point in component {
                component_by_point.insert(*point, component_index);
            }
        }
        let points = component_by_point.keys().copied().collect();
        SchematicConnectivity {
            points,
            components,
            component_by_point,
        }
    }
}

fn source_error(source_path: &str, message: impl Into<String>) -> SourceBundleError {
    SourceBundleError::new(SourceBundleErrorKind::Schematic, Some(source_path), message)
}

fn limit_error(source_path: &str, message: impl Into<String>) -> SourceBundleError {
    SourceBundleError::new(
        SourceBundleErrorKind::ResourceLimit,
        Some(source_path),
        message,
    )
}

#[cfg(test)]
mod tests {
    use super::parse_iu;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct CoordinateVectors {
        cases: Vec<CoordinateCase>,
    }

    #[derive(Deserialize)]
    struct CoordinateCase {
        name: String,
        millimetres: String,
        expected_iu: Option<String>,
    }

    #[test]
    fn exact_decimal_coordinates_match_shared_ties_even_and_range_vectors() {
        let vectors: CoordinateVectors = serde_json::from_str(include_str!(
            "../../../../tests/parity/schematic_coordinate_iu_vectors.json"
        ))
        .expect("coordinate vectors");
        for case in vectors.cases {
            let actual = parse_iu(&case.millimetres, "vector");
            match case.expected_iu {
                Some(expected) => assert_eq!(
                    actual.expect(&case.name).to_string(),
                    expected,
                    "{}",
                    case.name
                ),
                None => assert!(actual.is_err(), "{} unexpectedly decoded", case.name),
            }
        }
    }
}
