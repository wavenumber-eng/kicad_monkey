//! Native, source-backed views and focused edits for KiCad PCB documents.
//!
//! The board model indexes exact source spans and decodes domain records on
//! demand. It intentionally does not construct the generic compatibility tree.

use crate::sexpr::{
    Error, ErrorKind, ErrorPhase, Lexer, Patch, Position, Sexp, Token, TokenKind,
    apply_patches_with_limit, build_with_limit, decode_quoted,
};
use crate::sexpr_projection::{FormSpan, ProjectionLimits, Selector, scan_form_spans_with_limits};
use std::collections::BTreeMap;
use std::ops::Range;

/// Resource ceilings for one native PCB read or focused edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PcbLimits {
    pub max_source_bytes: usize,
    pub max_output_bytes: usize,
    pub max_depth: usize,
    pub max_top_level_forms: usize,
    pub max_layers: usize,
    pub max_nets: usize,
    pub max_properties: usize,
    pub max_footprints: usize,
    pub max_footprint_children: usize,
    pub max_pad_children: usize,
    pub max_model_children: usize,
    pub max_pads: usize,
    pub max_models: usize,
    pub max_segments: usize,
    pub max_vias: usize,
    pub max_zones: usize,
}

impl Default for PcbLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 512 * 1024 * 1024,
            max_output_bytes: 512 * 1024 * 1024,
            max_depth: 512,
            max_top_level_forms: 4_000_000,
            max_layers: 256,
            max_nets: 2_000_000,
            max_properties: 100_000,
            max_footprints: 1_000_000,
            max_footprint_children: 1_000_000,
            max_pad_children: 256,
            max_model_children: 32,
            max_pads: 4_000_000,
            max_models: 1_000_000,
            max_segments: 8_000_000,
            max_vias: 4_000_000,
            max_zones: 1_000_000,
        }
    }
}

/// Collection counts used to compare the lazy view with the Python model.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PcbCounts {
    pub layers: usize,
    pub nets: usize,
    pub properties: usize,
    pub footprints: usize,
    pub pads: usize,
    pub models: usize,
    pub segments: usize,
    pub vias: usize,
    pub zones: usize,
    pub arcs: usize,
    pub graphics: usize,
    pub groups: usize,
    pub dimensions: usize,
    pub generated_items: usize,
    pub embedded_files: usize,
    pub unknown_top_level: usize,
}

