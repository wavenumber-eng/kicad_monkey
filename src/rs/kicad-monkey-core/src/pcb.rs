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

mod document;
mod extended;
mod footprints;
mod physical;
mod selection;
mod zones;
pub use extended::{
    PcbBarcode, PcbBoardMetadata, PcbBoardVariant, PcbImage, PcbTable, PcbTableCell,
};
pub use footprints::{PcbFootprintGraphic, PcbFootprintProperty};
pub use physical::{
    PcbFootprintTransform, PcbHole, PcbHoleOwner, PcbHoleShape, PcbPadDrill, PcbProfileOwner,
    PcbProfilePrimitive,
};
pub use selection::{PcbFamily, PcbSelection};
pub use zones::{
    PcbZone, PcbZoneFilledPolygon, PcbZoneKeepout, PcbZoneLayerProperty, PcbZonePlacement,
    PcbZonePlacementSource, PcbZonePolygon,
};

/// Resource ceilings for one native PCB read or focused edit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PcbLimits {
    pub max_source_bytes: usize,
    pub max_output_bytes: usize,
    pub max_depth: usize,
    pub max_top_level_forms: usize,
    pub max_object_children: usize,
    pub max_layers: usize,
    pub max_nets: usize,
    pub max_properties: usize,
    pub max_footprints: usize,
    pub max_footprint_children: usize,
    pub max_footprint_attributes: usize,
    pub max_footprint_properties: usize,
    pub max_footprint_graphics: usize,
    pub max_pad_children: usize,
    pub max_model_children: usize,
    pub max_pads: usize,
    pub max_models: usize,
    pub max_segments: usize,
    pub max_vias: usize,
    pub max_zones: usize,
    pub max_zone_polygons: usize,
    pub max_zone_points: usize,
    pub max_zone_layer_properties: usize,
    pub max_arcs: usize,
    pub max_graphics: usize,
    pub max_graphic_points: usize,
    pub max_dimensions: usize,
    pub max_groups: usize,
    pub max_generated_items: usize,
    pub max_generated_children: usize,
    pub max_members: usize,
    pub max_embedded_files: usize,
    pub max_variants: usize,
    pub max_images: usize,
    pub max_image_data_parts: usize,
    pub max_barcodes: usize,
    pub max_tables: usize,
    pub max_table_cells: usize,
    pub max_table_values: usize,
}

impl Default for PcbLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 512 * 1024 * 1024,
            max_output_bytes: 512 * 1024 * 1024,
            max_depth: 512,
            max_top_level_forms: 4_000_000,
            max_object_children: 1_000_000,
            max_layers: 256,
            max_nets: 2_000_000,
            max_properties: 100_000,
            max_footprints: 1_000_000,
            max_footprint_children: 1_000_000,
            max_footprint_attributes: 256,
            max_footprint_properties: 4_000_000,
            max_footprint_graphics: 4_000_000,
            max_pad_children: 256,
            max_model_children: 32,
            max_pads: 4_000_000,
            max_models: 1_000_000,
            max_segments: 8_000_000,
            max_vias: 4_000_000,
            max_zones: 1_000_000,
            max_zone_polygons: 1_000_000,
            max_zone_points: 16_000_000,
            max_zone_layer_properties: 1_000_000,
            max_arcs: 8_000_000,
            max_graphics: 4_000_000,
            max_graphic_points: 4_000_000,
            max_dimensions: 1_000_000,
            max_groups: 1_000_000,
            max_generated_items: 1_000_000,
            max_generated_children: 4_000_000,
            max_members: 4_000_000,
            max_embedded_files: 100_000,
            max_variants: 100_000,
            max_images: 100_000,
            max_image_data_parts: 1_000_000,
            max_barcodes: 100_000,
            max_tables: 100_000,
            max_table_cells: 1_000_000,
            max_table_values: 1_000_000,
        }
    }
}

/// Collection counts used to compare the lazy view with the Python model.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PcbCounts {
    pub layers: usize,
    pub nets: usize,
    pub properties: usize,
    pub variants: usize,
    pub footprints: usize,
    pub footprint_properties: usize,
    pub pads: usize,
    pub models: usize,
    pub footprint_graphics: usize,
    pub segments: usize,
    pub vias: usize,
    pub zones: usize,
    pub arcs: usize,
    pub graphics: usize,
    pub gr_texts: usize,
    pub gr_lines: usize,
    pub gr_rects: usize,
    pub gr_arcs: usize,
    pub gr_circles: usize,
    pub gr_polys: usize,
    pub gr_curves: usize,
    pub gr_text_boxes: usize,
    pub images: usize,
    pub barcodes: usize,
    pub tables: usize,
    pub table_cells: usize,
    pub groups: usize,
    pub dimensions: usize,
    pub generated_items: usize,
    pub embedded_files: usize,
    pub unknown_top_level: usize,
}

impl PcbCounts {
    fn retain_selection(&mut self, selection: PcbSelection) {
        self.retain_primary_selection(selection);
        self.retain_extended_selection(selection);
    }

