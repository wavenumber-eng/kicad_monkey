//! Bounded KiCad multiline and tabbed outline-text layout.

use crate::text_contours::HintedTextContourSession;
use crate::text_layout::{transform_contours, validated_transform};
use crate::{
    TextContour, TextContourError, TextContourErrorKind, TextContourLimits, TextContourOutput,
    TextHorizontalAlignment, TextLayoutRequest, TextVerticalAlignment,
};
use kicad_monkey_contracts::generated::shaping_record::{ShapingFeature, ShapingInput};
use std::ops::Range;

/// KiCad's baseline-to-baseline multiplier for outline text.
pub const KICAD_TEXT_INTERLINE_FACTOR: f64 = 1.68;
/// KiCad's four-character tab stop using an average character width of 0.6.
pub const KICAD_TEXT_TAB_WIDTH_FACTOR: f64 = 2.4;

/// One plain multiline text request before markup and fake-style synthesis.
#[derive(Clone, Copy, Debug)]
pub struct TextBlockLayoutRequest<'a> {
    pub shaping: &'a ShapingInput,
    pub size_x: f64,
    pub size_y: f64,
    pub position_x: f64,
    pub position_y: f64,
    pub angle_degrees: f64,
    pub mirrored: bool,
    pub horizontal_alignment: TextHorizontalAlignment,
    pub vertical_alignment: TextVerticalAlignment,
    pub line_spacing: f64,
    pub max_error: f64,
}

/// Aggregate ceilings for one multiline layout operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextBlockLayoutLimits {
    pub contours: TextContourLimits,
    pub max_lines: usize,
    pub max_runs: usize,
    pub max_run_metadata_bytes: usize,
    pub max_feature_inspections: usize,
    pub max_feature_applications: usize,
}

impl Default for TextBlockLayoutLimits {
    fn default() -> Self {
        Self {
            contours: TextContourLimits::default(),
            max_lines: 1024 * 1024,
            max_runs: 2 * 1024 * 1024,
            max_run_metadata_bytes: 128 * 1024 * 1024,
            max_feature_inspections: 16 * 1024 * 1024,
            max_feature_applications: 16 * 1024 * 1024,
        }
    }
}

/// Positioned block contours and bounded construction accounting.
#[derive(Clone, Debug)]
pub struct TextBlockLayoutOutput {
    pub contours: Vec<TextContour>,
    pub line_widths: Vec<f64>,
    pub width: f64,
    pub height: f64,
    pub glyphs: usize,
    pub outline_commands: usize,
    pub bezier_work_items: usize,
    pub peak_temporary_bezier_points: usize,
}

struct PendingRun {
    x: f64,
    output: TextContourOutput,
}

struct PendingLine {
    runs: Vec<PendingRun>,
    width: f64,
}

struct BlockMetrics {
    interline: f64,
    height: f64,
    vertical_offset: f64,
    width: f64,
    line_widths: Vec<f64>,
}

#[derive(Default)]
struct AggregateWork {
    runs: usize,
    run_metadata_bytes: usize,
    feature_inspections: usize,
    feature_applications: usize,
    glyphs: usize,
    outline_commands: usize,
    bezier_work_items: usize,
    contours: usize,
    points: usize,
    peak_temporary_bezier_points: usize,
}

/// Render plain multiline/tabbed text with one reusable shaping and hinted face.
pub fn layout_text_block_hinted_a0(
    font_bytes: &[u8],
    request: TextBlockLayoutRequest<'_>,
    limits: TextBlockLayoutLimits,
) -> Result<TextBlockLayoutOutput, TextContourError> {
    validate_request(request, limits)?;
    let line_spans = delimited_spans(&request.shaping.text, '\n', limits.max_lines, "$.lines")?;
    let mut work = AggregateWork::default();
    charge(
        &mut work.feature_inspections,
        request.shaping.features.len(),
        limits.max_feature_inspections,
        "$.features.inspections",
        "aggregate feature inspection limit exceeded",
    )?;
    let run_metadata_bytes = shaping_metadata_bytes(request.shaping)?;
    let session = HintedTextContourSession::new(font_bytes, request.shaping, limits.contours)?;
    let mut lines = Vec::with_capacity(line_spans.len().min(256));
    for line_span in line_spans {
        lines.push(build_line(
            &session,
            request,
            limits,
            line_span,
            run_metadata_bytes,
            &mut work,
        )?);
    }

    let metrics = block_metrics(request, &lines)?;
    place_lines(request, lines, work, metrics)
}