/// One exact top-level PCB property and its editable value range.
#[derive(Clone, Debug, PartialEq)]
pub struct PcbProperty {
    pub name: String,
    pub value: String,
    pub source_range: Range<usize>,
    value_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbLayer {
    pub ordinal: i64,
    pub name: String,
    pub kind: String,
    pub user_name: Option<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbNet {
    pub code: i64,
    pub name: String,
    pub source_range: Range<usize>,
}

/// A board-object net reference, which KiCad may encode by ordinal or name.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PcbNetRef {
    pub ordinal: Option<i64>,
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbFootprint {
    pub library_link: String,
    pub reference: Option<String>,
    pub value: Option<String>,
    pub layer: Option<String>,
    pub at_x: Option<f64>,
    pub at_y: Option<f64>,
    pub angle: Option<f64>,
    pub uuid: Option<String>,
    pub pad_count: usize,
    pub model_count: usize,
    pub source_range: Range<usize>,
}

/// One typed footprint pad in board source order.
#[derive(Clone, Debug, PartialEq)]
pub struct PcbPad {
    pub footprint_index: usize,
    pub number: String,
    pub kind: String,
    pub shape: String,
    pub at_x: f64,
    pub at_y: f64,
    pub angle: f64,
    pub size_x: f64,
    pub size_y: f64,
    pub layers: Vec<String>,
    pub net: PcbNetRef,
    pub uuid: Option<String>,
    pub source_range: Range<usize>,
}

/// One typed footprint 3D-model reference in board source order.
#[derive(Clone, Debug, PartialEq)]
pub struct PcbModelReference {
    pub footprint_index: usize,
    pub path: String,
    pub offset: [f64; 3],
    pub scale: [f64; 3],
    pub rotate: [f64; 3],
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbSegment {
    pub start_x: f64,
    pub start_y: f64,
    pub end_x: f64,
    pub end_y: f64,
    pub width: Option<f64>,
    pub layer: Option<String>,
    pub net: PcbNetRef,
    pub uuid: Option<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbVia {
    pub at_x: f64,
    pub at_y: f64,
    pub size: Option<f64>,
    pub drill: Option<f64>,
    pub layers: Vec<String>,
    pub net: PcbNetRef,
    pub uuid: Option<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbZone {
    pub net: PcbNetRef,
    pub net_name: Option<String>,
    pub layers: Vec<String>,
    pub uuid: Option<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcbEdit {
    pub source: String,
    pub changed: bool,
}

#[derive(Clone, Debug)]
struct IndexedFootprint {
    span: FormSpan,
    pad_count: usize,
    model_count: usize,
}

#[derive(Clone, Debug)]
struct IndexedNestedForm {
    footprint_index: usize,
    span: FormSpan,
}

#[derive(Clone, Debug, Default)]
struct NetResolver {
    name_by_ordinal: BTreeMap<i64, String>,
    ordinal_by_name: BTreeMap<String, i64>,
}

impl NetResolver {
    fn resolve(&self, mut net: PcbNetRef) -> PcbNetRef {
        if net.name.is_none()
            && let Some(name) = net
                .ordinal
                .and_then(|ordinal| self.name_by_ordinal.get(&ordinal))
                .filter(|name| !name.is_empty())
        {
            net.name = Some(name.clone());
        }
        if net.ordinal.is_none()
            && let Some(ordinal) = net
                .name
                .as_ref()
                .and_then(|name| self.ordinal_by_name.get(name))
        {
            net.ordinal = Some(*ordinal);
        }
        net
    }
}

#[derive(Debug, Default)]
struct PcbIndex {
    layer_forms: Vec<FormSpan>,
    properties: Vec<FormSpan>,
    nets: Vec<FormSpan>,
    footprints: Vec<IndexedFootprint>,
    pads: Vec<IndexedNestedForm>,
    models: Vec<IndexedNestedForm>,
    segments: Vec<FormSpan>,
    vias: Vec<FormSpan>,
    zones: Vec<FormSpan>,
    counts: PcbCounts,
}

/// A board view backed by one source buffer and exact selected form spans.
#[derive(Clone, Debug)]
pub struct PcbView<'a> {
    source: &'a str,
    root: FormSpan,
    top_level: Vec<FormSpan>,
    layer_forms: Vec<FormSpan>,
    properties: Vec<FormSpan>,
    nets: Vec<FormSpan>,
    footprints: Vec<IndexedFootprint>,
    pads: Vec<IndexedNestedForm>,
    models: Vec<IndexedNestedForm>,
    segments: Vec<FormSpan>,
    vias: Vec<FormSpan>,
    zones: Vec<FormSpan>,
    net_resolver: NetResolver,
    counts: PcbCounts,
    limits: PcbLimits,
}

impl<'a> PcbView<'a> {
    /// Validate one `kicad_pcb` root and index its major domain families.
    pub fn parse(source: &'a str, limits: PcbLimits) -> Result<Self, Error> {
        let selected_limit = limits
            .max_top_level_forms
            .checked_add(1)
            .ok_or_else(limit_error)?;
        let spans = scan_form_spans_with_limits(
            source,
            &Selector {
                min_depth: Some(0),
                max_depth: Some(1),
                ..Selector::default()
            },
            projection_limits(limits, selected_limit),
        )?;
        let roots: Vec<_> = spans.iter().filter(|span| span.depth == 0).collect();
        let [root] = roots.as_slice() else {
            return Err(source_error(
                "Expected exactly one top-level PCB form",
                Position::START,
            ));
        };
        if root.head.as_deref() != Some("kicad_pcb") {
            return Err(source_error("Expected a kicad_pcb root", root.start));
        }
        let root = (*root).clone();
        let top_level: Vec<_> = spans.into_iter().filter(|span| span.depth == 1).collect();
        if top_level.len() > limits.max_top_level_forms {
            return Err(limit_error());
        }

        let index = index_top_level(source, &top_level, limits)?;
        let net_resolver = net_resolver_from_spans(source, &index.nets)?;

        Ok(Self {
            source,
            root,
            top_level,
            layer_forms: index.layer_forms,
            properties: index.properties,
            nets: index.nets,
            footprints: index.footprints,
            pads: index.pads,
            models: index.models,
            segments: index.segments,
            vias: index.vias,
            zones: index.zones,
            net_resolver,
            counts: index.counts,
            limits,
        })
    }

    pub fn counts(&self) -> PcbCounts {
        self.counts
    }

    pub fn source(&self) -> &'a str {
        self.source
    }

    pub fn root_span(&self) -> &FormSpan {
        &self.root
    }

    /// Iterate exact top-level spans in source order, including unknown forms.
    pub fn top_level_forms(&self) -> impl Iterator<Item = &FormSpan> {
        self.top_level.iter()
    }

    pub fn unknown_top_level_forms(&self) -> impl Iterator<Item = &FormSpan> {
        self.top_level.iter().filter(|span| {
            span.head
                .as_deref()
                .is_none_or(|head| !is_known_top_level(head))
        })
    }

    pub fn layers(&self) -> impl Iterator<Item = Result<PcbLayer, Error>> + '_ {
        self.layer_forms
            .iter()
            .map(|span| layer_from_span(self.source, span))
    }

    pub fn nets(&self) -> impl Iterator<Item = Result<PcbNet, Error>> + '_ {
        self.nets
            .iter()
            .map(|span| net_from_span(self.source, span))
    }

    pub fn properties(&self) -> impl Iterator<Item = Result<PcbProperty, Error>> + '_ {
        self.properties
            .iter()
            .map(|span| property_from_span(self.source, span))
    }

    pub fn footprints(&self) -> impl Iterator<Item = Result<PcbFootprint, Error>> + '_ {
        self.footprints
            .iter()
            .map(|indexed| footprint_from_span(self.source, indexed, self.limits))
    }