    fn retain_primary_selection(&mut self, selection: PcbSelection) {
        if !selection.contains(PcbFamily::Layers) {
            self.layers = 0;
        }
        if !selection.contains(PcbFamily::Nets) {
            self.nets = 0;
        }
        if !selection.contains(PcbFamily::Properties) {
            self.properties = 0;
        }
        if !selection.contains(PcbFamily::Footprints) {
            self.footprints = 0;
        }
        if !selection.contains(PcbFamily::Pads) {
            self.pads = 0;
        }
        if !selection.contains(PcbFamily::Models) {
            self.models = 0;
        }
        if !selection.contains(PcbFamily::FootprintProperties) {
            self.footprint_properties = 0;
        }
        if !selection.contains(PcbFamily::FootprintGraphics) {
            self.footprint_graphics = 0;
        }
        if !selection.contains(PcbFamily::Segments) {
            self.segments = 0;
        }
        if !selection.contains(PcbFamily::Vias) {
            self.vias = 0;
        }
        if !selection.contains(PcbFamily::Zones) {
            self.zones = 0;
        }
        if !selection.contains(PcbFamily::Arcs) {
            self.arcs = 0;
        }
    }

    fn retain_extended_selection(&mut self, selection: PcbSelection) {
        if !selection.contains(PcbFamily::Graphics) {
            self.graphics = 0;
            self.gr_texts = 0;
            self.gr_lines = 0;
            self.gr_rects = 0;
            self.gr_arcs = 0;
            self.gr_circles = 0;
            self.gr_polys = 0;
            self.gr_curves = 0;
            self.gr_text_boxes = 0;
        }
        if !selection.contains(PcbFamily::Images) {
            self.images = 0;
        }
        if !selection.contains(PcbFamily::Barcodes) {
            self.barcodes = 0;
        }
        if !selection.contains(PcbFamily::Tables) {
            self.tables = 0;
            self.table_cells = 0;
        }
        if !selection.contains(PcbFamily::Groups) {
            self.groups = 0;
        }
        if !selection.contains(PcbFamily::Dimensions) {
            self.dimensions = 0;
        }
        if !selection.contains(PcbFamily::GeneratedItems) {
            self.generated_items = 0;
        }
        if !selection.contains(PcbFamily::EmbeddedFiles) {
            self.embedded_files = 0;
        }
        if !selection.contains(PcbFamily::Variants) {
            self.variants = 0;
        }
    }
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
    pub locked: bool,
    pub placement_path: Option<String>,
    pub placement_sheet_name: Option<String>,
    pub placement_sheet_file: Option<String>,
    pub uuid: Option<String>,
    pub description: String,
    pub tags: String,
    pub attributes: Vec<String>,
    pub embedded_fonts: bool,
    pub duplicate_pad_numbers_are_jumpers: Option<bool>,
    pub solder_mask_margin: Option<f64>,
    pub solder_paste_margin: Option<f64>,
    pub solder_paste_margin_ratio: Option<f64>,
    pub clearance: Option<f64>,
    pub zone_connect: Option<i64>,
    pub property_count: usize,
    pub graphic_count: usize,
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
    pub drill: Option<PcbPadDrill>,
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PcbPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PcbGraphicKind {
    Text,
    Line,
    Rect,
    Arc,
    Circle,
    Poly,
    Curve,
    TextBox,
}

/// A producer-neutral typed view of one board graphic carrier.
#[derive(Clone, Debug, PartialEq)]
pub struct PcbGraphic {
    pub kind: PcbGraphicKind,
    pub text: Option<String>,
    pub at: Option<PcbPoint>,
    pub start: Option<PcbPoint>,
    pub mid: Option<PcbPoint>,
    pub end: Option<PcbPoint>,
    pub center: Option<PcbPoint>,
    pub points: Vec<PcbPoint>,
    pub layer: Option<String>,
    pub stroke_width: Option<f64>,
    pub stroke_kind: Option<String>,
    pub fill: Option<String>,
    pub uuid: Option<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbRoutingArc {
    pub start: PcbPoint,
    pub mid: PcbPoint,
    pub end: PcbPoint,
    pub width: Option<f64>,
    pub layer: Option<String>,
    pub net: PcbNetRef,
    pub uuid: Option<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbDimension {
    pub kind: String,
    pub layer: String,
    pub points: Vec<PcbPoint>,
    pub height: f64,
    pub leader_length: Option<f64>,
    pub orientation: Option<i64>,
    pub locked: bool,
    pub uuid: Option<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcbGroup {
    pub name: String,
    pub uuid: Option<String>,
    pub locked: bool,
    pub members: Vec<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcbGeneratedItem {
    pub kind: Option<String>,
    pub name: Option<String>,
    pub layer: Option<String>,
    pub uuid: Option<String>,
    pub locked: bool,
    pub members: Vec<String>,
    pub property_heads: Vec<String>,
    pub source_range: Range<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PcbEmbeddedFile {
    pub name: String,
    pub file_type: String,
    pub checksum: Option<String>,
    pub encoded_data_bytes: usize,
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
    property_count: usize,
    graphic_count: usize,
    pad_count: usize,
    model_count: usize,
}

#[derive(Clone, Debug)]
struct IndexedNestedForm {
    parent_index: usize,
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
    footprint_properties: Vec<IndexedNestedForm>,
    pads: Vec<IndexedNestedForm>,
    models: Vec<IndexedNestedForm>,
    footprint_graphics: Vec<IndexedNestedForm>,
    segments: Vec<FormSpan>,
    vias: Vec<FormSpan>,
    zones: Vec<FormSpan>,
    graphics: Vec<FormSpan>,
    arcs: Vec<FormSpan>,
    dimensions: Vec<FormSpan>,
    groups: Vec<FormSpan>,
    generated_items: Vec<FormSpan>,
    embedded_files: Vec<FormSpan>,
    variants: Vec<FormSpan>,
    images: Vec<FormSpan>,
    barcodes: Vec<FormSpan>,
    tables: Vec<extended::IndexedTable>,
    table_cells: Vec<IndexedNestedForm>,
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
    footprint_properties: Vec<IndexedNestedForm>,
    pads: Vec<IndexedNestedForm>,
    models: Vec<IndexedNestedForm>,
    footprint_graphics: Vec<IndexedNestedForm>,
    segments: Vec<FormSpan>,
    vias: Vec<FormSpan>,
    zones: Vec<FormSpan>,
    graphics: Vec<FormSpan>,
    arcs: Vec<FormSpan>,
    dimensions: Vec<FormSpan>,
    groups: Vec<FormSpan>,
    generated_items: Vec<FormSpan>,
    embedded_files: Vec<FormSpan>,
    variants: Vec<FormSpan>,
    images: Vec<FormSpan>,
    barcodes: Vec<FormSpan>,
    tables: Vec<extended::IndexedTable>,
    table_cells: Vec<IndexedNestedForm>,
    net_resolver: NetResolver,
    counts: PcbCounts,
    limits: PcbLimits,
    selection: PcbSelection,
}

impl<'a> PcbView<'a> {
    /// Validate one `kicad_pcb` root and index its major domain families.
    pub fn parse(source: &'a str, limits: PcbLimits) -> Result<Self, Error> {
        Self::parse_selected(source, limits, PcbSelection::all())
    }

    /// Validate a board and index only the requested families plus dependencies.
    pub fn parse_selected(
        source: &'a str,
        limits: PcbLimits,
        selection: PcbSelection,
    ) -> Result<Self, Error> {
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

        let effective = selection.dependencies();
        let mut index = index_top_level(source, &top_level, limits, effective)?;
        let net_resolver = net_resolver_from_spans(source, &index.nets)?;
        index.counts.retain_selection(selection);

        Ok(Self {
            source,
            root,
            top_level,
            layer_forms: index.layer_forms,
            properties: index.properties,
            nets: index.nets,
            footprints: index.footprints,
            footprint_properties: index.footprint_properties,
            pads: index.pads,
            models: index.models,
            footprint_graphics: index.footprint_graphics,
            segments: index.segments,
            vias: index.vias,
            zones: index.zones,
            graphics: index.graphics,
            arcs: index.arcs,
            dimensions: index.dimensions,
            groups: index.groups,
            generated_items: index.generated_items,
            embedded_files: index.embedded_files,
            variants: index.variants,
            images: index.images,
            barcodes: index.barcodes,
            tables: index.tables,
            table_cells: index.table_cells,
            net_resolver,
            counts: index.counts,
            limits,
            selection,
        })
    }

    pub fn counts(&self) -> PcbCounts {
        self.counts
    }

    pub fn source(&self) -> &'a str {
        self.source
    }

    pub fn selection(&self) -> PcbSelection {
        self.selection
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
            .filter(move |_| self.selection.contains(PcbFamily::Layers))
            .map(|span| layer_from_span(self.source, span))
    }

    pub fn nets(&self) -> impl Iterator<Item = Result<PcbNet, Error>> + '_ {
        self.nets
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::Nets))
            .map(|span| net_from_span(self.source, span))
    }

    pub fn properties(&self) -> impl Iterator<Item = Result<PcbProperty, Error>> + '_ {
        self.properties
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::Properties))
            .map(|span| property_from_span(self.source, span))
    }

    pub fn footprints(&self) -> impl Iterator<Item = Result<PcbFootprint, Error>> + '_ {
        self.footprints
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::Footprints))
            .map(|indexed| footprint_from_span(self.source, indexed, self.limits))
    }

    pub fn pads(&self) -> impl Iterator<Item = Result<PcbPad, Error>> + '_ {
        self.pads
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::Pads))
            .map(|indexed| {
                pad_from_span(self.source, indexed, self.limits).map(|mut pad| {
                    pad.net = self.net_resolver.resolve(pad.net);
                    pad
                })
            })
    }

    pub fn models(&self) -> impl Iterator<Item = Result<PcbModelReference, Error>> + '_ {
        self.models
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::Models))
            .map(|indexed| model_from_span(self.source, indexed, self.limits))
    }

    pub fn segments(&self) -> impl Iterator<Item = Result<PcbSegment, Error>> + '_ {
        self.segments
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::Segments))
            .map(|span| {
                segment_from_span(self.source, span, self.limits).map(|mut segment| {
                    segment.net = self.net_resolver.resolve(segment.net);
                    segment
                })
            })
    }

    pub fn vias(&self) -> impl Iterator<Item = Result<PcbVia, Error>> + '_ {
        self.vias
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::Vias))
            .map(|span| {
                via_from_span(self.source, span, self.limits).map(|mut via| {
                    via.net = self.net_resolver.resolve(via.net);
                    via
                })
            })
    }

    pub fn zones(&self) -> impl Iterator<Item = Result<PcbZone, Error>> + '_ {
        self.zones
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::Zones))
            .map(|span| {
                zones::zone_from_span(self.source, span, self.limits).map(|mut zone| {
                    zone.net = self.net_resolver.resolve(zone.net);
                    zone
                })
            })
    }

    pub fn graphics(&self) -> impl Iterator<Item = Result<PcbGraphic, Error>> + '_ {
        self.graphics
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::Graphics))
            .map(|span| graphic_from_span(self.source, span, self.limits))
    }

    pub fn arcs(&self) -> impl Iterator<Item = Result<PcbRoutingArc, Error>> + '_ {
        self.arcs
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::Arcs))
            .map(|span| {
                routing_arc_from_span(self.source, span, self.limits).map(|mut arc| {
                    arc.net = self.net_resolver.resolve(arc.net);
                    arc
                })
            })
    }

    pub fn dimensions(&self) -> impl Iterator<Item = Result<PcbDimension, Error>> + '_ {
        self.dimensions
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::Dimensions))
            .map(|span| dimension_from_span(self.source, span, self.limits))
    }

    pub fn groups(&self) -> impl Iterator<Item = Result<PcbGroup, Error>> + '_ {
        self.groups
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::Groups))
            .map(|span| group_from_span(self.source, span, self.limits))
    }

    pub fn generated_items(&self) -> impl Iterator<Item = Result<PcbGeneratedItem, Error>> + '_ {
        self.generated_items
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::GeneratedItems))
            .map(|span| generated_from_span(self.source, span, self.limits))
    }

    pub fn embedded_files(&self) -> impl Iterator<Item = Result<PcbEmbeddedFile, Error>> + '_ {
        self.embedded_files
            .iter()
            .filter(move |_| self.selection.contains(PcbFamily::EmbeddedFiles))
            .map(|span| embedded_file_from_span(self.source, span, self.limits))
    }

    /// Remove one unambiguous identified top-level object by `uuid` or legacy `id`.
    pub fn remove_top_level_by_id(&self, identifier: &str) -> Result<PcbEdit, Error> {
        if self.source.len() > self.limits.max_output_bytes {
            return Err(output_limit_error());
        }
        if identifier.is_empty() {
            return Err(source_error(
                "PCB object identifier cannot be empty",
                self.root.start,
            ));
        }
        let mut matches = Vec::new();
        for span in &self.top_level {
            if top_level_identifier(self.source, span, self.limits)?.as_deref() == Some(identifier)
            {
                matches.push(span);
            }
        }
        match matches.as_slice() {
            [] => Ok(PcbEdit {
                source: self.source.to_owned(),
                changed: false,
            }),
            [span] => Ok(PcbEdit {
                source: apply_patches_with_limit(
                    self.source,
                    &[Patch::new(span.range.start, span.range.end, "")],
                    self.limits.max_output_bytes,
                )?,
                changed: true,
            }),
            _ => Err(source_error(
                "PCB object identifier is ambiguous",
                self.root.start,
            )),
        }
    }

    /// Replace one unambiguous top-level board property without rewriting the board.
    pub fn set_property(&self, name: &str, value: &str) -> Result<PcbEdit, Error> {
        if self.source.len() > self.limits.max_output_bytes {
            return Err(output_limit_error());
        }
        let matches = self
            .top_level
            .iter()
            .filter(|span| span.head.as_deref() == Some("property"))
            .map(|span| property_from_span(self.source, span))
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

fn top_level_identifier(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<Option<String>, Error> {
    let maximum = match span.head.as_deref() {
        Some("footprint" | "module") => limits.max_footprint_children,
        Some("generated") => limits.max_generated_children,
        _ => limits.max_object_children,
    };
    let children = direct_children(source, span, maximum, limits)?;
    optional_uuid_or_id(source, &children)
}

fn index_top_level(
    source: &str,
    top_level: &[FormSpan],
    limits: PcbLimits,
    selection: PcbSelection,
) -> Result<PcbIndex, Error> {
    let mut index = PcbIndex::default();
    for span in top_level {
        let Some(head) = span.head.as_deref() else {
            index.counts.unknown_top_level += 1;
            continue;
        };
        if top_level_family(head).is_some_and(|family| !selection.contains(family)) {
            continue;
        }
        match head {
            "layers" => {
                index_layers(source, span, limits, &mut index)?;
            }
            "net" => {
                bounded_push(&mut index.nets, span.clone(), limits.max_nets)?;
                index.counts.nets += 1;
            }
            "property" => {
                bounded_push(&mut index.properties, span.clone(), limits.max_properties)?;
                index.counts.properties += 1;
            }
            "footprint" | "module" => {
                index_footprint(source, span, limits, selection, &mut index)?;
            }
            "segment" => {
                bounded_push(&mut index.segments, span.clone(), limits.max_segments)?;
                index.counts.segments += 1;
            }
            "via" => {
                bounded_push(&mut index.vias, span.clone(), limits.max_vias)?;
                index.counts.vias += 1;
            }
            "zone" => {
                bounded_push(&mut index.zones, span.clone(), limits.max_zones)?;
                index.counts.zones += 1;
            }
            "arc" => {
                bounded_push(&mut index.arcs, span.clone(), limits.max_arcs)?;
                index.counts.arcs += 1;
            }
            head if graphic_kind(head).is_some() => {
                bounded_push(&mut index.graphics, span.clone(), limits.max_graphics)?;
                increment_graphic_count(&mut index.counts, head);
            }
            "group" => {
                bounded_push(&mut index.groups, span.clone(), limits.max_groups)?;
                index.counts.groups += 1;
            }
            "dimension" => {
                bounded_push(&mut index.dimensions, span.clone(), limits.max_dimensions)?;
                index.counts.dimensions += 1;
            }
            "generated" => {
                bounded_push(
                    &mut index.generated_items,
                    span.clone(),
                    limits.max_generated_items,
                )?;
                index.counts.generated_items += 1;
            }
            "embedded_files" => {
                index_embedded_files(source, span, limits, &mut index)?;
            }
            "variants" => {
                extended::index_variants(source, span, limits, &mut index)?;
            }
            "image" => {
                bounded_push(&mut index.images, span.clone(), limits.max_images)?;
                index.counts.images += 1;
            }
            "barcode" => {
                bounded_push(&mut index.barcodes, span.clone(), limits.max_barcodes)?;
                index.counts.barcodes += 1;
            }
            "table" => {
                extended::index_table(source, span, limits, &mut index)?;
            }
            head if is_known_top_level(head) => {}
            _ => index.counts.unknown_top_level += 1,
        }
    }
    Ok(index)
}

fn top_level_family(head: &str) -> Option<PcbFamily> {
    match head {
        "layers" => Some(PcbFamily::Layers),
        "net" => Some(PcbFamily::Nets),
        "property" => Some(PcbFamily::Properties),
        "footprint" | "module" => Some(PcbFamily::Footprints),
        "segment" => Some(PcbFamily::Segments),
        "via" => Some(PcbFamily::Vias),
        "zone" => Some(PcbFamily::Zones),
        "arc" => Some(PcbFamily::Arcs),
        head if graphic_kind(head).is_some() => Some(PcbFamily::Graphics),
        "group" => Some(PcbFamily::Groups),
        "dimension" => Some(PcbFamily::Dimensions),
        "generated" => Some(PcbFamily::GeneratedItems),
        "embedded_files" => Some(PcbFamily::EmbeddedFiles),
        "variants" => Some(PcbFamily::Variants),
        "image" => Some(PcbFamily::Images),
        "barcode" => Some(PcbFamily::Barcodes),
        "table" => Some(PcbFamily::Tables),
        _ => None,
    }
}

fn increment_graphic_count(counts: &mut PcbCounts, head: &str) {
    counts.graphics += 1;
    match head {
        "gr_text" => counts.gr_texts += 1,
        "gr_line" => counts.gr_lines += 1,
        "gr_rect" => counts.gr_rects += 1,
        "gr_arc" => counts.gr_arcs += 1,
        "gr_circle" => counts.gr_circles += 1,
        "gr_poly" => counts.gr_polys += 1,
        "gr_curve" => counts.gr_curves += 1,
        "gr_text_box" => counts.gr_text_boxes += 1,
        _ => {}
    }
}

fn index_embedded_files(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
    index: &mut PcbIndex,
) -> Result<(), Error> {
    let children = direct_children(source, span, limits.max_embedded_files, limits)?;
    for child in children
        .into_iter()
        .filter(|child| child.head.as_deref() == Some("file"))
    {
        bounded_push(&mut index.embedded_files, child, limits.max_embedded_files)?;
    }
    index.counts.embedded_files = index.embedded_files.len();
    Ok(())
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
    selection: PcbSelection,
    index: &mut PcbIndex,
) -> Result<(), Error> {
    if index.footprints.len() == limits.max_footprints {
        return Err(limit_error());
    }
    let children = direct_children(source, span, limits.max_footprint_children, limits)?;
    let footprint_index = index.footprints.len();
    let mut property_count = 0usize;
    let mut graphic_count = 0usize;
    let mut pad_count = 0usize;
    let mut model_count = 0usize;
    for child in children {
        let indexed = IndexedNestedForm {
            parent_index: footprint_index,
            span: child,
        };
        match indexed.span.head.as_deref() {
            Some("property") => {
                property_count = property_count.checked_add(1).ok_or_else(limit_error)?;
                if selection.contains(PcbFamily::FootprintProperties) {
                    bounded_push(
                        &mut index.footprint_properties,
                        indexed,
                        limits.max_footprint_properties,
                    )?;
                }
            }
            Some("pad") => {
                pad_count = pad_count.checked_add(1).ok_or_else(limit_error)?;
                if selection.contains(PcbFamily::Pads) {
                    bounded_push(&mut index.pads, indexed, limits.max_pads)?;
                }
            }
            Some("model") => {
                model_count = model_count.checked_add(1).ok_or_else(limit_error)?;
                if selection.contains(PcbFamily::Models) {
                    bounded_push(&mut index.models, indexed, limits.max_models)?;
                }
            }
            Some(head) if physical::is_footprint_profile_head(head) => {
                graphic_count = graphic_count.checked_add(1).ok_or_else(limit_error)?;
                if selection.contains(PcbFamily::FootprintGraphics)
                    || selection.contains(PcbFamily::Profile)
                {
                    bounded_push(
                        &mut index.footprint_graphics,
                        indexed,
                        limits.max_footprint_graphics,
                    )?;
                }
            }
            _ => {}
        }
    }
    index.footprints.push(IndexedFootprint {
        span: span.clone(),
        property_count,
        graphic_count,
        pad_count,
        model_count,
    });
    index.counts.footprint_properties = index.footprint_properties.len();
    index.counts.pads = index.pads.len();
    index.counts.models = index.models.len();
    index.counts.footprint_graphics = index.footprint_graphics.len();
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
        locked: false,
        placement_path: None,
        placement_sheet_name: None,
        placement_sheet_file: None,
        uuid: None,
        description: String::new(),
        tags: String::new(),
        attributes: Vec::new(),
        embedded_fonts: false,
        duplicate_pad_numbers_are_jumpers: None,
        solder_mask_margin: None,
        solder_paste_margin: None,
        solder_paste_margin_ratio: None,
        clearance: None,
        zone_connect: None,
        property_count: indexed.property_count,
        graphic_count: indexed.graphic_count,
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
            Some("locked") => result.locked = child_bool(source, &children, "locked")?,
            Some("path") => result.placement_path = first_string(source, child)?,
            Some("sheetname") => result.placement_sheet_name = first_string(source, child)?,
            Some("sheetfile") => result.placement_sheet_file = first_string(source, child)?,
            Some("descr") => result.description = first_string(source, child)?.unwrap_or_default(),
            Some("tags") => result.tags = first_string(source, child)?.unwrap_or_default(),
            Some("attr") => {
                result.attributes =
                    bounded_scalar_values(source, child, limits.max_footprint_attributes)?
                        .iter()
                        .map(token_string)
                        .collect();
            }
            Some("embedded_fonts") => {
                result.embedded_fonts = child_bool(source, &children, "embedded_fonts")?;
            }
            Some("duplicate_pad_numbers_are_jumpers") => {
                result.duplicate_pad_numbers_are_jumpers = Some(child_bool(
                    source,
                    &children,
                    "duplicate_pad_numbers_are_jumpers",
                )?);
            }
            Some("solder_mask_margin") => {
                result.solder_mask_margin = first_f64(source, child)?;
            }
            Some("solder_paste_margin") => {
                result.solder_paste_margin = first_f64(source, child)?;
            }
            Some("solder_paste_margin_ratio") => {
                result.solder_paste_margin_ratio = first_f64(source, child)?;
            }
            Some("clearance") => result.clearance = first_f64(source, child)?,
            Some("zone_connect") => result.zone_connect = first_i64(source, child)?,
            Some("uuid" | "tstamp") if result.uuid.is_none() => {
                result.uuid = first_string(source, child)?;
            }
            _ => {}
        }
    }
    result.locked |= has_flag(&header, "locked");
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
    let layers = child_strings(source, &children, "layers", limits.max_layers)?;
    Ok(PcbPad {
        footprint_index: indexed.parent_index,
        number: required_string(header.first(), "Expected pad number", &indexed.span)?,
        kind: required_string(header.get(1), "Expected pad kind", &indexed.span)?,
        shape: required_string(header.get(2), "Expected pad shape", &indexed.span)?,
        at_x: at[0],
        at_y: at[1],
        angle: at[2],
        size_x: size[0],
        size_y: size[1],
        drill: physical::pad_drill_from_children(source, &children, limits)?,
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
        footprint_index: indexed.parent_index,
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
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    let start = required_xy(source, &children, "start", span)?;
    let end = required_xy(source, &children, "end", span)?;
    Ok(PcbSegment {
        start_x: start.0,
        start_y: start.1,
        end_x: end.0,
        end_y: end.1,
        width: optional_child_f64(source, &children, "width")?,
        layer: optional_child_string(source, &children, "layer")?,
        net: child_net_ref_or_zero(source, &children)?,
        uuid: optional_uuid(source, &children)?,
        source_range: span.range.clone(),
    })
}

fn via_from_span(source: &str, span: &FormSpan, limits: PcbLimits) -> Result<PcbVia, Error> {
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    let at = required_xy(source, &children, "at", span)?;
    let layers = child_strings(source, &children, "layers", limits.max_layers)?;
    Ok(PcbVia {
        at_x: at.0,
        at_y: at.1,
        size: optional_child_f64(source, &children, "size")?,
        drill: optional_child_f64(source, &children, "drill")?,
        layers,
        net: child_net_ref_or_zero(source, &children)?,
        uuid: optional_uuid(source, &children)?,
        source_range: span.range.clone(),
    })
}

fn graphic_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbGraphic, Error> {
    let kind = span
        .head
        .as_deref()
        .and_then(graphic_kind)
        .ok_or_else(|| source_error("Expected board graphic form", span.start))?;
    let header = scalar_values(source, span)?;
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    let points = child(&children, "pts")
        .map(|points| points_from_span(source, points, limits))
        .transpose()?
        .unwrap_or_default();
    let (stroke_width, stroke_kind) = if let Some(stroke) = child(&children, "stroke") {
        let fields = direct_children(source, stroke, 16, limits)?;
        (
            optional_child_f64(source, &fields, "width")?,
            optional_child_string(source, &fields, "type")?,
        )
    } else {
        (None, None)
    };
    Ok(PcbGraphic {
        kind,
        text: matches!(kind, PcbGraphicKind::Text | PcbGraphicKind::TextBox)
            .then(|| header.first().map(token_string))
            .flatten(),
        at: optional_child_point(source, &children, "at")?,
        start: optional_child_point(source, &children, "start")?,
        mid: optional_child_point(source, &children, "mid")?,
        end: optional_child_point(source, &children, "end")?,
        center: optional_child_point(source, &children, "center")?,
        points,
        layer: optional_child_string(source, &children, "layer")?,
        stroke_width,
        stroke_kind,
        fill: optional_child_string(source, &children, "fill")?,
        uuid: optional_uuid(source, &children)?,
        source_range: span.range.clone(),
    })
}

fn routing_arc_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbRoutingArc, Error> {
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    Ok(PcbRoutingArc {
        start: required_point(source, &children, "start", span)?,
        mid: required_point(source, &children, "mid", span)?,
        end: required_point(source, &children, "end", span)?,
        width: optional_child_f64(source, &children, "width")?,
        layer: optional_child_string(source, &children, "layer")?,
        net: child_net_ref_or_zero(source, &children)?,
        uuid: optional_uuid(source, &children)?,
        source_range: span.range.clone(),
    })
}

fn dimension_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbDimension, Error> {
    let header = scalar_values(source, span)?;
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    let points = child(&children, "pts")
        .map(|points| points_from_span(source, points, limits))
        .transpose()?
        .unwrap_or_default();
    Ok(PcbDimension {
        kind: optional_child_string(source, &children, "type")?
            .unwrap_or_else(|| "aligned".to_owned()),
        layer: optional_child_string(source, &children, "layer")?
            .unwrap_or_else(|| "Cmts.User".to_owned()),
        points,
        height: optional_child_f64(source, &children, "height")?.unwrap_or(0.0),
        leader_length: optional_child_f64(source, &children, "leader_length")?,
        orientation: optional_child_i64(source, &children, "orientation")?,
        locked: has_flag(&header, "locked") || child_bool(source, &children, "locked")?,
        uuid: optional_uuid(source, &children)?,
        source_range: span.range.clone(),
    })
}

fn group_from_span(source: &str, span: &FormSpan, limits: PcbLimits) -> Result<PcbGroup, Error> {
    let header = scalar_values(source, span)?;
    let children = direct_children(source, span, limits.max_object_children, limits)?;
    Ok(PcbGroup {
        name: required_string(header.first(), "Expected group name", span)?,
        uuid: optional_uuid_or_id(source, &children)?,
        locked: has_flag(&header, "locked") || child_bool(source, &children, "locked")?,
        members: child_strings(source, &children, "members", limits.max_members)?,
        source_range: span.range.clone(),
    })
}

fn generated_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbGeneratedItem, Error> {
    let header = scalar_values(source, span)?;
    let children = direct_children(source, span, limits.max_generated_children, limits)?;
    let known = ["id", "type", "name", "layer", "locked", "members"];
    let property_heads = children
        .iter()
        .filter_map(|child| child.head.as_ref())
        .filter(|head| !known.contains(&head.as_str()))
        .cloned()
        .collect();
    Ok(PcbGeneratedItem {
        kind: optional_child_string(source, &children, "type")?,
        name: optional_child_string(source, &children, "name")?,
        layer: optional_child_string(source, &children, "layer")?,
        uuid: optional_child_string(source, &children, "id")?,
        locked: has_flag(&header, "locked") || child_bool(source, &children, "locked")?,
        members: child_strings(source, &children, "members", limits.max_members)?,
        property_heads,
        source_range: span.range.clone(),
    })
}

fn embedded_file_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<PcbEmbeddedFile, Error> {
    let children = direct_children(source, span, 16, limits)?;
    let encoded_data_bytes = child(&children, "data")
        .map(|data| {
            scalar_values(source, data).map(|tokens| {
                tokens
                    .iter()
                    .map(token_string)
                    .map(|part| part.trim_matches('|').len())
                    .sum()
            })
        })
        .transpose()?
        .unwrap_or(0);
    Ok(PcbEmbeddedFile {
        name: optional_child_string(source, &children, "name")?.unwrap_or_default(),
        file_type: optional_child_string(source, &children, "type")?
            .unwrap_or_else(|| "other".to_owned()),
        checksum: optional_child_string(source, &children, "checksum")?,
        encoded_data_bytes,
        source_range: span.range.clone(),
    })
}

fn scalar_values<'a>(source: &'a str, span: &FormSpan) -> Result<Vec<Token<'a>>, Error> {
    bounded_scalar_values(source, span, usize::MAX)
}