fn block_metrics(
    request: TextBlockLayoutRequest<'_>,
    lines: &[PendingLine],
) -> Result<BlockMetrics, TextContourError> {
    let interline = request.size_y * KICAD_TEXT_INTERLINE_FACTOR * request.line_spacing;
    let height = request.size_y * crate::KICAD_TEXT_HEIGHT_FUDGE_FACTOR
        + interline * lines.len().saturating_sub(1) as f64;
    let vertical_offset = match request.vertical_alignment {
        TextVerticalAlignment::Top => request.size_y,
        TextVerticalAlignment::Center => request.size_y - height / 2.0,
        TextVerticalAlignment::Bottom => request.size_y - height,
    };
    if !interline.is_finite() || !height.is_finite() || !vertical_offset.is_finite() {
        return Err(invalid(
            "$.line_spacing",
            "derived text block metrics are not finite",
        ));
    }
    let width = lines.iter().map(|line| line.width).fold(0.0_f64, f64::max);
    let line_widths = lines.iter().map(|line| line.width).collect::<Vec<_>>();
    Ok(BlockMetrics {
        interline,
        height,
        vertical_offset,
        width,
        line_widths,
    })
}

fn place_lines(
    request: TextBlockLayoutRequest<'_>,
    lines: Vec<PendingLine>,
    work: AggregateWork,
    metrics: BlockMetrics,
) -> Result<TextBlockLayoutOutput, TextContourError> {
    let single_request = single_line_request(request, request.shaping);
    let transform = validated_transform(single_request)?;
    let mut contours = Vec::with_capacity(work.contours.min(256));
    for (line_index, line) in lines.into_iter().enumerate() {
        let horizontal_offset = horizontal_offset(request.horizontal_alignment, line.width);
        let line_y =
            request.position_y + metrics.vertical_offset + line_index as f64 * metrics.interline;
        for mut run in line.runs {
            let translation = (request.position_x + horizontal_offset + run.x, line_y);
            if !translation.0.is_finite() || !translation.1.is_finite() {
                return Err(invalid(
                    "$.position",
                    "aligned text run position is not finite",
                ));
            }
            transform_contours(&mut run.output.contours, translation, transform)?;
            contours.extend(run.output.contours);
        }
    }
    Ok(TextBlockLayoutOutput {
        contours,
        line_widths: metrics.line_widths,
        width: metrics.width,
        height: metrics.height,
        glyphs: work.glyphs,
        outline_commands: work.outline_commands,
        bezier_work_items: work.bezier_work_items,
        peak_temporary_bezier_points: work.peak_temporary_bezier_points,
    })
}

fn horizontal_offset(alignment: TextHorizontalAlignment, width: f64) -> f64 {
    match alignment {
        TextHorizontalAlignment::Left => 0.0,
        TextHorizontalAlignment::Center => -width / 2.0,
        TextHorizontalAlignment::Right => -width,
    }
}