    pub fn pads(&self) -> impl Iterator<Item = Result<PcbPad, Error>> + '_ {
        self.pads.iter().map(|indexed| {
            pad_from_span(self.source, indexed, self.limits).map(|mut pad| {
                pad.net = self.net_resolver.resolve(pad.net);
                pad
            })
        })
    }

    pub fn models(&self) -> impl Iterator<Item = Result<PcbModelReference, Error>> + '_ {
        self.models
            .iter()
            .map(|indexed| model_from_span(self.source, indexed, self.limits))
    }

    pub fn segments(&self) -> impl Iterator<Item = Result<PcbSegment, Error>> + '_ {
        self.segments.iter().map(|span| {
            segment_from_span(self.source, span, self.limits).map(|mut segment| {
                segment.net = self.net_resolver.resolve(segment.net);
                segment
            })
        })
    }

    pub fn vias(&self) -> impl Iterator<Item = Result<PcbVia, Error>> + '_ {
        self.vias.iter().map(|span| {
            via_from_span(self.source, span, self.limits).map(|mut via| {
                via.net = self.net_resolver.resolve(via.net);
                via
            })
        })
    }

    pub fn zones(&self) -> impl Iterator<Item = Result<PcbZone, Error>> + '_ {
        self.zones.iter().map(|span| {
            zone_from_span(self.source, span, self.limits).map(|mut zone| {
                if zone.net.name.is_none() {
                    zone.net.name.clone_from(&zone.net_name);
                }
                zone.net = self.net_resolver.resolve(zone.net);
                zone
            })
        })
    }

    /// Replace one unambiguous top-level board property without rewriting the board.
    pub fn set_property(&self, name: &str, value: &str) -> Result<PcbEdit, Error> {
        if self.source.len() > self.limits.max_output_bytes {
            return Err(output_limit_error());
        }
        let matches = self
            .properties()
            .filter_map(|property| match property {
                Ok(property) if property.name == name => Some(Ok(property)),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let [property] = matches.as_slice() else {
            return Err(source_error(
                if matches.is_empty() {
                    "PCB property was not found"
                } else {
                    "PCB property name is ambiguous"
                },
                self.root.start,
            ));
        };
        if property.value == value {
            return Ok(PcbEdit {
                source: self.source.to_owned(),
                changed: false,
            });
        }
        let replacement = build_with_limit(
            &Sexp::Quoted(value.to_owned()),
            self.limits.max_output_bytes,
        )?;
        let source = apply_patches_with_limit(
            self.source,
            &[Patch::new(
                property.value_range.start,
                property.value_range.end,
                replacement,
            )],
            self.limits.max_output_bytes,
        )?;
        Ok(PcbEdit {
            source,
            changed: true,
        })
    }
}

fn index_top_level(
    source: &str,
    top_level: &[FormSpan],
    limits: PcbLimits,
) -> Result<PcbIndex, Error> {
    let mut index = PcbIndex::default();
    for span in top_level {
        match span.head.as_deref() {
            Some("layers") => index_layers(source, span, limits, &mut index)?,
            Some("net") => {
                bounded_push(&mut index.nets, span.clone(), limits.max_nets)?;
                index.counts.nets += 1;
            }
            Some("property") => {
                bounded_push(&mut index.properties, span.clone(), limits.max_properties)?;
                index.counts.properties += 1;
            }
            Some("footprint" | "module") => index_footprint(source, span, limits, &mut index)?,
            Some("segment") => {
                bounded_push(&mut index.segments, span.clone(), limits.max_segments)?;
                index.counts.segments += 1;
            }
            Some("via") => {
                bounded_push(&mut index.vias, span.clone(), limits.max_vias)?;
                index.counts.vias += 1;
            }
            Some("zone") => {
                bounded_push(&mut index.zones, span.clone(), limits.max_zones)?;
                index.counts.zones += 1;
            }
            Some("arc") => index.counts.arcs += 1,
            Some(head) if head.starts_with("gr_") => index.counts.graphics += 1,
            Some("group") => index.counts.groups += 1,
            Some("dimension") => index.counts.dimensions += 1,
            Some("generated") => index.counts.generated_items += 1,
            Some("embedded_files") => index.counts.embedded_files += 1,
            Some("image" | "barcode" | "table") => {}
            Some(head) if is_known_metadata(head) => {}
            _ => index.counts.unknown_top_level += 1,
        }
    }
    Ok(index)
}

fn index_layers(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
    index: &mut PcbIndex,
) -> Result<(), Error> {
    let forms = direct_children(source, span, limits.max_layers, limits)?;
    index.counts.layers = index
        .counts
        .layers
        .checked_add(forms.len())
        .ok_or_else(limit_error)?;
    if index.counts.layers > limits.max_layers {
        return Err(limit_error());
    }
    index.layer_forms.extend(forms);
    Ok(())
}

fn index_footprint(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
    index: &mut PcbIndex,
) -> Result<(), Error> {
    if index.footprints.len() == limits.max_footprints {
        return Err(limit_error());
    }
    let children = direct_children(source, span, limits.max_footprint_children, limits)?;
    let footprint_index = index.footprints.len();
    let pad_forms: Vec<_> = children
        .iter()
        .filter(|child| child.head.as_deref() == Some("pad"))
        .cloned()
        .collect();
    let model_forms: Vec<_> = children
        .iter()
        .filter(|child| child.head.as_deref() == Some("model"))
        .cloned()
        .collect();
    let pad_count = pad_forms.len();
    let model_count = model_forms.len();
    index.counts.pads = index
        .counts
        .pads
        .checked_add(pad_count)
        .ok_or_else(limit_error)?;
    index.counts.models = index
        .counts
        .models
        .checked_add(model_count)
        .ok_or_else(limit_error)?;
    if index.counts.pads > limits.max_pads || index.counts.models > limits.max_models {
        return Err(limit_error());
    }
    index.footprints.push(IndexedFootprint {
        span: span.clone(),
        pad_count,
        model_count,
    });
    index
        .pads
        .extend(pad_forms.into_iter().map(|span| IndexedNestedForm {
            footprint_index,
            span,
        }));
    index
        .models
        .extend(model_forms.into_iter().map(|span| IndexedNestedForm {
            footprint_index,
            span,
        }));
    index.counts.footprints += 1;
    Ok(())
}

fn layer_from_span(source: &str, span: &FormSpan) -> Result<PcbLayer, Error> {
    let values = scalar_values(source, span)?;
    let ordinal = span
        .head
        .as_deref()
        .ok_or_else(|| source_error("Expected layer ordinal", span.start))?
        .parse()
        .map_err(|_| source_error("Expected layer ordinal", span.start))?;
    Ok(PcbLayer {
        ordinal,
        name: required_string(values.first(), "Expected layer name", span)?,
        kind: required_string(values.get(1), "Expected layer kind", span)?,
        user_name: values.get(2).map(token_string),
        source_range: span.range.clone(),
    })
}

fn net_from_span(source: &str, span: &FormSpan) -> Result<PcbNet, Error> {
    let values = scalar_values(source, span)?;
    Ok(PcbNet {
        code: required_i64(values.first(), "Expected net code", span)?,
        name: required_string(values.get(1), "Expected net name", span)?,
        source_range: span.range.clone(),
    })
}

fn property_from_span(source: &str, span: &FormSpan) -> Result<PcbProperty, Error> {
    let values = scalar_values(source, span)?;
    let name = required_string(values.first(), "Expected property name", span)?;
    let token = values
        .get(1)
        .ok_or_else(|| source_error("Expected property value", span.end))?;
    Ok(PcbProperty {
        name,
        value: token_string(token),
        source_range: span.range.clone(),
        value_range: (span.range.start + token.position.offset)
            ..(span.range.start + token.position.offset + token.lexeme.len()),
    })
}

fn footprint_from_span(
    source: &str,
    indexed: &IndexedFootprint,
    limits: PcbLimits,
) -> Result<PcbFootprint, Error> {
    let header = scalar_values(source, &indexed.span)?;
    let library_link = required_string(
        header.first(),
        "Expected footprint library link",
        &indexed.span,
    )?;
    let children = direct_children(source, &indexed.span, limits.max_footprint_children, limits)?;
    let mut result = PcbFootprint {
        library_link,
        reference: None,
        value: None,
        layer: None,
        at_x: None,
        at_y: None,
        angle: None,
        uuid: None,
        pad_count: indexed.pad_count,
        model_count: indexed.model_count,
        source_range: indexed.span.range.clone(),
    };
    for child in &children {
        match child.head.as_deref() {
            Some("property") => {
                let property = property_from_span(source, child)?;
                if property.name == "Reference" && result.reference.is_none() {
                    result.reference = Some(property.value);
                } else if property.name == "Value" && result.value.is_none() {
                    result.value = Some(property.value);
                }
            }
            Some("layer") => result.layer = first_string(source, child)?,
            Some("at") => {
                let values = scalar_values(source, child)?;
                result.at_x = optional_f64(values.first(), child)?;
                result.at_y = optional_f64(values.get(1), child)?;
                result.angle = optional_f64(values.get(2), child)?;
            }
            Some("uuid" | "tstamp") if result.uuid.is_none() => {
                result.uuid = first_string(source, child)?;
            }
            _ => {}
        }
    }
    Ok(result)
}

fn pad_from_span(
    source: &str,
    indexed: &IndexedNestedForm,
    limits: PcbLimits,
) -> Result<PcbPad, Error> {
    let header = scalar_values(source, &indexed.span)?;
    let children = direct_children(source, &indexed.span, limits.max_pad_children, limits)?;
    let at = optional_vector(source, &children, "at", [0.0, 0.0, 0.0])?;
    let size = optional_pair(source, &children, "size", [0.0, 0.0])?;
    let layers = child(&children, "layers")
        .map(|item| {
            scalar_values(source, item).map(|values| values.iter().map(token_string).collect())
        })
        .transpose()?
        .unwrap_or_default();
    Ok(PcbPad {
        footprint_index: indexed.footprint_index,
        number: required_string(header.first(), "Expected pad number", &indexed.span)?,
        kind: required_string(header.get(1), "Expected pad kind", &indexed.span)?,
        shape: required_string(header.get(2), "Expected pad shape", &indexed.span)?,
        at_x: at[0],
        at_y: at[1],
        angle: at[2],
        size_x: size[0],
        size_y: size[1],
        layers,
        net: child_net_ref(source, &children)?,
        uuid: optional_uuid(source, &children)?,
        source_range: indexed.span.range.clone(),
    })
}

fn model_from_span(
    source: &str,
    indexed: &IndexedNestedForm,
    limits: PcbLimits,
) -> Result<PcbModelReference, Error> {
    let header = scalar_values(source, &indexed.span)?;
    let children = direct_children(source, &indexed.span, limits.max_model_children, limits)?;
    Ok(PcbModelReference {
        footprint_index: indexed.footprint_index,
        path: required_string(header.first(), "Expected model path", &indexed.span)?,
        offset: nested_xyz(source, &children, "offset", [0.0, 0.0, 0.0], limits)?,
        scale: nested_xyz(source, &children, "scale", [1.0, 1.0, 1.0], limits)?,
        rotate: nested_xyz(source, &children, "rotate", [0.0, 0.0, 0.0], limits)?,
        source_range: indexed.span.range.clone(),
    })
}

fn net_resolver_from_spans(source: &str, spans: &[FormSpan]) -> Result<NetResolver, Error> {
    let mut resolver = NetResolver::default();
    for span in spans {
        let net = net_from_span(source, span)?;
        resolver.name_by_ordinal.insert(net.code, net.name.clone());
        if !net.name.is_empty() {
            resolver.ordinal_by_name.insert(net.name, net.code);
        }
    }
    Ok(resolver)
}

fn segment_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbSegment, Error> {
    let children = direct_children(source, span, 64, limits)?;
    let start = required_xy(source, &children, "start", span)?;
    let end = required_xy(source, &children, "end", span)?;
    Ok(PcbSegment {
        start_x: start.0,
        start_y: start.1,
        end_x: end.0,
        end_y: end.1,
        width: optional_child_f64(source, &children, "width")?,
        layer: optional_child_string(source, &children, "layer")?,
        net: child_net_ref(source, &children)?,
        uuid: optional_uuid(source, &children)?,
        source_range: span.range.clone(),
    })
}

fn via_from_span(source: &str, span: &FormSpan, limits: PcbLimits) -> Result<PcbVia, Error> {
    let children = direct_children(source, span, 64, limits)?;
    let at = required_xy(source, &children, "at", span)?;
    let layers = child(&children, "layers")
        .map(|item| {
            scalar_values(source, item).map(|values| values.iter().map(token_string).collect())
        })
        .transpose()?
        .unwrap_or_default();
    Ok(PcbVia {
        at_x: at.0,
        at_y: at.1,
        size: optional_child_f64(source, &children, "size")?,
        drill: optional_child_f64(source, &children, "drill")?,
        layers,
        net: child_net_ref(source, &children)?,
        uuid: optional_uuid(source, &children)?,
        source_range: span.range.clone(),
    })
}

fn zone_from_span(source: &str, span: &FormSpan, limits: PcbLimits) -> Result<PcbZone, Error> {
    let children = direct_children(source, span, 256, limits)?;
    let layers = if let Some(item) = child(&children, "layers") {
        scalar_values(source, item)?
            .iter()
            .map(token_string)
            .collect()
    } else if let Some(layer) = optional_child_string(source, &children, "layer")? {
        vec![layer]
    } else {
        Vec::new()
    };
    Ok(PcbZone {
        net: child_net_ref(source, &children)?,
        net_name: optional_child_string(source, &children, "net_name")?,
        layers,
        uuid: optional_uuid(source, &children)?,
        source_range: span.range.clone(),
    })
}

fn scalar_values<'a>(source: &'a str, span: &FormSpan) -> Result<Vec<Token<'a>>, Error> {
    let text = span.text(source)?;
    (|| {
        let mut lexer = Lexer::new(text);
        expect_kind(
            lexer.next(),
            TokenKind::Left,
            "Expected form opening parenthesis",
        )?;
        let _head = next_scalar(lexer.next(), "Expected form head")?;
        let mut values = Vec::new();
        for token in lexer {
            let token = token?;
            match token.kind {
                TokenKind::Left | TokenKind::Right => break,
                _ => values.push(token),
            }
        }
        Ok(values)
    })()
    .map_err(|error| rebase_error(error, span))
}

