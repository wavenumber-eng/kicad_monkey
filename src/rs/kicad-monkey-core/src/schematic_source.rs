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
            add_points(point_count, 2, source_path, limits)?;
            carriers
                .bus_entries
                .push(parse_bus_entry(text, source_path, limits)?);
        }
        Some("junction") => {
            add_points(point_count, 1, source_path, limits)?;
            carriers
                .junctions
                .push(parse_point_marker(text, "junction", source_path, limits)?);
        }
        Some("no_connect") => {
            add_points(point_count, 1, source_path, limits)?;
            let marker = parse_point_marker(text, "no_connect", source_path, limits)?;
            carriers.no_connects.push(SchematicNoConnect {
                uuid: marker.uuid,
                at: marker.at,
            });
        }
        Some(head @ ("label" | "global_label" | "hierarchical_label")) => {
            add_points(point_count, 1, source_path, limits)?;
            carriers
                .labels
                .push(parse_label(text, head, source_path, limits)?);
        }
        _ => {}
    }
    Ok(())
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
    let millimetres = value.parse::<f64>().map_err(|_| {
        source_error(
            source_path,
            format!("invalid schematic coordinate {value:?}"),
        )
    })?;
    let scaled = millimetres * SCHEMATIC_IU_PER_MM as f64;
    if !scaled.is_finite() || scaled < i64::MIN as f64 || scaled > i64::MAX as f64 {
        return Err(source_error(
            source_path,
            "schematic coordinate is outside the internal-unit range",
        ));
    }
    Ok(scaled.round_ties_even() as i64)
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
