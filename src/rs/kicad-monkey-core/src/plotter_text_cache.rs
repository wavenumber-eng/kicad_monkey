//! Caller-supplied native outline-font resources for plotter producers.
//!
//! Core never discovers platform fonts. Callers bind one exact KiCad
//! face/style request to deterministic font bytes and a complete shaping
//! template; the accepted hinted layout/cache engine validates the declared
//! font identity and applies every caller-owned work ceiling.

use crate::sexpr::{Error, ErrorKind, ErrorPhase, Position};
use crate::{
    TextBlockLayoutLimits, TextBlockLayoutRequest, TextHorizontalAlignment, TextLinebreakLimits,
    TextRenderCache, TextRenderCacheErrorKind, TextRenderCacheLimits, TextVerticalAlignment,
    generate_text_render_cache_block_hinted_a0, layout_text_block_hinted_a0,
    linebreak_text_block_hinted_a0,
};
use kicad_monkey_contracts::generated::shaping_record::ShapingInput;
use kicad_monkey_contracts::validate_shaping_input_contract;
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::cmp::Ordering;

/// One deterministic font/style selection supplied outside a KiCad source.
#[derive(Clone, Debug)]
pub struct PlotterTextFont<'a> {
    pub face: &'a str,
    pub bold: bool,
    pub italic: bool,
    pub font_bytes: &'a [u8],
    /// Complete shaping context. `text` is replaced by each carrier's
    /// resolved text; all other fields, including direction/script/language,
    /// scale, features, face index, variations, ID, and digest, are retained.
    pub shaping: ShapingInput,
    pub fake_bold: bool,
    pub fake_italic: bool,
}

/// Independent sidecar and accepted-engine ceilings.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlotterTextCacheLimits {
    pub max_fonts: usize,
    pub max_face_bytes: usize,
    /// Aggregate bytes across every supplied font buffer.
    pub max_font_bytes: usize,
    /// Aggregate font bytes hashed during validation and carrier sessions.
    pub max_hash_bytes: usize,
    pub max_selection_bytes: usize,
    pub linebreak: TextLinebreakLimits,
    pub layout: TextBlockLayoutLimits,
    pub cache: TextRenderCacheLimits,
    /// KiCad outline flattening error in internal face units.
    pub max_error: f64,
}

impl Default for PlotterTextCacheLimits {
    fn default() -> Self {
        Self {
            max_fonts: 4096,
            max_face_bytes: 16 * 1024 * 1024,
            max_font_bytes: 256 * 1024 * 1024,
            max_hash_bytes: 2 * 1024 * 1024 * 1024,
            max_selection_bytes: 64 * 1024 * 1024,
            linebreak: TextLinebreakLimits::default(),
            layout: TextBlockLayoutLimits::default(),
            cache: TextRenderCacheLimits::default(),
            max_error: 2.0,
        }
    }
}

/// Read-only font/cache generation resources shared across plotter carriers.
#[derive(Clone, Copy, Debug)]
pub struct PlotterTextCacheResources<'a> {
    pub fonts: &'a [PlotterTextFont<'a>],
    pub limits: PlotterTextCacheLimits,
}

/// Per-document work state over immutable caller-supplied font resources.
pub(crate) struct PlotterTextCacheSession<'a> {
    resources: &'a PlotterTextCacheResources<'a>,
    hash_bytes: Cell<usize>,
}

/// Carrier layout facts passed to the neutral native cache bridge.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PlotterTextLayout<'a> {
    pub text: &'a str,
    pub face: &'a str,
    pub bold: bool,
    pub italic: bool,
    pub size_x: f64,
    pub size_y: f64,
    pub position_x: f64,
    pub position_y: f64,
    pub angle_degrees: f64,
    pub mirrored: bool,
    pub horizontal_alignment: TextHorizontalAlignment,
    pub vertical_alignment: TextVerticalAlignment,
    pub line_spacing: f64,
    pub stroke_width: f64,
}

/// Deterministic outline-font dimensions in the same coordinate units as the
/// corresponding [`PlotterTextLayout`].
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlotterTextMetrics {
    pub width: f64,
    pub height: f64,
    /// KiCad/FreeType face ascender plus descender after its high-resolution
    /// metric grid is reduced to schematic millimetres.
    pub line_height: f64,
    pub line_widths: Vec<f64>,
}