fn direct_children(
    source: &str,
    parent: &FormSpan,
    max_selected_forms: usize,
    limits: PcbLimits,
) -> Result<Vec<FormSpan>, Error> {
    let text = parent.text(source)?;
    let local = scan_form_spans_with_limits(
        text,
        &Selector {
            min_depth: Some(1),
            max_depth: Some(1),
            ..Selector::default()
        },
        ProjectionLimits {
            max_source_bytes: limits.max_source_bytes,
            max_depth: limits.max_depth,
            max_selected_forms,
            ..ProjectionLimits::default()
        },
    )
    .map_err(|error| rebase_error(error, parent))?;
    Ok(local
        .into_iter()
        .map(|mut span| {
            span.range.start += parent.range.start;
            span.range.end += parent.range.start;
            span.start = rebase_position(span.start, parent);
            span.end = rebase_position(span.end, parent);
            span.depth = 1;
            span.path = vec![
                parent.head.clone().unwrap_or_default(),
                span.head.clone().unwrap_or_default(),
            ];
            span
        })
        .collect())
}

fn required_xy(
    source: &str,
    children: &[FormSpan],
    head: &str,
    parent: &FormSpan,
) -> Result<(f64, f64), Error> {
    let span = child(children, head)
        .ok_or_else(|| source_error("Expected coordinate form", parent.start))?;
    let values = scalar_values(source, span)?;
    Ok((
        required_f64(values.first(), "Expected x coordinate", span)?,
        required_f64(values.get(1), "Expected y coordinate", span)?,
    ))
}

