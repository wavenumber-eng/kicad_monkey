//! Native end-to-end Toon composition used by the public Rust CLI.
//!
//! Toon has an explicit preset surface and does not imply that the broader
//! config-driven `pcb-svg` adapter is ready. This module exercises the production
//! source model, physical SVG renderer, embedded STEP resolver, and released
//! Geometer process in one composition.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{self, Write};
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use geo::{
    ConvexHull, MultiPoint, MultiPolygon, Point as GeoPoint, Polygon,
    algorithm::bool_ops::unary_union,
};
use geometer_client::ModelIllustrationGeometry;
use geometer_client::contracts::{MeshIllustrationGeometryA0, ModelIllustrationGeometryRequestA0};
use geometer_client::contracts::{ModelIllustrationGeometryResultA0, Validate};
use kicad_monkey_core::{
    BoardNetClassAssignments, BoardPlotLimits, BoardTextVariables, PcbFootprint, PcbLimits,
    PcbModelReference, PcbPad, PcbPadPrimitiveGeometry, PcbPoint, PcbPolygonPoint, PcbView,
    PlotDocumentMetadata, board_plot_artifact_with_sidecars,
};
use kicad_monkey_svg::{
    LayerPattern, LayerSelection, SvgBackground, SvgColor, SvgContextLimits, SvgFillMode,
    SvgFitOptions, SvgRenderContextA1, SvgRenderLimits, SvgStyleOverride, SvgViewport,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use super::PcbSvgJob;
use super::designator_layout::{
    DesignatorFitSession, normalize_designator_axis, pad_envelope, region_from_evenodd_layers,
    ring_polygon,
};
use super::geometer::{
    GEOMETER_C_ABI_GENERATION, GEOMETER_RELEASE, GEOMETER_SOURCE_REVISION, NativeGeometer,
};
use super::models::read_embedded_models;
use super::virtual_layers::{board_material_layers, footprint_hole_layers};
use crate::design::sha256_hex;

const BOARD_MODEL_Z_OFFSET_MM: f64 = 0.05;
const HLR_OUTLINE_WIDTH_MM: f64 = 0.025;
const HLR_DETAIL_WIDTH_MM: f64 = 0.01375;
pub const DEFAULT_TOON_SOLDERMASK_COLOR: &str = "auto";
const FALLBACK_TOON_SOLDERMASK_COLOR: &str = "#EEEEEE";
const LIGHT_TOON_SILKSCREEN_COLOR: &str = "#F5F5F5";
const DARK_TOON_SILKSCREEN_COLOR: &str = "#000000";
const MODEL_CACHE_SCHEMA: &str = "kicad_cruncher.toon_model_cache.a0";
const MAX_MODEL_CACHE_ENTRY_BYTES: u64 = 64 * 1024 * 1024;
const ILLUSTRATION_MARKER: &str = "<!--kicad-cruncher-toon-illustration-->";
const ASSEMBLY_DESIGNATORS_MARKER: &str = "<!--kicad-cruncher-toon-assembly-designators-->";

#[derive(Debug)]
pub struct ToonPreview {
    pub svg: String,
    pub viewport: SvgViewport,
    pub rendered_model_count: usize,
    pub geometer_request_count: usize,
    pub model_cache_hit_count: usize,
    pub persistent_model_cache_hit_count: usize,
    pub warnings: Vec<String>,
    pub soldermask_color: String,
    pub silkscreen_color: String,
    pub timings: ToonPreviewTimings,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ToonPreviewTimings {
    pub preparation_ms: f64,
    pub physical_render_ms: f64,
    pub model_illustration_ms: f64,
    pub composition_ms: f64,
    pub total_ms: f64,
}

struct RenderedModel {
    reference: String,
    sha256: String,
    footprint_index: usize,
    source_order: usize,
    painter_depth: f64,
    illustration: Arc<ModelIllustrationGeometry>,
}

struct PreparedModel {
    reference: String,
    sha256: String,
    footprint_index: usize,
    source_order: usize,
    cache_key: (String, String),
    illustration: Option<Arc<ModelIllustrationGeometry>>,
}

pub(crate) struct ToonIllustrationJob {
    pub request: ModelIllustrationGeometryRequestA0,
    pub step: Arc<[u8]>,
}

/// Command-scoped successful illustration reuse. Failures are never cached.
#[derive(Default)]
pub(crate) struct ToonModelCache {
    illustrations: BTreeMap<(String, String), Arc<ModelIllustrationGeometry>>,
    persistent_dir: Option<PathBuf>,
    geometer_request_count: usize,
    model_cache_hit_count: usize,
    persistent_model_cache_hit_count: usize,
}

#[derive(Deserialize, Serialize)]
struct CachedModelIllustration {
    schema: String,
    key: String,
    geometer_release: String,
    geometer_c_abi_generation: u32,
    geometer_source_revision: String,
    metadata: ModelIllustrationGeometryResultA0,
    geometry: MeshIllustrationGeometryA0,
}

impl ToonModelCache {
    pub(crate) fn with_persistent_dir(persistent_dir: Option<PathBuf>) -> Self {
        Self {
            persistent_dir,
            ..Self::default()
        }
    }

    fn get(
        &mut self,
        model_sha256: &str,
        request_identity: &str,
    ) -> Option<Arc<ModelIllustrationGeometry>> {
        let memory_key = (model_sha256.to_owned(), request_identity.to_owned());
        if let Some(rendered) = self.illustrations.get(&memory_key) {
            self.model_cache_hit_count += 1;
            return Some(Arc::clone(rendered));
        }
        let persistent_key = persistent_model_cache_key(model_sha256, request_identity);
        let path = self
            .persistent_dir
            .as_ref()
            .map(|root| root.join(format!("{persistent_key}.json")))?;
        let size = fs::metadata(&path).ok()?.len();
        if size == 0 || size > MAX_MODEL_CACHE_ENTRY_BYTES {
            return None;
        }
        let bytes = fs::read(path).ok()?;
        if bytes.is_empty() || bytes.len() as u64 > MAX_MODEL_CACHE_ENTRY_BYTES {
            return None;
        }
        let cached = serde_json::from_slice::<CachedModelIllustration>(&bytes).ok()?;
        if cached.schema != MODEL_CACHE_SCHEMA
            || cached.key != persistent_key
            || cached.geometer_release != GEOMETER_RELEASE
            || cached.geometer_c_abi_generation != GEOMETER_C_ABI_GENERATION
            || cached.geometer_source_revision != GEOMETER_SOURCE_REVISION
            || cached.metadata.validate_at("").is_err()
            || cached.geometry.validate_at("").is_err()
        {
            return None;
        }
        let rendered = Arc::new(ModelIllustrationGeometry {
            metadata: cached.metadata,
            geometry: cached.geometry,
        });
        self.illustrations.insert(memory_key, Arc::clone(&rendered));
        self.model_cache_hit_count += 1;
        self.persistent_model_cache_hit_count += 1;
        Some(rendered)
    }

    fn insert(
        &mut self,
        model_sha256: &str,
        request_identity: &str,
        rendered: Arc<ModelIllustrationGeometry>,
    ) {
        self.illustrations.insert(
            (model_sha256.to_owned(), request_identity.to_owned()),
            Arc::clone(&rendered),
        );
        let Some(root) = &self.persistent_dir else {
            return;
        };
        let persistent_key = persistent_model_cache_key(model_sha256, request_identity);
        let cached = CachedModelIllustration {
            schema: MODEL_CACHE_SCHEMA.to_owned(),
            key: persistent_key.clone(),
            geometer_release: GEOMETER_RELEASE.to_owned(),
            geometer_c_abi_generation: GEOMETER_C_ABI_GENERATION,
            geometer_source_revision: GEOMETER_SOURCE_REVISION.to_owned(),
            metadata: rendered.metadata.clone(),
            geometry: rendered.geometry.clone(),
        };
        let Ok(bytes) = serde_json::to_vec(&cached) else {
            return;
        };
        if bytes.len() as u64 > MAX_MODEL_CACHE_ENTRY_BYTES || fs::create_dir_all(root).is_err() {
            return;
        }
        let target = root.join(format!("{persistent_key}.json"));
        let temporary = root.join(format!(
            ".{persistent_key}.{}-{}.tmp",
            std::process::id(),
            self.illustrations.len()
        ));
        if fs::write(&temporary, bytes).is_err() {
            return;
        }
        if target.exists() {
            let _ = fs::remove_file(&target);
        }
        if fs::rename(&temporary, &target).is_err() {
            let _ = fs::remove_file(temporary);
        }
    }
}

fn persistent_model_cache_key(model_sha256: &str, request_identity: &str) -> String {
    sha256_hex(
        format!(
            "{MODEL_CACHE_SCHEMA}\0{GEOMETER_RELEASE}\0{GEOMETER_C_ABI_GENERATION}\0{GEOMETER_SOURCE_REVISION}\0{model_sha256}\0{request_identity}"
        )
        .as_bytes(),
    )
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToonPresentation {
    soldermask_color: Option<SvgColor>,
    silkscreen_color: Option<SvgColor>,
    style: ToonStyleSettings,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToonStyleSettings {
    pub substrate_color: String,
    pub substrate_opacity: f64,
    pub soldermask_opacity: f64,
    pub copper_color: String,
    pub board_outline_color: String,
    pub board_outline_width_mm: f64,
    pub cutout_outline_color: String,
    pub cutout_outline_width_mm: f64,
    pub cutout_outline_opacity: f64,
    pub cutout_hatch_color: String,
    pub cutout_hatch_spacing_mm: f64,
    pub cutout_hatch_angle_deg: f64,
    pub cutout_hatch_line_width_mm: f64,
    pub cutout_hatch_opacity: f64,
    pub plated_drill_color: String,
    pub non_plated_drill_color: String,
    pub drill_opacity: f64,
    pub drill_outline: bool,
    pub drill_outline_width_mm: f64,
    pub drill_respect_tenting: bool,
    pub plated_slot_color: String,
    pub non_plated_slot_color: String,
    pub slot_opacity: f64,
    pub slot_outline: bool,
    pub slot_outline_width_mm: f64,
    pub slot_respect_tenting: bool,
    pub illustration_opacity: f64,
    pub illustration_outline_width_mm: f64,
    pub illustration_detail_width_mm: f64,
    pub assembly_designator_color: String,
    pub assembly_designator_fill_ratio: f64,
    pub assembly_designator_max_font_size_mm: f64,
    pub assembly_designator_min_font_size_mm: f64,
    pub assembly_designator_font_family: String,
    pub assembly_designator_font_weight: String,
    pub assembly_designator_opacity: f64,
    pub assembly_designator_stroke_color: String,
    pub assembly_designator_stroke_width_mm: f64,
    pub assembly_hidden_designators: BTreeSet<String>,
}

impl Default for ToonStyleSettings {
    fn default() -> Self {
        Self {
            substrate_color: "#B6A26B".to_owned(),
            substrate_opacity: 1.0,
            soldermask_opacity: 0.75,
            copper_color: "#DFC951".to_owned(),
            board_outline_color: "#000000".to_owned(),
            board_outline_width_mm: 0.25,
            cutout_outline_color: "#555555".to_owned(),
            cutout_outline_width_mm: 0.24,
            cutout_outline_opacity: 0.25,
            cutout_hatch_color: "#777777".to_owned(),
            cutout_hatch_spacing_mm: 1.0,
            cutout_hatch_angle_deg: 45.0,
            cutout_hatch_line_width_mm: 0.12,
            cutout_hatch_opacity: 0.18,
            plated_drill_color: "#D3D3D3".to_owned(),
            non_plated_drill_color: "#D3D3D3".to_owned(),
            drill_opacity: 1.0,
            drill_outline: false,
            drill_outline_width_mm: 0.1,
            drill_respect_tenting: true,
            plated_slot_color: "#D3D3D3".to_owned(),
            non_plated_slot_color: "#D3D3D3".to_owned(),
            slot_opacity: 1.0,
            slot_outline: false,
            slot_outline_width_mm: 0.1,
            slot_respect_tenting: true,
            illustration_opacity: 1.0,
            illustration_outline_width_mm: HLR_OUTLINE_WIDTH_MM,
            illustration_detail_width_mm: HLR_DETAIL_WIDTH_MM,
            assembly_designator_color: "#FF0000".to_owned(),
            assembly_designator_fill_ratio: 0.8,
            assembly_designator_max_font_size_mm: 2.5,
            assembly_designator_min_font_size_mm: 0.35,
            assembly_designator_font_family: "Arial, sans-serif".to_owned(),
            assembly_designator_font_weight: "700".to_owned(),
            assembly_designator_opacity: 1.0,
            assembly_designator_stroke_color: "#FFFFFF".to_owned(),
            assembly_designator_stroke_width_mm: 0.0,
            assembly_hidden_designators: BTreeSet::new(),
        }
    }
}

impl ToonPresentation {
    pub fn new(soldermask_color: impl Into<String>) -> Result<Self, ToonPreviewError> {
        let soldermask_color = soldermask_color.into();
        Ok(Self {
            soldermask_color: if soldermask_color.eq_ignore_ascii_case("auto") {
                None
            } else {
                Some(SvgColor::parse(soldermask_color).map_err(error)?)
            },
            silkscreen_color: None,
            style: ToonStyleSettings::default(),
        })
    }

    pub fn new_with_silkscreen(
        soldermask_color: impl Into<String>,
        silkscreen_color: impl Into<String>,
    ) -> Result<Self, ToonPreviewError> {
        let mut presentation = Self::new(soldermask_color)?;
        presentation.silkscreen_color =
            Some(SvgColor::parse(silkscreen_color.into()).map_err(error)?);
        Ok(presentation)
    }

    pub fn new_configured(
        soldermask_color: impl Into<String>,
        silkscreen_color: impl Into<String>,
        style: ToonStyleSettings,
    ) -> Result<Self, ToonPreviewError> {
        validate_style(&style)?;
        let mut presentation = Self::new_with_silkscreen(soldermask_color, silkscreen_color)?;
        presentation.style = style;
        Ok(presentation)
    }

    pub fn requested_soldermask_color(&self) -> &str {
        self.soldermask_color
            .as_ref()
            .map_or("auto", SvgColor::as_str)
    }

    pub fn requested_silkscreen_color(&self) -> &str {
        self.silkscreen_color
            .as_ref()
            .map_or("contrast", SvgColor::as_str)
    }

    fn resolve_soldermask_color(
        &self,
        view: &PcbView<'_>,
        side: BoardSide,
    ) -> Result<SvgColor, ToonPreviewError> {
        if let Some(color) = &self.soldermask_color {
            return Ok(color.clone());
        }
        let saved = view
            .setup()
            .map_err(error)?
            .and_then(|setup| setup.stackup)
            .and_then(|stackup| {
                let layer_name = format!("{}.Mask", side.prefix());
                stackup
                    .layers
                    .into_iter()
                    .find(|layer| layer.name.eq_ignore_ascii_case(&layer_name))
                    .map(|layer| layer.color)
            });
        Ok(saved
            .as_deref()
            .and_then(kicad_soldermask_color)
            .unwrap_or_else(|| {
                SvgColor::parse(FALLBACK_TOON_SOLDERMASK_COLOR)
                    .expect("fallback Toon color is valid")
            }))
    }

    fn resolve_silkscreen_color(&self, soldermask_color: &SvgColor) -> SvgColor {
        self.silkscreen_color
            .clone()
            .unwrap_or_else(|| contrasting_silkscreen_color(soldermask_color))
    }
}

impl Default for ToonPresentation {
    fn default() -> Self {
        Self::new(DEFAULT_TOON_SOLDERMASK_COLOR).expect("default Toon color is valid")
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "keeping all user-facing style diagnostics in one validator makes the config contract auditable"
)]
fn validate_style(style: &ToonStyleSettings) -> Result<(), ToonPreviewError> {
    for (name, color) in [
        ("board_substrate.color", &style.substrate_color),
        ("board_outline.color", &style.board_outline_color),
        ("copper.color", &style.copper_color),
        ("board_cutouts.color", &style.cutout_outline_color),
        ("board_cutouts.hatch_color", &style.cutout_hatch_color),
        ("drills.plated_color", &style.plated_drill_color),
        ("drills.non_plated_color", &style.non_plated_drill_color),
        ("slots.plated_color", &style.plated_slot_color),
        ("slots.non_plated_color", &style.non_plated_slot_color),
        (
            "assembly_designators.color",
            &style.assembly_designator_color,
        ),
        (
            "assembly_designators.stroke_color",
            &style.assembly_designator_stroke_color,
        ),
    ] {
        SvgColor::parse(color)
            .map_err(|error| ToonPreviewError::new(format!("{name}: {error}")))?;
    }
    for (name, value) in [
        ("board_substrate.opacity", style.substrate_opacity),
        ("soldermask_film.opacity", style.soldermask_opacity),
        ("drills.opacity", style.drill_opacity),
        ("slots.opacity", style.slot_opacity),
        ("illustration.opacity", style.illustration_opacity),
        (
            "assembly_designators.opacity",
            style.assembly_designator_opacity,
        ),
        (
            "assembly_designators.fill_ratio",
            style.assembly_designator_fill_ratio,
        ),
        (
            "board_cutouts.outline_opacity",
            style.cutout_outline_opacity,
        ),
        ("board_cutouts.hatch_opacity", style.cutout_hatch_opacity),
    ] {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(ToonPreviewError::new(format!(
                "{name} must be between 0 and 1"
            )));
        }
    }
    for (name, value) in [
        ("board_outline.line_width_mm", style.board_outline_width_mm),
        (
            "illustration.outline_width_mm",
            style.illustration_outline_width_mm,
        ),
        (
            "illustration.detail_width_mm",
            style.illustration_detail_width_mm,
        ),
        ("drills.outline_width_mm", style.drill_outline_width_mm),
        ("slots.outline_width_mm", style.slot_outline_width_mm),
        (
            "board_cutouts.outline_width_mm",
            style.cutout_outline_width_mm,
        ),
        (
            "board_cutouts.hatch_spacing_mm",
            style.cutout_hatch_spacing_mm,
        ),
        (
            "board_cutouts.hatch_line_width_mm",
            style.cutout_hatch_line_width_mm,
        ),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(ToonPreviewError::new(format!(
                "{name} must be greater than zero"
            )));
        }
    }
    for (name, value) in [
        (
            "assembly_designators.min_font_size_mm",
            style.assembly_designator_min_font_size_mm,
        ),
        (
            "assembly_designators.max_font_size_mm",
            style.assembly_designator_max_font_size_mm,
        ),
    ] {
        if !value.is_finite() || value <= 0.0 {
            return Err(ToonPreviewError::new(format!(
                "{name} must be greater than zero"
            )));
        }
    }
    if style.assembly_designator_min_font_size_mm > style.assembly_designator_max_font_size_mm {
        return Err(ToonPreviewError::new(
            "assembly_designators.min_font_size_mm must not exceed max_font_size_mm",
        ));
    }
    if !style.assembly_designator_stroke_width_mm.is_finite()
        || style.assembly_designator_stroke_width_mm < 0.0
    {
        return Err(ToonPreviewError::new(
            "assembly_designators.stroke_width_mm must be non-negative",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BoardSide {
    Top,
    Bottom,
}

impl BoardSide {
    const fn prefix(self) -> &'static str {
        match self {
            Self::Top => "F",
            Self::Bottom => "B",
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Bottom => "bottom",
        }
    }

    fn owns_footprint(self, footprint: &PcbFootprint) -> bool {
        let bottom = footprint
            .layer
            .as_deref()
            .is_some_and(|layer| layer.starts_with("B."));
        matches!((self, bottom), (Self::Top, false) | (Self::Bottom, true))
    }
}

#[derive(Debug)]
pub struct ToonPreviewError(String);

impl ToonPreviewError {
    pub(super) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    pub(super) fn from_display(value: impl fmt::Display) -> Self {
        Self(value.to_string())
    }
}

impl fmt::Display for ToonPreviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ToonPreviewError {}

pub(crate) enum ToonIllustrationFailure {
    Startup(String),
}

pub(crate) trait ToonModelIllustrator {
    fn illustrate_models(
        &self,
        jobs: Vec<ToonIllustrationJob>,
    ) -> Result<Vec<Result<ModelIllustrationGeometry, String>>, ToonIllustrationFailure>;
}

impl ToonModelIllustrator for NativeGeometer {
    fn illustrate_models(
        &self,
        jobs: Vec<ToonIllustrationJob>,
    ) -> Result<Vec<Result<ModelIllustrationGeometry, String>>, ToonIllustrationFailure> {
        Ok(jobs
            .into_iter()
            .map(|job| {
                NativeGeometer::illustrate_model(self, job.request, job.step.to_vec())
                    .map_err(|error| error.to_string())
            })
            .collect())
    }
}

/// Render a top-side physical board and all supported embedded STEP models.
pub fn render_top_toon_preview(
    source: &str,
    document_id: impl Into<String>,
    geometer: &NativeGeometer,
) -> Result<ToonPreview, ToonPreviewError> {
    render_toon_preview(source, document_id, BoardSide::Top, geometer)
}

/// Render one board side with physical material layers and embedded STEP models.
pub fn render_toon_preview(
    source: &str,
    document_id: impl Into<String>,
    side: BoardSide,
    geometer: &NativeGeometer,
) -> Result<ToonPreview, ToonPreviewError> {
    render_toon_preview_with_presentation(
        source,
        document_id,
        side,
        geometer,
        &ToonPresentation::default(),
    )
}

/// Render one board side with an explicit Toon presentation.
pub fn render_toon_preview_with_presentation(
    source: &str,
    document_id: impl Into<String>,
    side: BoardSide,
    geometer: &NativeGeometer,
    presentation: &ToonPresentation,
) -> Result<ToonPreview, ToonPreviewError> {
    render_toon_preview_with_cache(
        source,
        document_id,
        side,
        geometer,
        presentation,
        &mut ToonModelCache::default(),
    )
}

pub(crate) fn render_toon_preview_with_cache<I: ToonModelIllustrator + ?Sized>(
    source: &str,
    document_id: impl Into<String>,
    side: BoardSide,
    geometer: &I,
    presentation: &ToonPresentation,
    model_cache: &mut ToonModelCache,
) -> Result<ToonPreview, ToonPreviewError> {
    render_toon_preview_selection(
        source,
        document_id,
        side,
        geometer,
        presentation,
        model_cache,
        None,
        None,
        None,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "the internal board entry point carries explicit render, cache, layer, and population contexts"
)]
pub(crate) fn render_toon_preview_with_cache_and_layers<I: ToonModelIllustrator + ?Sized>(
    source: &str,
    document_id: impl Into<String>,
    side: BoardSide,
    geometer: &I,
    presentation: &ToonPresentation,
    model_cache: &mut ToonModelCache,
    layers: &[String],
    excluded_components: Option<&BTreeSet<String>>,
) -> Result<ToonPreview, ToonPreviewError> {
    render_toon_preview_selection(
        source,
        document_id,
        side,
        geometer,
        presentation,
        model_cache,
        None,
        Some(layers),
        excluded_components,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "the internal footprint entry point mirrors the board renderer while adding an explicit reference selection"
)]
pub(crate) fn render_footprint_toon_preview_with_cache_and_layers<
    I: ToonModelIllustrator + ?Sized,
>(
    source: &str,
    document_id: impl Into<String>,
    side: BoardSide,
    reference: &str,
    geometer: &I,
    presentation: &ToonPresentation,
    model_cache: &mut ToonModelCache,
    layers: &[String],
    excluded_components: Option<&BTreeSet<String>>,
) -> Result<ToonPreview, ToonPreviewError> {
    render_toon_preview_selection(
        source,
        document_id,
        side,
        geometer,
        presentation,
        model_cache,
        Some(reference),
        Some(layers),
        excluded_components,
    )
}

#[allow(
    clippy::cognitive_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the render transaction keeps parse, cache, illustration, and timing state in one deterministic flow"
)]
fn render_toon_preview_selection<I: ToonModelIllustrator + ?Sized>(
    source: &str,
    document_id: impl Into<String>,
    side: BoardSide,
    geometer: &I,
    presentation: &ToonPresentation,
    model_cache: &mut ToonModelCache,
    footprint_reference: Option<&str>,
    layer_order: Option<&[String]>,
    excluded_components: Option<&BTreeSet<String>>,
) -> Result<ToonPreview, ToonPreviewError> {
    let total_started = Instant::now();
    let starting_geometer_requests = model_cache.geometer_request_count;
    let starting_cache_hits = model_cache.model_cache_hit_count;
    let starting_persistent_cache_hits = model_cache.persistent_model_cache_hit_count;
    let view = PcbView::parse(source, PcbLimits::default()).map_err(error)?;
    let metadata = view.metadata().map_err(error)?;
    let footprints = view
        .footprints()
        .collect::<Result<Vec<_>, _>>()
        .map_err(error)?;
    let pads = view.pads().collect::<Result<Vec<_>, _>>().map_err(error)?;
    let selected_footprint = footprint_reference
        .map(|reference| selected_footprint_index(&footprints, reference))
        .transpose()?;
    let models = read_embedded_models(&view).map_err(error)?;
    let soldermask_color = presentation.resolve_soldermask_color(&view, side)?;
    let silkscreen_color = presentation.resolve_silkscreen_color(&soldermask_color);
    let mut warnings = models
        .warnings
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    let preparation_ms = milliseconds(total_started.elapsed());

    let physical_started = Instant::now();
    let (physical_svg, viewport) = render_physical_board(
        source,
        document_id.into(),
        &view,
        &footprints,
        side,
        &soldermask_color,
        &silkscreen_color,
        &presentation.style,
        footprint_reference.zip(selected_footprint),
        layer_order,
    )?;
    let physical_render_ms = milliseconds(physical_started.elapsed());
    let illustration_enabled = physical_svg.contains(ILLUSTRATION_MARKER);
    let mut overlay = format!(
        "<g id=\"illustration-{}\" data-layer-token=\"ILLUSTRATION_{}\" data-hlr-outline-width-mm=\"{}\" data-hlr-detail-width-mm=\"{}\" fill-rule=\"evenodd\" stroke-linecap=\"round\" stroke-linejoin=\"round\">",
        side.name(),
        side.name().to_ascii_uppercase(),
        number(presentation.style.illustration_outline_width_mm),
        number(presentation.style.illustration_detail_width_mm),
    );
    let model_started = Instant::now();
    let mut prepared_models = Vec::new();
    let mut pending = BTreeMap::<(String, String), ToonIllustrationJob>::new();
    for (source_order, model) in models.instances.into_iter().enumerate() {
        if !illustration_enabled {
            break;
        }
        let Some(footprint) = footprints.get(model.attachment.footprint_index) else {
            warnings.push(format!(
                "{}: model attachment has no owning footprint",
                model.reference
            ));
            continue;
        };
        if selected_footprint.is_some_and(|index| index != model.attachment.footprint_index) {
            continue;
        }
        if excluded_components.is_some_and(|references| references.contains(&model.reference)) {
            continue;
        }
        if !side.owns_footprint(footprint) {
            continue;
        }
        let request = illustration_request(
            &model.attachment,
            metadata.thickness,
            side,
            presentation.style.illustration_outline_width_mm,
            presentation.style.illustration_detail_width_mm,
        )
        .map_err(error)?;
        let request_identity = illustration_request_identity(&request)?;
        let cache_key = (model.sha256.clone(), request_identity.clone());
        let illustration = if let Some(rendered) = model_cache.get(&model.sha256, &request_identity)
        {
            Some(rendered)
        } else {
            if pending.contains_key(&cache_key) {
                model_cache.model_cache_hit_count += 1;
            } else {
                pending.insert(
                    cache_key.clone(),
                    ToonIllustrationJob {
                        request,
                        step: Arc::clone(&model.step),
                    },
                );
            }
            None
        };
        prepared_models.push(PreparedModel {
            reference: model.reference,
            sha256: model.sha256,
            footprint_index: model.attachment.footprint_index,
            source_order,
            cache_key,
            illustration,
        });
    }
    model_cache.geometer_request_count += pending.len();
    let pending_keys = pending.keys().cloned().collect::<Vec<_>>();
    let jobs = pending.into_values().collect::<Vec<_>>();
    let outcomes = geometer
        .illustrate_models(jobs)
        .map_err(|failure| match failure {
            ToonIllustrationFailure::Startup(failure) => {
                ToonPreviewError::new(format!("could not start Geometer: {failure}"))
            }
        })?;
    if outcomes.len() != pending_keys.len() {
        return Err(ToonPreviewError::new(
            "Geometer worker returned a different number of results than requests",
        ));
    }
    let mut completed = BTreeMap::new();
    let mut failures = BTreeMap::new();
    for (cache_key, outcome) in pending_keys.into_iter().zip(outcomes) {
        match outcome {
            Ok(rendered) => {
                let rendered = Arc::new(rendered);
                model_cache.insert(&cache_key.0, &cache_key.1, Arc::clone(&rendered));
                completed.insert(cache_key, rendered);
            }
            Err(failure) => {
                failures.insert(cache_key, failure);
            }
        }
    }
    let mut rendered_models = Vec::new();
    for prepared in prepared_models {
        let rendered = prepared
            .illustration
            .or_else(|| completed.get(&prepared.cache_key).cloned());
        let Some(rendered) = rendered else {
            if let Some(failure) = failures.get(&prepared.cache_key) {
                warnings.push(format!(
                    "{}: Geometer illustration omitted: {failure}",
                    prepared.reference
                ));
            }
            continue;
        };
        warnings.extend(
            rendered
                .geometry
                .warnings
                .iter()
                .map(|warning| format!("{}: {warning}", prepared.reference)),
        );
        rendered_models.push(RenderedModel {
            reference: prepared.reference,
            sha256: prepared.sha256,
            footprint_index: prepared.footprint_index,
            source_order: prepared.source_order,
            painter_depth: model_painter_depth(side, &rendered.metadata.bounds_mm),
            illustration: rendered,
        });
    }
    rendered_models.sort_by(|left, right| {
        left.painter_depth
            .total_cmp(&right.painter_depth)
            .then(left.source_order.cmp(&right.source_order))
    });
    let assembly = if physical_svg.contains(ASSEMBLY_DESIGNATORS_MARKER) {
        render_assembly_designators(
            &rendered_models,
            &footprints,
            &pads,
            viewport,
            side,
            &presentation.style,
            excluded_components,
        )?
    } else {
        String::new()
    };
    let model_illustration_ms = milliseconds(model_started.elapsed());
    let composition_started = Instant::now();
    let rendered_model_count = rendered_models.len();
    for model in &rendered_models {
        let footprint = &footprints[model.footprint_index];
        write!(
            overlay,
            "<g data-component=\"{}\" data-model-sha256=\"{}\" opacity=\"{}\">",
            escape_xml(&model.reference),
            model.sha256,
            number(presentation.style.illustration_opacity),
        )
        .expect("writing to String cannot fail");
        append_geometry(
            &mut overlay,
            &model.illustration.geometry,
            footprint,
            viewport,
            side,
        );
        overlay.push_str("</g>");
    }
    overlay.push_str("</g>");

    let svg = if let Some(insert_at) = physical_svg.find(ILLUSTRATION_MARKER) {
        let mut svg =
            String::with_capacity(physical_svg.len() + overlay.len() - ILLUSTRATION_MARKER.len());
        svg.push_str(&physical_svg[..insert_at]);
        svg.push_str(&overlay);
        svg.push_str(&physical_svg[insert_at + ILLUSTRATION_MARKER.len()..]);
        svg
    } else {
        physical_svg
    };
    let svg = if let Some(insert_at) = svg.find(ASSEMBLY_DESIGNATORS_MARKER) {
        let mut composed =
            String::with_capacity(svg.len() + assembly.len() - ASSEMBLY_DESIGNATORS_MARKER.len());
        composed.push_str(&svg[..insert_at]);
        composed.push_str(&assembly);
        composed.push_str(&svg[insert_at + ASSEMBLY_DESIGNATORS_MARKER.len()..]);
        composed
    } else {
        svg
    };
    let svg = if side == BoardSide::Bottom {
        mirror_drawing(&svg, viewport)?
    } else {
        svg
    };
    let composition_ms = milliseconds(composition_started.elapsed());
    Ok(ToonPreview {
        svg,
        viewport,
        rendered_model_count,
        geometer_request_count: model_cache.geometer_request_count - starting_geometer_requests,
        model_cache_hit_count: model_cache.model_cache_hit_count - starting_cache_hits,
        persistent_model_cache_hit_count: model_cache.persistent_model_cache_hit_count
            - starting_persistent_cache_hits,
        warnings,
        soldermask_color: soldermask_color.as_str().to_owned(),
        silkscreen_color: silkscreen_color.as_str().to_owned(),
        timings: ToonPreviewTimings {
            preparation_ms,
            physical_render_ms,
            model_illustration_ms,
            composition_ms,
            total_ms: milliseconds(total_started.elapsed()),
        },
    })
}

fn milliseconds(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

#[allow(
    clippy::too_many_lines,
    reason = "the assembly layer keeps Autodoc fit selection and exact SVG text transforms together"
)]
fn render_assembly_designators(
    models: &[RenderedModel],
    footprints: &[PcbFootprint],
    pads: &[PcbPad],
    viewport: SvgViewport,
    side: BoardSide,
    style: &ToonStyleSettings,
    excluded_components: Option<&BTreeSet<String>>,
) -> Result<String, ToonPreviewError> {
    let mut regions = BTreeMap::<usize, Vec<MultiPolygon<f64>>>::new();
    for model in models {
        let footprint = footprints
            .get(model.footprint_index)
            .ok_or_else(|| ToonPreviewError::new("illustrated model has no owning footprint"))?;
        if let Some(region) = model_annotation_region(
            &model.illustration.geometry,
            footprint,
            viewport,
            style.illustration_outline_width_mm,
        ) {
            regions
                .entry(model.footprint_index)
                .or_default()
                .push(region);
        }
    }
    let width_mm = viewport.width_nm as f64 / 1_000_000.0;
    let mut session = DesignatorFitSession::default();
    let mut svg = format!(
        "<g id=\"assembly-designators-{}\" data-layer-token=\"ASSEMBLY_DESIGNATORS_{}\">",
        side.name(),
        side.name().to_ascii_uppercase(),
    );
    for (index, footprint) in footprints.iter().enumerate() {
        if !side.owns_footprint(footprint) {
            continue;
        }
        let Some(reference) = footprint
            .reference
            .as_deref()
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if style.assembly_hidden_designators.contains(reference) {
            continue;
        }
        if excluded_components.is_some_and(|references| references.contains(reference)) {
            continue;
        }
        let (region, source) = if let Some(model_regions) = regions.get(&index) {
            (unary_union(model_regions), "model")
        } else if footprint.model_count > 0 {
            continue;
        } else {
            let Some(region) = footprint_pad_region(index, footprint, pads, viewport, side) else {
                continue;
            };
            (region, "pads")
        };
        let Some(fit) = session.fit(
            reference,
            &region,
            style.assembly_designator_fill_ratio,
            style.assembly_designator_max_font_size_mm,
            -footprint.angle.unwrap_or(0.0),
            None,
        ) else {
            continue;
        };
        if fit.font_size_mm + 1.0e-9 < style.assembly_designator_min_font_size_mm {
            continue;
        }
        let (center, rotation, counter_mirror) = match side {
            BoardSide::Top => (fit.center_mm, fit.rotation_degrees, None),
            BoardSide::Bottom => (
                [width_mm - fit.center_mm[0], fit.center_mm[1]],
                normalize_designator_axis(180.0 - fit.rotation_degrees),
                Some(format!("translate({} 0) scale(-1 1)", number(width_mm))),
            ),
        };
        write!(
            svg,
            "<g data-component=\"{}\" data-geometry-source=\"{}\"{}><text x=\"{}\" y=\"{}\" transform=\"rotate({} {} {})\" text-anchor=\"middle\" dominant-baseline=\"central\" font-size=\"{}\" font-family=\"{}\" font-weight=\"{}\" fill=\"{}\" opacity=\"{}\"",
            escape_xml(reference),
            source,
            counter_mirror.as_ref().map_or_else(String::new, |value| format!(" transform=\"{value}\"")),
            number(center[0]),
            number(center[1]),
            number(rotation),
            number(center[0]),
            number(center[1]),
            number(fit.font_size_mm),
            escape_xml(&style.assembly_designator_font_family),
            escape_xml(&style.assembly_designator_font_weight),
            style.assembly_designator_color,
            number(style.assembly_designator_opacity),
        )
        .expect("writing to String cannot fail");
        if style.assembly_designator_stroke_width_mm > 0.0 {
            write!(
                svg,
                " stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"round\" paint-order=\"stroke fill\"",
                style.assembly_designator_stroke_color,
                number(style.assembly_designator_stroke_width_mm),
            )
            .expect("writing to String cannot fail");
        } else {
            svg.push_str(" stroke=\"none\"");
        }
        write!(svg, ">{}</text></g>", escape_xml(reference))
            .expect("writing to String cannot fail");
    }
    svg.push_str("</g>");
    Ok(svg)
}

fn model_annotation_region(
    geometry: &MeshIllustrationGeometryA0,
    footprint: &PcbFootprint,
    viewport: SvgViewport,
    configured_outline_width_mm: f64,
) -> Option<MultiPolygon<f64>> {
    let outline_width = geometry
        .lines
        .iter()
        .map(|line| line.width)
        .min_by(|left, right| {
            (left - configured_outline_width_mm)
                .abs()
                .total_cmp(&(right - configured_outline_width_mm).abs())
        });
    if let Some(outline_width) = outline_width {
        let segments = geometry
            .lines
            .iter()
            .filter(|line| (line.width - outline_width).abs() <= 1.0e-9)
            .map(|line| {
                (
                    canvas_point(line.start, footprint, viewport),
                    canvas_point(line.end, footprint, viewport),
                )
            })
            .collect::<Vec<_>>();
        let rings = closed_segment_rings(&segments);
        if !rings.is_empty()
            && let Some(region) = region_from_evenodd_layers([rings])
        {
            return Some(region);
        }
    }
    let points = geometry
        .surfaces
        .iter()
        .flat_map(|surface| &surface.layers)
        .flat_map(|layer| &layer.rings)
        .flat_map(|ring| &ring.points)
        .map(|point| {
            let point = canvas_point(*point, footprint, viewport);
            GeoPoint::new(point[0], point[1])
        })
        .collect::<Vec<_>>();
    (points.len() >= 3).then(|| MultiPolygon::new(vec![MultiPoint::new(points).convex_hull()]))
}

fn closed_segment_rings(segments: &[([f64; 2], [f64; 2])]) -> Vec<Vec<[f64; 2]>> {
    let mut adjacency = BTreeMap::<(i64, i64), Vec<usize>>::new();
    for (index, (start, end)) in segments.iter().enumerate() {
        adjacency
            .entry(annotation_point_key(*start))
            .or_default()
            .push(index);
        adjacency
            .entry(annotation_point_key(*end))
            .or_default()
            .push(index);
    }
    let mut used = vec![false; segments.len()];
    let mut rings = Vec::new();
    for first in 0..segments.len() {
        if used[first] {
            continue;
        }
        used[first] = true;
        let (start, end) = segments[first];
        let start_key = annotation_point_key(start);
        let mut current = end;
        let mut ring = vec![start, end];
        while annotation_point_key(current) != start_key {
            let Some(next) = adjacency
                .get(&annotation_point_key(current))
                .and_then(|edges| edges.iter().copied().find(|edge| !used[*edge]))
            else {
                break;
            };
            used[next] = true;
            let (left, right) = segments[next];
            current = if annotation_point_key(left) == annotation_point_key(current) {
                right
            } else {
                left
            };
            ring.push(current);
            if ring.len() > segments.len() + 1 {
                break;
            }
        }
        if annotation_point_key(current) == start_key && ring.len() >= 4 {
            ring.pop();
            rings.push(ring);
        }
    }
    rings
}

fn annotation_point_key(point: [f64; 2]) -> (i64, i64) {
    (
        (point[0] * 1_000_000.0).round() as i64,
        (point[1] * 1_000_000.0).round() as i64,
    )
}

fn footprint_pad_region(
    footprint_index: usize,
    footprint: &PcbFootprint,
    pads: &[PcbPad],
    viewport: SvgViewport,
    side: BoardSide,
) -> Option<MultiPolygon<f64>> {
    let candidates = pads
        .iter()
        .filter(|pad| pad.footprint_index == footprint_index)
        .filter(|pad| pad_is_visible_on_side(pad, side))
        .collect::<Vec<_>>();
    let electrical = candidates
        .iter()
        .copied()
        .filter(|pad| pad.kind != "np_thru_hole")
        .collect::<Vec<_>>();
    let selected = if electrical.is_empty() {
        candidates
    } else {
        electrical
    };
    let polygons = selected
        .into_iter()
        .flat_map(|pad| pad_polygons(pad, footprint, viewport))
        .collect::<Vec<_>>();
    pad_envelope(&polygons)
}

fn pad_is_visible_on_side(pad: &PcbPad, side: BoardSide) -> bool {
    let copper = format!("{}.Cu", side.prefix());
    pad.layers
        .iter()
        .any(|layer| layer == &copper || layer == "*.Cu")
}

fn pad_polygons(
    pad: &PcbPad,
    footprint: &PcbFootprint,
    viewport: SvgViewport,
) -> Vec<Polygon<f64>> {
    let mut rings = Vec::new();
    let shape = match pad.shape.as_str() {
        "circle" => ellipse_ring(pad.size_x / 2.0, pad.size_y / 2.0, 32),
        "oval" => oval_ring(pad.size_x, pad.size_y, 32),
        "roundrect" => roundrect_ring(
            pad.size_x,
            pad.size_y,
            pad.roundrect_rratio.unwrap_or(0.25),
            6,
        ),
        "trapezoid" => trapezoid_ring(
            pad.size_x,
            pad.size_y,
            pad.rect_delta_x.unwrap_or(0.0),
            pad.rect_delta_y.unwrap_or(0.0),
        ),
        _ => rectangle_ring(pad.size_x, pad.size_y),
    };
    rings.push(shape);
    if pad.shape == "custom" {
        for primitive in &pad.custom_primitives {
            match primitive.geometry.as_ref() {
                Some(PcbPadPrimitiveGeometry::Polygon { points }) => {
                    let points = points
                        .iter()
                        .filter_map(|point| match point {
                            PcbPolygonPoint::Xy(point) => Some([point.x, point.y]),
                            PcbPolygonPoint::Arc { .. } => None,
                        })
                        .collect::<Vec<_>>();
                    if points.len() >= 3 {
                        rings.push(points);
                    }
                }
                Some(PcbPadPrimitiveGeometry::Rect { start, end, .. }) => rings.push(vec![
                    [start.x, start.y],
                    [end.x, start.y],
                    [end.x, end.y],
                    [start.x, end.y],
                ]),
                Some(PcbPadPrimitiveGeometry::Circle { center, end }) => {
                    let radius = ((end.x - center.x).powi(2) + (end.y - center.y).powi(2)).sqrt();
                    rings.push(
                        ellipse_ring(radius, radius, 32)
                            .into_iter()
                            .map(|point| [point[0] + center.x, point[1] + center.y])
                            .collect(),
                    );
                }
                _ => {}
            }
        }
    }
    rings
        .into_iter()
        .filter_map(|ring| {
            let transformed = ring
                .into_iter()
                .map(|point| pad_canvas_point(point, pad, footprint, viewport))
                .collect::<Vec<_>>();
            ring_polygon(&transformed)
        })
        .collect()
}

fn pad_canvas_point(
    point: [f64; 2],
    pad: &PcbPad,
    footprint: &PcbFootprint,
    viewport: SvgViewport,
) -> [f64; 2] {
    let pad_angle = -pad.angle.to_radians();
    let (pad_sin, pad_cos) = pad_angle.sin_cos();
    let local = PcbPoint {
        x: pad.at_x + point[0] * pad_cos - point[1] * pad_sin,
        y: pad.at_y + point[0] * pad_sin + point[1] * pad_cos,
    };
    let footprint_angle = -footprint.angle.unwrap_or(0.0).to_radians();
    let (footprint_sin, footprint_cos) = footprint_angle.sin_cos();
    [
        footprint.at_x.unwrap_or(0.0) + local.x * footprint_cos
            - local.y * footprint_sin
            - viewport.min_x_nm as f64 / 1_000_000.0,
        footprint.at_y.unwrap_or(0.0) + local.x * footprint_sin + local.y * footprint_cos
            - viewport.min_y_nm as f64 / 1_000_000.0,
    ]
}

fn rectangle_ring(width: f64, height: f64) -> Vec<[f64; 2]> {
    let (x, y) = (width / 2.0, height / 2.0);
    vec![[-x, -y], [x, -y], [x, y], [-x, y]]
}

fn ellipse_ring(radius_x: f64, radius_y: f64, segments: usize) -> Vec<[f64; 2]> {
    (0..segments)
        .map(|index| {
            let angle = std::f64::consts::TAU * index as f64 / segments as f64;
            [radius_x * angle.cos(), radius_y * angle.sin()]
        })
        .collect()
}

fn oval_ring(width: f64, height: f64, segments: usize) -> Vec<[f64; 2]> {
    if (width - height).abs() <= 1.0e-12 {
        return ellipse_ring(width / 2.0, height / 2.0, segments);
    }
    let horizontal = width > height;
    let radius = width.min(height) / 2.0;
    let offset = (width.max(height) - 2.0 * radius) / 2.0;
    (0..segments)
        .map(|index| {
            let angle = std::f64::consts::TAU * index as f64 / segments as f64;
            if horizontal {
                [
                    radius * angle.cos() + if angle.cos() >= 0.0 { offset } else { -offset },
                    radius * angle.sin(),
                ]
            } else {
                [
                    radius * angle.cos(),
                    radius * angle.sin() + if angle.sin() >= 0.0 { offset } else { -offset },
                ]
            }
        })
        .collect()
}

fn roundrect_ring(width: f64, height: f64, ratio: f64, segments: usize) -> Vec<[f64; 2]> {
    let radius = (width.min(height) * ratio.clamp(0.0, 0.5)).min(width.min(height) / 2.0);
    if radius <= 1.0e-12 {
        return rectangle_ring(width, height);
    }
    let centers = [
        [width / 2.0 - radius, height / 2.0 - radius],
        [-width / 2.0 + radius, height / 2.0 - radius],
        [-width / 2.0 + radius, -height / 2.0 + radius],
        [width / 2.0 - radius, -height / 2.0 + radius],
    ];
    let starts = [0.0, 90.0, 180.0, 270.0];
    let mut result = Vec::new();
    for (center, start) in centers.into_iter().zip(starts) {
        for index in 0..=segments {
            let angle = (start + 90.0 * index as f64 / segments as f64).to_radians();
            result.push([
                center[0] + radius * angle.cos(),
                center[1] + radius * angle.sin(),
            ]);
        }
    }
    result
}

fn trapezoid_ring(width: f64, height: f64, delta_x: f64, delta_y: f64) -> Vec<[f64; 2]> {
    vec![
        [-width / 2.0 - delta_x / 2.0, -height / 2.0 - delta_y / 2.0],
        [width / 2.0 + delta_x / 2.0, -height / 2.0 + delta_y / 2.0],
        [width / 2.0 - delta_x / 2.0, height / 2.0 - delta_y / 2.0],
        [-width / 2.0 + delta_x / 2.0, height / 2.0 + delta_y / 2.0],
    ]
}

fn illustration_request_identity(
    request: &ModelIllustrationGeometryRequestA0,
) -> Result<String, ToonPreviewError> {
    serde_json::to_vec(request)
        .map(|bytes| sha256_hex(&bytes))
        .map_err(error)
}

/// Return a far-to-near painter key matching Altium Cruncher's top/bottom rule.
/// Geometer bounds include the KiCad model transform and board-side placement.
fn model_painter_depth(side: BoardSide, bounds_mm: &[f64]) -> f64 {
    debug_assert_eq!(bounds_mm.len(), 6);
    match side {
        BoardSide::Top => bounds_mm[5],
        BoardSide::Bottom => -bounds_mm[2],
    }
}

fn selected_footprint_index(
    footprints: &[PcbFootprint],
    reference: &str,
) -> Result<usize, ToonPreviewError> {
    let matches = footprints
        .iter()
        .enumerate()
        .filter(|(_, footprint)| footprint.reference.as_deref() == Some(reference))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [index] => Ok(*index),
        [] => Err(ToonPreviewError::new(format!(
            "footprint reference {reference:?} was not found"
        ))),
        _ => Err(ToonPreviewError::new(format!(
            "footprint reference {reference:?} is not unique"
        ))),
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "physical composition receives the already-resolved view inputs and emits one ordered SVG"
)]
fn render_physical_board(
    source: &str,
    document_id: String,
    view: &PcbView<'_>,
    footprints: &[PcbFootprint],
    side: BoardSide,
    soldermask_color: &SvgColor,
    silkscreen_color: &SvgColor,
    style: &ToonStyleSettings,
    footprint_selection: Option<(&str, usize)>,
    layer_order: Option<&[String]>,
) -> Result<(String, SvgViewport), ToonPreviewError> {
    let plot = board_plot_artifact_with_sidecars(
        source,
        BoardPlotLimits::default(),
        PcbLimits::default(),
        &BoardNetClassAssignments::default(),
        &BoardTextVariables::default(),
    )
    .map_err(error)?;
    let mut job = PcbSvgJob::new(
        plot,
        PlotDocumentMetadata {
            document_id,
            source_path: None,
        },
        SvgRenderLimits::default(),
    )
    .map_err(error)?;
    let fit = SvgFitOptions {
        padding_nm: if footprint_selection.is_some() {
            3_000_000
        } else {
            1_000_000
        },
        min_extent_nm: 5_000_000,
        fallback: None,
    };
    let fitted = match footprint_selection {
        Some((reference, _)) => job.render_footprint_fit(reference, fit, &base_context(side)?),
        None => job.render_physical_fit(fit, &base_context(side)?),
    }
    .map_err(error)?;
    let base = match footprint_selection {
        Some((reference, _)) => job.render_footprint(
            reference,
            fitted.viewport,
            &copper_context(side, &style.copper_color)?,
        ),
        None => job.render_physical(fitted.viewport, &copper_context(side, &style.copper_color)?),
    }
    .map_err(error)?;
    let prefix = side.prefix();
    let mask = match footprint_selection {
        Some((reference, _)) => job.render_footprint(
            reference,
            base.viewport,
            &single_layer_context(&format!("{prefix}.Mask"), "#000000")?,
        ),
        None => job.render_physical(
            base.viewport,
            &single_layer_context(&format!("{prefix}.Mask"), "#000000")?,
        ),
    }
    .map_err(error)?;
    let silk = match footprint_selection {
        Some((reference, _)) => job.render_footprint(
            reference,
            base.viewport,
            &single_layer_context(&format!("{prefix}.SilkS"), silkscreen_color.as_str())?,
        ),
        None => job.render_physical(
            base.viewport,
            &single_layer_context(&format!("{prefix}.SilkS"), silkscreen_color.as_str())?,
        ),
    }
    .map_err(error)?;
    let mask_group = root_drawing_group(&mask.svg)?;
    let silk_group = root_drawing_group(&silk.svg)?;
    if let Some((reference, footprint_index)) = footprint_selection {
        let (svg, viewport) = compose_footprint_physical(
            view,
            footprints,
            base,
            prefix,
            reference,
            footprint_index,
            silk_group,
            style,
        )?;
        return Ok((
            insert_footprint_overlay_markers(svg, side, layer_order)?,
            viewport,
        ));
    }
    let materials = board_material_layers(
        view,
        footprints,
        base.viewport,
        prefix,
        mask_group,
        soldermask_color.as_str(),
        style,
    )?;
    let Some(group_at) = base.svg.find("<g transform=") else {
        return Err(ToonPreviewError::new(
            "physical renderer returned an SVG without a drawing group",
        ));
    };
    let Some(close_at) = base.svg.rfind("</svg>") else {
        return Err(ToonPreviewError::new(
            "physical renderer returned an SVG without a closing root",
        ));
    };
    let (copper_token, silk_token) = match side {
        BoardSide::Top => ("TOP", "TOPOVERLAY"),
        BoardSide::Bottom => ("BOTTOM", "BOTTOMOVERLAY"),
    };
    let mut copper = String::new();
    write!(
        copper,
        "<g id=\"layer-{copper_token}\" data-layer-token=\"{copper_token}\" data-source-layer=\"{prefix}.Cu\">"
    )
    .expect("writing to String cannot fail");
    copper.push_str(&base.svg[group_at..close_at]);
    copper.push_str("</g>");
    let mut silk = String::new();
    write!(
        silk,
        "<g id=\"layer-{silk_token}\" data-layer-token=\"{silk_token}\" data-source-layer=\"{prefix}.SilkS\">"
    )
    .expect("writing to String cannot fail");
    silk.push_str(silk_group);
    silk.push_str("</g>");
    let default_layers = default_board_layer_order(side);
    let layers = layer_order.unwrap_or(&default_layers);
    let mut svg = String::with_capacity(
        base.svg.len()
            + materials.substrate.len()
            + materials.soldermask_film.len()
            + materials.cutouts.len()
            + materials.drills.len()
            + materials.slots.len()
            + materials.outline.len()
            + silk.len()
            + ILLUSTRATION_MARKER.len()
            + ASSEMBLY_DESIGNATORS_MARKER.len(),
    );
    svg.push_str(&base.svg[..group_at]);
    let mut seen = BTreeSet::new();
    for layer in layers {
        let canonical = canonical_toon_layer(layer, side)?;
        if !seen.insert(canonical) {
            return Err(ToonPreviewError::new(format!(
                "Toon view repeats layer {layer:?}"
            )));
        }
        match canonical {
            "BOARD_SUBSTRATE" => svg.push_str(&materials.substrate),
            "COPPER" => svg.push_str(&copper),
            "SOLDERMASK_FILM" => svg.push_str(&materials.soldermask_film),
            "SILKSCREEN" => svg.push_str(&silk),
            "BOARD_CUTOUTS" => svg.push_str(&materials.cutouts),
            "DRILLS" => svg.push_str(&materials.drills),
            "SLOTS" => svg.push_str(&materials.slots),
            "BOARD_OUTLINE" => svg.push_str(&materials.outline),
            "ILLUSTRATION" => svg.push_str(ILLUSTRATION_MARKER),
            "ASSEMBLY_DESIGNATORS" => svg.push_str(ASSEMBLY_DESIGNATORS_MARKER),
            _ => unreachable!("canonical Toon layer"),
        }
    }
    svg.push_str(&base.svg[close_at..]);
    Ok((svg, base.viewport))
}

fn insert_footprint_overlay_markers(
    svg: String,
    side: BoardSide,
    layer_order: Option<&[String]>,
) -> Result<String, ToonPreviewError> {
    let markers = match layer_order {
        Some(layers) => {
            let mut markers = String::new();
            for layer in layers {
                match canonical_toon_layer(layer, side)? {
                    "ILLUSTRATION" => markers.push_str(ILLUSTRATION_MARKER),
                    "ASSEMBLY_DESIGNATORS" => {
                        markers.push_str(ASSEMBLY_DESIGNATORS_MARKER);
                    }
                    _ => {}
                }
            }
            markers
        }
        None => ILLUSTRATION_MARKER.to_owned(),
    };
    let Some(close_at) = svg.rfind("</svg>") else {
        return Err(ToonPreviewError::new(
            "footprint renderer returned an SVG without a closing root",
        ));
    };
    let mut composed = String::with_capacity(svg.len() + markers.len());
    composed.push_str(&svg[..close_at]);
    composed.push_str(&markers);
    composed.push_str(&svg[close_at..]);
    Ok(composed)
}

fn default_board_layer_order(side: BoardSide) -> Vec<String> {
    let prefix = side.prefix();
    vec![
        "BOARD_SUBSTRATE".to_owned(),
        format!("{prefix}.Cu"),
        format!("SOLDERMASK_FILM_{}", side.name().to_ascii_uppercase()),
        format!("{prefix}.SilkS"),
        "BOARD_CUTOUTS".to_owned(),
        "DRILLS".to_owned(),
        "SLOTS".to_owned(),
        "BOARD_OUTLINE".to_owned(),
        format!("ILLUSTRATION_{}", side.name().to_ascii_uppercase()),
    ]
}

fn canonical_toon_layer(layer: &str, side: BoardSide) -> Result<&'static str, ToonPreviewError> {
    let (copper, copper_alias, mask, silk, silk_alias, illustration, designators) = match side {
        BoardSide::Top => (
            "F.Cu",
            "TOP",
            "SOLDERMASK_FILM_TOP",
            "F.SilkS",
            "TOPOVERLAY",
            "ILLUSTRATION_TOP",
            "ASSEMBLY_DESIGNATORS_TOP",
        ),
        BoardSide::Bottom => (
            "B.Cu",
            "BOTTOM",
            "SOLDERMASK_FILM_BOTTOM",
            "B.SilkS",
            "BOTTOMOVERLAY",
            "ILLUSTRATION_BOTTOM",
            "ASSEMBLY_DESIGNATORS_BOTTOM",
        ),
    };
    match layer {
        "BOARD_SUBSTRATE" => Ok("BOARD_SUBSTRATE"),
        value if value == copper || value == copper_alias => Ok("COPPER"),
        value if value == mask => Ok("SOLDERMASK_FILM"),
        value if value == silk || value == silk_alias => Ok("SILKSCREEN"),
        "BOARD_CUTOUTS" => Ok("BOARD_CUTOUTS"),
        "DRILLS" => Ok("DRILLS"),
        "SLOTS" => Ok("SLOTS"),
        "BOARD_OUTLINE" => Ok("BOARD_OUTLINE"),
        value if value == illustration => Ok("ILLUSTRATION"),
        value if value == designators => Ok("ASSEMBLY_DESIGNATORS"),
        _ => Err(ToonPreviewError::new(format!(
            "Toon view layer {layer:?} is not supported for the {} side; use pcb-svg for other compositions",
            side.name()
        ))),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "footprint composition needs the selected source identity, geometry, viewport, and presentation"
)]
fn compose_footprint_physical(
    view: &PcbView<'_>,
    footprints: &[PcbFootprint],
    base: Arc<kicad_monkey_svg::SvgArtifact>,
    prefix: &str,
    reference: &str,
    footprint_index: usize,
    silk_group: &str,
    style: &ToonStyleSettings,
) -> Result<(String, SvgViewport), ToonPreviewError> {
    let holes = footprint_hole_layers(
        view,
        footprints,
        footprint_index,
        base.viewport,
        prefix,
        style,
    )?;
    let Some(group_at) = base.svg.find("<g transform=") else {
        return Err(ToonPreviewError::new(
            "footprint renderer returned an SVG without a drawing group",
        ));
    };
    let Some(close_at) = base.svg.rfind("</svg>") else {
        return Err(ToonPreviewError::new(
            "footprint renderer returned an SVG without a closing root",
        ));
    };
    let (copper_token, silk_token) = match prefix {
        "F" => ("TOP", "TOPOVERLAY"),
        "B" => ("BOTTOM", "BOTTOMOVERLAY"),
        _ => return Err(ToonPreviewError::new("invalid footprint board side")),
    };
    let mut svg = String::with_capacity(
        base.svg.len() + silk_group.len() + holes.drills.len() + holes.slots.len() + 256,
    );
    svg.push_str(&base.svg[..group_at]);
    write!(
        svg,
        "<g data-toon-scope=\"footprint\" data-footprint=\"{}\"><g id=\"layer-{copper_token}\" data-layer-token=\"{copper_token}\" data-source-layer=\"{prefix}.Cu\">",
        escape_xml(reference),
    )
    .expect("writing to String cannot fail");
    svg.push_str(&base.svg[group_at..close_at]);
    svg.push_str("</g>");
    write!(
        svg,
        "<g id=\"layer-{silk_token}\" data-layer-token=\"{silk_token}\" data-source-layer=\"{prefix}.SilkS\">"
    )
    .expect("writing to String cannot fail");
    svg.push_str(silk_group);
    svg.push_str("</g>");
    svg.push_str(&holes.drills);
    svg.push_str(&holes.slots);
    svg.push_str("</g>");
    svg.push_str(&base.svg[close_at..]);
    Ok((svg, base.viewport))
}

