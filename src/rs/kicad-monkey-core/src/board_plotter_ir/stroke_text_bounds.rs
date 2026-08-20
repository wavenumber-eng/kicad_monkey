//! Bounded Newstroke text extents used by PCB review viewports.

use super::stroke_font_widths::{
    NEWSTROKE_GLYPH_DATA, NEWSTROKE_GLYPH_OFFSETS, NEWSTROKE_WIDTH_UNITS,
};
use super::{
    BoardDimensionOperation, BoardFootprintOperation, BoardPlotDocument, BoardPlotRecord,
    BoardTableOperation, BoardTextBoxOperation, BoardTextHAlign, BoardTextOperation,
    BoardTextVAlign,
};
use crate::pcb::PcbView;
use crate::plotter_ir::mm_to_nm;
use crate::plotter_text_cache::{
    PlotterTextCacheResources, PlotterTextCacheSession, PlotterTextLayout,
};
use crate::sexpr::{Error, ErrorKind, ErrorPhase, Position};
use crate::text_markup::{TextMarkupMarker, TextMarkupNode, parse_text_markup};
use crate::{PlotterOperation, PlotterText};
use crate::{TextContourErrorKind, TextHorizontalAlignment, TextVerticalAlignment};

const STROKE_SCALE: f64 = 1.0 / 21.0;
const FONT_OFFSET: f64 = -8.0;
const ITALIC_TILT: f64 = 1.0 / 8.0;
const SUPER_SUB_SIZE_MULTIPLIER: f64 = 0.8;
const SUPER_HEIGHT_OFFSET: f64 = 0.35;
const SUB_HEIGHT_OFFSET: f64 = 0.15;
const OVERBAR_POSITION_FACTOR: f64 = 1.23;
const OVERBAR_TRIM_RATIO: f64 = 0.1;

/// Independent ceilings for the board text extent pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoardBoundsLimits {
    pub max_footprints: usize,
    pub max_images: usize,
    pub max_image_encoded_bytes: usize,
    pub max_image_decoded_bytes: usize,
    pub max_image_decode_work: usize,
    pub max_image_pixels: usize,
    pub max_operations: usize,
    pub max_geometry_points: usize,
    pub max_text_bytes: usize,
    pub max_markup_nodes: usize,
    pub max_glyph_points: usize,
}

impl Default for BoardBoundsLimits {
    fn default() -> Self {
        Self {
            max_footprints: 1_000_000,
            max_images: 100_000,
            max_image_encoded_bytes: 64 * 1024 * 1024,
            max_image_decoded_bytes: 64 * 1024 * 1024,
            max_image_decode_work: 256 * 1024 * 1024,
            max_image_pixels: 100_000_000,
            max_operations: 4_000_000,
            max_geometry_points: 16_000_000,
            max_text_bytes: 256 * 1024 * 1024,
            max_markup_nodes: 1_000_000,
            max_glyph_points: 16_000_000,
        }
    }
}

/// Exact nanometre bounds of Newstroke text families considered by Python's
/// all-layer PCB SVG viewport authority.
pub(super) fn board_bounds(
    document: &BoardPlotDocument,
    view: &PcbView<'_>,
    outline_fonts: Option<&PlotterTextCacheResources<'_>>,
    limits: BoardBoundsLimits,
) -> Result<Option<[i64; 4]>, Error> {
    let mut budget = BoundsBudget {
        points: 0,
        maximum_points: limits.max_glyph_points,
        operations: 0,
        maximum_operations: limits.max_operations,
        geometry_points: 0,
        maximum_geometry_points: limits.max_geometry_points,
        text_bytes: 0,
        maximum_text_bytes: limits.max_text_bytes,
    };
    let mut bounds = None;
    include_document_geometry(document, &mut budget, &mut bounds)?;
    include_zone_source_geometry(view, &mut budget, &mut bounds)?;
    include_board_images(view, limits, &mut budget, &mut bounds)?;
    let outline_session = outline_fonts
        .map(PlotterTextCacheSession::new)
        .transpose()?;
    if let Some(session) = &outline_session {
        include_document_outline_text(document, session, &mut budget, &mut bounds)?;
    } else if document_requires_outline(document)? {
        return Err(missing_outline_resources());
    }
    include_footprint_properties(
        view,
        outline_session.as_ref(),
        limits,
        &mut budget,
        &mut bounds,
    )?;
    Ok(bounds)
}

fn include_board_images(
    view: &PcbView<'_>,
    limits: BoardBoundsLimits,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    if view.counts().images > limits.max_images {
        return Err(resource_limit_error());
    }
    let mut count = 0_usize;
    let mut encoded_bytes = 0_usize;
    let mut decoded_bytes = 0_usize;
    let mut decode_work = 0_usize;
    let mut pixels = 0_usize;
    for image in view.images() {
        let image = image?;
        count = count.checked_add(1).ok_or_else(resource_limit_error)?;
        if count > limits.max_images {
            return Err(resource_limit_error());
        }
        encoded_bytes = encoded_bytes
            .checked_add(image.encoded_data_bytes)
            .filter(|value| *value <= limits.max_image_encoded_bytes)
            .ok_or_else(resource_limit_error)?;
        let encoded = view.image_data(
            &image,
            limits
                .max_image_encoded_bytes
                .saturating_sub(encoded_bytes - image.encoded_data_bytes),
        )?;
        decode_work = decode_work
            .checked_add(encoded.len())
            .filter(|value| *value <= limits.max_image_decode_work)
            .ok_or_else(resource_limit_error)?;
        let Some(decoded) = decode_board_base64(
            &encoded,
            limits.max_image_decoded_bytes.saturating_sub(decoded_bytes),
        )?
        else {
            continue;
        };
        decoded_bytes = decoded_bytes
            .checked_add(decoded.len())
            .filter(|value| *value <= limits.max_image_decoded_bytes)
            .ok_or_else(resource_limit_error)?;
        decode_work = decode_work
            .checked_add(decoded.len())
            .filter(|value| *value <= limits.max_image_decode_work)
            .ok_or_else(resource_limit_error)?;
        let (dimensions, metadata_work) = board_image_dimensions(
            &decoded,
            limits.max_image_decode_work.saturating_sub(decode_work),
        )?;
        decode_work = decode_work
            .checked_add(metadata_work)
            .filter(|value| *value <= limits.max_image_decode_work)
            .ok_or_else(resource_limit_error)?;
        let Some((width_px, height_px)) = dimensions else {
            continue;
        };
        let image_pixels = (width_px as usize)
            .checked_mul(height_px as usize)
            .ok_or_else(resource_limit_error)?;
        pixels = pixels
            .checked_add(image_pixels)
            .filter(|value| *value <= limits.max_image_pixels)
            .ok_or_else(resource_limit_error)?;
        let center_x = mm_to_nm(image.at.x)?;
        let center_y = mm_to_nm(image.at.y)?;
        let scale = if image.scale == 0.0 {
            1.0
        } else {
            image.scale.abs()
        };
        let width = rounded_i64(width_px as f64 * 100_000.0 * scale)?;
        let height = rounded_i64(height_px as f64 * 100_000.0 * scale)?;
        let min_x = center_x
            .checked_sub(width / 2)
            .ok_or_else(resource_limit_error)?;
        let min_y = center_y
            .checked_sub(height / 2)
            .ok_or_else(resource_limit_error)?;
        let max_x = center_x
            .checked_add(width / 2)
            .ok_or_else(resource_limit_error)?;
        let max_y = center_y
            .checked_add(height / 2)
            .ok_or_else(resource_limit_error)?;
        include_geometry_point(min_x, min_y, ParentTransform::default(), budget, bounds)?;
        include_geometry_point(max_x, max_y, ParentTransform::default(), budget, bounds)?;
    }
    Ok(())
}