impl PlotterTextCacheResources<'_> {
    pub(crate) fn validate(&self) -> Result<(), Error> {
        validate_resource_header(self)?;
        let mut retained = 0usize;
        for (index, font) in self.fonts.iter().enumerate() {
            let metadata_bytes = shaping_metadata_bytes(&font.shaping)?;
            retained = validate_font(self, font, index, metadata_bytes, retained)?;
        }
        Ok(())
    }

    pub(crate) fn linebreak(
        &self,
        layout: PlotterTextLayout<'_>,
        column_width: f64,
    ) -> Result<String, Error> {
        let (font, shaping) = self.selection_and_shaping(layout)?;
        linebreak_text_block_hinted_a0(
            font.font_bytes,
            block_request(
                layout,
                &shaping,
                self.limits.max_error,
                font.fake_bold,
                font.fake_italic,
            ),
            column_width,
            self.limits.linebreak,
        )
        .map_err(text_pipeline_error)
    }

    pub(crate) fn generate(
        &self,
        layout: PlotterTextLayout<'_>,
        max_retained_points: usize,
        max_polygons: usize,
        max_contours: usize,
    ) -> Result<TextRenderCache, Error> {
        let (font, shaping) = self.selection_and_shaping(layout)?;
        let (layout_limits, cache_limits) =
            bounded_generation_limits(self.limits, max_retained_points, max_polygons, max_contours);
        generate_text_render_cache_block_hinted_a0(
            font.font_bytes,
            block_request(
                layout,
                &shaping,
                self.limits.max_error,
                font.fake_bold,
                font.fake_italic,
            ),
            layout_limits,
            cache_limits,
        )
        .map_err(|error| match error.kind {
            TextRenderCacheErrorKind::ResourceLimit => resource(error.message),
            _ => invalid(error.message),
        })
    }

    pub(crate) fn measure(
        &self,
        layout: PlotterTextLayout<'_>,
    ) -> Result<PlotterTextMetrics, Error> {
        let (font, shaping) = self.selection_and_shaping(layout)?;
        let output = layout_text_block_hinted_a0(
            font.font_bytes,
            block_request(
                layout,
                &shaping,
                self.limits.max_error,
                font.fake_bold,
                font.fake_italic,
            ),
            self.limits.layout,
        )
        .map_err(|error| match error.kind {
            crate::TextContourErrorKind::ResourceLimit => resource(error.message),
            _ => invalid(error.message),
        })?;
        Ok(PlotterTextMetrics {
            width: output.width,
            height: output.height,
            line_height: hinted_line_height(font, layout)?,
            line_widths: output.line_widths,
        })
    }

    fn selection_and_shaping(
        &self,
        layout: PlotterTextLayout<'_>,
    ) -> Result<(&PlotterTextFont<'_>, ShapingInput), Error> {
        let font = self.selection(layout)?;
        let mut shaping = font.shaping.clone();
        shaping.text = layout.text.to_owned();
        Ok((font, shaping))
    }

    fn selection(&self, layout: PlotterTextLayout<'_>) -> Result<&PlotterTextFont<'_>, Error> {
        let key = (layout.face, layout.bold, layout.italic);
        let index = self
            .fonts
            .binary_search_by(|font| font_key(font).cmp(&key))
            .map_err(|_| invalid("Native plotter font selection is missing"))?;
        Ok(&self.fonts[index])
    }
}

fn hinted_line_height(
    font: &PlotterTextFont<'_>,
    layout: PlotterTextLayout<'_>,
) -> Result<f64, Error> {
    let face = ttf_parser::Face::parse(font.font_bytes, font.shaping.face_index)
        .map_err(|_| invalid("Native plotter font vertical metrics are invalid"))?;
    let units_per_em = f64::from(face.units_per_em());
    let hinted = |metric: i16| {
        // The Python authority requests a 1433/64 point face at 1152 dpi,
        // then reduces FreeType's 26.6 metrics by 16. The resulting values
        // occupy a four-unit grid on KiCad's 1433-unit outline face.
        ((f64::from(metric).abs() * crate::KICAD_OUTLINE_FACE_SCALER / units_per_em / 4.0).ceil())
            * 4.0
    };
    let size_y_iu = (layout.size_y * 10_000.0).round_ties_even();
    let scale =
        size_y_iu / crate::KICAD_OUTLINE_FACE_SCALER * crate::KICAD_OUTLINE_SIZE_COMPENSATION;
    let height_iu =
        (hinted(face.ascender()) * scale) as i64 + (hinted(face.descender()) * scale) as i64;
    Ok(height_iu as f64 / 10_000.0)
}

impl<'a> PlotterTextCacheSession<'a> {
    pub(crate) fn new(resources: &'a PlotterTextCacheResources<'a>) -> Result<Self, Error> {
        resources.validate()?;
        let validated_hash_bytes = total_font_bytes(resources)?;
        Ok(Self {
            resources,
            hash_bytes: Cell::new(validated_hash_bytes),
        })
    }