fn base_context(
    side: BoardSide,
) -> Result<kicad_monkey_svg::ValidatedSvgRenderContextA1, ToonPreviewError> {
    let color = |value| SvgColor::parse(value).map_err(error);
    let filled = |value| {
        color(value).map(|color| {
            SvgStyleOverride::new()
                .with_stroke(color.clone())
                .with_fill(color)
                .with_fill_mode(SvgFillMode::Solid)
        })
    };
    let copper = format!("{}.Cu", side.prefix());
    SvgRenderContextA1::builder()
        .background(SvgBackground::Opaque(color("#F3F1EA")?))
        .layer_selection(LayerSelection::include(
            [copper.as_str(), "Edge.Cuts"]
                .into_iter()
                .map(LayerPattern::parse)
                .collect::<Result<Vec<_>, _>>()
                .map_err(error)?,
            false,
        ))
        .layer_style(
            LayerPattern::parse(copper).map_err(error)?,
            filled("#D6A84A")?,
        )
        .layer_style(
            LayerPattern::parse("Edge.Cuts").map_err(error)?,
            SvgStyleOverride::new()
                .with_stroke(color("#7BC9A8")?)
                .with_fill_mode(SvgFillMode::None)
                .with_stroke_width_nm(120_000),
        )
        .build()
        .validate(SvgContextLimits::default())
        .map_err(error)
}

