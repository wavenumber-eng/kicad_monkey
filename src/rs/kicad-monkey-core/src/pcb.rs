//! Native, source-backed views and focused edits for KiCad PCB documents.
//!
//! The board model indexes exact source spans and decodes domain records on
//! demand. It intentionally does not construct the generic compatibility tree.

use crate::sexpr::{
    Error, ErrorKind, ErrorPhase, Lexer, Patch, Position, Sexp, Token, TokenKind,
    apply_patches_with_limit, build_with_limit, decode_quoted, is_teardrop_numeric_key,
};
use crate::sexpr_projection::{FormSpan, ProjectionLimits, Selector, scan_form_spans_with_limits};
use crate::{KiCadPaper, KiCadTextEffects, KiCadTitleBlock};
use std::collections::BTreeMap;
use std::ops::Range;

mod counts;
mod decode;
mod document;
mod extended;
mod footprints;
mod indexing;
mod manufacturing;
mod pads;
mod physical;
mod scalars;
mod selection;
mod setup;
mod vias;
mod zones;
use self::{decode::*, indexing::*, scalars::*};
pub use extended::{
    PcbBarcode, PcbBoardMetadata, PcbBoardVariant, PcbImage, PcbTable, PcbTableCell,
};
pub use footprints::{
    PcbFootprintGraphic, PcbFootprintProperty, PcbFootprintText, PcbFootprintTextBox,
};
pub use manufacturing::{
    PcbDrillLayerSpan, PcbDrillProperties, PcbPostMachiningProperties, PcbTeardropParameters,
    PcbZoneLayerConnections,
};
pub use pads::{PcbPad, PcbPadCustomOptions, PcbPadCustomPrimitive};
pub use physical::{
    PcbFootprintTransform, PcbHole, PcbHoleOwner, PcbHoleShape, PcbPadDrill, PcbProfileOwner,
    PcbProfilePrimitive,
};
pub use selection::{PcbFamily, PcbSelection};
pub use setup::{PcbSetup, PcbStackup, PcbStackupLayer};
pub use vias::{PcbFrontBackOptionalBool, PcbVia};
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
    pub max_setup_children: usize,
    pub max_stackup_layers: usize,
    pub max_title_block_children: usize,
    pub max_title_block_comments: usize,
    pub max_layers: usize,
    pub max_nets: usize,
    pub max_properties: usize,
    pub max_footprints: usize,
    pub max_footprint_children: usize,
    pub max_footprint_attributes: usize,
    pub max_footprint_properties: usize,
    pub max_footprint_graphics: usize,
    pub max_footprint_texts: usize,
    pub max_footprint_text_boxes: usize,
    pub max_text_effect_children: usize,
    pub max_text_font_children: usize,
    pub max_text_justify_tokens: usize,
    pub max_text_box_points: usize,
    pub max_pad_header_scalars: usize,
    pub max_pad_children: usize,
    pub max_pad_chamfer_corners: usize,
    pub max_pad_custom_primitives: usize,
    pub max_pad_custom_point_forms: usize,
    pub max_pad_custom_points: usize,
    pub max_via_header_scalars: usize,
    pub max_via_children: usize,
    pub max_via_policy_children: usize,
    pub max_manufacturing_children: usize,
    pub max_teardrop_scalars: usize,
    pub max_zone_layer_connections: usize,
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
            max_setup_children: 16_384,
            max_stackup_layers: 512,
            max_title_block_children: 4_096,
            max_title_block_comments: 1_024,
            max_layers: 256,
            max_nets: 2_000_000,
            max_properties: 100_000,
            max_footprints: 1_000_000,
            max_footprint_children: 1_000_000,
            max_footprint_attributes: 256,
            max_footprint_properties: 4_000_000,
            max_footprint_graphics: 4_000_000,
            max_footprint_texts: 4_000_000,
            max_footprint_text_boxes: 4_000_000,
            max_text_effect_children: 4_096,
            max_text_font_children: 4_096,
            max_text_justify_tokens: 1_024,
            max_text_box_points: 1_000_000,
            max_pad_header_scalars: 256,
            max_pad_children: 256,
            max_pad_chamfer_corners: 64,
            max_pad_custom_primitives: 1_000_000,
            max_pad_custom_point_forms: 4_000_000,
            max_pad_custom_points: 4_000_000,
            max_via_header_scalars: 32,
            max_via_children: 4_096,
            max_via_policy_children: 256,
            max_manufacturing_children: 4_096,
            max_teardrop_scalars: 256,
            max_zone_layer_connections: 4_096,
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
    pub footprint_texts: usize,
    pub footprint_text_boxes: usize,
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
    pub text_count: usize,
    pub text_box_count: usize,
    pub pad_count: usize,
    pub model_count: usize,
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
    /// Bare `locked` header flag only; the Python oracle's `has_flag` does
    /// not honor the `(locked yes)` child form on segments.
    pub locked: bool,
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
    /// Optional `gr_text_box` border state, including bare/empty forms.
    pub border: Option<bool>,
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
pub struct PcbDimensionFormat {
    pub prefix: String,
    pub suffix: String,
    pub units: i64,
    pub units_format: i64,
    pub precision: i64,
    pub override_value: Option<String>,
    pub suppress_zeroes: bool,
}