fn optional_pair(
    source: &str,
    children: &[FormSpan],
    head: &str,
    default: [f64; 2],
) -> Result<[f64; 2], Error> {
    let Some(span) = child(children, head) else {
        return Ok(default);
    };
    let values = scalar_values(source, span)?;
    Ok([
        required_f64(values.first(), "Expected first numeric value", span)?,
        required_f64(values.get(1), "Expected second numeric value", span)?,
    ])
}

fn optional_vector(
    source: &str,
    children: &[FormSpan],
    head: &str,
    default: [f64; 3],
) -> Result<[f64; 3], Error> {
    let Some(span) = child(children, head) else {
        return Ok(default);
    };
    let values = scalar_values(source, span)?;
    Ok([
        required_f64(values.first(), "Expected first numeric value", span)?,
        required_f64(values.get(1), "Expected second numeric value", span)?,
        optional_f64(values.get(2), span)?.unwrap_or(default[2]),
    ])
}

fn nested_xyz(
    source: &str,
    children: &[FormSpan],
    head: &str,
    default: [f64; 3],
    limits: PcbLimits,
) -> Result<[f64; 3], Error> {
    let Some(container) = child(children, head) else {
        return Ok(default);
    };
    let nested = direct_children(source, container, limits.max_model_children, limits)?;
    let Some(xyz) = child(&nested, "xyz") else {
        return Ok(default);
    };
    let values = scalar_values(source, xyz)?;
    Ok([
        required_f64(values.first(), "Expected model x value", xyz)?,
        required_f64(values.get(1), "Expected model y value", xyz)?,
        required_f64(values.get(2), "Expected model z value", xyz)?,
    ])
}