fn copper_context(
    side: BoardSide,
    configured_color: &str,
) -> Result<kicad_monkey_svg::ValidatedSvgRenderContextA1, ToonPreviewError> {
    let color = |value| SvgColor::parse(value).map_err(error);
    let copper = format!("{}.Cu", side.prefix());
    let copper_color = color(configured_color)?;
    SvgRenderContextA1::builder()
        .background(SvgBackground::Transparent)
        .layer_selection(LayerSelection::include(
            vec![LayerPattern::parse(&copper).map_err(error)?],
            false,
        ))
        .layer_style(
            LayerPattern::parse(copper).map_err(error)?,
            SvgStyleOverride::new()
                .with_stroke(copper_color.clone())
                .with_fill(copper_color)
                .with_fill_mode(SvgFillMode::Solid),
        )
        .build()
        .validate(SvgContextLimits::default())
        .map_err(error)
}

fn single_layer_context(
    layer: &str,
    color: &str,
) -> Result<kicad_monkey_svg::ValidatedSvgRenderContextA1, ToonPreviewError> {
    let color = SvgColor::parse(color).map_err(error)?;
    SvgRenderContextA1::builder()
        .background(SvgBackground::Transparent)
        .layer_selection(LayerSelection::include(
            vec![LayerPattern::parse(layer).map_err(error)?],
            false,
        ))
        .layer_style(
            LayerPattern::parse(layer).map_err(error)?,
            SvgStyleOverride::new()
                .with_stroke(color.clone())
                .with_fill(color)
                .with_fill_mode(SvgFillMode::Solid),
        )
        .build()
        .validate(SvgContextLimits::default())
        .map_err(error)
}