fn build_line(
    session: &HintedTextContourSession<'_>,
    request: TextBlockLayoutRequest<'_>,
    limits: TextBlockLayoutLimits,
    line_span: Range<usize>,
    run_metadata_bytes: usize,
    work: &mut AggregateWork,
) -> Result<PendingLine, TextContourError> {
    let text = &request.shaping.text;
    let run_spans = delimited_spans(&text[line_span.clone()], '\t', limits.max_runs, "$.runs")?;
    let tab_width = request.size_x * KICAD_TEXT_TAB_WIDTH_FACTOR;
    let mut line = PendingLine {
        runs: Vec::with_capacity(run_spans.len().min(32)),
        width: 0.0,
    };
    for (run_index, local_span) in run_spans.iter().enumerate() {
        charge(
            &mut work.runs,
            1,
            limits.max_runs,
            "$.runs",
            "text run limit exceeded",
        )?;
        let run_span = (line_span.start + local_span.start)..(line_span.start + local_span.end);
        charge(
            &mut work.run_metadata_bytes,
            run_metadata_bytes,
            limits.max_run_metadata_bytes,
            "$.runs.metadata",
            "aggregate text run metadata limit exceeded",
        )?;
        let run_input = shaping_run(request.shaping, run_span.clone(), limits, work)?;
        let run_limits = remaining_contour_limits(limits.contours, work);
        let output = session.shape_input(
            crate::TextContourRequest {
                shaping: &run_input,
                size_x: request.size_x,
                size_y: request.size_y,
                origin_x: 0.0,
                origin_y: 0.0,
                max_error: request.max_error,
            },
            run_limits,
        )?;
        charge_output(work, &output, limits.contours)?;
        let advance = output.advance_x;
        line.runs.push(PendingRun {
            x: line.width,
            output,
        });
        line.width += advance;
        if run_index + 1 < run_spans.len() {
            let intrusion = line.width.rem_euclid(tab_width);
            line.width += tab_width - intrusion;
        }
        if !line.width.is_finite() {
            return Err(invalid("$.runs", "derived tabbed line width is not finite"));
        }
    }
    Ok(line)
}

fn single_line_request<'a>(
    request: TextBlockLayoutRequest<'a>,
    shaping: &'a ShapingInput,
) -> TextLayoutRequest<'a> {
    TextLayoutRequest {
        shaping,
        size_x: request.size_x,
        size_y: request.size_y,
        position_x: request.position_x,
        position_y: request.position_y,
        angle_degrees: request.angle_degrees,
        mirrored: request.mirrored,
        horizontal_alignment: request.horizontal_alignment,
        vertical_alignment: request.vertical_alignment,
        max_error: request.max_error,
    }
}

fn validate_request(
    request: TextBlockLayoutRequest<'_>,
    limits: TextBlockLayoutLimits,
) -> Result<(), TextContourError> {
    if request.shaping.text.len() > limits.contours.shaping.max_text_bytes {
        return Err(resource("$.text", "text block byte limit exceeded"));
    }
    if !request.line_spacing.is_finite() || request.line_spacing <= 0.0 {
        return Err(invalid(
            "$.line_spacing",
            "line spacing must be positive and finite",
        ));
    }
    let tab_width = request.size_x * KICAD_TEXT_TAB_WIDTH_FACTOR;
    if !tab_width.is_finite() || tab_width <= 0.0 {
        return Err(invalid(
            "$.size_x",
            "derived tab width must be positive and finite",
        ));
    }
    Ok(())
}

fn delimited_spans(
    text: &str,
    delimiter: char,
    maximum: usize,
    path: &'static str,
) -> Result<Vec<Range<usize>>, TextContourError> {
    // Do not pre-scan the entire input merely to size this allocation. A small
    // caller limit must reject early even for a delimiter-heavy source.
    let mut spans = Vec::with_capacity(maximum.min(16));
    let mut start = 0usize;
    for (index, character) in text.char_indices() {
        if character == delimiter {
            if spans.len() >= maximum {
                return Err(resource(path, "text delimiter span limit exceeded"));
            }
            spans.push(start..index);
            start = index + character.len_utf8();
        }
    }
    if spans.len() >= maximum {
        return Err(resource(path, "text delimiter span limit exceeded"));
    }
    spans.push(start..text.len());
    Ok(spans)
}

fn shaping_run(
    base: &ShapingInput,
    span: Range<usize>,
    limits: TextBlockLayoutLimits,
    work: &mut AggregateWork,
) -> Result<ShapingInput, TextContourError> {
    Ok(ShapingInput {
        buffer_properties: base.buffer_properties.clone(),
        direction: base.direction,
        face_index: base.face_index,
        features: rebase_features(&base.features, span.clone(), limits, work)?,
        font_id: base.font_id.clone(),
        font_sha256: base.font_sha256.clone(),
        language: base.language.clone(),
        scale_x: base.scale_x,
        scale_y: base.scale_y,
        script: base.script.clone(),
        text: base.text[span].to_owned(),
        text_index_unit: base.text_index_unit.clone(),
        variations: base.variations.clone(),
    })
}