fn child<'a>(children: &'a [FormSpan], head: &str) -> Option<&'a FormSpan> {
    children
        .iter()
        .find(|span| span.head.as_deref() == Some(head))
}

fn first_string(source: &str, span: &FormSpan) -> Result<Option<String>, Error> {
    Ok(scalar_values(source, span)?.first().map(token_string))
}

fn optional_child_string(
    source: &str,
    children: &[FormSpan],
    head: &str,
) -> Result<Option<String>, Error> {
    child(children, head)
        .map(|span| first_string(source, span))
        .transpose()
        .map(Option::flatten)
}

fn optional_child_f64(
    source: &str,
    children: &[FormSpan],
    head: &str,
) -> Result<Option<f64>, Error> {
    let Some(span) = child(children, head) else {
        return Ok(None);
    };
    let values = scalar_values(source, span)?;
    optional_f64(values.first(), span)
}

fn child_net_ref(source: &str, children: &[FormSpan]) -> Result<PcbNetRef, Error> {
    let Some(span) = child(children, "net") else {
        return Ok(PcbNetRef::default());
    };
    let values = scalar_values(source, span)?;
    let Some(token) = values.first() else {
        return Ok(PcbNetRef::default());
    };
    if let Ok(ordinal) = token.lexeme.parse() {
        Ok(PcbNetRef {
            ordinal: Some(ordinal),
            name: values
                .get(1)
                .map(token_string)
                .filter(|name| !name.is_empty()),
        })
    } else {
        Ok(PcbNetRef {
            ordinal: None,
            name: Some(token_string(token)),
        })
    }
}