fn root_drawing_group(svg: &str) -> Result<&str, ToonPreviewError> {
    let start = svg.find("<g transform=").ok_or_else(|| {
        ToonPreviewError::new("physical layer SVG has no translated drawing group")
    })?;
    let end = svg
        .rfind("</svg>")
        .ok_or_else(|| ToonPreviewError::new("physical layer SVG has no closing root"))?;
    Ok(&svg[start..end])
}

fn mirror_drawing(svg: &str, viewport: SvgViewport) -> Result<String, ToonPreviewError> {
    let start = svg
        .find("<g ")
        .ok_or_else(|| ToonPreviewError::new("Toon SVG has no drawing group to mirror"))?;
    let end = svg
        .rfind("</svg>")
        .ok_or_else(|| ToonPreviewError::new("Toon SVG has no closing root"))?;
    let width = viewport.width_nm as f64 / 1_000_000.0;
    let wrapper = format!(
        "<g id=\"bottom-view-mirror\" transform=\"translate({} 0) scale(-1 1)\">",
        number(width)
    );
    let mut mirrored = String::with_capacity(svg.len() + wrapper.len() + 4);
    mirrored.push_str(&svg[..start]);
    mirrored.push_str(&wrapper);
    mirrored.push_str(&svg[start..end]);
    mirrored.push_str("</g>");
    mirrored.push_str(&svg[end..]);
    Ok(mirrored)
}