    pub(crate) fn linebreak(
        &self,
        layout: PlotterTextLayout<'_>,
        column_width: f64,
    ) -> Result<String, Error> {
        if column_width.is_finite()
            && column_width > 0.0
            && !layout.text.is_empty()
            && layout.text.contains(' ')
        {
            self.charge_session_hashes(layout)?;
        }
        self.resources.linebreak(layout, column_width)
    }

    pub(crate) fn generate(
        &self,
        layout: PlotterTextLayout<'_>,
        max_retained_points: usize,
        max_polygons: usize,
        max_contours: usize,
    ) -> Result<TextRenderCache, Error> {
        self.charge_session_hashes(layout)?;
        self.resources
            .generate(layout, max_retained_points, max_polygons, max_contours)
    }

    pub(crate) fn measure(
        &self,
        layout: PlotterTextLayout<'_>,
    ) -> Result<PlotterTextMetrics, Error> {
        self.charge_session_hashes(layout)?;
        self.resources.measure(layout)
    }

    fn charge_session_hashes(&self, layout: PlotterTextLayout<'_>) -> Result<(), Error> {
        let font = self.resources.selection(layout)?;
        let passes = 2 + usize::from(layout.text.contains("_{") || layout.text.contains("^{"));
        let amount = font
            .font_bytes
            .len()
            .checked_mul(passes)
            .ok_or_else(|| resource("Native plotter font hash work overflowed"))?;
        let next = self
            .hash_bytes
            .get()
            .checked_add(amount)
            .filter(|bytes| *bytes <= self.resources.limits.max_hash_bytes)
            .ok_or_else(|| resource("Native plotter font hash work exceeds max_hash_bytes"))?;
        self.hash_bytes.set(next);
        Ok(())
    }
}

fn validate_resource_header(resources: &PlotterTextCacheResources<'_>) -> Result<(), Error> {
    if !resources.limits.max_error.is_finite() || resources.limits.max_error <= 0.0 {
        return Err(invalid(
            "Native plotter text max_error must be positive and finite",
        ));
    }
    if resources.fonts.len() > resources.limits.max_fonts {
        return Err(resource("Native plotter text font count exceeds max_fonts"));
    }
    let font_bytes = total_font_bytes(resources).ok();
    if font_bytes.is_none_or(|bytes| bytes > resources.limits.max_font_bytes) {
        return Err(resource(
            "Native plotter font buffers exceed max_font_bytes",
        ));
    }
    if font_bytes.is_none_or(|bytes| bytes > resources.limits.max_hash_bytes) {
        return Err(resource(
            "Native plotter font validation exceeds max_hash_bytes",
        ));
    }
    Ok(())
}

fn total_font_bytes(resources: &PlotterTextCacheResources<'_>) -> Result<usize, Error> {
    resources
        .fonts
        .iter()
        .try_fold(0usize, |total, font| {
            total.checked_add(font.font_bytes.len())
        })
        .ok_or_else(|| resource("Native plotter font byte count overflowed"))
}

fn validate_font(
    resources: &PlotterTextCacheResources<'_>,
    font: &PlotterTextFont<'_>,
    index: usize,
    metadata_bytes: usize,
    retained: usize,
) -> Result<usize, Error> {
    if font.face.len() > resources.limits.max_face_bytes {
        return Err(resource("Native plotter font face exceeds max_face_bytes"));
    }
    if index > 0 && font_key(&resources.fonts[index - 1]).cmp(&font_key(font)) != Ordering::Less {
        return Err(invalid(
            "Native plotter font selection keys must be strictly sorted and unique",
        ));
    }
    validate_font_identity(font)?;
    validate_font_engine_limits(resources.limits, font, metadata_bytes)?;
    retained
        .checked_add(font.face.len())
        .and_then(|value| value.checked_add(font.shaping.text.len()))
        .and_then(|value| value.checked_add(metadata_bytes))
        .filter(|value| *value <= resources.limits.max_selection_bytes)
        .ok_or_else(|| resource("Native plotter font selections exceed max_selection_bytes"))
}

fn validate_font_identity(font: &PlotterTextFont<'_>) -> Result<(), Error> {
    if validate_shaping_input_contract(&font.shaping).is_err() {
        return Err(invalid("Native plotter shaping template is invalid"));
    }
    if font
        .shaping
        .features
        .iter()
        .any(|feature| feature.start != 0 || feature.end != u32::MAX)
    {
        return Err(invalid(
            "Native plotter shaping templates require global feature ranges",
        ));
    }
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let matches = Sha256::digest(font.font_bytes)
        .iter()
        .zip(font.shaping.font_sha256.0.as_bytes().chunks_exact(2))
        .all(|(byte, pair)| pair == [HEX[(byte >> 4) as usize], HEX[(byte & 0x0f) as usize]]);
    if !matches {
        return Err(invalid(
            "Native plotter font buffer does not match its declared SHA-256",
        ));
    }
    Ok(())
}