fn rebase_features(
    features: &[ShapingFeature],
    span: Range<usize>,
    limits: TextBlockLayoutLimits,
    work: &mut AggregateWork,
) -> Result<Vec<ShapingFeature>, TextContourError> {
    charge(
        &mut work.feature_inspections,
        features.len(),
        limits.max_feature_inspections,
        "$.features.inspections",
        "aggregate feature inspection limit exceeded",
    )?;
    let run_start = u32::try_from(span.start)
        .map_err(|_| resource("$.features", "text feature offset exceeds uint32"))?;
    let run_end = u32::try_from(span.end)
        .map_err(|_| resource("$.features", "text feature offset exceeds uint32"))?;
    let remaining = limits
        .max_feature_applications
        .saturating_sub(work.feature_applications);
    let mut rebased_features = Vec::with_capacity(features.len().min(remaining).min(16));
    for feature in features {
        if feature.end <= run_start || feature.start >= run_end {
            continue;
        }
        charge(
            &mut work.feature_applications,
            1,
            limits.max_feature_applications,
            "$.features.applications",
            "aggregate feature application limit exceeded",
        )?;
        let mut rebased = feature.clone();
        rebased.start = feature.start.max(run_start) - run_start;
        rebased.end = if feature.end == u32::MAX {
            u32::MAX
        } else {
            feature.end.min(run_end) - run_start
        };
        rebased_features.push(rebased);
    }
    Ok(rebased_features)
}

fn shaping_metadata_bytes(input: &ShapingInput) -> Result<usize, TextContourError> {
    let mut total = input.font_id.len().checked_add(input.font_sha256.0.len());
    total = total.and_then(|value| value.checked_add(input.text_index_unit.len()));
    total =
        total.and_then(|value| value.checked_add(input.language.as_deref().map_or(0, str::len)));
    total = total.and_then(|value| {
        value.checked_add(input.script.as_ref().map_or(0, |script| script.0.len()))
    });
    for feature in &input.features {
        total = total.and_then(|value| value.checked_add(feature.tag.0.len()));
    }
    for variation in &input.variations {
        total = total.and_then(|value| value.checked_add(variation.axis.0.len()));
    }
    total.ok_or_else(|| resource("$.runs.metadata", "text run metadata byte count overflowed"))
}

fn remaining_contour_limits(
    mut limits: TextContourLimits,
    work: &AggregateWork,
) -> TextContourLimits {
    limits.shaping.max_glyphs = limits.shaping.max_glyphs.saturating_sub(work.glyphs);
    limits.max_outline_commands = limits
        .max_outline_commands
        .saturating_sub(work.outline_commands);
    limits.max_bezier_work_items = limits
        .max_bezier_work_items
        .saturating_sub(work.bezier_work_items);
    limits.max_contours = limits.max_contours.saturating_sub(work.contours);
    limits.max_points = limits.max_points.saturating_sub(work.points);
    limits
}

fn charge_output(
    work: &mut AggregateWork,
    output: &TextContourOutput,
    limits: TextContourLimits,
) -> Result<(), TextContourError> {
    charge(
        &mut work.glyphs,
        output.glyphs,
        limits.shaping.max_glyphs,
        "$.glyphs",
        "aggregate shaped glyph limit exceeded",
    )?;
    charge(
        &mut work.outline_commands,
        output.outline_commands,
        limits.max_outline_commands,
        "$.outline_commands",
        "aggregate outline command limit exceeded",
    )?;
    charge(
        &mut work.bezier_work_items,
        output.bezier_work_items,
        limits.max_bezier_work_items,
        "$.bezier_work",
        "aggregate Bezier work limit exceeded",
    )?;
    charge(
        &mut work.contours,
        output.contours.len(),
        limits.max_contours,
        "$.contours",
        "aggregate contour limit exceeded",
    )?;
    let points = output
        .contours
        .iter()
        .try_fold(0usize, |total, contour| {
            total.checked_add(contour.points.len())
        })
        .ok_or_else(|| resource("$.points", "aggregate point count overflowed"))?;
    charge(
        &mut work.points,
        points,
        limits.max_points,
        "$.points",
        "aggregate point limit exceeded",
    )?;
    work.peak_temporary_bezier_points = work
        .peak_temporary_bezier_points
        .max(output.peak_temporary_bezier_points);
    Ok(())
}