fn illustration_request(
    attachment: &PcbModelReference,
    board_thickness_mm: f64,
    side: BoardSide,
    outline_width_mm: f64,
    detail_width_mm: f64,
) -> Result<ModelIllustrationGeometryRequestA0, serde_json::Error> {
    serde_json::from_value(json!({
        "schema": "geometry.model_illustration_geometry.request.a0",
        "source": {
            "kind": "model",
            "attachment": "model",
            "transform": column_major(local_pose(attachment, board_thickness_mm, side)),
        },
        "view": {
            "direction": if side == BoardSide::Top {
                [0.0, 0.0, 1.0]
            } else {
                [0.0, 0.0, -1.0]
            },
            "up": [0.0, 1.0, 0.0],
            "mirror_x": false,
        },
        "linework": {
            "fast": {
                "include_hidden": false,
                "suppress_coplanar_seams": false,
                "crease_angle_rad": 25.0_f64.to_radians(),
            },
            "outline_width_mm": outline_width_mm,
            "detail_width_mm": detail_width_mm,
        },
        "style": {
            "shading": "toon",
            "ambient": 0.28,
            "key_intensity": 0.9,
            "rim_amount": 0.12,
            "light_direction": if side == BoardSide::Top {
                [0.35, 0.8, 0.48]
            } else {
                [-0.35, 0.8, -0.48]
            },
            "bands": 3,
            "source_colors": true,
            "fallback_color": [113.0 / 255.0, 166.0 / 255.0, 160.0 / 255.0],
            "transparent_background": true,
            "fuse_surfaces": true,
            "layer_coplanar_materials": true,
            "double_sided": false,
            "show_outlines": false,
            "show_creases": false,
            "show_hlr_outline": true,
            "show_hlr_detail": true,
            "outline_color": "#000000",
            "crease_color": "#000000",
        },
    }))
}