fn optional_uuid(source: &str, children: &[FormSpan]) -> Result<Option<String>, Error> {
    if let Some(span) = child(children, "uuid").or_else(|| child(children, "tstamp")) {
        first_string(source, span)
    } else {
        Ok(None)
    }
}

fn required_string(
    token: Option<&Token<'_>>,
    message: &'static str,
    span: &FormSpan,
) -> Result<String, Error> {
    token
        .map(token_string)
        .ok_or_else(|| source_error(message, span.end))
}

fn token_string(token: &Token<'_>) -> String {
    if token.kind == TokenKind::QuotedString {
        decode_quoted(token.lexeme)
    } else {
        token.lexeme.to_owned()
    }
}

fn required_i64(
    token: Option<&Token<'_>>,
    message: &'static str,
    span: &FormSpan,
) -> Result<i64, Error> {
    token
        .map(|value| parse_i64(value, span))
        .unwrap_or_else(|| Err(source_error(message, span.end)))
}

fn parse_i64(token: &Token<'_>, span: &FormSpan) -> Result<i64, Error> {
    token.lexeme.parse().map_err(|_| {
        source_error(
            "Expected integer value",
            rebase_position(token.position, span),
        )
    })
}

fn required_f64(
    token: Option<&Token<'_>>,
    message: &'static str,
    span: &FormSpan,
) -> Result<f64, Error> {
    token
        .map(|value| parse_f64(value, span))
        .unwrap_or_else(|| Err(source_error(message, span.end)))
}