fn charge(
    total: &mut usize,
    amount: usize,
    maximum: usize,
    path: &'static str,
    message: &'static str,
) -> Result<(), TextContourError> {
    let next = total
        .checked_add(amount)
        .filter(|next| *next <= maximum)
        .ok_or_else(|| resource(path, message))?;
    *total = next;
    Ok(())
}

fn invalid(path: &'static str, message: &'static str) -> TextContourError {
    TextContourError {
        kind: TextContourErrorKind::InvalidInput,
        path: path.to_owned(),
        message,
    }
}

fn resource(path: &'static str, message: &'static str) -> TextContourError {
    TextContourError {
        kind: TextContourErrorKind::ResourceLimit,
        path: path.to_owned(),
        message,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kicad_monkey_contracts::generated::shaping_record::OpenTypeTag;

    fn feature(tag: &str, start: u32, end: u32) -> ShapingFeature {
        ShapingFeature {
            end,
            start,
            tag: OpenTypeTag(tag.to_owned()),
            value: 0,
        }
    }

    #[test]
    fn utf8_and_global_feature_ranges_rebase_in_source_order_under_exact_work() {
        let features = vec![
            feature("kern", 0, u32::MAX),
            feature("liga", 0, 2),
            feature("calt", 3, 4),
        ];
        let limits = TextBlockLayoutLimits {
            max_feature_inspections: 6,
            max_feature_applications: 4,
            ..TextBlockLayoutLimits::default()
        };
        let mut work = AggregateWork::default();
        let first = rebase_features(&features, 0..2, limits, &mut work).unwrap();
        let second = rebase_features(&features, 3..4, limits, &mut work).unwrap();

        assert_eq!(
            first
                .iter()
                .map(|item| (item.tag.0.as_str(), item.start, item.end))
                .collect::<Vec<_>>(),
            [("kern", 0, u32::MAX), ("liga", 0, 2)]
        );
        assert_eq!(
            second
                .iter()
                .map(|item| (item.tag.0.as_str(), item.start, item.end))
                .collect::<Vec<_>>(),
            [("kern", 0, u32::MAX), ("calt", 0, 1)]
        );
        assert_eq!(work.feature_inspections, 6);
        assert_eq!(work.feature_applications, 4);
    }

    #[test]
    fn feature_inspection_and_application_work_fail_one_under() {
        let features = vec![
            feature("kern", 0, u32::MAX),
            feature("liga", 0, 2),
            feature("calt", 3, 4),
        ];
        let mut inspection_work = AggregateWork::default();
        let inspection_limits = TextBlockLayoutLimits {
            max_feature_inspections: 5,
            max_feature_applications: 4,
            ..TextBlockLayoutLimits::default()
        };
        rebase_features(&features, 0..2, inspection_limits, &mut inspection_work).unwrap();
        let error =
            rebase_features(&features, 3..4, inspection_limits, &mut inspection_work).unwrap_err();
        assert_eq!(error.kind, TextContourErrorKind::ResourceLimit);
        assert_eq!(error.path, "$.features.inspections");

        let mut application_work = AggregateWork::default();
        let application_limits = TextBlockLayoutLimits {
            max_feature_inspections: 6,
            max_feature_applications: 3,
            ..TextBlockLayoutLimits::default()
        };
        rebase_features(&features, 0..2, application_limits, &mut application_work).unwrap();
        let error = rebase_features(&features, 3..4, application_limits, &mut application_work)
            .unwrap_err();
        assert_eq!(error.kind, TextContourErrorKind::ResourceLimit);
        assert_eq!(error.path, "$.features.applications");
    }
}