fn decode_board_base64(value: &str, maximum: usize) -> Result<Option<Vec<u8>>, Error> {
    let mut output = Vec::new();
    let mut block = [0_u8; 4];
    let mut block_len = 0_usize;
    for byte in value.bytes() {
        let decoded = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => 64,
            _ => continue,
        };
        block[block_len] = decoded;
        block_len += 1;
        if block_len != 4 {
            continue;
        }
        if block[0] == 64 || block[1] == 64 || (block[2] == 64 && block[3] != 64) {
            return Ok(None);
        }
        push_image_byte(&mut output, (block[0] << 2) | (block[1] >> 4), maximum)?;
        if block[2] != 64 {
            push_image_byte(&mut output, (block[1] << 4) | (block[2] >> 2), maximum)?;
        }
        if block[3] != 64 {
            push_image_byte(&mut output, (block[2] << 6) | block[3], maximum)?;
        }
        block_len = 0;
    }
    Ok((block_len == 0).then_some(output))
}

fn push_image_byte(output: &mut Vec<u8>, byte: u8, maximum: usize) -> Result<(), Error> {
    if output.len() == maximum {
        return Err(resource_limit_error());
    }
    output.push(byte);
    Ok(())
}

fn board_image_dimensions(
    data: &[u8],
    maximum_work: usize,
) -> Result<(Option<(u32, u32)>, usize), Error> {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        if data.len() < 24 {
            if data.len() > maximum_work {
                return Err(resource_limit_error());
            }
            return Ok((None, data.len()));
        }
        if maximum_work < 24 {
            return Err(resource_limit_error());
        }
        let width = u32::from_be_bytes(data[16..20].try_into().expect("PNG width slice"));
        let height = u32::from_be_bytes(data[20..24].try_into().expect("PNG height slice"));
        return Ok(((width != 0 && height != 0).then_some((width, height)), 24));
    }
    if !data.starts_with(b"\xff\xd8") {
        let work = data.len().min(8);
        if work > maximum_work {
            return Err(resource_limit_error());
        }
        return Ok((None, work));
    }
    if maximum_work < 2 {
        return Err(resource_limit_error());
    }
    let mut index = 2_usize;
    let mut work = 2_usize;
    while index + 10 < data.len() {
        if work >= maximum_work {
            return Err(resource_limit_error());
        }
        if data[index] != 0xff {
            index += 1;
            work += 1;
            continue;
        }
        let marker = data[index + 1];
        if marker == 0xd8 {
            index += 2;
            work += 2;
            continue;
        }
        if marker == 0xd9 {
            break;
        }
        if matches!(marker, 0x00 | 0xff) {
            index += 1;
            work += 1;
            continue;
        }
        if index + 4 > data.len() {
            break;
        }
        let length = usize::from(u16::from_be_bytes([data[index + 2], data[index + 3]]));
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) && index + 9 <= data.len()
        {
            let height = u32::from(u16::from_be_bytes([data[index + 5], data[index + 6]]));
            let width = u32::from(u16::from_be_bytes([data[index + 7], data[index + 8]]));
            let work = work.checked_add(9).ok_or_else(resource_limit_error)?;
            if work > maximum_work {
                return Err(resource_limit_error());
            }
            return Ok(((width != 0 && height != 0).then_some((width, height)), work));
        }
        let advance = 2_usize
            .checked_add(length)
            .ok_or_else(resource_limit_error)?;
        index = index
            .checked_add(advance)
            .ok_or_else(resource_limit_error)?;
        work = work.checked_add(advance).ok_or_else(resource_limit_error)?;
        if work > maximum_work {
            return Err(resource_limit_error());
        }
    }
    Ok((None, work.min(maximum_work)))
}

fn document_requires_outline(document: &BoardPlotDocument) -> Result<bool, Error> {
    let mut required = false;
    for_each_text_operation(document, |operation, _| {
        required |= !operation.text.is_empty() && operation.render_cache.is_none();
        Ok(())
    })?;
    Ok(required)
}

fn include_footprint_properties(
    view: &PcbView<'_>,
    outline_session: Option<&PlotterTextCacheSession<'_>>,
    limits: BoardBoundsLimits,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    let footprints = view
        .footprints()
        .take(limits.max_footprints.saturating_add(1))
        .collect::<Result<Vec<_>, _>>()?;
    if footprints.len() > limits.max_footprints {
        return Err(resource_limit_error());
    }
    for property in view.footprint_properties() {
        let property = property?;
        if property.hidden
            || property.value.is_empty()
            || !property.graphical
            || property.name.starts_with("ki_")
        {
            continue;
        }
        let footprint = footprints.get(property.footprint_index).ok_or_else(|| {
            Error::at(
                ErrorPhase::Tree,
                ErrorKind::InvalidBuildValue,
                "Footprint property owner is missing",
                Position::START,
            )
        })?;
        let (h_align, v_align, mirror) = property.effects.justify.iter().fold(
            (BoardTextHAlign::Center, BoardTextVAlign::Center, false),
            |(mut horizontal, mut vertical, mut mirror), value| {
                match value.as_str() {
                    "left" => horizontal = BoardTextHAlign::Left,
                    "right" => horizontal = BoardTextHAlign::Right,
                    "top" => vertical = BoardTextVAlign::Top,
                    "bottom" => vertical = BoardTextVAlign::Bottom,
                    "mirror" => mirror = true,
                    _ => {}
                }
                (horizontal, vertical, mirror)
            },
        );
        let operation = BoardTextOperation {
            x: mm_to_nm(property.at.x)?,
            y: mm_to_nm(property.at.y)?,
            text: property.value,
            color: String::new(),
            orient_deg: property.angle,
            size_x_nm: mm_to_nm(property.effects.font.size_x)?,
            size_y_nm: mm_to_nm(property.effects.font.size_y)?,
            h_align,
            v_align,
            pen_width_nm: mm_to_nm(property.effects.font.thickness.unwrap_or(0.127))?,
            italic: property.effects.font.italic,
            bold: property.effects.font.bold,
            multiline: false,
            font_face: property.effects.font.face.unwrap_or_default(),
            layer: Some(property.layer),
            mirror,
            text_as_polygons: false,
            polyline_per_segment: false,
            knockout: false,
            render_cache_polygons: Vec::new(),
            render_cache: None,
        };
        let parent = ParentTransform {
            x: footprint.at_x.unwrap_or_default() * 1_000_000.0,
            y: footprint.at_y.unwrap_or_default() * 1_000_000.0,
            angle: -footprint.angle.unwrap_or_default(),
        };
        if operation.font_face.is_empty() {
            include_text(&operation, parent, limits, budget, bounds)?;
        } else {
            let session = outline_session.ok_or_else(missing_outline_resources)?;
            include_outline_text(&operation, session, parent, budget, bounds)?;
        }
    }
    Ok(())
}