fn append_geometry(
    output: &mut String,
    geometry: &MeshIllustrationGeometryA0,
    footprint: &PcbFootprint,
    viewport: SvgViewport,
    side: BoardSide,
) {
    for surface in &geometry.surfaces {
        for layer in &surface.layers {
            let mut commands = String::new();
            for ring in &layer.rings {
                let Some(first) = ring.points.first() else {
                    continue;
                };
                let first = canvas_point(projected_point(*first, side), footprint, viewport);
                write!(commands, "M{} {}", number(first[0]), number(first[1]))
                    .expect("writing to String cannot fail");
                for point in &ring.points[1..] {
                    let point = canvas_point(projected_point(*point, side), footprint, viewport);
                    write!(commands, "L{} {}", number(point[0]), number(point[1]))
                        .expect("writing to String cannot fail");
                }
                commands.push('Z');
            }
            if commands.is_empty() {
                continue;
            }
            write!(
                output,
                "<path d=\"{commands}\" fill=\"{}\" stroke=\"{}\" stroke-width=\"{}\" stroke-linejoin=\"{}\"{} />",
                escape_xml(&layer.fill),
                escape_xml(&layer.fill),
                number(geometry.presentation.seam_width),
                escape_xml(&geometry.presentation.line_join),
                if layer.opacity < 0.999 {
                    format!(" opacity=\"{}\"", number(layer.opacity))
                } else {
                    String::new()
                }
            )
            .expect("writing to String cannot fail");
        }
    }
    for line in &geometry.lines {
        let start = canvas_point(projected_point(line.start, side), footprint, viewport);
        let end = canvas_point(projected_point(line.end, side), footprint, viewport);
        write!(
            output,
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" />",
            number(start[0]),
            number(start[1]),
            number(end[0]),
            number(end[1]),
            escape_xml(&line.color),
            number(line.width),
        )
        .expect("writing to String cannot fail");
    }
}