impl Default for PcbDimensionFormat {
    fn default() -> Self {
        Self {
            prefix: String::new(),
            suffix: String::new(),
            units: 2,
            units_format: 1,
            precision: 4,
            override_value: None,
            suppress_zeroes: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbDimensionStyle {
    pub thickness: f64,
    pub arrow_length: f64,
    pub text_position_mode: i64,
    pub arrow_direction: String,
    pub extension_height: f64,
    pub extension_offset: f64,
    pub keep_text_aligned: bool,
    pub text_frame: Option<i64>,
}

impl Default for PcbDimensionStyle {
    fn default() -> Self {
        Self {
            thickness: 0.2,
            arrow_length: 1.27,
            text_position_mode: 0,
            arrow_direction: "outward".to_owned(),
            extension_height: 0.58642,
            extension_offset: 0.0,
            keep_text_aligned: true,
            text_frame: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct PcbDimension {
    pub kind: String,
    pub layer: String,
    pub points: Vec<PcbPoint>,
    pub height: f64,
    pub leader_length: Option<f64>,
    pub orientation: Option<i64>,
    pub format: PcbDimensionFormat,
    pub style: PcbDimensionStyle,
    pub text: Option<PcbGraphic>,
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
    text_count: usize,
    text_box_count: usize,
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
        // Python's `NetRef.resolve_name` fills any falsy name, so a
        // present-but-empty `(net_name "")` still resolves from the table.
        if net.name.as_deref().is_none_or(str::is_empty)
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
    footprint_texts: Vec<IndexedNestedForm>,
    footprint_text_boxes: Vec<IndexedNestedForm>,
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
    footprint_texts: Vec<IndexedNestedForm>,
    footprint_text_boxes: Vec<IndexedNestedForm>,
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
            footprint_texts: index.footprint_texts,
            footprint_text_boxes: index.footprint_text_boxes,
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
                pads::pad_from_span(self.source, indexed, self.limits).map(|mut pad| {
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
                vias::via_from_span(self.source, span, self.limits).map(|mut via| {
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
        let matches = self.matching_properties(name)?;
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

    /// Update or append one unambiguous top-level board property.
    pub fn upsert_property(&self, name: &str, value: &str) -> Result<PcbEdit, Error> {
        let matches = self.matching_properties(name)?;
        match matches.as_slice() {
            [_] => self.set_property(name, value),
            [] => self.insert_property(name, value),
            _ => Err(source_error(
                "PCB property name is ambiguous",
                self.root.start,
            )),
        }
    }

    /// Remove one unambiguous top-level board property by name.
    pub fn remove_property(&self, name: &str) -> Result<PcbEdit, Error> {
        if self.source.len() > self.limits.max_output_bytes {
            return Err(output_limit_error());
        }
        let matches = self.matching_properties(name)?;
        match matches.as_slice() {
            [] => Ok(PcbEdit {
                source: self.source.to_owned(),
                changed: false,
            }),
            [property] => Ok(PcbEdit {
                source: apply_patches_with_limit(
                    self.source,
                    &[Patch::new(
                        property.source_range.start,
                        property.source_range.end,
                        "",
                    )],
                    self.limits.max_output_bytes,
                )?,
                changed: true,
            }),
            _ => Err(source_error(
                "PCB property name is ambiguous",
                self.root.start,
            )),
        }
    }

    fn matching_properties(&self, name: &str) -> Result<Vec<PcbProperty>, Error> {
        self.top_level
            .iter()
            .filter(|span| span.head.as_deref() == Some("property"))
            .map(|span| property_from_span(self.source, span))
            .filter_map(|property| match property {
                Ok(property) if property.name == name => Some(Ok(property)),
                Ok(_) => None,
                Err(error) => Some(Err(error)),
            })
            .collect()
    }

    fn insert_property(&self, name: &str, value: &str) -> Result<PcbEdit, Error> {
        let property_count = self
            .top_level
            .iter()
            .filter(|span| span.head.as_deref() == Some("property"))
            .count();
        if property_count >= self.limits.max_properties {
            return Err(limit_error());
        }
        let form = build_with_limit(
            &Sexp::List(vec![
                Sexp::Atom("property".to_owned()),
                Sexp::Quoted(name.to_owned()),
                Sexp::Quoted(value.to_owned()),
            ]),
            self.limits.max_output_bytes,
        )?;
        let offset = self.root.range.end.saturating_sub(1);
        let newline = if self.source.contains("\r\n") {
            "\r\n"
        } else {
            "\n"
        };
        let prefix =
            if self.source[..offset].ends_with('\n') || self.source[..offset].ends_with('\r') {
                ""
            } else {
                newline
            };
        let replacement = format!("{prefix}  {form}{newline}");
        Ok(PcbEdit {
            source: apply_patches_with_limit(
                self.source,
                &[Patch::new(offset, offset, replacement)],
                self.limits.max_output_bytes,
            )?,
            changed: true,
        })
    }
}

pub use document::PcbDocument;