fn optional_f64(token: Option<&Token<'_>>, span: &FormSpan) -> Result<Option<f64>, Error> {
    token.map(|value| parse_f64(value, span)).transpose()
}

fn parse_f64(token: &Token<'_>, span: &FormSpan) -> Result<f64, Error> {
    token.lexeme.parse().map_err(|_| {
        source_error(
            "Expected numeric value",
            rebase_position(token.position, span),
        )
    })
}

fn expect_kind(
    token: Option<Result<Token<'_>, Error>>,
    kind: TokenKind,
    message: &'static str,
) -> Result<(), Error> {
    let token = token
        .transpose()?
        .ok_or_else(|| source_error(message, Position::START))?;
    if token.kind != kind {
        return Err(source_error(message, token.position));
    }
    Ok(())
}

fn next_scalar<'a>(
    token: Option<Result<Token<'a>, Error>>,
    message: &'static str,
) -> Result<Token<'a>, Error> {
    let token = token
        .transpose()?
        .ok_or_else(|| source_error(message, Position::START))?;
    if matches!(token.kind, TokenKind::Left | TokenKind::Right) {
        return Err(source_error(message, token.position));
    }
    Ok(token)
}

fn bounded_push<T>(values: &mut Vec<T>, value: T, maximum: usize) -> Result<(), Error> {
    if values.len() == maximum {
        return Err(limit_error());
    }
    values.push(value);
    Ok(())
}

fn projection_limits(limits: PcbLimits, max_selected_forms: usize) -> ProjectionLimits {
    ProjectionLimits {
        max_source_bytes: limits.max_source_bytes,
        max_depth: limits.max_depth,
        max_selected_forms,
        ..ProjectionLimits::default()
    }
}

fn is_known_metadata(head: &str) -> bool {
    matches!(
        head,
        "version"
            | "generator"
            | "generator_version"
            | "general"
            | "paper"
            | "title_block"
            | "setup"
            | "variants"
            | "embedded_fonts"
    )
}

fn is_known_top_level(head: &str) -> bool {
    is_known_metadata(head)
        || matches!(
            head,
            "layers"
                | "net"
                | "property"
                | "footprint"
                | "module"
                | "zone"
                | "dimension"
                | "segment"
                | "via"
                | "arc"
                | "group"
                | "generated"
                | "embedded_files"
                | "gr_text"
                | "gr_line"
                | "gr_rect"
                | "gr_arc"
                | "gr_circle"
                | "gr_poly"
                | "gr_curve"
                | "gr_text_box"
                | "image"
                | "barcode"
                | "table"
        )
}

fn rebase_error(mut error: Error, span: &FormSpan) -> Error {
    if let Some(position) = error.position {
        error.position = Some(rebase_position(position, span));
    }
    error
}

fn rebase_position(position: Position, span: &FormSpan) -> Position {
    Position {
        offset: span.range.start.saturating_add(position.offset),
        line: span
            .start
            .line
            .saturating_add(position.line.saturating_sub(1)),
        column: if position.line == 1 {
            span.start
                .column
                .saturating_add(position.column.saturating_sub(1))
        } else {
            position.column
        },
    }
}

fn source_error(message: &'static str, position: Position) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::UnexpectedToken,
        message,
        position,
    )
}

fn limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "PCB typed read exceeds configured limits",
        Position::START,
    )
}

fn output_limit_error() -> Error {
    Error::build(
        ErrorKind::ResourceLimit,
        "PCB output exceeds max_output_bytes",
    )
}