fn projected_point(point: [f64; 2], side: BoardSide) -> [f64; 2] {
    if side == BoardSide::Bottom {
        [-point[0], point[1]]
    } else {
        point
    }
}

fn canvas_point(point: [f64; 2], footprint: &PcbFootprint, viewport: SvgViewport) -> [f64; 2] {
    let angle = footprint.angle.unwrap_or(0.0).to_radians();
    let (sin, cos) = angle.sin_cos();
    let world_x = footprint.at_x.unwrap_or(0.0) + point[0] * cos - point[1] * sin;
    let world_y = -footprint.at_y.unwrap_or(0.0) + point[0] * sin + point[1] * cos;
    [
        world_x - viewport.min_x_nm as f64 / 1_000_000.0,
        -world_y - viewport.min_y_nm as f64 / 1_000_000.0,
    ]
}

type Matrix = [[f64; 4]; 4];

fn local_pose(model: &PcbModelReference, board_thickness_mm: f64, side: BoardSide) -> Matrix {
    let mut matrix = identity();
    if side == BoardSide::Bottom {
        matrix = multiply(matrix, rotation_x(std::f64::consts::PI));
    }
    matrix = multiply(
        matrix,
        translation(
            model.offset[0],
            model.offset[1],
            model.offset[2] + BOARD_MODEL_Z_OFFSET_MM + board_thickness_mm / 2.0,
        ),
    );
    matrix = multiply(matrix, rotation_z(-model.rotate[2].to_radians()));
    matrix = multiply(matrix, rotation_y(-model.rotate[1].to_radians()));
    matrix = multiply(matrix, rotation_x(-model.rotate[0].to_radians()));
    multiply(matrix, scale(model.scale))
}