fn missing_outline_resources() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::InvalidBuildValue,
        "Board bounds require outline-font resources",
        Position::START,
    )
}

fn include_zone_source_geometry(
    view: &PcbView<'_>,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    const STANDARD_COPPER: [&str; 3] = ["F.Cu", "B.Cu", "In1.Cu"];
    for zone in view.zones() {
        let zone = zone?;
        if !zone.filled_polygons.is_empty()
            && zone
                .layers
                .first()
                .is_some_and(|layer| STANDARD_COPPER.contains(&layer.as_str()))
        {
            for polygon in &zone.polygons {
                budget.operation()?;
                for point in &polygon.points {
                    include_geometry_point(
                        mm_to_nm(point.x)?,
                        mm_to_nm(point.y)?,
                        ParentTransform::default(),
                        budget,
                        bounds,
                    )?;
                }
            }
        }
        for polygon in &zone.filled_polygons {
            budget.operation()?;
            for point in &polygon.points {
                include_geometry_point(
                    mm_to_nm(point.x)?,
                    mm_to_nm(point.y)?,
                    ParentTransform::default(),
                    budget,
                    bounds,
                )?;
            }
        }
    }
    Ok(())
}

fn include_document_outline_text(
    document: &BoardPlotDocument,
    session: &PlotterTextCacheSession<'_>,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    for_each_text_operation(document, |operation, parent| {
        include_outline_text(operation, session, parent, budget, bounds)
    })
}