fn bounded_scalar_values<'a>(
    source: &'a str,
    span: &FormSpan,
    maximum: usize,
) -> Result<Vec<Token<'a>>, Error> {
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
        let mut depth = 1usize;
        for token in lexer {
            let token = token?;
            match token.kind {
                TokenKind::Left => depth += 1,
                TokenKind::Right => {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                _ if depth == 1 => {
                    if values.len() >= maximum {
                        return Err(limit_error());
                    }
                    values.push(token);
                }
                _ => {}
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

fn required_point(
    source: &str,
    children: &[FormSpan],
    head: &str,
    parent: &FormSpan,
) -> Result<PcbPoint, Error> {
    let (x, y) = required_xy(source, children, head, parent)?;
    Ok(PcbPoint { x, y })
}

fn optional_child_point(
    source: &str,
    children: &[FormSpan],
    head: &str,
) -> Result<Option<PcbPoint>, Error> {
    let Some(span) = child(children, head) else {
        return Ok(None);
    };
    let values = scalar_values(source, span)?;
    Ok(Some(PcbPoint {
        x: required_f64(values.first(), "Expected x coordinate", span)?,
        y: required_f64(values.get(1), "Expected y coordinate", span)?,
    }))
}

fn points_from_span(
    source: &str,
    span: &FormSpan,
    limits: PcbLimits,
) -> Result<Vec<PcbPoint>, Error> {
    direct_children(source, span, limits.max_graphic_points, limits)?
        .into_iter()
        .filter(|point| point.head.as_deref() == Some("xy"))
        .map(|point| {
            let values = scalar_values(source, &point)?;
            Ok(PcbPoint {
                x: required_f64(values.first(), "Expected point x", &point)?,
                y: required_f64(values.get(1), "Expected point y", &point)?,
            })
        })
        .collect()
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
    Ok(first_scalar_value(source, span)?.as_ref().map(token_string))
}

fn first_f64(source: &str, span: &FormSpan) -> Result<Option<f64>, Error> {
    let value = first_scalar_value(source, span)?;
    optional_f64(value.as_ref(), span)
}

fn first_i64(source: &str, span: &FormSpan) -> Result<Option<i64>, Error> {
    first_scalar_value(source, span)?
        .as_ref()
        .map(|token| parse_i64(token, span))
        .transpose()
}

fn first_scalar_value<'a>(source: &'a str, span: &FormSpan) -> Result<Option<Token<'a>>, Error> {
    let text = span.text(source)?;
    (|| {
        let mut lexer = Lexer::new(text);
        expect_kind(
            lexer.next(),
            TokenKind::Left,
            "Expected form opening parenthesis",
        )?;
        let _head = next_scalar(lexer.next(), "Expected form head")?;
        let mut depth = 1usize;
        for token in lexer {
            let token = token?;
            match token.kind {
                TokenKind::Left => depth += 1,
                TokenKind::Right => {
                    depth -= 1;
                    if depth == 0 {
                        return Ok(None);
                    }
                }
                _ if depth == 1 => return Ok(Some(token)),
                _ => {}
            }
        }
        Ok(None)
    })()
    .map_err(|error| rebase_error(error, span))
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

fn optional_child_i64(
    source: &str,
    children: &[FormSpan],
    head: &str,
) -> Result<Option<i64>, Error> {
    let Some(span) = child(children, head) else {
        return Ok(None);
    };
    let values = scalar_values(source, span)?;
    values
        .first()
        .map(|token| parse_i64(token, span))
        .transpose()
}

fn child_strings(
    source: &str,
    children: &[FormSpan],
    head: &str,
    maximum: usize,
) -> Result<Vec<String>, Error> {
    let Some(span) = child(children, head) else {
        return Ok(Vec::new());
    };
    let values = bounded_scalar_values(source, span, maximum)?;
    Ok(values.iter().map(token_string).collect())
}

fn child_bool(source: &str, children: &[FormSpan], head: &str) -> Result<bool, Error> {
    let Some(span) = child(children, head) else {
        return Ok(false);
    };
    let values = scalar_values(source, span)?;
    Ok(values
        .first()
        .is_none_or(|value| matches!(token_string(value).as_str(), "yes" | "true" | "1")))
}

fn has_flag(values: &[Token<'_>], expected: &str) -> bool {
    values
        .iter()
        .any(|value| value.kind == TokenKind::Atom && value.lexeme == expected)
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

fn child_net_ref_or_zero(source: &str, children: &[FormSpan]) -> Result<PcbNetRef, Error> {
    if child(children, "net").is_none() {
        Ok(PcbNetRef {
            ordinal: Some(0),
            name: None,
        })
    } else {
        child_net_ref(source, children)
    }
}

fn optional_uuid(source: &str, children: &[FormSpan]) -> Result<Option<String>, Error> {
    if let Some(span) = child(children, "uuid").or_else(|| child(children, "tstamp")) {
        first_string(source, span)
    } else {
        Ok(None)
    }
}

fn optional_uuid_or_id(source: &str, children: &[FormSpan]) -> Result<Option<String>, Error> {
    if let Some(span) = child(children, "uuid").or_else(|| child(children, "id")) {
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

fn graphic_kind(head: &str) -> Option<PcbGraphicKind> {
    match head {
        "gr_text" | "fp_text" => Some(PcbGraphicKind::Text),
        "gr_line" | "fp_line" => Some(PcbGraphicKind::Line),
        "gr_rect" | "fp_rect" => Some(PcbGraphicKind::Rect),
        "gr_arc" | "fp_arc" => Some(PcbGraphicKind::Arc),
        "gr_circle" | "fp_circle" => Some(PcbGraphicKind::Circle),
        "gr_poly" | "fp_poly" => Some(PcbGraphicKind::Poly),
        "gr_curve" | "fp_curve" => Some(PcbGraphicKind::Curve),
        "gr_text_box" | "fp_text_box" => Some(PcbGraphicKind::TextBox),
        _ => None,
    }
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
pub use document::PcbDocument;