fn identity() -> Matrix {
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn translation(x: f64, y: f64, z: f64) -> Matrix {
    let mut result = identity();
    result[0][3] = x;
    result[1][3] = y;
    result[2][3] = z;
    result
}

fn scale(value: [f64; 3]) -> Matrix {
    [
        [value[0], 0.0, 0.0, 0.0],
        [0.0, value[1], 0.0, 0.0],
        [0.0, 0.0, value[2], 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn rotation_x(angle: f64) -> Matrix {
    let (sin, cos) = angle.sin_cos();
    [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, cos, -sin, 0.0],
        [0.0, sin, cos, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn rotation_y(angle: f64) -> Matrix {
    let (sin, cos) = angle.sin_cos();
    [
        [cos, 0.0, sin, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [-sin, 0.0, cos, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn rotation_z(angle: f64) -> Matrix {
    let (sin, cos) = angle.sin_cos();
    [
        [cos, -sin, 0.0, 0.0],
        [sin, cos, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn multiply(left: Matrix, right: Matrix) -> Matrix {
    let mut result = [[0.0; 4]; 4];
    for row in 0..4 {
        for column in 0..4 {
            result[row][column] = (0..4)
                .map(|index| left[row][index] * right[index][column])
                .sum();
        }
    }
    result
}

fn column_major(matrix: Matrix) -> [f64; 16] {
    let mut result = [0.0; 16];
    for column in 0..4 {
        for row in 0..4 {
            result[column * 4 + row] = matrix[row][column];
        }
    }
    result
}

fn number(value: f64) -> String {
    if value.abs() < 0.000_000_5 {
        "0".to_owned()
    } else {
        format!("{value:.6}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    }
}

fn escape_xml(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn error(value: impl fmt::Display) -> ToonPreviewError {
    ToonPreviewError(value.to_string())
}

fn kicad_soldermask_color(value: &str) -> Option<SvgColor> {
    let color = match value.trim().to_ascii_lowercase().as_str() {
        "black" => "#000000",
        "blue" => "#1D4F91",
        "green" => "#176B3A",
        "purple" => "#6F3C8A",
        "red" => "#A62A2A",
        "white" => "#EEEEEE",
        "yellow" => "#D6A600",
        _ => value.trim(),
    };
    SvgColor::parse(color).ok()
}

fn contrasting_silkscreen_color(soldermask_color: &SvgColor) -> SvgColor {
    let color = if soldermask_color.as_str() == "#EEEEEE" {
        DARK_TOON_SILKSCREEN_COLOR
    } else {
        LIGHT_TOON_SILKSCREEN_COLOR
    };
    SvgColor::parse(color).expect("Toon silkscreen color is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    const OUTLINED_BOARD: &str = r#"(kicad_pcb
      (version 20250830) (generator pcbnew)
      (layers
        (0 "F.Cu" signal) (31 "B.Cu" signal)
        (36 "B.SilkS" user "Back Silkscreen")
        (37 "F.SilkS" user "Front Silkscreen")
        (44 "Edge.Cuts" user))
      (gr_line (start 0 0) (end 10 0)
        (stroke (width 0.1) (type solid)) (layer "Edge.Cuts") (uuid "edge-1"))
      (gr_line (start 10 0) (end 10 5)
        (stroke (width 0.1) (type solid)) (layer "Edge.Cuts") (uuid "edge-2"))
      (gr_line (start 10 5) (end 0 5)
        (stroke (width 0.1) (type solid)) (layer "Edge.Cuts") (uuid "edge-3"))
      (gr_line (start 0 5) (end 0 0)
        (stroke (width 0.1) (type solid)) (layer "Edge.Cuts") (uuid "edge-4")))"#;

    #[test]
    fn point_placement_matches_kicad_board_coordinates() {
        let footprint = PcbFootprint {
            at_x: Some(10.0),
            at_y: Some(20.0),
            angle: Some(90.0),
            ..empty_footprint()
        };
        let point = canvas_point(
            [2.0, 1.0],
            &footprint,
            SvgViewport {
                min_x_nm: 0,
                min_y_nm: 0,
                width_nm: 50_000_000,
                height_nm: 50_000_000,
            },
        );
        assert!((point[0] - 9.0).abs() < 1e-9);
        assert!((point[1] - 18.0).abs() < 1e-9);
    }

    #[test]
    fn illustration_request_matches_altium_toon_linework() {
        let model = PcbModelReference {
            owner: kicad_monkey_core::PcbFootprintMemberOwner::StandaloneFootprint,
            footprint_index: 0,
            path: "model.step".to_owned(),
            offset: [0.0; 3],
            scale: [1.0; 3],
            rotate: [0.0; 3],
            hidden: None,
            opacity: None,
            source_range: 0..0,
        };
        let top = serde_json::to_value(
            illustration_request(
                &model,
                1.6,
                BoardSide::Top,
                HLR_OUTLINE_WIDTH_MM,
                HLR_DETAIL_WIDTH_MM,
            )
            .expect("top request"),
        )
        .expect("serialize top request");
        let bottom = serde_json::to_value(
            illustration_request(
                &model,
                1.6,
                BoardSide::Bottom,
                HLR_OUTLINE_WIDTH_MM,
                HLR_DETAIL_WIDTH_MM,
            )
            .expect("bottom request"),
        )
        .expect("serialize bottom request");
        assert_eq!(top["linework"]["outline_width_mm"], 0.025);
        assert_eq!(top["linework"]["detail_width_mm"], 0.01375);
        assert_eq!(top["style"]["light_direction"], json!([0.35, 0.8, 0.48]));
        assert_eq!(
            bottom["style"]["light_direction"],
            json!([-0.35, 0.8, -0.48])
        );
    }

    #[test]
    fn model_painter_depth_places_outward_models_last_on_each_side() {
        let top_low = [0.0, 0.0, 1.65, 2.0, 2.0, 2.2];
        let top_shield = [0.0, 0.0, 1.65, 10.0, 10.0, 4.5];
        assert!(
            model_painter_depth(BoardSide::Top, &top_low)
                < model_painter_depth(BoardSide::Top, &top_shield)
        );

        let bottom_low = [0.0, 0.0, -2.2, 2.0, 2.0, -1.65];
        let bottom_shield = [0.0, 0.0, -4.5, 10.0, 10.0, -1.65];
        assert!(
            model_painter_depth(BoardSide::Bottom, &bottom_low)
                < model_painter_depth(BoardSide::Bottom, &bottom_shield)
        );
    }

    #[test]
    fn physical_toon_board_keeps_the_canvas_outside_edge_cuts_transparent() {
        let view = PcbView::parse(OUTLINED_BOARD, PcbLimits::default()).unwrap();
        let footprints = view
            .footprints()
            .collect::<Result<Vec<_>, _>>()
            .expect("parse outlined board footprints");
        let (svg, _) = render_physical_board(
            OUTLINED_BOARD,
            "transparent-board".to_owned(),
            &view,
            &footprints,
            BoardSide::Top,
            &SvgColor::parse("#EEEEEE").unwrap(),
            &SvgColor::parse("#000000").unwrap(),
            &ToonStyleSettings::default(),
            None,
            None,
        )
        .expect("render physical Toon board");

        assert!(!svg.contains("fill=\"#F3F1EA\""), "{svg}");
        assert!(svg.contains("fill=\"#EEEEEE\""), "{svg}");
        assert!(
            svg.contains("data-layer-token=\"BOARD_SUBSTRATE\""),
            "{svg}"
        );
        assert!(svg.contains("fill=\"#B6A26B\""), "{svg}");
        assert!(
            svg.contains("mask=\"url(#board-substrate-openings)\""),
            "{svg}"
        );
    }

    #[test]
    fn physical_toon_board_honors_configured_layer_order_and_omissions() {
        let view = PcbView::parse(OUTLINED_BOARD, PcbLimits::default()).unwrap();
        let footprints = view
            .footprints()
            .collect::<Result<Vec<_>, _>>()
            .expect("parse outlined board footprints");
        let layers = vec![
            "BOARD_OUTLINE".to_owned(),
            "BOARD_SUBSTRATE".to_owned(),
            "ILLUSTRATION_TOP".to_owned(),
        ];
        let (svg, _) = render_physical_board(
            OUTLINED_BOARD,
            "ordered-board".to_owned(),
            &view,
            &footprints,
            BoardSide::Top,
            &SvgColor::parse("#EEEEEE").unwrap(),
            &SvgColor::parse("#000000").unwrap(),
            &ToonStyleSettings::default(),
            None,
            Some(&layers),
        )
        .expect("render ordered Toon board");

        let outline = svg.find("data-layer-token=\"BOARD_OUTLINE\"").unwrap();
        let substrate = svg.find("data-layer-token=\"BOARD_SUBSTRATE\"").unwrap();
        let illustration = svg.find(ILLUSTRATION_MARKER).unwrap();
        assert!(outline < substrate && substrate < illustration, "{svg}");
        assert!(!svg.contains("data-layer-token=\"SOLDERMASK_FILM_TOP\""));
        assert!(!svg.contains("data-layer-token=\"F.Cu\""));
        assert!(!svg.contains("data-layer-token=\"F.SilkS\""));
        assert!(!svg.contains("data-layer-token=\"DRILLS\""));
    }

    fn empty_footprint() -> PcbFootprint {
        PcbView::parse("(kicad_pcb (footprint \"Empty\"))", PcbLimits::default())
            .unwrap()
            .footprints()
            .next()
            .unwrap()
            .unwrap()
    }
}