fn for_each_text_operation(
    document: &BoardPlotDocument,
    mut apply: impl FnMut(&BoardTextOperation, ParentTransform) -> Result<(), Error>,
) -> Result<(), Error> {
    for record in &document.records {
        match record {
            BoardPlotRecord::Text(record) => {
                for operation in &record.operations {
                    apply(operation, ParentTransform::default())?;
                }
            }
            BoardPlotRecord::Dimension(record) => {
                for operation in &record.operations {
                    if let BoardDimensionOperation::Text(operation) = operation {
                        apply(operation, ParentTransform::default())?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn include_document_geometry(
    document: &BoardPlotDocument,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    for record in &document.records {
        match record {
            BoardPlotRecord::Graphic(record) => {
                include_operations(
                    &record.operations,
                    ParentTransform::default(),
                    budget,
                    bounds,
                )?;
            }
            BoardPlotRecord::Text(record) => {
                for operation in &record.operations {
                    include_text_cache(operation, ParentTransform::default(), budget, bounds)?;
                }
            }
            BoardPlotRecord::TextBox(record) => {
                include_text_box_geometry(&record.operations, budget, bounds)?;
            }
            BoardPlotRecord::Segment(record) => {
                include_operations(
                    &record.operations,
                    ParentTransform::default(),
                    budget,
                    bounds,
                )?;
            }
            BoardPlotRecord::TrackArc(record) => {
                include_operations(
                    &record.operations,
                    ParentTransform::default(),
                    budget,
                    bounds,
                )?;
            }
            BoardPlotRecord::Via(record) => {
                for operation in &record.operations {
                    budget.operation()?;
                    include_circle_bounds(
                        operation.x,
                        operation.y,
                        operation.diameter_nm,
                        ParentTransform::default(),
                        budget,
                        bounds,
                    )?;
                }
            }
            BoardPlotRecord::Table(record) => {
                for [min_x, min_y, max_x, max_y] in &record.cell_bounds_nm {
                    include_geometry_point(
                        *min_x,
                        *min_y,
                        ParentTransform::default(),
                        budget,
                        bounds,
                    )?;
                    include_geometry_point(
                        *max_x,
                        *max_y,
                        ParentTransform::default(),
                        budget,
                        bounds,
                    )?;
                }
                include_table_geometry(&record.operations, budget, bounds)?;
            }
            BoardPlotRecord::Dimension(record) => {
                include_dimension_geometry(&record.operations, budget, bounds)?;
            }
            BoardPlotRecord::Zone(record) => {
                include_operations(
                    &record.operations,
                    ParentTransform::default(),
                    budget,
                    bounds,
                )?;
            }
            BoardPlotRecord::Footprint(record) => {
                let parent = ParentTransform {
                    x: record.placement.x_nm as f64,
                    y: record.placement.y_nm as f64,
                    angle: -record.placement.angle_deg,
                };
                include_footprint_geometry(&record.operations, parent, budget, bounds)?;
            }
        }
    }
    Ok(())
}

fn include_text_box_geometry(
    operations: &[BoardTextBoxOperation],
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    for operation in operations {
        match operation {
            BoardTextBoxOperation::Border(operation) => {
                include_operation(operation, ParentTransform::default(), budget, bounds)?
            }
            BoardTextBoxOperation::Text(_) => {}
        }
    }
    Ok(())
}

fn include_table_geometry(
    operations: &[BoardTableOperation],
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    for operation in operations {
        match operation {
            BoardTableOperation::Segment(operation) => {
                include_operation(operation, ParentTransform::default(), budget, bounds)?
            }
            BoardTableOperation::Text(_) => {}
        }
    }
    Ok(())
}

fn include_dimension_geometry(
    operations: &[BoardDimensionOperation],
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    for operation in operations {
        match operation {
            // Python's viewport authority includes dimension text only.
            BoardDimensionOperation::Geometry(_) => {}
            BoardDimensionOperation::Text(operation) => {
                include_text_cache(operation, ParentTransform::default(), budget, bounds)?
            }
        }
    }
    Ok(())
}

fn include_footprint_geometry(
    operations: &[BoardFootprintOperation],
    parent: ParentTransform,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    for operation in operations {
        match operation {
            BoardFootprintOperation::Geometry {
                operation,
                metadata,
            } if metadata.data_ref != "fp_text_box" => {
                include_operation(operation, parent, budget, bounds)?;
            }
            BoardFootprintOperation::Pad(operation) => {
                include_footprint_pad(operation, parent, budget, bounds)?;
            }
            BoardFootprintOperation::Geometry { .. } | BoardFootprintOperation::Text { .. } => {}
            BoardFootprintOperation::StartBlock(_) | BoardFootprintOperation::EndBlock => {}
        }
    }
    Ok(())
}

fn include_footprint_pad(
    operation: &PlotterOperation,
    parent: ParentTransform,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    budget.operation()?;
    match operation {
        PlotterOperation::FlashPadCircle(value) => {
            include_circle_bounds(value.x, value.y, value.diameter_nm, parent, budget, bounds)
        }
        PlotterOperation::FlashPadOval(value) => include_pad_square(
            value.x,
            value.y,
            value.size_x_nm,
            value.size_y_nm,
            parent,
            budget,
            bounds,
        ),
        PlotterOperation::FlashPadRect(value) => include_pad_square(
            value.x,
            value.y,
            value.size_x_nm,
            value.size_y_nm,
            parent,
            budget,
            bounds,
        ),
        PlotterOperation::FlashPadRoundRect(value) => include_pad_square(
            value.x,
            value.y,
            value.size_x_nm,
            value.size_y_nm,
            parent,
            budget,
            bounds,
        ),
        PlotterOperation::FlashPadCustom(value) => include_pad_square(
            value.x,
            value.y,
            value.size_x_nm,
            value.size_y_nm,
            parent,
            budget,
            bounds,
        ),
        PlotterOperation::FlashPadTrapez(value) => include_pad_square(
            value.x,
            value.y,
            value.size_x_nm,
            value.size_y_nm,
            parent,
            budget,
            bounds,
        ),
        PlotterOperation::Circle(value) if value.role.is_some() => Ok(()),
        PlotterOperation::ThickSegment(value) if value.role.is_some() => Ok(()),
        _ => include_operation(operation, parent, budget, bounds),
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "one pad center, two source dimensions, and the bounded sink"
)]
fn include_pad_square(
    x: i64,
    y: i64,
    width: i64,
    height: i64,
    parent: ParentTransform,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    let radius = width.abs().max(height.abs()) / 2;
    include_geometry_point(x - radius, y - radius, parent, budget, bounds)?;
    include_geometry_point(x + radius, y + radius, parent, budget, bounds)
}

fn include_operations(
    operations: &[PlotterOperation],
    parent: ParentTransform,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    for operation in operations {
        include_operation(operation, parent, budget, bounds)?;
    }
    Ok(())
}

fn include_operation(
    operation: &PlotterOperation,
    parent: ParentTransform,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    budget.operation()?;
    match operation {
        PlotterOperation::ThickSegment(value) => {
            for (x, y) in [(value.start_x, value.start_y), (value.end_x, value.end_y)] {
                include_geometry_point(x, y, parent, budget, bounds)?;
            }
        }
        PlotterOperation::ArcThreePoint(value) => {
            for (x, y) in [
                (value.start_x, value.start_y),
                (value.mid_x, value.mid_y),
                (value.end_x, value.end_y),
            ] {
                include_geometry_point(x, y, parent, budget, bounds)?;
            }
        }
        PlotterOperation::Circle(value) => include_circle_bounds(
            value.cx,
            value.cy,
            value.diameter_nm,
            parent,
            budget,
            bounds,
        )?,
        PlotterOperation::Rect(value) => {
            for (x, y) in [(value.x1, value.y1), (value.x2, value.y2)] {
                include_geometry_point(x, y, parent, budget, bounds)?;
            }
        }
        PlotterOperation::PlotPoly(value) => {
            for [x, y] in &value.points {
                include_geometry_point(*x, *y, parent, budget, bounds)?;
            }
        }
        PlotterOperation::BezierCurve(value) => {
            include_cubic_bezier_bounds(value, parent, budget, bounds)?;
        }
        PlotterOperation::Text(value) => include_plotter_text(value, parent, budget, bounds)?,
        PlotterOperation::FlashPadCircle(value) => {
            include_circle_bounds(value.x, value.y, value.diameter_nm, parent, budget, bounds)?
        }
        PlotterOperation::FlashPadOval(value) => include_rotated_rect(
            value.x,
            value.y,
            value.size_x_nm,
            value.size_y_nm,
            value.orient_deg,
            parent,
            budget,
            bounds,
        )?,
        PlotterOperation::FlashPadRect(value) => include_rotated_rect(
            value.x,
            value.y,
            value.size_x_nm,
            value.size_y_nm,
            value.orient_deg,
            parent,
            budget,
            bounds,
        )?,
        PlotterOperation::FlashPadRoundRect(value) => include_rotated_rect(
            value.x,
            value.y,
            value.size_x_nm,
            value.size_y_nm,
            value.orient_deg,
            parent,
            budget,
            bounds,
        )?,
        PlotterOperation::FlashPadCustom(value) => {
            include_custom_pad(value, parent, budget, bounds)?;
        }
        PlotterOperation::FlashPadTrapez(value) => {
            include_trapezoid_pad(value, parent, budget, bounds)?;
        }
    }
    Ok(())
}

fn include_custom_pad(
    value: &crate::plotter_ir::FlashPadCustom,
    parent: ParentTransform,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    include_rotated_rect(
        value.x,
        value.y,
        value.size_x_nm,
        value.size_y_nm,
        value.orient_deg,
        parent,
        budget,
        bounds,
    )?;
    for polygon in &value.polygons {
        for [x, y] in polygon {
            let (x, y) = rotate(*x as f64, *y as f64, -value.orient_deg);
            include_geometry_point(
                rounded_i64(x + value.x as f64)?,
                rounded_i64(y + value.y as f64)?,
                parent,
                budget,
                bounds,
            )?;
        }
    }
    Ok(())
}

fn include_trapezoid_pad(
    value: &crate::plotter_ir::FlashPadTrapez,
    parent: ParentTransform,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    for [x, y] in value.corners {
        let (x, y) = rotate(x as f64, y as f64, -value.orient_deg);
        include_geometry_point(
            rounded_i64(x + value.x as f64)?,
            rounded_i64(y + value.y as f64)?,
            parent,
            budget,
            bounds,
        )?;
    }
    Ok(())
}

fn include_circle_bounds(
    x: i64,
    y: i64,
    diameter: i64,
    parent: ParentTransform,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    let radius = diameter.abs() / 2;
    include_geometry_point(x - radius, y - radius, parent, budget, bounds)?;
    include_geometry_point(x + radius, y + radius, parent, budget, bounds)
}

fn include_cubic_bezier_bounds(
    value: &crate::plotter_ir::BezierCurve,
    parent: ParentTransform,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    let transform = |x: i64, y: i64| {
        let (x, y) = rotate(x as f64, y as f64, parent.angle);
        (x + parent.x, y + parent.y)
    };
    let p0 = transform(value.start_x, value.start_y);
    let p1 = transform(value.ctrl1_x, value.ctrl1_y);
    let p2 = transform(value.ctrl2_x, value.ctrl2_y);
    let p3 = transform(value.end_x, value.end_y);
    let (min_x, max_x) = cubic_axis_bounds(p0.0, p1.0, p2.0, p3.0);
    let (min_y, max_y) = cubic_axis_bounds(p0.1, p1.1, p2.1, p3.1);
    include_geometry_point(
        rounded_i64(min_x)?,
        rounded_i64(min_y)?,
        ParentTransform::default(),
        budget,
        bounds,
    )?;
    include_geometry_point(
        rounded_i64(max_x)?,
        rounded_i64(max_y)?,
        ParentTransform::default(),
        budget,
        bounds,
    )
}

fn cubic_axis_bounds(v0: f64, v1: f64, v2: f64, v3: f64) -> (f64, f64) {
    let mut minimum = v0.min(v3);
    let mut maximum = v0.max(v3);
    let a = 3.0 * (-v0 + 3.0 * v1 - 3.0 * v2 + v3);
    let b = 6.0 * (v0 - 2.0 * v1 + v2);
    let c = 3.0 * (v1 - v0);
    let mut candidates = [None, None];
    if a.abs() < 1.0e-10 {
        if b.abs() > 1.0e-10 {
            candidates[0] = Some(-c / b);
        }
    } else {
        let discriminant = b * b - 4.0 * a * c;
        if discriminant >= 0.0 {
            let root = discriminant.sqrt();
            candidates = [Some((-b + root) / (2.0 * a)), Some((-b - root) / (2.0 * a))];
        }
    }
    for t in candidates
        .into_iter()
        .flatten()
        .filter(|t| *t > 0.0 && *t < 1.0)
    {
        let inverse = 1.0 - t;
        let value = inverse.powi(3) * v0
            + 3.0 * inverse.powi(2) * t * v1
            + 3.0 * inverse * t.powi(2) * v2
            + t.powi(3) * v3;
        minimum = minimum.min(value);
        maximum = maximum.max(value);
    }
    (minimum, maximum)
}

#[allow(
    clippy::too_many_arguments,
    reason = "one rotated box and its bounded sink"
)]
fn include_rotated_rect(
    x: i64,
    y: i64,
    width: i64,
    height: i64,
    angle: f64,
    parent: ParentTransform,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    for local_x in [-width as f64 / 2.0, width as f64 / 2.0] {
        for local_y in [-height as f64 / 2.0, height as f64 / 2.0] {
            let (local_x, local_y) = rotate(local_x, local_y, -angle);
            include_geometry_point(
                rounded_i64(local_x + x as f64)?,
                rounded_i64(local_y + y as f64)?,
                parent,
                budget,
                bounds,
            )?;
        }
    }
    Ok(())
}

fn include_plotter_text(
    value: &PlotterText,
    parent: ParentTransform,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    budget.text(&value.text)?;
    include_rotated_rect(
        value.x,
        value.y,
        value
            .size_x_nm
            .saturating_mul(value.text.chars().count() as i64),
        value.size_y_nm,
        value.orient_deg,
        parent,
        budget,
        bounds,
    )
}

fn include_text_cache(
    operation: &BoardTextOperation,
    parent: ParentTransform,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    budget.operation()?;
    for polygon in &operation.render_cache_polygons {
        for [x, y] in polygon {
            include_geometry_point(*x, *y, parent, budget, bounds)?;
        }
    }
    if let Some(cache) = &operation.render_cache {
        let cache_parent =
            if cache.coordinate_space == super::BoardTextRenderCacheCoordinateSpace::Board {
                ParentTransform::default()
            } else {
                parent
            };
        for polygon in &cache.polygons {
            for contour in polygon {
                for [x, y] in contour {
                    include_geometry_point(*x, *y, cache_parent, budget, bounds)?;
                }
            }
        }
    }
    Ok(())
}

fn include_geometry_point(
    x: i64,
    y: i64,
    parent: ParentTransform,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    budget.geometry_point()?;
    let (x, y) = rotate(x as f64, y as f64, parent.angle);
    let x = rounded_i64(x + parent.x)?;
    let y = rounded_i64(y + parent.y)?;
    match bounds {
        Some(value) => {
            value[0] = value[0].min(x);
            value[1] = value[1].min(y);
            value[2] = value[2].max(x);
            value[3] = value[3].max(y);
        }
        None => *bounds = Some([x, y, x, y]),
    }
    Ok(())
}

fn include_outline_text(
    operation: &BoardTextOperation,
    resources: &PlotterTextCacheSession<'_>,
    parent: ParentTransform,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    if operation.text.is_empty() || operation.render_cache.is_some() {
        return Ok(());
    }
    budget.text(&operation.text)?;
    let face = if operation.font_face.is_empty() {
        "Arial"
    } else {
        &operation.font_face
    };
    let layout = PlotterTextLayout {
        text: &operation.text,
        face,
        bold: operation.bold,
        italic: operation.italic,
        size_x: operation.size_x_nm as f64 / 1_000_000.0,
        size_y: operation.size_y_nm as f64 / 1_000_000.0,
        position_x: 0.0,
        position_y: 0.0,
        angle_degrees: operation.orient_deg - parent.angle,
        mirrored: operation.mirror,
        horizontal_alignment: match operation.h_align {
            BoardTextHAlign::Left => TextHorizontalAlignment::Left,
            BoardTextHAlign::Center => TextHorizontalAlignment::Center,
            BoardTextHAlign::Right => TextHorizontalAlignment::Right,
        },
        vertical_alignment: match operation.v_align {
            BoardTextVAlign::Top => TextVerticalAlignment::Top,
            BoardTextVAlign::Center => TextVerticalAlignment::Center,
            BoardTextVAlign::Bottom => TextVerticalAlignment::Bottom,
        },
        line_spacing: 1.0,
        stroke_width: operation.pen_width_nm as f64 / 1_000_000.0,
    };
    let remaining = budget.maximum_points.saturating_sub(budget.points);
    let generated =
        resources.generate_single_line_unhinted(layout, remaining, remaining, remaining)?;
    for polygon in generated.polygons {
        for contour in polygon.contours {
            for point in contour.points {
                budget.point()?;
                let (position_x, position_y) =
                    rotate(operation.x as f64, operation.y as f64, parent.angle);
                let x = rounded_i64(point.x * 1_000_000.0 + position_x + parent.x)?;
                let y = rounded_i64(point.y * 1_000_000.0 + position_y + parent.y)?;
                match bounds {
                    Some(value) => {
                        value[0] = value[0].min(x);
                        value[1] = value[1].min(y);
                        value[2] = value[2].max(x);
                        value[3] = value[3].max(y);
                    }
                    None => *bounds = Some([x, y, x, y]),
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Default)]
struct ParentTransform {
    x: f64,
    y: f64,
    angle: f64,
}

struct BoundsBudget {
    points: usize,
    maximum_points: usize,
    operations: usize,
    maximum_operations: usize,
    geometry_points: usize,
    maximum_geometry_points: usize,
    text_bytes: usize,
    maximum_text_bytes: usize,
}

impl BoundsBudget {
    fn point(&mut self) -> Result<(), Error> {
        self.points = self
            .points
            .checked_add(1)
            .filter(|value| *value <= self.maximum_points)
            .ok_or_else(resource_limit_error)?;
        Ok(())
    }

    fn operation(&mut self) -> Result<(), Error> {
        self.operations = self
            .operations
            .checked_add(1)
            .filter(|value| *value <= self.maximum_operations)
            .ok_or_else(resource_limit_error)?;
        Ok(())
    }

    fn geometry_point(&mut self) -> Result<(), Error> {
        self.geometry_points = self
            .geometry_points
            .checked_add(1)
            .filter(|value| *value <= self.maximum_geometry_points)
            .ok_or_else(resource_limit_error)?;
        Ok(())
    }

    fn text(&mut self, text: &str) -> Result<(), Error> {
        self.text_bytes = self
            .text_bytes
            .checked_add(text.len())
            .filter(|value| *value <= self.maximum_text_bytes)
            .ok_or_else(resource_limit_error)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Style {
    Normal,
    Subscript,
    Superscript,
}

struct Frame<'a> {
    nodes: &'a [TextMarkupNode],
    index: usize,
    marker: Option<TextMarkupMarker>,
    bar_start: f64,
    style: Style,
}

fn include_text(
    operation: &BoardTextOperation,
    parent: ParentTransform,
    limits: BoardBoundsLimits,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    if operation.text.is_empty()
        || !operation.font_face.is_empty()
        || operation.render_cache.is_some()
        || !operation.render_cache_polygons.is_empty()
    {
        return Ok(());
    }
    budget.text(&operation.text)?;
    let mut node_count = 0;
    let nodes = parse_text_markup(&operation.text, &mut node_count, limits.max_markup_nodes)
        .map_err(|error| {
            Error::at(
                ErrorPhase::Tree,
                if error.kind == TextContourErrorKind::ResourceLimit {
                    ErrorKind::ResourceLimit
                } else {
                    ErrorKind::UnexpectedToken
                },
                error.message,
                Position::START,
            )
        })?;
    let total_width = markup_width(&operation.text, &nodes) * operation.size_x_nm as f64;
    let mut cursor = match operation.h_align {
        BoardTextHAlign::Left => 0.0,
        BoardTextHAlign::Center => -total_width / 2.0,
        BoardTextHAlign::Right => -total_width,
    };
    let cap_top = -20.0 / 21.0;
    let cap_bottom = 1.0 / 21.0;
    let cap_center = (cap_top + cap_bottom) / 2.0;
    let offset_y = match operation.v_align {
        BoardTextVAlign::Center => (-cap_center - cap_bottom + 0.0024) * operation.size_y_nm as f64,
        BoardTextVAlign::Top => (-cap_top + 0.0024) * operation.size_y_nm as f64,
        BoardTextVAlign::Bottom => (-cap_bottom + 0.0024) * operation.size_y_nm as f64,
    };
    let mut frames = vec![Frame {
        nodes: &nodes,
        index: 0,
        marker: None,
        bar_start: cursor,
        style: Style::Normal,
    }];
    while let Some(frame) = frames.last_mut() {
        let Some(node) = frame.nodes.get(frame.index) else {
            let closed = frames.pop().expect("frame presence was checked");
            if closed.marker == Some(TextMarkupMarker::Overbar) {
                let trim = operation.size_x_nm as f64 * OVERBAR_TRIM_RATIO;
                let y = offset_y - operation.size_y_nm as f64 * OVERBAR_POSITION_FACTOR;
                for x in [closed.bar_start + trim, cursor - trim] {
                    include_point(operation, parent, x, y, false, budget, bounds)?;
                }
            }
            continue;
        };
        frame.index += 1;
        match node {
            TextMarkupNode::Text(span) => include_chars(
                operation,
                parent,
                &operation.text[span.clone()],
                frame.style,
                &mut cursor,
                offset_y,
                budget,
                bounds,
            )?,
            TextMarkupNode::Group { marker, children } => {
                let style = child_style(frame.style, *marker);
                frames.push(Frame {
                    nodes: children,
                    index: 0,
                    marker: Some(*marker),
                    bar_start: cursor,
                    style,
                });
            }
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_arguments,
    reason = "the streaming glyph pass carries one operation, transform, cursor, and bounded sink"
)]
fn include_chars(
    operation: &BoardTextOperation,
    parent: ParentTransform,
    characters: &str,
    style: Style,
    cursor: &mut f64,
    offset_y: f64,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    let scale = if style == Style::Normal {
        1.0
    } else {
        SUPER_SUB_SIZE_MULTIPLIER
    };
    let size_x = operation.size_x_nm as f64 * scale;
    let size_y = operation.size_y_nm as f64 * scale;
    let style_y = match style {
        Style::Normal => 0.0,
        Style::Subscript => size_y * SUB_HEIGHT_OFFSET,
        Style::Superscript => -size_y * SUPER_HEIGHT_OFFSET,
    };
    for character in characters.chars() {
        let (glyph, width) = glyph(character)
            .or_else(|| glyph('?'))
            .unwrap_or((&[], 0.0));
        if character == ' ' {
            *cursor += width * size_x;
            continue;
        }
        let start_x = glyph.first().map_or(0.0, |value| {
            (f64::from(*value) - f64::from(b'R')) * STROKE_SCALE
        });
        let mut index = 2;
        while index + 1 < glyph.len() {
            if glyph[index] == b' ' && glyph[index + 1] == b'R' {
                index += 2;
                continue;
            }
            let x = (f64::from(glyph[index]) - f64::from(b'R')) * STROKE_SCALE - start_x;
            let y = (f64::from(glyph[index + 1]) - f64::from(b'R') + FONT_OFFSET) * STROKE_SCALE;
            include_point(
                operation,
                parent,
                x * size_x + *cursor,
                y * size_y + offset_y + style_y,
                operation.italic,
                budget,
                bounds,
            )?;
            index += 2;
        }
        *cursor += width * size_x;
    }
    Ok(())
}

fn include_point(
    operation: &BoardTextOperation,
    parent: ParentTransform,
    mut x: f64,
    y: f64,
    italic: bool,
    budget: &mut BoundsBudget,
    bounds: &mut Option<[i64; 4]>,
) -> Result<(), Error> {
    budget.point()?;
    if italic {
        x += y * ITALIC_TILT;
    }
    if operation.mirror {
        x = -x;
    }
    let (x, y) = rotate(x, y, -operation.orient_deg);
    let (x, y) = (x + operation.x as f64, y + operation.y as f64);
    let (x, y) = rotate(x, y, parent.angle);
    let x = rounded_i64(x + parent.x)?;
    let y = rounded_i64(y + parent.y)?;
    match bounds {
        Some(value) => {
            value[0] = value[0].min(x);
            value[1] = value[1].min(y);
            value[2] = value[2].max(x);
            value[3] = value[3].max(y);
        }
        None => *bounds = Some([x, y, x, y]),
    }
    Ok(())
}

fn rotate(x: f64, y: f64, angle: f64) -> (f64, f64) {
    let radians = angle.to_radians();
    let (sine, cosine) = radians.sin_cos();
    (x * cosine - y * sine, x * sine + y * cosine)
}

fn markup_width(text: &str, nodes: &[TextMarkupNode]) -> f64 {
    let mut width = 0.0;
    let mut frames = vec![Frame {
        nodes,
        index: 0,
        marker: None,
        bar_start: 0.0,
        style: Style::Normal,
    }];
    while let Some(frame) = frames.last_mut() {
        let Some(node) = frame.nodes.get(frame.index) else {
            frames.pop();
            continue;
        };
        frame.index += 1;
        match node {
            TextMarkupNode::Text(span) => {
                let scale = if frame.style == Style::Normal {
                    1.0
                } else {
                    SUPER_SUB_SIZE_MULTIPLIER
                };
                width += text[span.clone()]
                    .chars()
                    .filter_map(glyph)
                    .map(|(_, value)| value * scale)
                    .sum::<f64>();
            }
            TextMarkupNode::Group { marker, children } => {
                let style = child_style(frame.style, *marker);
                frames.push(Frame {
                    nodes: children,
                    index: 0,
                    marker: Some(*marker),
                    bar_start: width,
                    style,
                });
            }
        }
    }
    width
}

fn child_style(style: Style, marker: TextMarkupMarker) -> Style {
    match marker {
        TextMarkupMarker::Overbar => style,
        TextMarkupMarker::Subscript => Style::Subscript,
        TextMarkupMarker::Superscript if style == Style::Subscript => Style::Subscript,
        TextMarkupMarker::Superscript => Style::Superscript,
    }
}

fn glyph(character: char) -> Option<(&'static [u8], f64)> {
    let index = (character as usize).checked_sub(0x20)?;
    let start = usize::try_from(*NEWSTROKE_GLYPH_OFFSETS.get(index)?).ok()?;
    let end = usize::try_from(*NEWSTROKE_GLYPH_OFFSETS.get(index + 1)?).ok()?;
    Some((
        NEWSTROKE_GLYPH_DATA.as_bytes().get(start..end)?,
        f64::from(*NEWSTROKE_WIDTH_UNITS.get(index)?) * STROKE_SCALE,
    ))
}

fn rounded_i64(value: f64) -> Result<i64, Error> {
    let rounded = value.round_ties_even();
    if !rounded.is_finite() || rounded < i64::MIN as f64 || rounded > i64::MAX as f64 {
        return Err(Error::at(
            ErrorPhase::Tree,
            ErrorKind::InvalidBuildValue,
            "Board text bounds exceed i64",
            Position::START,
        ));
    }
    Ok(rounded as i64)
}

fn resource_limit_error() -> Error {
    Error::at(
        ErrorPhase::Tree,
        ErrorKind::ResourceLimit,
        "Board text bounds exceed configured limits",
        Position::START,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BoardNetClassAssignments, BoardPlotLimits, BoardTextVariables, PcbLimits,
        PlotterTextCacheLimits, PlotterTextFont, board_plot_facts_with_sidecars,
    };
    use kicad_monkey_contracts::generated::shaping_record::ShapingInput;
    use serde::Deserialize;

    const FONT_BYTES: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../assets/fonts/kicad-stroke.ttf"
    ));

    #[derive(Deserialize)]
    struct ShapingVectors {
        records: Vec<ShapingRecord>,
    }

    #[derive(Deserialize)]
    struct ShapingRecord {
        shaping: ShapingInput,
    }

    fn outline_font() -> PlotterTextFont<'static> {
        let vectors: ShapingVectors = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../tests/parity/text_layout_vectors.json"
        )))
        .expect("shaping fixture");
        let mut shaping = vectors.records.into_iter().next().expect("record").shaping;
        shaping.text.clear();
        shaping.features.clear();
        PlotterTextFont {
            face: "Native Fixture",
            bold: false,
            italic: false,
            font_bytes: FONT_BYTES,
            shaping,
            fake_bold: false,
            fake_italic: false,
        }
    }

    fn facts(source: &str) -> super::super::BoardPlotFacts<'_> {
        board_plot_facts_with_sidecars(
            source,
            BoardPlotLimits::default(),
            PcbLimits::default(),
            &BoardNetClassAssignments::default(),
            &BoardTextVariables::default(),
        )
        .expect("source-bound board facts")
    }

    fn bounds(source: &str, limits: BoardBoundsLimits) -> Result<Option<[i64; 4]>, Error> {
        facts(source).bounds(None, limits)
    }

    fn is_resource_limit(result: Result<Option<[i64; 4]>, Error>) -> bool {
        result.is_err_and(|error| error.kind == ErrorKind::ResourceLimit)
    }

    const HEADER: &str = r#"(version 20240108) (generator pcbnew)
      (general (thickness 1.6)) (paper "A4")
      (layers (0 "F.Cu" signal) (31 "B.Cu" signal) (36 "B.SilkS" user "b.silkscreen"))"#;

    #[test]
    fn board_bounds_reject_footprints_before_retaining_pcb_owners() {
        let source = r#"(kicad_pcb (version 20240108) (generator pcbnew)
          (general (thickness 1.6))
          (paper "A4")
          (layers (0 "F.Cu" signal) (31 "B.Cu" signal))
          (footprint "Demo:One" (layer "F.Cu") (at 1 2)))"#;
        let limits = BoardBoundsLimits {
            max_footprints: 0,
            ..BoardBoundsLimits::default()
        };
        let error = bounds(source, limits).expect_err("owner retention limit");
        assert_eq!(error.kind, ErrorKind::ResourceLimit);
    }

    #[test]
    fn circular_board_families_contribute_full_extents() {
        let graphic = format!(
            r#"(kicad_pcb {HEADER}
              (gr_circle (center 10 20) (end 15 20)
                (stroke (width 0.1) (type solid)) (fill none) (layer "B.SilkS")))"#
        );
        assert_eq!(
            bounds(&graphic, BoardBoundsLimits::default()).unwrap(),
            Some([5_000_000, 15_000_000, 15_000_000, 25_000_000])
        );

        let via = format!(
            r#"(kicad_pcb {HEADER}
              (via (at 20 20) (size 4) (drill 1)
                (layers "F.Cu" "B.Cu")))"#
        );
        assert_eq!(
            bounds(&via, BoardBoundsLimits::default()).unwrap(),
            Some([18_000_000, 18_000_000, 22_000_000, 22_000_000])
        );

        let pad = format!(
            r#"(kicad_pcb {HEADER}
              (footprint "Demo:Pad" (layer "F.Cu") (at 10 20)
                (property "Reference" "")
                (pad "1" smd circle (at 1 2) (size 4 4)
                  (layers "F.Cu"))))"#
        );
        assert_eq!(
            bounds(&pad, BoardBoundsLimits::default()).unwrap(),
            Some([9_000_000, 20_000_000, 13_000_000, 24_000_000])
        );
    }

    #[test]
    fn cubic_table_trapezoid_and_image_bounds_match_python_families() {
        let curve = format!(
            r#"(kicad_pcb {HEADER}
              (gr_curve (pts (xy 0 0) (xy 10 10) (xy 10 -10) (xy 20 0))
                (stroke (width 0.1) (type solid)) (layer "B.SilkS")))"#
        );
        let curve_bounds = bounds(&curve, BoardBoundsLimits::default())
            .unwrap()
            .expect("curve bounds");
        assert_eq!(curve_bounds[0], 0);
        assert_eq!(curve_bounds[2], 20_000_000);
        assert!((curve_bounds[1] + 2_886_751).abs() <= 1);
        assert!((curve_bounds[3] - 2_886_751).abs() <= 1);

        let table = format!(
            r#"(kicad_pcb {HEADER}
              (table (layer "B.SilkS")
                (border (external no)) (separators (rows no) (cols no))
                (cells (table_cell "" (start 100 200) (end 110 205)
                  (layer "B.SilkS")))))"#
        );
        assert_eq!(
            bounds(&table, BoardBoundsLimits::default()).unwrap(),
            Some([100_000_000, 200_000_000, 110_000_000, 205_000_000])
        );

        let trapezoid = format!(
            r#"(kicad_pcb {HEADER}
              (footprint "Demo:Pad" (layer "F.Cu") (at 10 20)
                (property "Reference" "")
                (pad "1" smd trapezoid (at 0 0) (size 4 2) (rect_delta 0 10)
                  (layers "F.Cu"))))"#
        );
        assert_eq!(
            bounds(&trapezoid, BoardBoundsLimits::default()).unwrap(),
            Some([8_000_000, 18_000_000, 12_000_000, 22_000_000])
        );

        let image = format!(
            r#"(kicad_pcb {HEADER}
              (image (at 100 200) (layer "B.SilkS") (scale 0)
                (data "iVBORw0KGgoAAAANSUhEUgAAAAEAAAAB"))
              (image (at 1000 2000) (layer "B.SilkS") (data "@@@")))"#
        );
        assert_eq!(
            bounds(&image, BoardBoundsLimits::default()).unwrap(),
            Some([99_950_000, 199_950_000, 100_050_000, 200_050_000])
        );
    }

    #[test]
    fn faced_footprint_property_and_dimension_cache_set_extents() {
        let property = format!(
            r#"(kicad_pcb {HEADER}
              (footprint "Demo:Cache" (layer "F.Cu") (at 100 20)
                (property "Label" "cached" (at 0 0) (layer "B.SilkS")
                  (effects (font (face "Native Fixture") (size 1 1)))
                  (render_cache "cached" 0
                    (polygon (pts (xy 100 20) (xy 101 20) (xy 100 21)))))))"#
        );
        let fonts = [outline_font()];
        let resources = PlotterTextCacheResources {
            fonts: &fonts,
            limits: PlotterTextCacheLimits::default(),
        };
        let property_bounds = facts(&property)
            .bounds(Some(&resources), BoardBoundsLimits::default())
            .unwrap()
            .expect("faced property bounds");
        assert!(property_bounds[0] > 95_000_000 && property_bounds[2] > 100_000_000);

        let dimension = format!(
            r#"(kicad_pcb {HEADER}
              (dimension (type center) (layer "B.SilkS")
                (pts (xy 0 0) (xy 1 0))
                (format (override_value "far") (units_format 0) (precision 0))
                (gr_text "old" (effects (font (face "Arial") (size 1 1)))
                  (render_cache "far" 0
                    (polygon (pts (xy 100 100) (xy 110 100) (xy 100 110)))))))"#
        );
        let value = bounds(&dimension, BoardBoundsLimits::default())
            .unwrap()
            .expect("dimension bounds");
        assert!(value[0] >= 100_000_000 && value[1] >= 100_000_000);
        assert!(value[2] >= 110_000_000 && value[3] >= 110_000_000);
    }

    #[test]
    fn board_image_limits_accept_exact_and_reject_one_under() {
        let source = format!(
            r#"(kicad_pcb {HEADER}
              (image (at 100 200) (layer "B.SilkS")
                (data "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=")))"#
        );
        let facts = facts(&source);
        let image = facts
            .view()
            .images()
            .next()
            .expect("image")
            .expect("decoded image carrier");
        let encoded = facts
            .view()
            .image_data(&image, usize::MAX)
            .expect("image data");
        let decoded = decode_board_base64(&encoded, usize::MAX)
            .expect("base64 limit")
            .expect("base64 shape");
        let (dimensions, metadata_work) =
            board_image_dimensions(&decoded, usize::MAX).expect("metadata");
        let (width, height) = dimensions.expect("dimensions");
        let exact = BoardBoundsLimits {
            max_images: 1,
            max_image_encoded_bytes: encoded.len(),
            max_image_decoded_bytes: decoded.len(),
            max_image_decode_work: encoded.len() + decoded.len() + metadata_work,
            max_image_pixels: width as usize * height as usize,
            ..BoardBoundsLimits::default()
        };
        facts.bounds(None, exact).expect("inclusive image limits");
        for limits in [
            BoardBoundsLimits {
                max_images: 0,
                ..exact
            },
            BoardBoundsLimits {
                max_image_encoded_bytes: exact.max_image_encoded_bytes - 1,
                ..exact
            },
            BoardBoundsLimits {
                max_image_decoded_bytes: exact.max_image_decoded_bytes - 1,
                ..exact
            },
            BoardBoundsLimits {
                max_image_decode_work: exact.max_image_decode_work - 1,
                ..exact
            },
            BoardBoundsLimits {
                max_image_pixels: exact.max_image_pixels - 1,
                ..exact
            },
        ] {
            assert!(is_resource_limit(facts.bounds(None, limits)));
        }
    }

    #[test]
    fn board_bounds_limits_accept_exact_and_reject_one_under() {
        let circle = format!(
            r#"(kicad_pcb {HEADER}
              (gr_circle (center 10 20) (end 15 20)
                (stroke (width 0.1) (type solid)) (fill none) (layer "B.SilkS")))"#
        );
        for limits in [
            BoardBoundsLimits {
                max_operations: 1,
                max_geometry_points: 2,
                ..BoardBoundsLimits::default()
            },
            BoardBoundsLimits::default(),
        ] {
            bounds(&circle, limits).expect("inclusive circle boundary");
        }
        assert!(is_resource_limit(bounds(
            &circle,
            BoardBoundsLimits {
                max_operations: 0,
                ..BoardBoundsLimits::default()
            }
        )));
        assert!(is_resource_limit(bounds(
            &circle,
            BoardBoundsLimits {
                max_geometry_points: 1,
                ..BoardBoundsLimits::default()
            }
        )));

        let text = format!(
            r#"(kicad_pcb {HEADER}
              (footprint "Demo:Text" (layer "F.Cu") (at 0 0)
                (property "Label" "A_{{B}}" (at 0 0) (layer "B.SilkS")
                  (effects (font (size 1 1))))))"#
        );
        let minimum_markup = (0..16)
            .find(|maximum| {
                bounds(
                    &text,
                    BoardBoundsLimits {
                        max_markup_nodes: *maximum,
                        ..BoardBoundsLimits::default()
                    },
                )
                .is_ok()
            })
            .expect("finite markup node boundary");
        assert!(minimum_markup > 0);
        assert!(is_resource_limit(bounds(
            &text,
            BoardBoundsLimits {
                max_markup_nodes: minimum_markup - 1,
                ..BoardBoundsLimits::default()
            }
        )));
        let minimum_points = (0..256)
            .find(|maximum| {
                bounds(
                    &text,
                    BoardBoundsLimits {
                        max_glyph_points: *maximum,
                        ..BoardBoundsLimits::default()
                    },
                )
                .is_ok()
            })
            .expect("finite glyph point boundary");
        assert!(minimum_points > 0);
        assert!(is_resource_limit(bounds(
            &text,
            BoardBoundsLimits {
                max_glyph_points: minimum_points - 1,
                ..BoardBoundsLimits::default()
            }
        )));
        bounds(
            &text,
            BoardBoundsLimits {
                max_text_bytes: 5,
                ..BoardBoundsLimits::default()
            },
        )
        .expect("exact text-byte boundary");
        assert!(is_resource_limit(bounds(
            &text,
            BoardBoundsLimits {
                max_text_bytes: 4,
                ..BoardBoundsLimits::default()
            }
        )));
    }
}