fn validate_font_engine_limits(
    limits: PlotterTextCacheLimits,
    font: &PlotterTextFont<'_>,
    metadata_bytes: usize,
) -> Result<(), Error> {
    let contour_limits = [
        limits.linebreak.layout.contours,
        limits.layout.contours,
        limits.cache.contours,
    ];
    if contour_limits.iter().any(|limits| {
        font.font_bytes.len() > limits.shaping.max_font_bytes
            || font.font_bytes.len() > limits.outline.max_font_bytes
            || font.shaping.features.len() > limits.shaping.max_features
            || font.shaping.variations.len() > limits.shaping.max_variations
            || font.shaping.variations.len() > limits.outline.max_variations
            || metadata_bytes > limits.shaping.max_metadata_bytes
            || metadata_bytes > limits.outline.max_metadata_bytes
    }) {
        return Err(resource(
            "Native plotter font or shaping metadata exceeds engine limits",
        ));
    }
    Ok(())
}

fn font_key<'a>(font: &'a PlotterTextFont<'_>) -> (&'a str, bool, bool) {
    (font.face, font.bold, font.italic)
}

fn bounded_generation_limits(
    limits: PlotterTextCacheLimits,
    max_retained_points: usize,
    max_polygons: usize,
    max_contours: usize,
) -> (TextBlockLayoutLimits, TextRenderCacheLimits) {
    let mut layout = limits.layout;
    let mut cache = limits.cache;
    // Bound the engine's primary topology by the board remainder before it is
    // materialized. Attachment then accounts for the legacy exterior
    // projection exactly; using the full primary ceiling avoids rejecting a
    // valid hole-heavy cache whose duplicated exterior is comparatively small.
    let source_points = max_retained_points;
    layout.contours.max_points = layout.contours.max_points.min(source_points);
    layout.contours.max_contours = layout.contours.max_contours.min(max_contours);
    cache.contours.max_points = cache.contours.max_points.min(source_points);
    cache.contours.max_contours = cache.contours.max_contours.min(max_contours);
    cache.max_points = cache.max_points.min(source_points);
    cache.max_polygons = cache.max_polygons.min(max_polygons);
    cache.max_contours = cache.max_contours.min(max_contours);
    (layout, cache)
}

fn shaping_metadata_bytes(input: &ShapingInput) -> Result<usize, Error> {
    let mut total = input.font_id.len().checked_add(input.font_sha256.0.len());
    total = total.and_then(|value| value.checked_add(input.text_index_unit.len()));
    total =
        total.and_then(|value| value.checked_add(input.language.as_deref().map_or(0, str::len)));
    total = total.and_then(|value| {
        value.checked_add(input.script.as_ref().map_or(0, |script| script.0.len()))
    });
    for feature in &input.features {
        total = total.and_then(|value| {
            value
                .checked_add(feature.tag.0.len())
                .and_then(|value| value.checked_add(std::mem::size_of_val(feature)))
        });
    }
    for variation in &input.variations {
        total = total.and_then(|value| {
            value
                .checked_add(variation.axis.0.len())
                .and_then(|value| value.checked_add(std::mem::size_of_val(variation)))
        });
    }
    total.ok_or_else(|| resource("Native plotter shaping metadata byte count overflowed"))
}

fn block_request<'a>(
    layout: PlotterTextLayout<'_>,
    shaping: &'a ShapingInput,
    max_error: f64,
    fake_bold: bool,
    fake_italic: bool,
) -> TextBlockLayoutRequest<'a> {
    TextBlockLayoutRequest {
        shaping,
        size_x: layout.size_x,
        size_y: layout.size_y,
        position_x: layout.position_x,
        position_y: layout.position_y,
        angle_degrees: layout.angle_degrees,
        mirrored: layout.mirrored,
        horizontal_alignment: layout.horizontal_alignment,
        vertical_alignment: layout.vertical_alignment,
        line_spacing: layout.line_spacing,
        stroke_width: layout.stroke_width,
        max_error,
        fake_bold,
        fake_italic,
    }
}

fn text_pipeline_error(error: crate::TextContourError) -> Error {
    match error.kind {
        crate::TextContourErrorKind::ResourceLimit => resource(error.message),
        _ => invalid(error.message),
    }
}

fn resource(message: &'static str) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        message,
        Position::START,
    )
}

fn invalid(message: &'static str) -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::InvalidBuildValue,
        message,
        Position::START,
    )
}
